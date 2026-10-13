//! Thin egui projection of the simulation's common connector/socket interface.
use bevy_egui::egui;
use cloud_provider_sim::{Connector, ConnectorSocket, DeviceKind, PortConnector, RackSide};

pub(super) fn connector_sprite_uv(name: &str) -> egui::Rect {
    let [l, t, r, b] = cloud_provider_sim::connector_catalog().sprite(name).uv;
    egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b))
}

fn panel_rect(socket: ConnectorSocket, panel: egui::Rect) -> egui::Rect {
    let [l, t, r, b] = socket.rect;
    egui::Rect::from_min_max(
        egui::pos2(
            panel.left() + panel.width() * l,
            panel.top() + panel.height() * t,
        ),
        egui::pos2(
            panel.left() + panel.width() * r,
            panel.top() + panel.height() * b,
        ),
    )
}

pub(super) fn equipment_socket_rect(
    kind: &DeviceKind,
    connector: &dyn Connector,
    side: RackSide,
    index: usize,
    panel: egui::Rect,
) -> Option<egui::Rect> {
    kind.connector_socket(connector, side, index)
        .map(|socket| panel_rect(socket, panel))
}

pub(super) fn equipment_face_uv(kind: &DeviceKind, side: RackSide) -> Option<egui::Rect> {
    let [l, t, r, b] = kind.connector_face_uv(side)?;
    Some(egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b)))
}

pub(super) fn equipment_port_rect(
    kind: &DeviceKind,
    connector: PortConnector,
    side: RackSide,
    index: usize,
    panel: egui::Rect,
) -> Option<egui::Rect> {
    equipment_socket_rect(kind, &connector, side, index, panel)
}

pub(super) fn equipment_port_position(
    kind: &DeviceKind,
    connector: PortConnector,
    side: RackSide,
    index: usize,
) -> Option<(f32, f32)> {
    kind.connector_socket(&connector, side, index)
        .map(|socket| socket.center())
}

#[cfg(test)]
pub(super) fn equipment_power_port_rect(
    kind: &DeviceKind,
    connector: &str,
    index: usize,
    panel: egui::Rect,
) -> Option<egui::Rect> {
    let connector: cloud_provider_sim::ConnectorType =
        serde_json::from_value(serde_json::Value::String(connector.into())).ok()?;
    [RackSide::Front, RackSide::Rear]
        .into_iter()
        .find_map(|side| equipment_socket_rect(kind, &connector, side, index, panel))
}

#[cfg(test)]
fn configs() -> &'static [cloud_provider_sim::EquipmentSocketConfig] {
    cloud_provider_sim::equipment_socket_configs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_switch_map_is_readable() {
        let position = equipment_port_position(
            &DeviceKind::Switch(cloud_provider_sim::Switch {
                model: cloud_provider_sim::SwitchModel::default(),
                services: cloud_provider_sim::SwitchServices::default(),
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
    fn four_x_front_socket_centers_match_its_dedicated_photo() {
        let kind = DeviceKind::Switch(cloud_provider_sim::Switch {
            model: cloud_provider_sim::SwitchModel::Catalyst24T4X,
            services: Default::default(),
            ports: vec![],
            vlans: vec![],
        });
        let mut positions = Vec::new();
        for index in 0..24 {
            let position =
                equipment_port_position(&kind, PortConnector::Rj45, RackSide::Front, index)
                    .unwrap();
            assert!((0.0..1.0).contains(&position.0) && (0.0..1.0).contains(&position.1));
            assert!(!positions.contains(&position));
            positions.push(position);
        }
        assert!(positions[0].1 < positions[1].1);
        assert!(positions[12].0 > positions[10].0);
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
