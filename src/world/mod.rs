//! Composition root for world data and world-processing subsystems.

use bevy::prelude::*;

use crate::{
    block::BlockPlugin, chunk::ChunkPlugin, generation::GenerationPlugin,
    meshing::ChunkMeshingPlugin,
};

pub use crate::coordinates::{ChunkPos, LocalBlockPos, WorldBlockPos};

/// Coordinates world data, generation, and meshing subsystems.
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            BlockPlugin,
            ChunkPlugin,
            GenerationPlugin,
            ChunkMeshingPlugin,
        ));
    }
}
