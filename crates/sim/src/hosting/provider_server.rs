use super::*;
use std::net::Ipv4Addr;

const CONTROL_SERVICE: &str = "hosting-provider";

impl NetworkSim {
    pub(crate) fn hosting_server_operational(&self, server: DeviceId) -> bool {
        self.device(server).is_some_and(|d| {
            d.powered && matches!(&d.kind, DeviceKind::Server(s) if s.hardware.as_ref().is_none_or(|h| h.ready()))
        })
    }

    /// Resolve the address using configured interfaces, rather than a UI-selected device.
    fn hosting_address_owner(&self, address: Ipv4Addr) -> Option<(DeviceId, PortId)> {
        let revision = (self.topology_revision, self.routing_revision);
        if let Some(owner) = self.runtime.hosting_addresses.get(revision, address) {
            return owner;
        }
        let owner = self.find_hosting_address_owner(address);
        self.runtime
            .hosting_addresses
            .insert(revision, address, owner);
        owner
    }

    fn find_hosting_address_owner(&self, address: Ipv4Addr) -> Option<(DeviceId, PortId)> {
        let mut matches = self.ports().filter(|p| match &p.config {
            PortConfig::Server(c) => c.addresses().any(|a| a.address == address),
            PortConfig::Router(c) => c.interfaces.iter().any(|i| i.address == Some(address)),
            PortConfig::Switch(_) => self
                .switch_management(p.device)
                .is_some_and(|m| m.address == address),
            _ => false,
        });
        let port = matches.next()?;
        (matches.next().is_none()
            && self.device(port.device).is_some_and(|d| {
                matches!(d.kind, DeviceKind::Server(_)) && d.ports().contains(&port.id)
            }))
        .then_some((port.device, port.id))
    }

    pub fn hosting_server_address_port(
        &self,
        server: DeviceId,
        address: Ipv4Addr,
    ) -> Option<PortId> {
        self.hosting_address_owner(address)
            .filter(|(owner, _)| *owner == server)
            .map(|(_, port)| port)
    }

    pub fn hosting_provider_installed(&self, server: DeviceId) -> bool {
        self.server_os(server)
            .is_some_and(|os| os.services.contains_key(CONTROL_SERVICE))
    }

    fn hosting_provider_operational(&self, server: DeviceId) -> bool {
        self.hosting_server_operational(server)
            && self
                .server_os(server)
                .and_then(|os| os.services.get(CONTROL_SERVICE))
                .is_some_and(|s| s.active)
    }

    /// The storefront destination follows the public address even if it moves to another machine.
    pub fn hosting_provider_server(&self) -> Option<DeviceId> {
        self.hosting_address_owner(self.hosting.provider_site_ipv4?)
            .map(|(server, _)| server)
    }

    pub(crate) fn hosting_management_controller(&self, address: Ipv4Addr) -> Option<DeviceId> {
        if !address.is_private() {
            return None;
        }
        self.hosting_address_owner(address)
            .map(|(server, _)| server)
            .filter(|server| self.hosting_provider_operational(*server))
    }

    pub fn validate_hosting_provider_site_address(&self, address: Ipv4Addr) -> Result<(), String> {
        if address.is_private()
            || !self
                .provider
                .pools()
                .iter()
                .any(|p| p.prefix.usable(address))
        {
            return Err(
                "The provider site's public IPv4 must be usable and belong to a purchased range"
                    .into(),
            );
        }
        Ok(())
    }

    pub(crate) fn set_hosting_provider_site_address(
        &mut self,
        address: Option<Ipv4Addr>,
    ) -> Result<(), String> {
        if let Some(address) = address {
            self.validate_hosting_provider_site_address(address)?;
        }
        self.hosting.provider_site_ipv4 = address;
        self.hosting.provider_server = None;
        self.hosting.site_traffic = HostingSiteTraffic::default();
        self.routing_revision += 1;
        Ok(())
    }

