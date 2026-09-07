//! Inventory data and inventory-management systems.

use bevy::prelude::*;

/// Shared limits for future player inventories.
#[derive(Debug, Resource)]
pub struct InventorySettings {
    pub hotbar_slots: u8,
}

impl Default for InventorySettings {
    fn default() -> Self {
        Self { hotbar_slots: 9 }
    }
}

/// Owns inventory data, item movement, and crafting integrations.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventorySettings>();
    }
}
