use crate::*;
use serde::{Deserialize, Serialize};

/// Jacket colors for copper leads and catalog cable assemblies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CableColor {
    #[default]
    White,
    Gray,
    Blue,
    Orange,
    Red,
    Aqua,
    Yellow,
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

/// Saved defaults for automatically sized copper leads. The extra allowance is
/// added to the routed minimum, which already includes installation slack.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CableSettings {
    pub extra_percent: u16,
    pub extra_cm: u32,
    pub reuse_longer_leads: bool,
}

impl CableSettings {
    pub fn validate(self) -> Result<(), SimError> {
        if self.extra_percent > 100 || self.extra_cm > 10_000 {
            return Err(SimError::InvalidCableSettings);
        }
        Ok(())
    }

    fn automatic_length(self, minimum_cm: u32) -> Result<u32, SimError> {
        self.validate()?;
        let minimum = u64::from(minimum_cm);
        let length = minimum
            + (minimum * u64::from(self.extra_percent)).div_ceil(100)
            + u64::from(self.extra_cm);
        if length > 10_000 {
            return Err(SimError::CableTooLong);
        }
        Ok(length as u32)
    }
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

    pub fn cable_settings(&self) -> CableSettings {
        self.cable_settings
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

    /// Straight socket-to-socket distance plus 5% installation slack, rounded
    /// up to a cm. Routed connections use `minimum_routed_cable_length` instead.
    pub fn minimum_cable_length(&self, a: PortId, b: PortId) -> Result<u32, SimError> {
        let placements = [self.port_position(a)?, self.port_position(b)?];
        let cm = if placements[0].0 == placements[1].0 {
            (placements[0].1 - placements[1].1).hypot(placements[0].2 - placements[1].2)
        } else {
            self.room_distance(placements[0], placements[1])?
        };
        Ok((cm * 1.05).ceil() as u32)
    }

    /// Routed leads follow their selected fixing points. The port legs receive
    /// 5% installation slack; spans between anchors remain exact.
    pub fn minimum_routed_cable_length(
        &self,
        a: PortId,
        b: PortId,
        route: &[crate::CableRoutePoint],
    ) -> Result<u32, SimError> {
        if route.is_empty() {
            return self.minimum_cable_length(a, b);
        }
        let endpoints = [self.port_position(a)?, self.port_position(b)?];
        let mut points = Vec::with_capacity(route.len() + 2);
        points.push(endpoints[0]);
        for point in route {
            self.validate_route_point(point)?;
            if let Some(id) = point.room_anchor_id() {
                let anchor = self
                    .room
                    .cable_anchors
                    .iter()
                    .find(|anchor| anchor.id == id)
                    .ok_or(SimError::RoomAnchorNotFound(id))?;
                points.push((
                    RackId(0),
                    f32::from(anchor.position.x_cm),
                    f32::from(anchor.position.y_cm),
                ));
            } else {
                points.push((
                    point.rack,
                    f32::from(point.offset_cm) / 48.0 * RACK_FACE_WIDTH_CM,
                    -f32::from(point.unit) * (RACK_FACE_HEIGHT_CM + RACK_GAP_CM),
                ));
            }
        }
        points.push(endpoints[1]);
        let mut total = 0.0;
        for (index, pair) in points.windows(2).enumerate() {
            let previous = pair[0];
            let next = pair[1];
            let segment = if previous.0 == next.0 {
                (previous.1 - next.1).hypot(previous.2 - next.2)
            } else {
                self.room_distance(previous, next)?
            };
            total += if index == 0 || index + 1 == points.len() - 1 {
                segment * 1.05
            } else {
                segment
            };
        }
        Ok(total.ceil() as u32)
    }

    fn room_distance(&self, a: (RackId, f32, f32), b: (RackId, f32, f32)) -> Result<f32, SimError> {
        let coordinates = |point: (RackId, f32, f32)| -> Result<(f32, f32, f32), SimError> {
            if point.0 == RackId(0) {
                return Ok((point.1, point.2, 250.0));
            }
            if self.rack(point.0).is_none() {
                return Err(SimError::RackNotFound(point.0));
            }
            let room = self.rack_room_position(point.0);
            Ok((
                f32::from(room.x_cm) + point.1 - RACK_FACE_WIDTH_CM * 0.5,
                f32::from(room.y_cm),
                -point.2,
            ))
        };
        let a = coordinates(a)?;
        let b = coordinates(b)?;
        Ok(((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt())
    }

    fn port_position(&self, id: PortId) -> Result<(crate::RackId, f32, f32), SimError> {
        if let Some(outlet) = self.network_outlet(id) {
            return Ok(match outlet.kind {
                crate::NetworkOutletKind::Uplink { position } => (
                    crate::RackId(0),
                    f32::from(position.x_cm),
                    f32::from(position.y_cm),
                ),
                crate::NetworkOutletKind::Lan { rack } => {
                    let units = self.rack(rack).ok_or(SimError::RackNotFound(rack))?.units;
                    (
                        rack,
                        RACK_FACE_WIDTH_CM * 0.5,
                        -f32::from(units) * (RACK_FACE_HEIGHT_CM + RACK_GAP_CM),
                    )
                }
            });
        }
        let p = self.port(id).ok_or(SimError::PortNotFound(id))?;
        let device = self
            .device(p.device)
            .ok_or(SimError::CableDevicesNotInstalled)?;
        let placement = device.rack.ok_or(SimError::CableDevicesNotInstalled)?;
        let index = device
            .ports()
            .iter()
            .position(|port| port == &id)
            .ok_or(SimError::PortNotFound(id))?;
        let (x, y) = device.kind.port_position_normalized(index);
        Ok((
            placement.rack,
            x * RACK_FACE_WIDTH_CM,
            -f32::from(placement.unit) * (RACK_FACE_HEIGHT_CM + RACK_GAP_CM)
                + y * RACK_FACE_HEIGHT_CM,
        ))
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
        self.quote_cable_for_minimum(minimum_cm, length_cm, color)
    }

    fn quote_cable_for_minimum(
        &self,
        minimum_cm: u32,
        length_cm: Option<u32>,
        color: CableColor,
    ) -> Result<CableQuote, SimError> {
        let target = match length_cm {
            Some(length) => length,
            None => self.cable_settings.automatic_length(minimum_cm)?,
        };
        if target < minimum_cm {
            return Err(SimError::CableTooShort { minimum_cm });
        }
        if target > 10_000 {
            return Err(SimError::CableTooLong);
        }
        let allow_longer = length_cm.is_none() && self.cable_settings.reuse_longer_leads;
        let existing = self
            .cable_inventory
            .patch_cables_cm
            .iter()
            .copied()
            .enumerate()
            .filter(|(index, length)| {
                (*length == target || (allow_longer && *length >= target && *length <= 10_000))
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
            length_cm: existing.unwrap_or(target),
            reused: existing.is_some(),
            color,
        })
    }

    pub fn quote_routed_colored_cable(
        &self,
        a: PortId,
        b: PortId,
        length_cm: Option<u32>,
        color: CableColor,
        route: &[crate::CableRoutePoint],
    ) -> Result<CableQuote, SimError> {
        self.validate_cable_endpoints(a, b)?;
        let minimum_cm = self.minimum_routed_cable_length(a, b, route)?;
        self.quote_cable_for_minimum(minimum_cm, length_cm, color)
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
            let outlet = OutletId {
                source: SourceId::Rack(RackId(1)),
                index: unit - 1,
            };
            sim.execute(Command::ConnectPower {
                outlet,
                endpoint: PowerEndpoint::Device(device),
            })
            .unwrap();
            sim.execute(Command::SetPower {
                device,
                powered: false,
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
    fn loading_trims_legacy_automatic_leads_but_preserves_custom_cuts_and_inventory() {
        let mut sim = NetworkSim::new();
        for (outlet_index, unit) in [1, 12].into_iter().enumerate() {
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
            let outlet = OutletId {
                source: SourceId::Rack(RackId(1)),
                index: outlet_index as u8,
            };
            sim.execute(Command::ConnectPower {
                outlet,
                endpoint: PowerEndpoint::Device(device),
            })
            .unwrap();
            sim.execute(Command::SetPower {
                device,
                powered: false,
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
        let legacy = sim.links.get_mut(&auto).unwrap();
        legacy.length_cm = 125;
        legacy.auto_length = true;
        let inventory = sim.cable_inventory().clone();
        sim.rebuild_indexes();
        assert_eq!(sim.link(auto).unwrap().length_cm, expected);
        assert_eq!(sim.link_for_port(devices[0][1]).unwrap().length_cm, 200);
        assert_eq!(sim.cable_inventory(), &inventory);
        sim.rebuild_indexes();
        assert_eq!(sim.link(auto).unwrap().length_cm, expected);

        assert!(!sim.port_link_up(devices[0][0]));
        for device in sim.devices().map(|device| device.id).collect::<Vec<_>>() {
            sim.execute(Command::SetPower {
                device,
                powered: true,
            })
            .unwrap();
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
