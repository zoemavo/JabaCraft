//! Chunk meshing and rendering orchestration.

mod atlas;
mod lighting;
mod renderer;
mod voxel;

use bevy::prelude::*;

use crate::{
    generation::ChunkStreamingSet,
    scene::{EnvironmentUpdateSet, WorldLightTint},
};

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
            // Lighting and geometry are intentionally streamed one chunk at a
            // time so terrain loading cannot monopolize an entire frame.
            rebuild_budget_per_frame: 1,
        }
    }
}

#[derive(Resource)]
struct ChunkMaterial(Handle<StandardMaterial>, Handle<StandardMaterial>);

/// Owns discovery, creation, rebuilding, and removal of rendered chunks.
pub struct ChunkMeshingPlugin;

impl Plugin for ChunkMeshingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshingSettings>()
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

fn create_chunk_material(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    light_tint: Res<WorldLightTint>,
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
        materials.add(StandardMaterial {
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
        }),
        water,
    ));
}

fn update_chunk_light_tint(
    light_tint: Res<WorldLightTint>,
    chunk_material: Option<Res<ChunkMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    material.base_color = Color::srgb(light_tint.0[0], light_tint.0[1], light_tint.0[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_has_its_own_blended_double_sided_material() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(WorldLightTint([1.0; 3]))
            .add_systems(Startup, create_chunk_material);
        app.update();
        let handles = app.world().resource::<ChunkMaterial>();
        assert_ne!(handles.0.id(), handles.1.id());
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        assert!(matches!(
            materials.get(&handles.0).unwrap().alpha_mode,
            AlphaMode::Mask(_)
        ));
        let water = materials.get(&handles.1).unwrap();
        assert_eq!(water.alpha_mode, AlphaMode::Blend);
        assert!(water.base_color.alpha() > 0.0 && water.base_color.alpha() < 1.0);
        assert!(water.double_sided);
        assert_eq!(water.cull_mode, None);
    }
}
