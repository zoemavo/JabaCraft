//! Bootstrap entities shared by the playable 3D scene.

mod environment;
mod underwater;

use bevy::{
    core_pipeline::tonemapping::Tonemapping, light::ShadowFilteringMethod, prelude::*,
    render::view::ColorGrading, transform::TransformSystems,
};

pub(crate) use environment::SKY_COLOR;
pub(crate) use environment::{EnvironmentUpdateSet, WorldLightTint};
use environment::{distance_fog, install_environment, skybox, spawn_environment};

use crate::{
    game::settings::UserSettings,
    game_time::GameTime,
    generation::{
        BiomeSampler, GenerationSettings, SEA_LEVEL, feature_block_at, terrain_height_at,
    },
    persistence::PersistenceLoadSet,
    player::{
        Grounded, LookState, Noclip, Player, PlayerCamera, PlayerCollider, PlayerController,
        PlayerSettings, Velocity,
    },
    survival::{Health, Hunger, RespawnPoint, SurvivalTracker},
};

/// Creates the player and camera for the voxel world.
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        install_environment(app);
        app.add_systems(
            OnEnter(crate::game::GameState::Loading),
            setup_scene.after(PersistenceLoadSet),
        )
        .add_systems(
            PostUpdate,
            underwater::update_underwater_view
                .after(TransformSystems::Propagate)
                .run_if(in_state(crate::game::GameState::Playing)),
        );
    }
}

pub(crate) fn setup_scene(
    mut commands: Commands,
    player_settings: Res<PlayerSettings>,
    generation_settings: Res<GenerationSettings>,
    user_settings: Res<UserSettings>,
    game_time: Res<GameTime>,
    mut images: ResMut<Assets<Image>>,
    status: Res<crate::persistence::SaveStatus>,
) {
    if status.last_error.is_some() {
        return;
    }
    let skybox_image = spawn_environment(&mut commands, &mut images, &game_time);
    spawn_player(
        &mut commands,
        &player_settings,
        generation_settings.seed,
        user_settings.fov,
        skybox_image,
        &game_time,
    );
    info!("Playable voxel scene initialized");
}

fn spawn_player(
    commands: &mut Commands,
    settings: &PlayerSettings,
    terrain_seed: u64,
    fov_degrees: f32,
    skybox_image: Handle<Image>,
    game_time: &GameTime,
) {
    let collider = PlayerCollider::from_dimensions(settings.player_width, settings.player_height);
    let spawn = find_land_spawn(terrain_seed);
    let spawn_x = spawn.x as f32 + 0.5;
    let spawn_z = spawn.z as f32 + 0.5;
    let surface_y = spawn.surface_y as f32 + 1.0;
    let player_position = Vec3::new(spawn_x, surface_y + collider.half_extents.y, spawn_z);
    commands.insert_resource(RespawnPoint(player_position));
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
            Health::default(),
            Hunger::default(),
            SurvivalTracker::new(player_position.y),
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
                // Keep bright voxel colors from clipping while preserving
                // detail in the darker propagated-light levels.
                Tonemapping::AcesFitted,
                ShadowFilteringMethod::Gaussian,
                underwater::UnderwaterView::default(),
                ColorGrading::default(),
                skybox(skybox_image, game_time),
                distance_fog(game_time),
                Projection::from(PerspectiveProjection {
                    fov: fov_degrees.to_radians(),
                    ..default()
                }),
                Transform::from_xyz(0.0, camera_height, 0.0)
                    .with_rotation(Quat::from_rotation_x(pitch)),
            ));
        });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LandSpawn {
    x: i64,
    z: i64,
    surface_y: i32,
}

const PREFERRED_SPAWN_X: i64 = 8;
const PREFERRED_SPAWN_Z: i64 = 10;
const SPAWN_SEARCH_STEP: i64 = 4;
const PREFERRED_SAFE_SPAWN_RINGS: i64 = 64;
const MAX_SPAWN_SEARCH_RINGS: i64 = 512;

