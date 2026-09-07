//! Player entities, input, movement, and camera systems.

mod collision;
mod components;
mod systems;

use crate::inventory::InventoryInputSet;
use bevy::prelude::*;

pub use components::{
    Grounded, LookState, Noclip, Player, PlayerCamera, PlayerCollider, PlayerController,
    PlayerSettings, Velocity,
};

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, SystemSet)]
pub enum PlayerPhysicsSet {
    Cursor,
    ReadInput,
    UpdateVelocity,
    ResolveCollisions,
    SyncView,
}

const PLAYER_PHYSICS_HZ: f64 = 64.0;

/// Owns player spawning, controls, movement, and camera behaviour.
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSettings>()
            .insert_resource(Time::<Fixed>::from_hz(PLAYER_PHYSICS_HZ))
            .configure_sets(
                PreUpdate,
                (PlayerPhysicsSet::Cursor, PlayerPhysicsSet::ReadInput).chain(),
            )
            .configure_sets(
                FixedUpdate,
                (
                    PlayerPhysicsSet::UpdateVelocity,
                    PlayerPhysicsSet::ResolveCollisions,
                )
                    .chain(),
            )
            .add_systems(Startup, systems::capture_cursor_on_startup)
            .add_systems(
                PreUpdate,
                systems::update_cursor_grab
                    .in_set(PlayerPhysicsSet::Cursor)
                    .after(InventoryInputSet::Toggle),
            )
            .add_systems(
                PreUpdate,
                (
                    systems::collect_movement_input,
                    systems::collect_look_input,
                    systems::toggle_noclip,
                )
                    .in_set(PlayerPhysicsSet::ReadInput),
            )
            .add_systems(
                FixedUpdate,
                systems::update_velocity.in_set(PlayerPhysicsSet::UpdateVelocity),
            )
            .add_systems(
                FixedUpdate,
                collision::move_players_with_voxel_collisions
                    .in_set(PlayerPhysicsSet::ResolveCollisions),
            )
            .add_systems(
                Update,
                systems::sync_view_transforms.in_set(PlayerPhysicsSet::SyncView),
            );
    }
}
