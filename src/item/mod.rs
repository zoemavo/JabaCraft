//! Item identities, metadata, registry, and stack operations.

mod definition;
mod dropped;
mod id;
mod registry;
mod stack;

use bevy::prelude::*;

pub use definition::{FoodProperties, ItemDefinition, ToolProperties, ToolType};
pub use dropped::{DroppedItem, DroppedItemAssets, spawn_dropped_item};
pub use id::{BUILTIN_ITEM_COUNT, InvalidItemId, ItemId};
pub use registry::ItemRegistry;
pub use stack::{ItemStack, ItemStackError};

/// Standard capacity used by the built-in placeable block items.
pub const BLOCK_ITEM_MAX_STACK_SIZE: u32 = 64;

/// Owns item definitions independently of blocks and inventory containers.
pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemRegistry>()
            .add_systems(Startup, dropped::create_dropped_item_assets)
            .add_systems(
                FixedUpdate,
                (
                    dropped::apply_dropped_item_physics,
                    dropped::pickup_dropped_items,
                )
                    .chain(),
            );
    }
}
