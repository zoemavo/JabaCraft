//! Authoritative world clock and debug time controls.

use bevy::prelude::*;

pub const DAYLIGHT_DURATION_SECONDS: f64 = 14.0 * 60.0;
pub const NIGHT_DURATION_SECONDS: f64 = 10.0 * 60.0;
pub const DEFAULT_DAY_LENGTH_SECONDS: f64 = DAYLIGHT_DURATION_SECONDS + NIGHT_DURATION_SECONDS;
pub const DAYLIGHT_FRACTION: f32 = (DAYLIGHT_DURATION_SECONDS / DEFAULT_DAY_LENGTH_SECONDS) as f32;
pub const DEBUG_TIME_MULTIPLIER: f64 = 60.0;

/// Persistent time elapsed in the current world.
#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub struct GameTime {
    elapsed_days: f64,
    debug_accelerated: bool,
}

impl GameTime {
    /// Starts a new world at noon so the initial terrain remains easy to read.
    pub const fn at_noon() -> Self {
        Self {
            elapsed_days: DAYLIGHT_FRACTION as f64 * 0.5,
            debug_accelerated: false,
        }
    }

    pub fn from_elapsed_days(elapsed_days: f64) -> Self {
        Self {
            elapsed_days: sanitize_elapsed_days(elapsed_days),
            debug_accelerated: false,
        }
    }

    pub const fn elapsed_days(self) -> f64 {
        self.elapsed_days
    }

    pub fn day_fraction(self) -> f32 {
        self.elapsed_days.rem_euclid(1.0) as f32
    }

    pub fn day_number(self) -> u64 {
        self.elapsed_days.floor().max(0.0) as u64
    }

    pub const fn debug_accelerated(self) -> bool {
        self.debug_accelerated
    }

    fn toggle_debug_acceleration(&mut self) {
        self.debug_accelerated = !self.debug_accelerated;
    }

    fn advance(&mut self, real_seconds: f64, day_length_seconds: f64) {
        let day_length_seconds = day_length_seconds.max(f64::EPSILON);
        let multiplier = if self.debug_accelerated {
            DEBUG_TIME_MULTIPLIER
        } else {
            1.0
        };
        self.elapsed_days = sanitize_elapsed_days(
            self.elapsed_days + real_seconds.max(0.0) * multiplier / day_length_seconds,
        );
    }
}

impl Default for GameTime {
    fn default() -> Self {
        Self::at_noon()
    }
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct GameTimeSettings {
    pub day_length_seconds: f64,
}

impl Default for GameTimeSettings {
    fn default() -> Self {
        Self {
            day_length_seconds: DEFAULT_DAY_LENGTH_SECONDS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) struct GameTimeUpdateSet;

pub struct GameTimePlugin;

impl Plugin for GameTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameTime>()
            .init_resource::<GameTimeSettings>()
            .add_systems(
                Update,
                (toggle_debug_time_acceleration, advance_game_time)
                    .chain()
                    .in_set(GameTimeUpdateSet),
            );
    }
}

fn toggle_debug_time_acceleration(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut game_time: ResMut<GameTime>,
) {
    if !keyboard.just_pressed(KeyCode::F6) {
        return;
    }

    game_time.toggle_debug_acceleration();
    let clock_hours = (game_time.day_fraction() * 24.0 + 6.0) % 24.0;
    info!(
        "Time acceleration: {}x (day {}, {:02}:{:02})",
        if game_time.debug_accelerated() {
            DEBUG_TIME_MULTIPLIER
        } else {
            1.0
        },
        game_time.day_number(),
        clock_hours.floor() as u32,
        ((clock_hours * 60.0).floor() as u32) % 60,
    );
}

fn advance_game_time(
    time: Res<Time>,
    settings: Res<GameTimeSettings>,
    mut game_time: ResMut<GameTime>,
) {
    game_time.advance(time.delta_secs_f64(), settings.day_length_seconds);
}

fn sanitize_elapsed_days(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        DAYLIGHT_FRACTION as f64 * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_cycle_is_fourteen_minutes_of_day_and_ten_of_night() {
        let mut time = GameTime::from_elapsed_days(0.0);

        time.advance(DEFAULT_DAY_LENGTH_SECONDS, DEFAULT_DAY_LENGTH_SECONDS);

        assert_eq!(time.elapsed_days(), 1.0);
        assert_eq!(time.day_number(), 1);
        assert_eq!(time.day_fraction(), 0.0);
        assert_eq!(DAYLIGHT_DURATION_SECONDS, 14.0 * 60.0);
        assert_eq!(NIGHT_DURATION_SECONDS, 10.0 * 60.0);
    }

    #[test]
    fn debug_mode_advances_time_sixty_times_faster() {
        let mut time = GameTime::from_elapsed_days(0.0);
        time.toggle_debug_acceleration();

        time.advance(24.0, DEFAULT_DAY_LENGTH_SECONDS);

        assert!((time.elapsed_days() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn loaded_time_is_sanitized_and_debug_mode_is_not_persistent() {
        let time = GameTime::from_elapsed_days(f64::NAN);

        assert_eq!(time, GameTime::at_noon());
        assert!(!time.debug_accelerated());
    }
}
