use super::*;
use crate::*;

impl NetworkSim {
    /// One authoritative L1 result for forwarding, counters, UI and consoles.
    pub fn link_status(&self, port: PortId) -> LinkStatus {
        if self.router_svi(port).is_some() {
            let speed = self.svi_link_speed(port);
            return LinkStatus {
                speed,
                fault: speed.is_none().then_some(LinkFault::NoCable),
                path: vec![port],
                optical: Vec::new(),
            };
        }
        let path = self.mechanical_path(port);
        if path.is_empty() {
            let fault = if self.cage_profile(port).is_some() && self.endpoint_module(port).is_none()
            {
                LinkFault::EmptyCage
            } else {
                LinkFault::NoCable
            };
            return LinkStatus::down(fault, path);
        }
        let failure = |fault| LinkStatus::down(fault, path.clone());
        for id in &path {
            let Some(p) = self.port(*id) else {
                return failure(LinkFault::NoCable);
            };
            if !p.enabled {
                return failure(LinkFault::Disabled);
            }
            if self.network_outlet(*id).is_none() {
                let Some(device) = self.device(p.device) else {
                    return failure(LinkFault::NotInstalled);
                };
                if device.rack.is_none() {
                    return failure(LinkFault::NotInstalled);
                }
                if !matches!(p.config, PortConfig::PatchPanel | PortConfig::CableManager)
                    && !device.powered
                {
                    return failure(LinkFault::Unpowered);
                }
            }
            if self.cage_profile(*id).is_some() {
                let Some(module) = self.endpoint_module(*id) else {
                    return failure(LinkFault::EmptyCage);
                };
                if !self.cage_profile(*id).unwrap().supports(module) {
                    return failure(LinkFault::UnsupportedModule);
                }
            }
        }
        let a = path[0];
        let b = *path.last().unwrap();
        let modes = |id| -> Vec<EthernetMode> {
            let p = self.port(id).unwrap();
            let ceiling = p.max_speed.min(p.advertised_speed);
            if let Some(module) = self.endpoint_module(id) {
                module
                    .modes
                    .iter()
                    .copied()
                    .filter(|m| {
                        m.speed <= ceiling
                            && self.cage_profile(id).is_some_and(|c| c.modes.contains(m))
                    })
                    .collect()
            } else {
                [
                    LinkSpeed::Mbps10,
                    LinkSpeed::Mbps100,
                    LinkSpeed::Gbps1,
                    LinkSpeed::Gbps10,
                    LinkSpeed::Gbps25,
                ]
                .into_iter()
                .filter(|speed| *speed <= ceiling)
                .map(|speed| EthernetMode {
                    speed,
                    lanes: 1,
                    fec: Fec::None,
                })
                .collect()
            }
        };
        let right_modes = modes(b);
        let Some(mode) = modes(a)
            .into_iter()
            .filter(|m| right_modes.contains(m))
            .max_by_key(|m| m.speed)
        else {
            return failure(LinkFault::ModeMismatch);
        };
        let segments: Vec<_> = path
            .windows(2)
            .filter_map(|pair| {
                self.link_for_port(pair[0])
                    .filter(|link| link.other(pair[0]) == Some(pair[1]))
            })
            .collect();
        if segments.iter().any(|link| !link.enabled) {
            return failure(LinkFault::Disabled);
        }
        if segments.iter().any(|link| {
            self.minimum_routed_cable_length(link.a, link.b, &link.route)
                .is_ok_and(|minimum| minimum > link.length_cm)
        }) {
            return failure(LinkFault::CableTooShort);
        }
        let length: u64 = segments.iter().map(|link| u64::from(link.length_cm)).sum();
        let assemblies: Vec<_> = segments
            .iter()
            .map(|link| self.connected_assembly(link.id))
            .collect();
        if assemblies.iter().all(Option::is_none) {
            if !self.port_is_copper(a)
                || !self.port_is_copper(b)
                || path
                    .iter()
                    .any(|id| self.port(*id).unwrap().connector == PortConnector::Lc)
            {
                return failure(LinkFault::ConnectorMismatch);
            }
            if length > 10_000 {
                return failure(LinkFault::TooLong);
            }
            return LinkStatus {
                speed: Some(mode.speed),
                fault: None,
                path,
                optical: Vec::new(),
            };
        }
        if assemblies.iter().any(Option::is_none) {
            return failure(LinkFault::ConnectorMismatch);
        }
        let assemblies: Vec<_> = assemblies.into_iter().flatten().collect();
        let Some(left) = self.endpoint_module(a) else {
            return failure(LinkFault::EmptyCage);
        };
        let Some(right) = self.endpoint_module(b) else {
            return failure(LinkFault::EmptyCage);
        };
        let specs: Vec<_> = assemblies
            .iter()
            .filter_map(|c| optics_catalog().cable(&c.model_id))
            .collect();
        if specs.len() != assemblies.len() {
            return failure(LinkFault::ConnectorMismatch);
        }
        if specs.iter().any(|c| {
            matches!(
                c.medium,
                AssemblyMedium::Dac { .. } | AssemblyMedium::Aoc { .. }
            )
        }) {
            if path.len() != 2
                || !matches!(left.medium, ModuleMedium::DirectAttach)
                || !matches!(right.medium, ModuleMedium::DirectAttach)
            {
                return failure(LinkFault::ConnectorMismatch);
            }
            return LinkStatus {
                speed: Some(mode.speed),
                fault: None,
                path,
                optical: Vec::new(),
            };
        }
        let ModuleMedium::Optical {
            strands,
            tx_nm,
            rx_nm,
            tx_mdbm,
            sensitivity_mdbm,
            overload_mdbm,
            reaches,
        } = &left.medium
        else {
            return failure(LinkFault::ConnectorMismatch);
        };
        let ModuleMedium::Optical {
            strands: right_strands,
            tx_nm: right_tx,
            rx_nm: right_rx,
            tx_mdbm: right_power,
            sensitivity_mdbm: right_sensitivity,
            overload_mdbm: right_overload,
            reaches: right_reaches,
        } = &right.medium
        else {
            return failure(LinkFault::ConnectorMismatch);
        };
        if tx_nm != right_rx || rx_nm != right_tx {
            return failure(LinkFault::WavelengthMismatch);
        }
        let mut loss: u64 = path
            .windows(2)
            .filter(|pair| {
                self.port(pair[0])
                    .is_some_and(|p| p.paired_port == Some(pair[1]))
            })
            .count() as u64
            * 200;
        for (link, spec) in segments.iter().zip(&specs) {
            let AssemblyMedium::Fiber {
                fiber,
                strands: cable_strands,
                connector_loss_mdb,
            } = spec.medium
            else {
                return failure(LinkFault::ConnectorMismatch);
            };
            if strands != right_strands || *strands != cable_strands {
                return failure(LinkFault::FiberMismatch);
            }
            let (Some(l), Some(r)) = (
                reaches.iter().find(|r| r.fiber == fiber),
                right_reaches.iter().find(|r| r.fiber == fiber),
            ) else {
                return failure(LinkFault::FiberMismatch);
            };
            if length > u64::from(l.max_length_cm.min(r.max_length_cm)) {
                return failure(LinkFault::TooLong);
            }
            loss += u64::from(link.length_cm)
                * u64::from(l.attenuation_mdb_per_km.max(r.attenuation_mdb_per_km))
                / 100_000
                + u64::from(connector_loss_mdb) * 2;
        }
        if *strands == 2 && assemblies.iter().filter(|c| c.crossed).count() % 2 != 1 {
            return failure(LinkFault::PolarityMismatch);
        }
        let loss = loss.min(i32::MAX as u64) as i32;
        let rx_left = right_power.saturating_sub(loss);
        let rx_right = tx_mdbm.saturating_sub(loss);
        let fault = if rx_left < *sensitivity_mdbm || rx_right < *right_sensitivity {
            Some(LinkFault::LowLight)
        } else if rx_left > *overload_mdbm || rx_right > *right_overload {
            Some(LinkFault::ReceiverOverload)
        } else {
            None
        };
        LinkStatus {
            speed: fault.is_none().then_some(mode.speed),
            fault,
            path,
            optical: vec![
                OpticalReading {
                    port: a,
                    tx_mdbm: *tx_mdbm,
                    rx_mdbm: rx_left,
                },
                OpticalReading {
                    port: b,
                    tx_mdbm: *right_power,
                    rx_mdbm: rx_right,
                },
            ],
        }
    }
}
