//! Chunk meshing and rendering orchestration.

mod atlas;
mod renderer;
mod voxel;

use bevy::prelude::*;

use crate::generation::ChunkStreamingSet;

pub use renderer::{ChunkMesh, ChunkRenderer};

use atlas::create_block_texture_atlas;
use renderer::sync_chunk_renderer;

/// Limits the amount of mesh work performed in one frame.
#[derive(Debug, Resource)]
pub struct MeshingSettings {
    pub rebuild_budget_per_frame: usize,
}

impl Default for MeshingSettings {
    fn default() -> Self {
        Self {
            rebuild_budget_per_frame: 8,
        }
    }
}

#[derive(Resource)]
struct ChunkMaterial(Handle<StandardMaterial>);

/// Owns discovery, creation, rebuilding, and removal of rendered chunks.
pub struct ChunkMeshingPlugin;

impl Plugin for ChunkMeshingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshingSettings>()
            .init_resource::<ChunkRenderer>()
            .add_systems(Startup, create_chunk_material)
            .add_systems(Update, sync_chunk_renderer.after(ChunkStreamingSet));
    }
}

fn create_chunk_material(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let atlas = images.add(create_block_texture_atlas());
    commands.insert_resource(ChunkMaterial(materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(atlas),
        alpha_mode: AlphaMode::Mask(0.5),
        perceptual_roughness: 0.92,
        ..default()
    })));
}
