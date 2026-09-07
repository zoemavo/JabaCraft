//! Player inventory data and inventory input orchestration.

mod data;
mod input;

use bevy::prelude::*;

pub use data::{
    HOTBAR_SLOT_COUNT, INVENTORY_SLOT_COUNT, InventoryClick, PlayerInventory, TOTAL_SLOT_COUNT,
};
pub use input::{InventoryInputSet, InventoryState, gameplay_input_enabled};

/// Shared inventory limits.
#[derive(Debug, Resource)]
pub struct InventorySettings {
    pub hotbar_slots: u8,
    pub inventory_slots: u8,
}

impl Default for InventorySettings {
    fn default() -> Self {
        Self {
            hotbar_slots: HOTBAR_SLOT_COUNT as u8,
            inventory_slots: INVENTORY_SLOT_COUNT as u8,
        }
    }
}

/// Owns inventory data and input; visual presentation lives in the UI module.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventorySettings>()
            .init_resource::<PlayerInventory>()
            .init_resource::<InventoryState>()
            .configure_sets(
                PreUpdate,
                (InventoryInputSet::Toggle, InventoryInputSet::Hotbar).chain(),
            )
            .add_systems(
                PreUpdate,
                input::toggle_inventory.in_set(InventoryInputSet::Toggle),
            )
            .add_systems(
                PreUpdate,
                input::select_hotbar_slot.in_set(InventoryInputSet::Hotbar),
            );
    }
}
