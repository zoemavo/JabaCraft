use bevy::{input::mouse::AccumulatedMouseScroll, prelude::*};

use super::{HOTBAR_SLOT_COUNT, PlayerInventory};

/// Whether the modal player inventory is currently visible.
#[derive(Debug, Default, Resource)]
pub struct InventoryState {
    open: bool,
}

impl InventoryState {
    pub const fn is_open(&self) -> bool {
        self.open
    }

    fn set_open(&mut self, open: bool) {
        self.open = open;
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, SystemSet)]
pub enum InventoryInputSet {
    Toggle,
    Hotbar,
}

pub fn gameplay_input_enabled(state: Res<InventoryState>) -> bool {
    !state.is_open()
}

pub(super) fn toggle_inventory(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InventoryState>,
    mut inventory: ResMut<PlayerInventory>,
) {
    let should_toggle = keyboard.just_pressed(KeyCode::KeyE);
    let should_close = state.is_open() && keyboard.just_pressed(KeyCode::Escape);
    if !should_toggle && !should_close {
        return;
    }

    let open = if should_close {
        false
    } else {
        !state.is_open()
    };
    state.set_open(open);
    if !open {
        inventory.stow_held_stack();
    }
}

pub(super) fn select_hotbar_slot(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    state: Res<InventoryState>,
    mut inventory: ResMut<PlayerInventory>,
) {
    if state.is_open() {
        return;
    }
    apply_hotbar_selection(&keyboard, mouse_scroll.delta.y, &mut inventory);
}

fn apply_hotbar_selection(
    keyboard: &ButtonInput<KeyCode>,
    scroll_y: f32,
    inventory: &mut PlayerInventory,
) {
    const SLOT_KEYS: [KeyCode; HOTBAR_SLOT_COUNT] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];

    if let Some(index) = SLOT_KEYS
        .into_iter()
        .position(|key| keyboard.just_pressed(key))
    {
        inventory.select_hotbar(index);
    } else if scroll_y > 0.0 {
        inventory.cycle_hotbar(-1);
    } else if scroll_y < 0.0 {
        inventory.cycle_hotbar(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_keys_select_matching_slots() {
        let mut keyboard = ButtonInput::default();
        keyboard.press(KeyCode::Digit6);
        let mut inventory = PlayerInventory::default();

        apply_hotbar_selection(&keyboard, 0.0, &mut inventory);

        assert_eq!(inventory.selected_hotbar_index(), 5);
    }

    #[test]
    fn mouse_wheel_cycles_in_both_directions() {
        let keyboard = ButtonInput::default();
        let mut inventory = PlayerInventory::default();

        apply_hotbar_selection(&keyboard, -1.0, &mut inventory);
        assert_eq!(inventory.selected_hotbar_index(), 1);
        apply_hotbar_selection(&keyboard, 1.0, &mut inventory);
        assert_eq!(inventory.selected_hotbar_index(), 0);
    }
}
