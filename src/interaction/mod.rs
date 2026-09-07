//! Block targeting and player-world interaction systems.

mod breaking;
mod input;
mod outline;
mod placing;
mod raycast;

use bevy::prelude::*;
use bevy::transform::TransformSystems;

use crate::{chunk::ChunkStorage, inventory::InventoryState, player::PlayerCamera};

pub use raycast::{VoxelRaycastHit, voxel_raycast};

/// Current block selected by the player camera.
#[derive(Debug, Default, Resource)]
pub struct CameraRaycast(pub Option<VoxelRaycastHit>);

/// Configuration for future block-selection raycasts.
#[derive(Debug, Resource)]
pub struct InteractionSettings {
    pub reach: f32,
    /// Minimum delay between blocks while the primary mouse button is held.
    pub break_repeat_interval: f32,
    /// Minimum delay between placements while the secondary mouse button is held.
    pub place_repeat_interval: f32,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            reach: 5.0,
            break_repeat_interval: 0.2,
            place_repeat_interval: 0.2,
        }
    }
}

/// Owns selecting, breaking, and placing blocks.
pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteractionSettings>()
            .init_resource::<CameraRaycast>()
            .init_resource::<breaking::BlockBreakInput>()
            .init_resource::<placing::BlockPlaceInput>()
            .init_gizmo_group::<outline::BlockOutlineGizmos>()
            .add_systems(Startup, outline::configure_block_outline)
            .add_systems(
                PostUpdate,
                (
                    update_camera_raycast,
                    breaking::break_selected_block,
                    placing::place_selected_block,
                    outline::draw_block_outline,
                )
                    .chain()
                    .after(TransformSystems::Propagate),
            );
    }
}

fn update_camera_raycast(
    settings: Res<InteractionSettings>,
    inventory_state: Res<InventoryState>,
    storage: Res<ChunkStorage>,
    camera: Single<&GlobalTransform, With<PlayerCamera>>,
    mut current: ResMut<CameraRaycast>,
) {
    if inventory_state.is_open() {
        current.0 = None;
        return;
    }

    let camera = camera.compute_transform();
    current.0 = voxel_raycast(
        &storage,
        camera.translation,
        camera.rotation * Vec3::NEG_Z,
        settings.reach.max(0.0),
    );
}
