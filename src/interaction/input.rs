//! Shared debouncing for mouse-driven voxel interactions.

const MIN_REPEAT_INTERVAL_SECONDS: f32 = 0.05;

#[derive(Debug, Default)]
pub(super) struct DebouncedButtonInput {
    cooldown_remaining: f32,
    cursor_was_captured: bool,
    suppress_until_release: bool,
}

impl DebouncedButtonInput {
    pub(super) fn update(
        &mut self,
        pressed: bool,
        just_pressed: bool,
        cursor_captured: bool,
        delta_seconds: f32,
        repeat_interval: f32,
    ) -> bool {
        self.cooldown_remaining = (self.cooldown_remaining - delta_seconds.max(0.0)).max(0.0);

        if just_pressed && !self.cursor_was_captured {
            self.suppress_until_release = true;
        } else if !pressed {
            self.suppress_until_release = false;
        }

        let triggered = cursor_captured
            && self.cursor_was_captured
            && !self.suppress_until_release
            && pressed
            && (just_pressed || self.cooldown_remaining == 0.0);

        if triggered {
            self.cooldown_remaining = repeat_interval.max(MIN_REPEAT_INTERVAL_SECONDS);
        } else if !pressed {
            self.cooldown_remaining = 0.0;
        }

        self.cursor_was_captured = cursor_captured;
        triggered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_button_repeats_only_after_debounce_interval() {
        let mut input = DebouncedButtonInput {
            cursor_was_captured: true,
            ..Default::default()
        };

        assert!(input.update(true, true, true, 0.0, 0.2));
        assert!(!input.update(true, false, true, 0.1, 0.2));
        assert!(input.update(true, false, true, 0.1, 0.2));
    }

    #[test]
    fn click_used_to_recapture_cursor_is_suppressed_until_release() {
        let mut input = DebouncedButtonInput::default();

        assert!(!input.update(true, true, true, 0.016, 0.2));
        assert!(!input.update(true, false, true, 0.2, 0.2));
        assert!(!input.update(false, false, true, 0.016, 0.2));
        assert!(input.update(true, true, true, 0.016, 0.2));
    }
}
