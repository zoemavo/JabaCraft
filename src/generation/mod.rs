//! Procedural world generation and player-relative chunk streaming.

mod biome;
mod cave;
mod features;
mod noise;
mod ore;
mod queue;
mod random;
mod streaming;
mod terrain;

use bevy::prelude::*;

use streaming::{ChunkGenerationTasks, stream_chunks_around_player};

pub use biome::{Biome, BiomeDefinition, BiomeSample, BiomeSampler};
pub use cave::CaveSettings;
pub use queue::{ChunkGenerationQueue, ChunkLifecycle};
pub use terrain::{is_chunk_potentially_empty, terrain_height_at};

pub const DEFAULT_WORLD_MIN_Y: i32 = -64;
pub const DEFAULT_WORLD_MAX_Y: i32 = 192;

/// Runtime configuration for the loaded chunk window.
#[derive(Debug, Resource)]
pub struct GenerationSettings {
    pub seed: u64,
    /// Horizontal render radius measured in chunks. The X/Z footprint is circular.
    pub render_distance_chunks: i32,
    /// Inclusive lower world-block boundary.
    pub world_min_y: i32,
    /// Exclusive upper world-block boundary.
    pub world_max_y: i32,
    /// Maximum number of terrain tasks running concurrently in the compute pool.
    pub max_concurrent_generation_jobs: usize,
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            seed: 0,
            render_distance_chunks: 8,
            world_min_y: DEFAULT_WORLD_MIN_Y,
            world_max_y: DEFAULT_WORLD_MAX_Y,
            max_concurrent_generation_jobs: 4,
        }
    }
}

/// Runs before chunk meshing so rendering observes the current loaded set.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, SystemSet)]
pub struct ChunkStreamingSet;

/// Owns terrain generation and player-relative chunk loading.
pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GenerationSettings>()
            .init_resource::<CaveSettings>()
            .init_resource::<ChunkGenerationQueue>()
            .init_resource::<ChunkGenerationTasks>()
            .add_systems(
                Update,
                stream_chunks_around_player.in_set(ChunkStreamingSet),
            );
    }
}
