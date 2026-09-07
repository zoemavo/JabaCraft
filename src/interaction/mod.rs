//! Block targeting and player-world interaction systems.

mod raycast;

use bevy::prelude::*;
use bevy::transform::TransformSystems;

use crate::{chunk::ChunkStorage, player::PlayerCamera};

pub use raycast::{VoxelRaycastHit, voxel_raycast};

/// Current block selected by the player camera.
#[derive(Debug, Default, Resource)]
pub struct CameraRaycast(pub Option<VoxelRaycastHit>);

/// Configuration for future block-selection raycasts.
#[derive(Debug, Resource)]
pub struct InteractionSettings {
    pub reach: f32,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self { reach: 5.0 }
    }
}

/// Owns selecting, breaking, and placing blocks.
pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractionSettings>()
            .init_resource::<CameraRaycast>()
            .add_systems(
                PostUpdate,
                update_camera_raycast.after(TransformSystems::Propagate),
            );
    }
}

fn update_camera_raycast(
    settings: Res<InteractionSettings>,
    storage: Res<ChunkStorage>,
    camera: Single<&GlobalTransform, With<PlayerCamera>>,
    mut current: ResMut<CameraRaycast>,
) {
    let camera = camera.compute_transform();
    current.0 = voxel_raycast(
        &storage,
        camera.translation,
        camera.rotation * Vec3::NEG_Z,
        settings.reach.max(0.0),
    );
}
