//! Game-session states and systems.

mod state;

use bevy::prelude::*;

pub use state::GameState;

/// Establishes the lifecycle state shared by gameplay subsystems.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>();
    }
}
