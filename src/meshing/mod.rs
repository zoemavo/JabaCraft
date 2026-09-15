//! Chunk meshing and rendering orchestration.

mod atlas;
mod lighting;
mod material;
use material::{AtlasRepeat, VoxelMaterial};
mod renderer;
mod voxel;

use bevy::prelude::*;

use crate::{
    generation::ChunkStreamingSet,
    scene::{EnvironmentUpdateSet, WorldLightTint},
};

pub use renderer::{ChunkMesh, ChunkRenderer};

use atlas::create_block_texture_atlas;
use renderer::{ChunkMeshingQueue, ChunkMeshingTasks, sync_chunk_renderer};

/// Bounds worker pressure and task setup performed in one frame.
#[derive(Debug, Resource)]
pub struct MeshingSettings {
    pub max_meshing_tasks: usize,
    pub start_budget_per_frame: usize,
    pub max_mesh_uploads_per_frame: usize,
}

/// Read-only worker and upload counters consumed by the F3 overlay.
#[derive(Clone, Copy, Debug, Default, Resource)]
pub struct ChunkMeshingStats {
    pub queued: usize,
    pub running: usize,
    pub awaiting_upload: usize,
    pub max_tasks: usize,
    pub max_uploads_per_frame: usize,
}

impl Default for MeshingSettings {
    fn default() -> Self {
        Self {
            max_meshing_tasks: 4,
            start_budget_per_frame: 2,
            max_mesh_uploads_per_frame: 2,
        }
    }
}

#[derive(Resource)]
struct ChunkMaterial(Handle<VoxelMaterial>, Handle<StandardMaterial>);

/// Owns discovery, creation, rebuilding, and removal of rendered chunks.
pub struct ChunkMeshingPlugin;

impl Plugin for ChunkMeshingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<VoxelMaterial>::default())
            .init_resource::<MeshingSettings>()
            .init_resource::<ChunkMeshingQueue>()
            .init_resource::<ChunkMeshingTasks>()
            .init_resource::<ChunkRenderer>()
            .init_resource::<ChunkMeshingStats>()
            .add_systems(Startup, create_chunk_material)
            .add_systems(
                Update,
                (
                    sync_chunk_renderer.after(ChunkStreamingSet),
                    update_chunk_light_tint.after(EnvironmentUpdateSet),
                ),
            );
    }
}

pub(crate) fn reset_session(world: &mut World) {
    world.insert_resource(ChunkMeshingQueue::default());
    world.insert_resource(ChunkMeshingTasks::default());
    world.insert_resource(ChunkMeshingStats::default());
}

fn create_chunk_material(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    light_tint: Res<WorldLightTint>,
    mut voxel_materials: ResMut<Assets<VoxelMaterial>>,
) {
    let atlas = images.add(create_block_texture_atlas());
    let water = materials.add(StandardMaterial {
        // Water faces already point at the Faithful water tile in the block
        // atlas. Keep the material white so its pixel art and original alpha
        // survive instead of being replaced by a flat color.
        base_color: Color::WHITE,
        base_color_texture: Some(atlas.clone()),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(ChunkMaterial(
        voxel_materials.add(VoxelMaterial {
            base: StandardMaterial {
                base_color: Color::srgb(light_tint.0[0], light_tint.0[1], light_tint.0[2]),
                base_color_texture: Some(atlas),
                alpha_mode: AlphaMode::Mask(0.5),
                perceptual_roughness: 1.0,
                // Blocks are matte.  Suppressing the default dielectric highlight
                // avoids broad plastic-looking glare on sun-facing terrain.
                reflectance: 0.0,
                // Preserve baked voxel occlusion while allowing the low-contrast sun,
                // ambient fill, and filtered cast shadows to reach the terrain.
                unlit: false,
                ..default()
            },
            extension: AtlasRepeat::default(),
        }),
        water,
    ));
}

fn update_chunk_light_tint(
    light_tint: Res<WorldLightTint>,
    chunk_material: Option<Res<ChunkMaterial>>,
    mut materials: ResMut<Assets<VoxelMaterial>>,
) {
    if !light_tint.is_changed() {
        return;
    }
    let Some(mut material) = chunk_material
        .as_deref()
        .and_then(|chunk_material| materials.get_mut(&chunk_material.0))
    else {
        return;
    };
    material.base.base_color = Color::srgb(light_tint.0[0], light_tint.0[1], light_tint.0[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_has_its_own_blended_double_sided_material() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<VoxelMaterial>>()
            .insert_resource(WorldLightTint([1.0; 3]))
            .add_systems(Startup, create_chunk_material);
        app.update();
        let handles = app.world().resource::<ChunkMaterial>();

        let materials = app.world().resource::<Assets<StandardMaterial>>();
        assert!(matches!(
            app.world()
                .resource::<Assets<VoxelMaterial>>()
                .get(&handles.0)
                .unwrap()
                .base
                .alpha_mode,
            AlphaMode::Mask(_)
        ));
        let water = materials.get(&handles.1).unwrap();
        assert_eq!(water.alpha_mode, AlphaMode::Blend);
        assert_eq!(water.base_color, Color::WHITE);
        assert_eq!(
            water.base_color_texture,
            app.world()
                .resource::<Assets<VoxelMaterial>>()
                .get(&handles.0)
                .unwrap()
                .base
                .base_color_texture
        );
        assert!(water.double_sided);
        assert_eq!(water.cull_mode, None);
    }
}
