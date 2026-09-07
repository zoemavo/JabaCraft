use bevy::prelude::States;

/// Lifecycle states for a game session.
#[derive(Debug, Clone, Copy, Default, Eq, Hash, PartialEq, States)]
pub enum GameState {
    /// Loading resources and preparing the initial world.
    #[default]
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
    fn game_state_starts_in_loading() {
        assert_eq!(GameState::default(), GameState::Loading);
    }

    #[test]
    fn lifecycle_states_are_distinct() {
        assert_ne!(GameState::Loading, GameState::Playing);
        assert_ne!(GameState::Playing, GameState::Paused);
    }
}
