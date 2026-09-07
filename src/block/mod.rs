//! Compact block IDs and centralized block metadata.

mod definition;
mod id;
mod registry;

use bevy::prelude::*;

pub use definition::{BlockDefinition, BlockFace, FaceTextureIndices, TextureIndex};
pub use id::{BUILTIN_BLOCK_COUNT, BlockId, InvalidBlockId};
pub use registry::BlockRegistry;

/// Owns block definitions independently of chunk storage and rendering.
pub struct BlockPlugin;

impl Plugin for BlockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BlockRegistry>();
    }
}
