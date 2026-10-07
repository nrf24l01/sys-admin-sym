use cloud_provider_sim::{DeviceKind, PortConnector, RackSide};
use serde::Deserialize;
use std::sync::OnceLock;

/// Data driven equipment artwork and socket map. The array order within a
/// connector group is the device port order, so a new asset needs no Rust
/// changes when its socket rectangles are measured.
#[derive(Debug, Deserialize)]
struct EquipmentConfig {
    #[serde(rename = "type")]
    kind: String,
    #[allow(dead_code)]
    texture: String,
    ports: std::collections::HashMap<String, Vec<PortRect>>,
}

#[derive(Debug, Deserialize)]
struct PortRect {
    left_down: [f32; 2],
    right_up: [f32; 2],
    #[allow(dead_code)]
    speed: Option<String>,
    side: String,
}

fn configs() -> &'static Vec<EquipmentConfig> {
    static CONFIGS: OnceLock<Vec<EquipmentConfig>> = OnceLock::new();
    CONFIGS.get_or_init(|| {
        [
            include_str!("../../../../assets/equipment/switch_config.json"),
            include_str!("../../../../assets/equipment/ups_config.json"),
            include_str!("../../../../assets/equipment/server_config.json"),
            include_str!("../../../../assets/equipment/router_config.json"),
            include_str!("../../../../assets/equipment/patch_panel_config.json"),
            include_str!("../../../../assets/equipment/pdu_config.json"),
            include_str!("../../../../assets/equipment/cable_manager_config.json"),
        ]
        .into_iter()
        .filter_map(|json| match serde_json::from_str(json) {
            Ok(config) => Some(config),
            Err(error) => {
                eprintln!("Ignoring invalid equipment config: {error}");
                None
            }
        })
        .collect()
    })
}

pub(super) fn equipment_port_position(
    kind: &DeviceKind,
    connector: PortConnector,
    side: RackSide,
    index: usize,
) -> Option<(f32, f32)> {
    let type_name = match kind {
        DeviceKind::Switch(_) => "switch",
        DeviceKind::Ups(_) => "ups",
        DeviceKind::Server(_) => "server",
        DeviceKind::Router(_) => "router",
        DeviceKind::PatchPanel(_) => "patch_panel",
        DeviceKind::Pdu(_) => "pdu",
        DeviceKind::CableManager(_) => "cable_manager",
    };
    let connector_name = match connector {
        PortConnector::Rj45 | PortConnector::Lc => "rj-45",
        PortConnector::Sfp => "sfp",
    };
    let side_name = match side {
        RackSide::Front => "front",
        RackSide::Rear => "back",
    };
    let group_index = match kind {
        DeviceKind::Switch(_) if matches!(connector, PortConnector::Sfp) => {
            index.saturating_sub(24)
        }
        DeviceKind::PatchPanel(_) => index / 2,
        _ => index,
    };
    configs()
        .iter()
        .find(|config| config.kind == type_name)
        .and_then(|config| config.ports.get(connector_name))
        .and_then(|ports| {
            ports
                .iter()
                .filter(|port| port.side == side_name)
                .nth(group_index)
        })
        .map(|port| {
            (
                (port.left_down[0] + port.right_up[0]) * 0.5,
                (port.left_down[1] + port.right_up[1]) * 0.5,
            )
        })
}

pub(super) fn equipment_power_port_position(
    kind: &DeviceKind,
    connector: &str,
    index: usize,
) -> Option<(f32, f32)> {
    let type_name = match kind {
        DeviceKind::Ups(_) => "ups",
        DeviceKind::Pdu(_) => "pdu",
        DeviceKind::Server(_) => "server",
        DeviceKind::Switch(_) => "switch",
        DeviceKind::Router(_) => "router",
        _ => return None,
    };
    configs()
        .iter()
        .find(|config| config.kind == type_name)
        .and_then(|config| config.ports.get(connector))
        .and_then(|ports| ports.get(index))
        .map(|port| {
            (
                (port.left_down[0] + port.right_up[0]) * 0.5,
                (port.left_down[1] + port.right_up[1]) * 0.5,
            )
        })
}

pub(super) fn equipment_power_port_rect(
    kind: &DeviceKind,
    connector: &str,
    index: usize,
    panel: bevy_egui::egui::Rect,
) -> Option<bevy_egui::egui::Rect> {
    let type_name = match kind {
        DeviceKind::Ups(_) => "ups",
        DeviceKind::Pdu(_) => "pdu",
        DeviceKind::Server(_) => "server",
        DeviceKind::Switch(_) => "switch",
        DeviceKind::Router(_) => "router",
        _ => return None,
    };
    let port = configs()
        .iter()
        .find(|c| c.kind == type_name)?
        .ports
        .get(connector)?
        .get(index)?;
    let position = |p: [f32; 2]| {
        bevy_egui::egui::pos2(
            panel.left() + panel.width() * p[0],
            panel.top() + panel.height() * p[1],
        )
    };
    Some(bevy_egui::egui::Rect::from_two_pos(
        position(port.left_down),
        position(port.right_up),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_switch_map_is_readable() {
        let position = equipment_port_position(
            &DeviceKind::Switch(cloud_provider_sim::Switch {
                ports: vec![],
                vlans: vec![],
            }),
            PortConnector::Rj45,
            RackSide::Front,
            0,
        );
        assert_eq!(position, Some((0.342, 0.37)));
        let config = configs()
            .iter()
            .find(|config| config.kind == "switch")
            .unwrap();
        assert_eq!(config.ports["rj-45"].len(), 24);
        assert_eq!(config.ports["sfp"].len(), 4);
    }

    #[test]
    fn patch_panel_has_all_24_paired_positions_on_each_face() {
        let kind = DeviceKind::PatchPanel(cloud_provider_sim::PatchPanel { ports: vec![] });
        let config = configs()
            .iter()
            .find(|config| config.kind == "patch_panel")
            .unwrap();
        assert_eq!(config.ports["rj-45"].len(), 48);
        let mut positions = Vec::new();
        for slot in 0..24 {
            let rear =
                equipment_port_position(&kind, PortConnector::Rj45, RackSide::Rear, slot * 2)
                    .unwrap();
            let front =
                equipment_port_position(&kind, PortConnector::Rj45, RackSide::Front, slot * 2 + 1)
                    .unwrap();
            assert_eq!(front, rear);
            let expected = kind.port_position_normalized(slot * 2);
            assert!((front.0 - expected.0).abs() < 0.0001 && (front.1 - expected.1).abs() < 0.0001);
            assert!(
                !positions.contains(&front),
                "every socket needs its own position"
            );
            positions.push(front);
        }
    }
}
