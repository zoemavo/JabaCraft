use std::f32::consts::PI;

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};

use crate::{interaction::MiningProgress, inventory::InventoryState, player::PlayerCamera};

const ARM_WIDTH: f32 = 0.15;
const ARM_HEIGHT: f32 = 0.45;
const ARM_DEPTH: f32 = 0.15;
// The pivot is the shoulder: it stays beside the hotbar while the arm extends
// diagonally toward the middle of the screen.
const IDLE_TRANSLATION: Vec3 = Vec3::new(0.42, -0.43, -0.72);
const IDLE_ROTATION: Vec3 = Vec3::new(0.35, -0.45, -2.62);

#[derive(Component)]
pub(super) struct FirstPersonHandRoot;

pub(super) fn spawn_first_person_hand(
    mut commands: Commands,
    camera: Single<Entity, With<PlayerCamera>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let skin = assets.load("ui/steve.png");
    let base_material = materials.add(StandardMaterial {
        base_color_texture: Some(skin.clone()),
        unlit: true,
        perceptual_roughness: 1.0,
        depth_bias: 10_000.0,
        ..default()
    });
    let sleeve_material = materials.add(StandardMaterial {
        base_color_texture: Some(skin),
        unlit: true,
        alpha_mode: AlphaMode::Mask(0.1),
        perceptual_roughness: 1.0,
        depth_bias: 10_001.0,
        ..default()
    });
    let base_mesh = meshes.add(arm_mesh(false));
    let sleeve_mesh = meshes.add(arm_mesh(true));
    let pose = hand_pose(&MiningProgress::default());

    commands.entity(*camera).with_children(|camera| {
        camera
            .spawn((
                Name::new("First Person Hand Root"),
                FirstPersonHandRoot,
                Transform::from_translation(pose.translation).with_rotation(pose.rotation),
                Visibility::Inherited,
            ))
            .with_children(|pivot| {
                pivot.spawn((
                    Name::new("Steve Right Arm"),
                    Mesh3d(base_mesh),
                    MeshMaterial3d(base_material),
                ));
                pivot.spawn((
                    Name::new("Steve Right Sleeve"),
                    Mesh3d(sleeve_mesh),
                    MeshMaterial3d(sleeve_material),
                ));
            });
    });
}

pub(super) fn sync_first_person_hand(
    inventory_state: Res<InventoryState>,
    mining: Res<MiningProgress>,
    mut hand: Single<(&mut Transform, &mut Visibility), With<FirstPersonHandRoot>>,
) {
    *hand.1 = if inventory_state.is_open() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    let pose = hand_pose(&mining);
    hand.0.translation = pose.translation;
    hand.0.rotation = pose.rotation;
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HandPose {
    translation: Vec3,
    rotation: Quat,
}

fn hand_pose(mining: &MiningProgress) -> HandPose {
    if !mining.is_active() {
        return HandPose {
            translation: IDLE_TRANSLATION,
            rotation: Quat::from_euler(
                EulerRot::XYZ,
                IDLE_ROTATION.x,
                IDLE_ROTATION.y,
                IDLE_ROTATION.z,
            ),
        };
    }

    let phase = (mining.elapsed() * 3.2).fract();
    let swing = (phase * PI).sin();
    let follow_through = (phase * PI * 2.0).sin();
    HandPose {
        translation: IDLE_TRANSLATION + Vec3::new(-0.03 * swing, 0.02 * swing, -0.04 * swing),
        rotation: Quat::from_euler(
            EulerRot::XYZ,
            IDLE_ROTATION.x + 0.85 * swing,
            IDLE_ROTATION.y + 0.22 * swing,
            IDLE_ROTATION.z + 0.16 * follow_through,
        ),
    }
}

#[derive(Clone, Copy)]
struct UvRect {
    min: Vec2,
    max: Vec2,
}

impl UvRect {
    const fn pixels(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            min: Vec2::new(x / 128.0, y / 128.0),
            max: Vec2::new((x + width) / 128.0, (y + height) / 128.0),
        }
    }

    fn corners(self) -> [[f32; 2]; 4] {
        [
            [self.min.x, self.max.y],
            [self.max.x, self.max.y],
            [self.max.x, self.min.y],
            [self.min.x, self.min.y],
        ]
    }
}

fn arm_mesh(sleeve: bool) -> Mesh {
    let expansion = if sleeve { 0.008 } else { 0.0 };
    let half_x = ARM_WIDTH * 0.5 + expansion;
    let half_z = ARM_DEPTH * 0.5 + expansion;
    let top = expansion;
    let bottom = -ARM_HEIGHT - expansion;

    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut uvs = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    let texture_y = if sleeve { 72.0 } else { 40.0 };
    let cap_y = if sleeve { 64.0 } else { 32.0 };
    let faces = [
        (
            [
                [-half_x, bottom, half_z],
                [half_x, bottom, half_z],
                [half_x, top, half_z],
                [-half_x, top, half_z],
            ],
            [0.0, 0.0, 1.0],
            UvRect::pixels(88.0, texture_y, 8.0, 24.0),
        ),
        (
            [
                [half_x, bottom, -half_z],
                [-half_x, bottom, -half_z],
                [-half_x, top, -half_z],
                [half_x, top, -half_z],
            ],
            [0.0, 0.0, -1.0],
            UvRect::pixels(104.0, texture_y, 8.0, 24.0),
        ),
        (
            [
                [half_x, bottom, half_z],
                [half_x, bottom, -half_z],
                [half_x, top, -half_z],
                [half_x, top, half_z],
            ],
            [1.0, 0.0, 0.0],
            UvRect::pixels(80.0, texture_y, 8.0, 24.0),
        ),
        (
            [
                [-half_x, bottom, -half_z],
                [-half_x, bottom, half_z],
                [-half_x, top, half_z],
                [-half_x, top, -half_z],
            ],
            [-1.0, 0.0, 0.0],
            UvRect::pixels(96.0, texture_y, 8.0, 24.0),
        ),
        (
            [
                [-half_x, top, half_z],
                [half_x, top, half_z],
                [half_x, top, -half_z],
                [-half_x, top, -half_z],
            ],
            [0.0, 1.0, 0.0],
            UvRect::pixels(88.0, cap_y, 8.0, 8.0),
        ),
        (
            [
                [-half_x, bottom, -half_z],
                [half_x, bottom, -half_z],
                [half_x, bottom, half_z],
                [-half_x, bottom, half_z],
            ],
            [0.0, -1.0, 0.0],
            UvRect::pixels(96.0, cap_y, 8.0, 8.0),
        ),
    ];

    for (face, normal, uv) in faces {
        let start = positions.len() as u32;
        positions.extend(face);
        normals.extend([normal; 4]);
        uvs.extend(uv.corners());
        indices.extend([start, start + 1, start + 2, start + 2, start + 3, start]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_hand_uses_stable_resting_pose() {
        let pose = hand_pose(&MiningProgress::default());
        assert_eq!(pose.translation, IDLE_TRANSLATION);
        assert_eq!(
            pose.rotation,
            Quat::from_euler(
                EulerRot::XYZ,
                IDLE_ROTATION.x,
                IDLE_ROTATION.y,
                IDLE_ROTATION.z
            )
        );
    }

    #[test]
    fn arm_is_a_complete_textured_cuboid() {
        let mesh = arm_mesh(false);
        assert_eq!(mesh.count_vertices(), 24);
        assert_eq!(mesh.indices().map(Indices::len), Some(36));
    }
}
