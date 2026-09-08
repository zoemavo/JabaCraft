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

/// Presentation state shared with the first-person hand animation.
#[derive(Debug, Default, Resource)]
pub struct MiningProgress {
    active: bool,
    normalized: f32,
    elapsed: f32,
}

impl MiningProgress {
    pub const fn is_active(&self) -> bool {
        self.active
    }

    pub const fn normalized(&self) -> f32 {
        self.normalized
    }

    pub const fn elapsed(&self) -> f32 {
        self.elapsed
    }

    fn update(&mut self, elapsed: f32, required: f32) {
        self.active = true;
        self.elapsed = elapsed.max(0.0);
        self.normalized = if required <= 0.0 {
            1.0
        } else {
            (elapsed / required).clamp(0.0, 1.0)
        };
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Configuration for future block-selection raycasts.
#[derive(Debug, Resource)]
pub struct InteractionSettings {
    pub reach: f32,
    /// Minimum delay between blocks while the primary mouse button is held.
    pub break_repeat_interval: f32,
    /// Bare-hand mining time is block hardness multiplied by this value.
    pub survival_break_time_multiplier: f32,
    /// Minimum delay between placements while the secondary mouse button is held.
    pub place_repeat_interval: f32,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            reach: 4.5,
            break_repeat_interval: 0.2,
            survival_break_time_multiplier: 1.5,
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
            .init_resource::<MiningProgress>()
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
