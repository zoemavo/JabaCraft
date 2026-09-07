//! Item identities, metadata, registry, and stack operations.

mod definition;
mod id;
mod registry;
mod stack;

use bevy::prelude::*;

pub use definition::ItemDefinition;
pub use id::{BUILTIN_ITEM_COUNT, InvalidItemId, ItemId};
pub use registry::ItemRegistry;
pub use stack::{ItemStack, ItemStackError};

/// Standard capacity used by the built-in placeable block items.
pub const BLOCK_ITEM_MAX_STACK_SIZE: u32 = 64;

/// Owns item definitions independently of blocks and inventory containers.
pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemRegistry>();
    }
}
