//! Minecraft-style survival rules for the local player.

mod stats;
mod systems;

use bevy::prelude::*;

use crate::player::PlayerPhysicsSet;

pub use stats::{Health, Hunger, MAX_FOOD_LEVEL, MAX_HEALTH, SurvivalTracker};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Resource)]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct RespawnPoint(pub Vec3);

#[derive(Debug, Resource)]
pub struct SurvivalSettings {
    pub safe_fall_distance: f32,
    pub sprint_exhaustion_per_block: f32,
    pub jump_exhaustion: f32,
    pub sprint_jump_exhaustion: f32,
    pub regeneration_food_level: u8,
    pub regeneration_interval_seconds: f32,
    pub regeneration_exhaustion: f32,
    pub starvation_interval_seconds: f32,
}

impl Default for SurvivalSettings {
    fn default() -> Self {
        Self {
            safe_fall_distance: 3.0,
            sprint_exhaustion_per_block: 0.1,
            jump_exhaustion: 0.05,
            sprint_jump_exhaustion: 0.2,
            regeneration_food_level: 18,
            regeneration_interval_seconds: 4.0,
            regeneration_exhaustion: 6.0,
            starvation_interval_seconds: 4.0,
        }
    }
}

#[derive(Debug, Resource)]
struct EatingProgress {
    item: Option<crate::item::ItemId>,
    elapsed: f32,
    required_seconds: f32,
}

impl EatingProgress {
    fn reset(&mut self) {
        self.item = None;
        self.elapsed = 0.0;
    }
}

impl Default for EatingProgress {
    fn default() -> Self {
        Self {
            item: None,
            elapsed: 0.0,
            required_seconds: 1.6,
        }
    }
}

pub struct SurvivalPlugin;

impl Plugin for SurvivalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameMode>()
            .init_resource::<SurvivalSettings>()
            .init_resource::<EatingProgress>()
            .add_systems(
                FixedUpdate,
                systems::update_survival_stats.after(PlayerPhysicsSet::ResolveCollisions),
            )
            .add_systems(Update, systems::eat_selected_food);
    }
}
