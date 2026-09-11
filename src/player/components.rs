use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

/// Marks the entity controlled by the local player.
#[derive(Component, Debug)]
#[require(PlayerInWater)]
pub struct Player;

/// Water overlap with the player's collision volume, sampled from actual voxels.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInWater {
    /// Fraction of the body volume inside water, from 0 to 1.
    pub submerged_fraction: f32,
}

impl PlayerInWater {
    pub fn is_in_water(self) -> bool {
        self.submerged_fraction > 0.0001
    }

    pub fn is_fully_submerged(self) -> bool {
        self.submerged_fraction >= 0.9999
    }
}

/// Marks the perspective camera parented to the player.
#[derive(Component, Debug)]
pub struct PlayerCamera;

/// Orientation state kept independently from the entity transform.
#[derive(Component, Debug, Default)]
pub struct LookState {
    pub yaw: f32,
    pub pitch: f32,
}

/// Linear player velocity in world units per second.
#[derive(Component, Debug, Default)]
pub struct Velocity(pub Vec3);

/// Whether collision resolution found supporting ground this frame.
#[derive(Component, Debug, Default)]
pub struct Grounded(pub bool);

/// Debug movement mode. While enabled, gravity and voxel collision are bypassed.
#[derive(Component, Debug, Default)]
pub struct Noclip(pub bool);

/// Input intent consumed by the kinematic movement systems.
#[derive(Component, Debug, Default)]
pub struct PlayerController {
    pub movement: Vec2,
    pub vertical_movement: f32,
    pub sprinting: bool,
    pub jump_requested: bool,
}

/// Axis-aligned player bounds used by current and future collision resolvers.
#[derive(Component, Debug)]
pub struct PlayerCollider {
    pub half_extents: Vec3,
}

impl PlayerCollider {
    pub fn from_dimensions(width: f32, height: f32) -> Self {
        let width = width.abs().max(0.01);
        let height = height.abs().max(0.01);
        Self {
            half_extents: Vec3::new(width * 0.5, height * 0.5, width * 0.5),
        }
    }
}

/// Runtime-tunable first-person controller settings.
#[derive(Debug, Resource)]
pub struct PlayerSettings {
    /// Horizontal walking speed in world units per second.
    pub walk_speed: f32,
    /// Horizontal sprinting speed in world units per second.
    pub sprint_speed: f32,
    /// Maximum horizontal speed while sprint-bunny-hopping.
    pub bunny_hop_speed: f32,
    /// Noclip flight speed in world units per second.
    pub noclip_speed: f32,
    /// Maximum horizontal acceleration while movement input is held.
    pub acceleration: f32,
    /// Friction/deceleration applied when horizontal input is released.
    pub deceleration: f32,
    /// Upward velocity applied at the start of a jump.
    pub jump_speed: f32,
    /// Downward acceleration in world units per second squared.
    pub gravity: f32,
    /// Maximum downward speed in world units per second.
    pub terminal_velocity: f32,
    pub swim_speed: f32,
    pub swim_up_speed: f32,
    pub water_acceleration: f32,
    pub water_gravity: f32,
    pub water_terminal_velocity: f32,
    /// Width and depth of the player's collision bounds.
    pub player_width: f32,
    /// Height of the player's collision bounds.
    pub player_height: f32,
    /// Camera rotation in radians per mouse pixel.
    pub mouse_sensitivity: f32,
    /// Maximum absolute pitch angle in radians.
    pub pitch_limit: f32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            walk_speed: 4.317,
            sprint_speed: 5.612,
            bunny_hop_speed: 7.25,
            noclip_speed: 10.0,
            acceleration: 28.0,
            deceleration: 34.0,
            jump_speed: 8.0,
            gravity: 24.0,
            terminal_velocity: 50.0,
            swim_speed: 2.2,
            swim_up_speed: 3.0,
            water_acceleration: 12.0,
            water_gravity: 4.0,
            water_terminal_velocity: 2.5,
            player_width: 0.6,
            player_height: 1.8,
            mouse_sensitivity: 0.0025,
            pitch_limit: FRAC_PI_2 - 0.01,
        }
    }
}
