use crate::*;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct RouterNetworkProfile {
    pub wan_ports: Vec<usize>,
    pub lan_ports: Vec<usize>,
    pub flex_ports: Vec<usize>,
    pub routing_enabled: bool,
}

pub fn router_network_profile() -> &'static RouterNetworkProfile {
    #[derive(Deserialize)]
    struct Config {
        network: RouterNetworkProfile,
    }
    static PROFILE: OnceLock<RouterNetworkProfile> = OnceLock::new();
    PROFILE.get_or_init(|| {
        let c: Config = crate::equipment_config::equipment_catalog(
            "router_config.json",
            include_str!("../../../assets/equipment/router_config.json"),
        );
        let p = c.network;
        let mut ports = p
            .wan_ports
            .iter()
            .chain(&p.lan_ports)
            .copied()
            .collect::<Vec<_>>();
        ports.sort_unstable();
        assert!(
            !p.wan_ports.is_empty()
                && !p.lan_ports.is_empty()
                && ports.iter().copied().eq(0..ports.len())
                && p.flex_ports.iter().all(|i| p.lan_ports.contains(i)),
            "invalid network port layout in assets/equipment/router_config.json"
        );
        p
    })
}

impl NetworkSim {
    pub(crate) fn device_vlans(&self, device: DeviceId) -> Option<&[Vlan]> {
        match &self.device(device)?.kind {
            DeviceKind::Switch(s) => Some(&s.vlans),
            DeviceKind::Router(r) => Some(&r.vlans),
            _ => None,
        }
    }

    pub(crate) fn device_vlans_mut(&mut self, device: DeviceId) -> Option<&mut Vec<Vlan>> {
        match &mut self.devices.get_mut(&device)?.kind {
            DeviceKind::Switch(s) => Some(&mut s.vlans),
            DeviceKind::Router(r) => Some(&mut r.vlans),
            _ => None,
        }
    }

    pub(crate) fn router_svi(&self, port: PortId) -> Option<&RouterInterface> {
        let DeviceKind::Router(r) = &self.device(self.port(port)?.device)?.kind else {
            return None;
        };
        r.svi_ports
            .contains(&port)
            .then(|| r.interfaces.iter().find(|i| i.port == port))
            .flatten()
    }

    pub(crate) fn svi_link_speed(&self, port: PortId) -> Option<LinkSpeed> {
        let interface = self.router_svi(port)?;
        let p = self.port(port)?;
        let device = self.device(p.device)?;
        if !p.enabled || !device.powered || device.rack.is_none() {
            return None;
        }
        let vlan = interface.vlan?;
        if !self.device_vlans(p.device)?.iter().any(|v| v.id == vlan) {
            return None;
        }
        let blocked = self
            .runtime
            .spanning_tree
            .get(&vlan)
            .cloned()
            .unwrap_or_else(|| self.spanning_tree_blocked_ports(vlan).into_iter().collect());
        device
            .ports()
            .iter()
            .filter(|id| {
                !blocked.contains(*id)
                    && matches!(
                        self.port(**id).map(|p| &p.config),
                        Some(PortConfig::Switch(_))
                    )
                    && self.carries_vlan(**id, vlan)
            })
            .filter_map(|id| self.physical_link_speed(*id))
            .max()
    }

    pub(crate) fn ios_ports(&self, device: DeviceId) -> Vec<PortId> {
        let Some(d) = self.device(device) else {
            return vec![];
        };
        let mut ports = d.ports().to_vec();
        if let DeviceKind::Router(r) = &d.kind {
            ports.extend(&r.svi_ports);
        }
        ports
    }

    pub(crate) fn ensure_router_svi(
        &mut self,
        device: DeviceId,
        vlan: VlanId,
    ) -> Result<PortId, String> {
        let DeviceKind::Router(r) = &self.devices[&device].kind else {
            return Err("% VLAN interfaces are supported on this router; Catalyst C1000 uses management ip.".into());
        };
        if let Some(port) = r
            .svi_ports
            .iter()
            .find(|id| self.router_svi(**id).is_some_and(|i| i.vlan == Some(vlan)))
        {
            return Ok(*port);
        }
        let name = format!("Vlan{}", vlan.0);
        let port = self.alloc_port(
            device,
            name.clone(),
            PortConnector::Rj45,
            PortConfig::Router(RouterPortConfig::default()),
        );
        self.ports.get_mut(&port).unwrap().enabled = false;
        let DeviceKind::Router(r) = &mut self.devices.get_mut(&device).unwrap().kind else {
            unreachable!()
        };
        r.svi_ports.push(port);
        self.execute(Command::ConfigureRouterInterface {
            port,
            name,
            vlan: Some(vlan),
            address: None,
            prefix: 24,
            internet_connected: false,
        })
        .map_err(|e| format!("% {e}"))?;
        Ok(port)
    }

    pub(crate) fn ios_set_switchport(
        &mut self,
        device: DeviceId,
        port: PortId,
        switching: bool,
    ) -> Result<(), String> {
        let DeviceKind::Router(r) = &self.devices[&device].kind else {
            return Err("% Catalyst C1000 ports support Layer 2 switching only.".into());
        };
        let Some(index) = r.ports.iter().position(|id| *id == port) else {
            return Err("% switchport applies to physical interfaces only.".into());
        };
        let profile = router_network_profile();
        if profile.wan_ports.contains(&index) {
            return if switching {
                Err("% WAN interfaces are routed ports; use a LAN switch port.".into())
            } else {
                Ok(())
            };
        }
        // Older saves may already contain routed fixed LAN ports. Preserve those
        // interfaces; this idempotent command does not convert new hardware.
        if matches!(self.ports[&port].config, PortConfig::Switch(_)) == switching {
            return Ok(());
        }
        if !switching && !profile.flex_ports.contains(&index) {
            let flex = profile
                .flex_ports
                .iter()
                .filter_map(|i| r.ports.get(*i))
                .map(|p| self.ios_interface_name(device, *p))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "% This is a fixed Layer 2 port. Use interface Vlan<N> for routing, or no switchport on {flex}."
            ));
        }
        self.ports.get_mut(&port).unwrap().config = if switching {
            PortConfig::Switch(SwitchPortConfig {
                mode: SwitchPortMode::Access { vlan: None },
            })
        } else {
            PortConfig::Router(RouterPortConfig::default())
        };
        if let DeviceKind::Router(r) = &mut self.devices.get_mut(&device).unwrap().kind {
            r.interfaces.retain(|i| i.port != port);
            r.routes.retain(|r| r.egress != port);
            r.domain_routes.retain(|r| r.port != port);
        }
        self.routing_revision += 1;
        self.topology_revision += 1;
        Ok(())
    }
}
