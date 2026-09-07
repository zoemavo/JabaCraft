//! Save/load boundaries for world and player data.

use bevy::prelude::*;

/// Identifies the future save slot used by persistence systems.
#[derive(Debug, Resource)]
pub struct PersistenceSettings {
    pub world_name: String,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            world_name: "default-world".to_owned(),
        }
    }
}

/// Owns future serialization, load, save, and migration systems.
pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PersistenceSettings>();
    }
}
