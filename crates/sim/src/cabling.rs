use crate::*;
use serde::{Deserialize, Serialize};

/// Common RJ45 jacket colors available for patch leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CableColor {
    #[default]
    White,
    Gray,
    Blue,
    Orange,
    Red,
}

/// Game economy prices, not a live supplier price list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CableSupply {
    CableBox305m,
    Rj45Pack20,
}
impl CableSupply {
    pub fn price(self) -> i64 {
        match self {
            Self::CableBox305m => 120,
            Self::Rj45Pack20 => 10,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CableInventory {
    pub cable_cm: u32,
    pub connectors: u32,
    /// Unplugged, already crimped leads. They cannot be converted back into raw stock.
    pub patch_cables_cm: Vec<u32>,
    /// Jacket color for each reusable lead, parallel to `patch_cables_cm`.
    /// Missing entries (including legacy saves) are treated as white.
    #[serde(default)]
    pub patch_cable_colors: Vec<CableColor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CableQuote {
    pub length_cm: u32,
    pub reused: bool,
    pub color: CableColor,
}
impl CableQuote {
    pub fn cable_required(self) -> u32 {
        if self.reused { 0 } else { self.length_cm }
    }
    pub fn connectors_required(self) -> u32 {
        if self.reused { 0 } else { 2 }
    }
}

impl NetworkSim {
    pub(crate) fn normalize_patch_cable_colors(&mut self) {
        self.cable_inventory.patch_cable_colors.resize(
            self.cable_inventory.patch_cables_cm.len(),
            CableColor::White,
        );
        self.cable_inventory
            .patch_cable_colors
            .truncate(self.cable_inventory.patch_cables_cm.len());
    }

    pub fn cable_inventory(&self) -> &CableInventory {
        &self.cable_inventory
    }

    pub(crate) fn buy_cable_supply(&mut self, supply: CableSupply) -> Result<(), SimError> {
        let price = supply.price();
        if self.money < price {
            return Err(SimError::InsufficientFunds {
                needed: price,
                available: self.money,
            });
        }
        match supply {
            CableSupply::CableBox305m => {
                self.cable_inventory.cable_cm = self
                    .cable_inventory
                    .cable_cm
                    .checked_add(30_500)
                    .ok_or(SimError::CableInventoryFull)?
            }
            CableSupply::Rj45Pack20 => {
                self.cable_inventory.connectors = self
                    .cable_inventory
                    .connectors
                    .checked_add(20)
                    .ok_or(SimError::CableInventoryFull)?
            }
        }
        self.money -= price;
        Ok(())
    }

    /// Straight socket-to-socket distance plus 10% slack, rounded up to a cm.
    pub fn minimum_cable_length(&self, a: PortId, b: PortId) -> Result<u32, SimError> {
        let placements: Vec<_> = [a, b]
            .iter()
            .map(|id| {
                let p = self.port(*id).ok_or(SimError::PortNotFound(*id))?;
                let device = self
                    .device(p.device)
                    .ok_or(SimError::CableDevicesNotInstalled)?;
                let placement = device.rack.ok_or(SimError::CableDevicesNotInstalled)?;
                let index = device
                    .ports()
                    .iter()
                    .position(|port| port == id)
                    .ok_or(SimError::PortNotFound(*id))?;
                let (x, y) = device.kind.port_position_normalized(index);
                Ok((
                    placement.rack,
                    x * RACK_FACE_WIDTH_CM,
                    -f32::from(placement.unit) * (RACK_FACE_HEIGHT_CM + RACK_GAP_CM)
                        + y * RACK_FACE_HEIGHT_CM,
                ))
            })
            .collect::<Result<_, _>>()?;
        let cm = if placements[0].0 == placements[1].0 {
            (placements[0].1 - placements[1].1).hypot(placements[0].2 - placements[1].2)
        } else {
            // Racks have no world positions yet; retain the inter-rack route allowance.
            500.0
        };
        Ok((cm * 1.10).ceil() as u32)
    }

    /// A quote is read-only; validation or canceling never spends stock.
    pub fn quote_cable(
        &self,
        a: PortId,
        b: PortId,
        length_cm: Option<u32>,
    ) -> Result<CableQuote, SimError> {
        self.quote_colored_cable(a, b, length_cm, CableColor::White)
    }

    pub fn quote_colored_cable(
        &self,
        a: PortId,
        b: PortId,
        length_cm: Option<u32>,
        color: CableColor,
    ) -> Result<CableQuote, SimError> {
        self.validate_cable_endpoints(a, b)?;
        let minimum_cm = self.minimum_cable_length(a, b)?;
        if let Some(length_cm) = length_cm {
            if length_cm < minimum_cm {
                return Err(SimError::CableTooShort { minimum_cm });
            }
            if length_cm > 10_000 {
                return Err(SimError::CableTooLong);
            }
        }
        let existing = self
            .cable_inventory
            .patch_cables_cm
            .iter()
            .copied()
            .enumerate()
            .filter(|(index, length)| {
                *length == length_cm.unwrap_or(minimum_cm)
                    && self
                        .cable_inventory
                        .patch_cable_colors
                        .get(*index)
                        .copied()
                        .unwrap_or_default()
                        == color
            })
            .map(|(_, length)| length)
            .min();
        Ok(CableQuote {
            length_cm: existing.or(length_cm).unwrap_or(minimum_cm),
            reused: existing.is_some(),
            color,
        })
    }

    pub(crate) fn consume_cable(&mut self, quote: CableQuote) -> Result<(), SimError> {
        if quote.reused {
            self.normalize_patch_cable_colors();
            let index = self
                .cable_inventory
                .patch_cables_cm
                .iter()
                .enumerate()
                .position(|(index, cm)| {
                    *cm == quote.length_cm
                        && self
                            .cable_inventory
                            .patch_cable_colors
                            .get(index)
                            .copied()
                            .unwrap_or_default()
                            == quote.color
                })
                .ok_or(SimError::CableInventoryFull)?;
            self.cable_inventory.patch_cables_cm.remove(index);
            if index < self.cable_inventory.patch_cable_colors.len() {
                self.cable_inventory.patch_cable_colors.remove(index);
            }
        } else {
            if self.cable_inventory.cable_cm < quote.length_cm {
                return Err(SimError::InsufficientCable {
                    needed_cm: quote.length_cm,
                    available_cm: self.cable_inventory.cable_cm,
                });
            }
            if self.cable_inventory.connectors < 2 {
                return Err(SimError::InsufficientConnectors {
                    available: self.cable_inventory.connectors,
                });
            }
            self.cable_inventory.cable_cm -= quote.length_cm;
            self.cable_inventory.connectors -= 2;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed_server_pair() -> (NetworkSim, PortId, PortId) {
        let mut sim = NetworkSim::new();
        let mut devices = Vec::new();
        for unit in [1, 2] {
            let SimEvent::DeviceAdded(device) = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Server,
                })
                .unwrap()[0]
            else {
                panic!("device expected")
            };
            sim.execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit,
            })
            .unwrap();
            devices.push(device);
        }
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::CableBox305m,
        })
        .unwrap();
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::Rj45Pack20,
        })
        .unwrap();
        for device in &devices {
            sim.execute(Command::SetPower {
                device: *device,
                powered: true,
            })
            .unwrap();
        }
        let ports = devices
            .iter()
            .map(|device| sim.device(*device).unwrap().ports()[0])
            .collect::<Vec<_>>();
        (sim, ports[0], ports[1])
    }

    #[test]
    fn physical_link_negotiates_slowest_endpoint() {
        let (mut sim, a, b) = installed_server_pair();
        sim.execute(Command::SetPortSpeed {
            port: a,
            speed: LinkSpeed::Mbps100,
        })
        .unwrap();
        sim.execute(Command::SetPortSpeed {
            port: b,
            speed: LinkSpeed::Mbps100,
        })
        .unwrap();
        sim.execute(Command::Connect { a, b }).unwrap();
        assert!(sim.port_link_up(a));
        assert_eq!(sim.port_link_speed(a), Some(LinkSpeed::Mbps100));
        assert_eq!(sim.port_link_speed(a).unwrap().mbps(), 100);
    }

    #[test]
    fn physical_link_requires_power_and_valid_copper_length() {
        let (mut sim, a, b) = installed_server_pair();
        sim.execute(Command::ConnectCable {
            a,
            b,
            length_cm: 10_001,
        })
        .expect_err("100 m is the copper limit");
        sim.execute(Command::Connect { a, b }).unwrap();
        assert!(sim.port_link_up(a));
        let device = sim.port(b).unwrap().device;
        sim.execute(Command::SetPower {
            device,
            powered: false,
        })
        .unwrap();
        assert!(!sim.port_link_up(a));
        assert_eq!(sim.port_link_speed(a), None);
    }

    #[test]
    fn copper_ports_can_connect_directly() {
        let (mut sim, a, b) = installed_server_pair();
        sim.execute(Command::Connect { a, b }).unwrap();
        assert!(sim.port_link_up(a));
    }

    #[test]
    fn loading_trims_automatic_leads_but_preserves_custom_cuts_and_inventory() {
        let mut sim = NetworkSim::new();
        for unit in [1, 12] {
            let SimEvent::DeviceAdded(device) = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Switch,
                })
                .unwrap()[0]
            else {
                panic!("device expected")
            };
            sim.execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit,
            })
            .unwrap();
        }
        for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
            sim.execute(Command::BuyCableSupply { supply }).unwrap();
        }
        let devices: Vec<_> = sim.devices().map(|d| d.ports().to_vec()).collect();
        sim.execute(Command::Connect {
            a: devices[0][0],
            b: devices[1][0],
        })
        .unwrap();
        sim.execute(Command::ConnectCable {
            a: devices[0][1],
            b: devices[1][1],
            length_cm: 200,
        })
        .unwrap();
        let auto = sim.link_for_port(devices[0][0]).unwrap().id;
        let expected = sim.link(auto).unwrap().length_cm;
        sim.links.get_mut(&auto).unwrap().length_cm = 125;
        let inventory = sim.cable_inventory().clone();
        sim.rebuild_indexes();
        assert_eq!(sim.link(auto).unwrap().length_cm, expected);
        assert_eq!(sim.link_for_port(devices[0][1]).unwrap().length_cm, 200);
        assert_eq!(sim.cable_inventory(), &inventory);
        sim.rebuild_indexes();
        assert_eq!(sim.link(auto).unwrap().length_cm, expected);

        assert!(!sim.port_link_up(devices[0][0]));
        for device in sim.devices.values_mut() {
            device.powered = true;
        }
        assert!(sim.port_link_up(devices[0][0]));
        sim.ports.get_mut(&devices[1][0]).unwrap().enabled = false;
        assert!(!sim.port_link_up(devices[0][0]));
        sim.ports.get_mut(&devices[1][0]).unwrap().enabled = true;
        let remote = sim.port(devices[1][0]).unwrap().device;
        sim.execute(Command::SetPower {
            device: remote,
            powered: false,
        })
        .unwrap();
        assert!(!sim.port_link_up(devices[0][0]));
    }
}
