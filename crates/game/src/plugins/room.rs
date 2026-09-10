use crate::app::{GameSet, SimSnapshot};
use bevy::prelude::*;
use cloud_provider_sim::{DeviceId, DeviceKind};

#[derive(Component)]
struct DomainDeviceVisual {
    _device: DeviceId,
}

#[derive(Component)]
struct DomainLinkVisual;

#[derive(Component)]
struct ProjectedVisual;

#[derive(Resource, Default)]
struct RenderedRevision(u64);

pub struct RoomPlugin;

impl Plugin for RoomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderedRevision>()
            .add_systems(Startup, setup_room)
            .add_systems(Update, sync_device_visuals.in_set(GameSet::Presentation));
    }
}

fn setup_room(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(7.5, 5.5, 10.0).looking_at(Vec3::new(0.0, 2.2, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 5_500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(14.0, 14.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.09, 0.11),
            perceptual_roughness: 0.9,
            ..default()
        })),
    ));
    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.045, 0.055),
        metallic: 0.8,
        perceptual_roughness: 0.35,
        ..default()
    });
    for x in [-1.65, 1.65] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.12, 5.6, 1.45))),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(x, 2.8, 0.0),
        ));
    }
    for y in [0.05, 5.55] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(3.4, 0.12, 1.45))),
            MeshMaterial3d(metal.clone()),
            Transform::from_xyz(0.0, y, 0.0),
        ));
    }
}

fn sync_device_visuals(
    mut commands: Commands,
    snapshot: Res<SimSnapshot>,
    mut rendered: ResMut<RenderedRevision>,
    old: Query<Entity, With<ProjectedVisual>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    if rendered.0 == snapshot.0.topology_revision && !snapshot.is_changed() {
        return;
    }
    for entity in &old {
        commands.entity(entity).despawn();
    }
    for device in snapshot.0.devices().filter(|device| device.rack.is_some()) {
        let placement = device.rack.expect("filtered");
        let y = 0.3 + (placement.unit as f32 - 1.0) * 0.42;
        let color = match (&device.kind, device.powered) {
            (_, false) => Color::srgb(0.12, 0.12, 0.13),
            (DeviceKind::Server(_), true) => Color::srgb(0.18, 0.22, 0.27),
            (DeviceKind::Switch(_), true) => Color::srgb(0.12, 0.24, 0.22),
            (DeviceKind::Router(_), true) => Color::srgb(0.24, 0.18, 0.12),
        };
        let equipment_texture = match device.kind {
            DeviceKind::Server(_) => "equipment/server_front.jpg",
            DeviceKind::Switch(_) => "equipment/switch_front.jpg",
            DeviceKind::Router(_) => "equipment/router_front.jpg",
        };
        commands.spawn((
            DomainDeviceVisual { _device: device.id },
            ProjectedVisual,
            Mesh3d(meshes.add(Cuboid::new(3.05, 0.34, 1.25))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                base_color_texture: Some(asset_server.load(equipment_texture)),
                metallic: 0.3,
                perceptual_roughness: 0.45,
                ..default()
            })),
            Transform::from_xyz(0.0, y, 0.0),
        ));
    }
    let cable_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.55, 0.32),
        emissive: LinearRgba::rgb(0.0, 0.08, 0.03),
        perceptual_roughness: 0.7,
        ..default()
    });
    for link in snapshot.0.links().filter(|link| link.enabled) {
        let Some(a) = port_position(&snapshot.0, link.a) else {
            continue;
        };
        let Some(b) = port_position(&snapshot.0, link.b) else {
            continue;
        };
        let delta = b - a;
        let length = delta.length();
        if length <= f32::EPSILON {
            continue;
        }
        commands.spawn((
            DomainLinkVisual,
            ProjectedVisual,
            Mesh3d(meshes.add(Cylinder::new(0.025, length))),
            MeshMaterial3d(cable_material.clone()),
            Transform {
                translation: (a + b) * 0.5,
                rotation: Quat::from_rotation_arc(Vec3::Y, delta / length),
                ..default()
            },
        ));
    }
    rendered.0 = snapshot.0.topology_revision;
}

fn port_position(
    sim: &cloud_provider_sim::NetworkSim,
    port_id: cloud_provider_sim::PortId,
) -> Option<Vec3> {
    let port = sim.port(port_id)?;
    let device = sim.device(port.device)?;
    let placement = device.rack?;
    let index = device
        .ports()
        .iter()
        .position(|candidate| *candidate == port_id)?;
    let count = device.ports().len().max(2);
    let x = -1.35 + (index as f32 / (count - 1) as f32) * 2.7;
    let y = 0.3 + (placement.unit as f32 - 1.0) * 0.42;
    Some(Vec3::new(x, y, 0.68))
}
