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
        base_color: Color::srgba(0.32, 0.58, 0.72, 0.55),
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
        assert!(water.base_color.alpha() > 0.0 && water.base_color.alpha() < 1.0);
        assert!(water.double_sided);
        assert_eq!(water.cull_mode, None);
    }
}
