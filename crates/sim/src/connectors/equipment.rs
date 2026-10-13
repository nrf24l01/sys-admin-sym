use super::*;
use crate::{DeviceKind, PciCard, ServerPartKind};
use std::{collections::HashMap, sync::OnceLock};

/// Shared JSON schema used by simulation, game UI and the texture mapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentSocketConfig {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub texture: String,
    #[serde(default)]
    pub textures: HashMap<String, String>,
    #[serde(default)]
    pub texture_uvs: HashMap<String, [f32; 4]>,
    #[serde(default)]
    pub ports: HashMap<String, Vec<SocketMapping>>,
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketMapping {
    pub left_down: [f32; 2],
    pub right_up: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    pub side: String,
}

impl EquipmentSocketConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (group, ports) in &self.ports {
            if !connector_catalog()
                .types
                .iter()
                .any(|definition| definition.id.key() == group)
            {
                return Err(format!("unknown connector group {group}"));
            }
            for (i, port) in ports.iter().enumerate() {
                let [l, t] = port.left_down;
                let [r, b] = port.right_up;
                if !rect_valid([l, t, r, b]) || !matches!(port.side.as_str(), "front" | "back") {
                    return Err(format!("invalid {group} socket {i}"));
                }
                if port.index.is_some()
                    && ports[..i]
                        .iter()
                        .any(|p| p.side == port.side && p.index == port.index)
                {
                    return Err(format!("duplicate {group} socket index on {}", port.side));
                }
            }
        }
        if self.texture_uvs.values().any(|uv| !rect_valid(*uv)) {
            return Err("invalid texture crop".into());
        }
        Ok(())
    }
}
fn rect_valid(rect: [f32; 4]) -> bool {
    let [l, t, r, b] = rect;
    rect.iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        && l < r
        && t < b
}

pub fn equipment_socket_configs() -> &'static [EquipmentSocketConfig] {
    static CONFIGS: OnceLock<Vec<EquipmentSocketConfig>> = OnceLock::new();
    CONFIGS.get_or_init(|| {
        connector_catalog()
            .equipment_maps
            .iter()
            .map(|filename| {
                let embedded = match filename.as_str() {
                    "switch_config.json" => {
                        include_str!("../../../../assets/equipment/switch_config.json")
                    }
                    "switch_4x_config.json" => {
                        include_str!("../../../../assets/equipment/switch_4x_config.json")
                    }
                    "server_config.json" => {
                        include_str!("../../../../assets/equipment/server_config.json")
                    }
                    "router_config.json" => {
                        include_str!("../../../../assets/equipment/router_config.json")
                    }
                    "ups_config.json" => {
                        include_str!("../../../../assets/equipment/ups_config.json")
                    }
                    "pdu_config.json" => {
                        include_str!("../../../../assets/equipment/pdu_config.json")
                    }
                    "patch_panel_config.json" => {
                        include_str!("../../../../assets/equipment/patch_panel_config.json")
                    }
                    "cable_manager_config.json" => {
                        include_str!("../../../../assets/equipment/cable_manager_config.json")
                    }
                    _ => "null", // Additional maps are read from the installed asset directory.
                };
                let config: EquipmentSocketConfig =
                    crate::equipment_config::equipment_catalog(filename, embedded);
                config
                    .validate()
                    .unwrap_or_else(|error| panic!("invalid socket map {filename}: {error}"));
                config
            })
            .collect()
    })
}