fn find_land_spawn(seed: u64) -> LandSpawn {
    let sampler = BiomeSampler::new(seed);
    let mut fallback_land = None;
    let mut highest_sample = None;

    for ring in 0..=MAX_SPAWN_SEARCH_RINGS {
        if ring == 0 {
            if let Some(spawn) = consider_land_spawn(
                seed,
                &sampler,
                PREFERRED_SPAWN_X,
                PREFERRED_SPAWN_Z,
                &mut fallback_land,
                &mut highest_sample,
            ) {
                return spawn;
            }
            continue;
        }

        let min = -ring;
        let max = ring;

        for offset_x in min..=max {
            for offset_z in [min, max] {
                let x = PREFERRED_SPAWN_X + offset_x * SPAWN_SEARCH_STEP;
                let z = PREFERRED_SPAWN_Z + offset_z * SPAWN_SEARCH_STEP;
                if let Some(spawn) = consider_land_spawn(
                    seed,
                    &sampler,
                    x,
                    z,
                    &mut fallback_land,
                    &mut highest_sample,
                ) {
                    return spawn;
                }
            }
        }

        for offset_z in (min + 1)..max {
            for offset_x in [min, max] {
                let x = PREFERRED_SPAWN_X + offset_x * SPAWN_SEARCH_STEP;
                let z = PREFERRED_SPAWN_Z + offset_z * SPAWN_SEARCH_STEP;
                if let Some(spawn) = consider_land_spawn(
                    seed,
                    &sampler,
                    x,
                    z,
                    &mut fallback_land,
                    &mut highest_sample,
                ) {
                    return spawn;
                }
            }
        }

        // Some seeds do not have a perfectly flat 3x3 patch near the origin.
        // Once the preferred search radius is exhausted, use the nearest
        // feature-free land already found instead of blocking Loading while
        // scanning the entire 4096x4096 fallback window.
        if ring >= PREFERRED_SAFE_SPAWN_RINGS
            && let Some(spawn) = fallback_land
        {
            return spawn;
        }
    }

    // A pathological seed may have no nine-block flat patch in the search
    // window. Prefer feature-free land over crashing during startup; the
    // sampled-height fallback is only reachable for an all-ocean window.
    fallback_land
        .or(highest_sample)
        .expect("spawn search always samples the preferred position")
}

fn consider_land_spawn(
    seed: u64,
    sampler: &BiomeSampler,
    world_x: i64,
    world_z: i64,
    fallback_land: &mut Option<LandSpawn>,
    highest_sample: &mut Option<LandSpawn>,
) -> Option<LandSpawn> {
    let sample = sampler.sample(world_x, world_z);
    let candidate = LandSpawn {
        x: world_x,
        z: world_z,
        surface_y: sample.terrain_height,
    };
    if highest_sample
        .as_ref()
        .is_none_or(|best| candidate.surface_y > best.surface_y)
    {
        *highest_sample = Some(candidate);
    }
    if fallback_land.is_none()
        && sample.terrain_height >= SEA_LEVEL
        && feature_block_at(seed, world_x, i64::from(sample.terrain_height) + 1, world_z).is_none()
        && feature_block_at(seed, world_x, i64::from(sample.terrain_height) + 2, world_z).is_none()
    {
        *fallback_land = Some(candidate);
    }
    safe_land_spawn_at(seed, sampler, world_x, world_z)
}

fn safe_land_spawn_at(
    seed: u64,
    sampler: &BiomeSampler,
    world_x: i64,
    world_z: i64,
) -> Option<LandSpawn> {
    let surface_y = sampler.sample(world_x, world_z).terrain_height;
    if surface_y < SEA_LEVEL {
        return None;
    }

    // Keep the player away from isolated one-block islands and steep ledges.
    for offset_z in -1..=1 {
        for offset_x in -1..=1 {
            let neighbor_y = sampler
                .sample(world_x + offset_x, world_z + offset_z)
                .terrain_height;
            if neighbor_y < SEA_LEVEL || (neighbor_y - surface_y).abs() > 1 {
                return None;
            }
        }
    }

    let feet_y = i64::from(surface_y) + 1;
    if feature_block_at(seed, world_x, feet_y, world_z).is_some()
        || feature_block_at(seed, world_x, feet_y + 1, world_z).is_some()
    {
        return None;
    }

    Some(LandSpawn {
        x: world_x,
        z: world_z,
        surface_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_search_moves_ocean_starts_onto_clear_land() {
        let mut checked_ocean_start = false;

        for seed in 0..128 {
            if terrain_height_at(seed, PREFERRED_SPAWN_X, PREFERRED_SPAWN_Z) < SEA_LEVEL {
                checked_ocean_start = true;
            }

            let spawn = find_land_spawn(seed);
            assert!(
                spawn.surface_y >= SEA_LEVEL,
                "seed {seed} produced an ocean spawn at {spawn:?}"
            );
            assert_eq!(terrain_height_at(seed, spawn.x, spawn.z), spawn.surface_y);
            let feet_y = i64::from(spawn.surface_y) + 1;
            assert_eq!(feature_block_at(seed, spawn.x, feet_y, spawn.z), None);
            assert_eq!(feature_block_at(seed, spawn.x, feet_y + 1, spawn.z), None);
        }

        assert!(
            checked_ocean_start,
            "test seeds should include at least one former ocean spawn"
        );
    }

    #[test]
    fn saved_world_seed_does_not_trigger_exhaustive_spawn_scan() {
        // Regression seed from a real save that previously kept Loading on the
        // main thread for roughly a minute before the world appeared.
        let spawn = find_land_spawn(1_789_450_179_375_016_518);

        assert!(spawn.surface_y >= SEA_LEVEL);
        assert!((spawn.x - PREFERRED_SPAWN_X).abs() <= 256);
        assert!((spawn.z - PREFERRED_SPAWN_Z).abs() <= 256);
    }
}
