use bevy::prelude::States;

/// Lifecycle states for a game session.
#[derive(Debug, Clone, Copy, Default, Eq, Hash, PartialEq, States)]
pub enum GameState {
    #[default]
    MainMenu,
    /// Loading resources and preparing the initial world.
    Loading,
    /// Running the interactive sandbox simulation.
    Playing,
    /// Suspending player-facing gameplay while menus are open.
    Paused,
}

#[cfg(test)]
mod tests {
    use super::GameState;

    #[test]
    fn game_state_starts_in_main_menu() {
        assert_eq!(GameState::default(), GameState::MainMenu);
    }

    #[test]
    fn lifecycle_states_are_distinct() {
        assert_ne!(GameState::Loading, GameState::Playing);
        assert_ne!(GameState::Playing, GameState::Paused);
    }
}
