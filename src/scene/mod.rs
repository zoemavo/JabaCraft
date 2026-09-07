//! Bootstrap entities shared by the playable 3D scene.

use std::f32::consts::FRAC_PI_4;

use bevy::{core_pipeline::tonemapping::Tonemapping, prelude::*};

use crate::{
    generation::{GenerationSettings, terrain_height_at},
    player::{
        Grounded, LookState, Noclip, Player, PlayerCamera, PlayerCollider, PlayerController,
        PlayerSettings, Velocity,
    },
};

/// Creates the player and lighting for the voxel world.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_scene);
    }
}

fn setup_scene(
    mut commands: Commands,
    player_settings: Res<PlayerSettings>,
    generation_settings: Res<GenerationSettings>,
) {
    spawn_directional_light(&mut commands);
    spawn_player(&mut commands, &player_settings, generation_settings.seed);
    info!("Playable voxel scene initialized");
}

fn spawn_directional_light(commands: &mut Commands) {
    commands.spawn((
        Name::new("Sun"),
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -FRAC_PI_4, -FRAC_PI_4, 0.0)),
    ));
}

fn spawn_player(commands: &mut Commands, settings: &PlayerSettings, terrain_seed: u64) {
    let collider = PlayerCollider::from_dimensions(settings.player_width, settings.player_height);
    let spawn_x = 8.0;
    let spawn_z = 10.0;
    let surface_y = terrain_height_at(terrain_seed, spawn_x as i64, spawn_z as i64) as f32 + 1.0;
    let player_position = Vec3::new(spawn_x, surface_y + collider.half_extents.y, spawn_z);
    let camera_height = settings.player_height.abs().max(0.01) * 0.4;
    let camera_position = player_position + Vec3::Y * camera_height;
    let target_surface_y = terrain_height_at(terrain_seed, 0, 0) as f32 + 1.0;
    let view_transform = Transform::from_translation(camera_position)
        .looking_at(Vec3::new(0.0, target_surface_y, 0.0), Vec3::Y);
    let (yaw, pitch, _) = view_transform.rotation.to_euler(EulerRot::YXZ);

    commands
        .spawn((
            Name::new("Player"),
            Player,
            PlayerController::default(),
            Velocity::default(),
            Grounded(true),
            Noclip::default(),
            collider,
            LookState { yaw, pitch },
            Transform::from_translation(player_position).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Player Camera"),
                PlayerCamera,
                Camera3d::default(),
                Tonemapping::None,
                Projection::from(PerspectiveProjection {
                    fov: 60.0_f32.to_radians(),
                    ..default()
                }),
                Transform::from_xyz(0.0, camera_height, 0.0)
                    .with_rotation(Quat::from_rotation_x(pitch)),
            ));
        });
}
