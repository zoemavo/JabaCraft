//! Minecraft-style block damage overlay driven by mining progress.

use bevy::prelude::*;

use super::{CameraRaycast, MiningProgress};

const CRACK_STAGE_COUNT: usize = 10;
const OVERLAY_SIZE: f32 = 1.006;

#[derive(Component)]
pub(super) struct BlockCrackOverlay;

#[derive(Resource)]
pub(super) struct BlockCrackMaterials([Handle<StandardMaterial>; CRACK_STAGE_COUNT]);

pub(super) fn spawn_block_crack_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::from_size(Vec3::splat(OVERLAY_SIZE)));
    let crack_materials = std::array::from_fn(|stage| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(
                asset_server.load(format!("textures/breaking/destroy_stage_{stage}.png")),
            ),
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            depth_bias: 1.0,
            ..default()
        })
    });
    let first_material = crack_materials[0].clone();
    commands.insert_resource(BlockCrackMaterials(crack_materials));
    commands.spawn((
        Name::new("Block Breaking Cracks"),
        BlockCrackOverlay,
        Mesh3d(mesh),
        MeshMaterial3d(first_material),
        Transform::default(),
        Visibility::Hidden,
    ));
}

pub(super) fn sync_block_crack_overlay(
    selected: Res<CameraRaycast>,
    progress: Res<MiningProgress>,
    materials: Res<BlockCrackMaterials>,
    overlay: Single<
        (
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<BlockCrackOverlay>,
    >,
) {
    let (mut transform, mut visibility, mut material) = overlay.into_inner();
    let Some(hit) = selected.0.filter(|_| progress.is_active()) else {
        *visibility = Visibility::Hidden;
        return;
    };

    let stage = crack_stage(progress.normalized());
    material.0 = materials.0[stage].clone();
    transform.translation = Vec3::new(
        hit.position.x as f32 + 0.5,
        hit.position.y as f32 + 0.5,
        hit.position.z as f32 + 0.5,
    );
    *visibility = Visibility::Visible;
}

fn crack_stage(progress: f32) -> usize {
    ((progress.clamp(0.0, 1.0) * CRACK_STAGE_COUNT as f32).floor() as usize)
        .min(CRACK_STAGE_COUNT - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_progress_selects_one_of_ten_crack_stages() {
        assert_eq!(crack_stage(-1.0), 0);
        assert_eq!(crack_stage(0.09), 0);
        assert_eq!(crack_stage(0.10), 1);
        assert_eq!(crack_stage(0.55), 5);
        assert_eq!(crack_stage(0.99), 9);
        assert_eq!(crack_stage(1.0), 9);
        assert_eq!(crack_stage(2.0), 9);
    }
}