    /// Installation is local to the console's machine and listens on its configured interfaces.
    pub fn setup_main_hoster_server(&mut self, server: DeviceId) -> Result<(), String> {
        if !self.hosting_server_operational(server) {
            return Err(
                "Main hoster installation requires a powered operational physical server".into(),
            );
        }
        if self.hosting.vms.values().any(|v| v.host == server) {
            return Err("The main hoster server cannot also host customer VMs".into());
        }
        self.guest_mut(server).filesystem.write(
            "/etc/hosting-provider.conf",
            "listen=0.0.0.0\n",
            false,
        )?;
        let unit = self
            .guest_mut(server)
            .services
            .entry(CONTROL_SERVICE.into())
            .or_insert(LinuxService {
                enabled: false,
                active: false,
                log: vec![],
            });
        if !unit.enabled || !unit.active {
            unit.log
                .push("Main hoster installed; listening on configured network interfaces".into());
        }
        unit.enabled = true;
        unit.active = true;
        self.routing_revision += 1;
        Ok(())
    }

    /// Public availability requires a running installed app and actual transit/return routing.
    pub fn hosting_provider_site_status(&self) -> Result<(), String> {
        let address = self
            .hosting
            .provider_site_ipv4
            .ok_or("Enter the main hoster server's public IPv4 in HOSTING")?;
        self.validate_hosting_provider_site_address(address)?;
        let (server, port) = self.hosting_address_owner(address).ok_or(
            "Provider site public IPv4 is missing, duplicated or not assigned to a physical server",
        )?;
        if !self.hosting_provider_installed(server) {
            return Err(
                "Run ./setup-main-hoster-server.sh in the server console at this public IP".into(),
            );
        }
        if !self.hosting_provider_operational(server) {
            return Err("Provider site server or hosting-provider service is offline".into());
        }
        let revision = (self.topology_revision, self.routing_revision);
        let key = (server, address);
        let reachable = if let Some(cached) = self.runtime.hosting_management.get(revision, key) {
            cached
        } else {
            let trace = self.probe_ipv4_address(port, address, Ipv4Addr::new(198, 51, 100, 1));
            let reachable = trace.reachable
                && trace.hops.iter().any(|hop| {
                    hop.ingress
                        .into_iter()
                        .chain(hop.egress)
                        .any(|p| self.provider.circuit(p).is_some_and(|c| c.enabled))
                });
            self.runtime
                .hosting_management
                .insert(revision, key, reachable);
            reachable
        };
        if reachable {
            Ok(())
        } else {
            Err("Provider site has no working public transit and return route".into())
        }
    }

    /// Storefront exchanges share the same NICs, uplinks and queues as guest workloads.
    pub(crate) fn hosting_site_flow(&self) -> Option<TrafficFlow> {
        self.hosting_provider_site_status().ok()?;
        let address = self.hosting.provider_site_ipv4?;
        let (_, source) = self.hosting_address_owner(address)?;
        let market_fraction = (1.0
            - self.hosting.vms.len() as f64 / f64::from(self.hosting.market.population))
        .max(0.0);
        // 64 KiB per prospect for the product page and order exchange, aggregated over time.
        let demand_mbps =
            market_fraction * f64::from(self.hosting.market.prospects_per_day) * 65536.0 * 8.0
                / 86_400.0
                / 1_000_000.0;
        Some(TrafficFlow {
            id: u64::MAX,
            source,
            source_ipv4: Some(address),
            destination: Ipv4Addr::new(198, 51, 100, 1),
            demand_mbps,
            limit_mbps: f64::INFINITY,
        })
    }

    pub(crate) fn hosting_site_delivery(&self) -> f64 {
        let traffic = self.hosting.site_traffic;
        if self.hosting_provider_site_status().is_err() {
            0.0
        } else if traffic.demand_mbps > 0.0 {
            (traffic.delivered_mbps / traffic.demand_mbps).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

pub(crate) struct MainHosterCommand;
impl LinuxCommand for MainHosterCommand {
    fn name(&self) -> &'static str {
        "main-hoster"
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _: &str,
    ) -> Result<String, String> {
        match args.first().map(String::as_str) {
            Some("setup") if args.len() == 1 => {
                sim.setup_main_hoster_server(device)?;
                Ok("Main hoster installed and started on this server. Enter its public IPv4 in HOSTING.\n".into())
            }
            _ => Err("usage: ./setup-main-hoster-server.sh (or main-hoster setup)".into()),
        }
    }
    fn suggest(&self, _: &CompletionContext<'_>) -> Vec<String> {
        vec!["setup".into()]
    }
}