impl DeviceKind {
    pub fn connector_config(&self) -> Option<&'static EquipmentSocketConfig> {
        let (kind, model) = match self {
            Self::Switch(sw) => ("switch", Some(sw.model.spec().id.as_str())),
            Self::Server(server) => ("server", Some(server.model.as_str())),
            Self::Router(_) => ("router", None),
            Self::Ups(_) => ("ups", None),
            Self::Pdu(_) => ("pdu", None),
            Self::PatchPanel(_) => ("patch_panel", None),
            Self::CableManager(_) => ("cable_manager", None),
        };
        let configs = equipment_socket_configs();
        configs
            .iter()
            .find(|c| c.kind == kind && c.model.as_deref() == model)
            .or_else(|| configs.iter().find(|c| c.kind == kind && c.model.is_none()))
    }

    pub fn connector_face_uv(&self, side: RackSide) -> Option<[f32; 4]> {
        if let Self::Server(server) = self {
            return Some(if side == RackSide::Front {
                server.model().artwork.front.uv
            } else {
                server.model().artwork.rear.uv
            });
        }
        if let Some(model) = self.network_model() {
            return Some(if side == RackSide::Front {
                model.artwork.front.uv
            } else {
                model.artwork.rear.uv
            });
        }
        self.connector_config()?
            .texture_uvs
            .get(side_name(side))
            .copied()
    }

    /// One geometry lookup for network sockets, power inlets/outlets and anchors.
    /// Network indices are device port indices; power indices are inlet/outlet indices.
    pub fn connector_socket(
        &self,
        connector: &dyn Connector,
        side: RackSide,
        index: usize,
    ) -> Option<ConnectorSocket> {
        let id = connector.connector_type();
        if let Some(model) = self.network_model() {
            if connector.definition().role == ConnectorRole::Inlet {
                let inlet = model.power_inlets.get(index)?;
                let expected = match inlet.connector {
                    crate::PowerInletConnector::IecC14 => ConnectorType::IecC14,
                    crate::PowerInletConnector::CiscoFourPin => ConnectorType::CiscoFourPin,
                };
                if inlet.side != side || expected != id {
                    return None;
                }
                return Some(ConnectorSocket {
                    connector: id,
                    index,
                    side,
                    rect: inlet.socket.rect,
                });
            }
            let port = model.ports.get(index)?;
            if port.side != side || port.connector.connector_type() != id {
                return None;
            }
            let rect = if id == ConnectorType::Sfp {
                port.optical_socket.as_ref().unwrap_or(&port.socket).rect
            } else {
                port.socket.rect
            };
            return Some(ConnectorSocket {
                connector: id,
                index,
                side,
                rect,
            });
        }
        if let Self::Server(server) = self {
            let rect = if connector.definition().role == ConnectorRole::Inlet {
                let inlet = server.model().power_inlets.get(index)?;
                if inlet.side() != side || inlet.connector.unwrap_or(ConnectorType::IecC14) != id {
                    return None;
                }
                inlet.rect
            } else if index < server.model().onboard_ports.len() {
                let port = &server.model().onboard_ports[index].socket;
                if port.side() != side || port.connector.unwrap_or(ConnectorType::Rj45) != id {
                    return None;
                }
                port.rect
            } else {
                if side != RackSide::Rear {
                    return None;
                }
                let hardware = server.hardware.as_ref()?;
                let port = server.ports.get(index)?;
                let (slot_index, card_index, ports) = hardware
                    .card_ports
                    .iter()
                    .enumerate()
                    .find_map(|(slot, ports)| {
                        ports
                            .iter()
                            .position(|p| p == port)
                            .map(|i| (slot, i, ports))
                    })?;
                let part_id = hardware.pcie.get(slot_index)?.as_deref()?;
                let part = crate::server_catalog()
                    .parts
                    .iter()
                    .find(|p| p.id == part_id)?;
                let ServerPartKind::PciCard {
                    card:
                        PciCard::Ethernet {
                            connector: card_connector,
                            socket_rects,
                            ..
                        },
                } = &part.kind
                else {
                    return None;
                };
                if card_connector.connector_type() != id {
                    return None;
                }
                let slot = server.model().chassis.pcie_slots.get(slot_index)?;
                let [l, t, r, b] = slot.face_rect;
                if let Some([sl, st, sr, sb]) = socket_rects.get(card_index) {
                    [
                        l + (r - l) * sl,
                        t + (b - t) * st,
                        l + (r - l) * sr,
                        t + (b - t) * sb,
                    ]
                } else {
                    // Legacy external card catalogs may omit their local socket layout.
                    let (x, y) = slot.port_position(card_index, ports.len());
                    let [width, height] = connector.definition().default_size;
                    let width = width.min((r - l) / (ports.len() + 1) as f32);
                    let height = height.min(b - t);
                    [
                        x - width * 0.5,
                        y - height * 0.5,
                        x + width * 0.5,
                        y + height * 0.5,
                    ]
                }
            };
            return Some(ConnectorSocket {
                connector: id,
                index,
                side,
                rect,
            });
        }
        let ports = self.connector_config()?.ports.get(id.key())?;
        let legacy_index = match self {
            Self::Switch(sw) if id == ConnectorType::Sfp => index.checked_sub(
                sw.model
                    .spec()
                    .ports
                    .iter()
                    .filter(|p| p.connector != crate::PortConnector::Sfp)
                    .count(),
            )?,
            Self::PatchPanel(_) => index / 2,
            _ => index,
        };
        let mapped = ports
            .iter()
            .filter(|p| p.side == side_name(side))
            .enumerate()
            .find(|(ordinal, p)| {
                p.index
                    .map_or(*ordinal == legacy_index, |mapped| mapped == index)
            })?
            .1;
        Some(ConnectorSocket {
            connector: id,
            index,
            side,
            rect: [
                mapped.left_down[0],
                mapped.left_down[1],
                mapped.right_up[0],
                mapped.right_up[1],
            ],
        })
    }

    pub fn find_connector_socket(
        &self,
        connector: &dyn Connector,
        index: usize,
    ) -> Option<ConnectorSocket> {
        [RackSide::Front, RackSide::Rear]
            .into_iter()
            .find_map(|side| self.connector_socket(connector, side, index))
    }

    pub fn power_connector_socket(&self, index: usize) -> Option<ConnectorSocket> {
        for definition in &connector_catalog().types {
            if definition.role != ConnectorRole::Inlet {
                continue;
            }
            if let Some(socket) = self.find_connector_socket(&definition.id, index) {
                return Some(socket);
            }
        }
        None
    }
}
fn side_name(side: RackSide) -> &'static str {
    match side {
        RackSide::Front => "front",
        RackSide::Rear => "back",
    }
}
