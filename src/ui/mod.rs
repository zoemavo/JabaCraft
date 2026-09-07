//! UI state and user-interface systems.

use bevy::prelude::*;

/// Runtime switches for future HUD and menu presentation.
#[derive(Debug, Default, Resource)]
pub struct UiSettings {
    pub show_debug_overlay: bool,
}

/// Owns the HUD, menus, and UI-driven game-state transitions.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiSettings>();
    }
}
