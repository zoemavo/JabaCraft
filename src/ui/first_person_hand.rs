use std::f32::consts::PI;

use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};

use crate::{interaction::MiningProgress, inventory::InventoryState, player::PlayerCamera};

const ARM_WIDTH: f32 = 0.22;
const ARM_HEIGHT: f32 = 0.62;
const ARM_DEPTH: f32 = 0.30;
const SLEEVE_HEIGHT: f32 = ARM_HEIGHT * (8.0 / 12.0);

// Camera-local first-person viewmodel tuning.
const VIEWMODEL_HAND_TRANSLATION: Vec3 = Vec3::new(0.60, -0.48, -0.72);
const VIEWMODEL_HAND_ROTATION: Vec3 = Vec3::new(-0.85, -1.18, -0.42);
const VIEWMODEL_HAND_SCALE: Vec3 = Vec3::splat(0.84);
const VIEWMODEL_FOV: f32 = 60.0;
// The mesh is authored with its long axis pointing down; this local asset
// rotation lets the runtime transform stay in the usual Minecraft-like range.
const VIEWMODEL_MESH_ROTATION: f32 = -1.92;

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
        perceptual_roughness: 1.0,
        depth_bias: 10_000.0,
        ..default()
    });
    let arm_mesh = meshes.add(arm_mesh());
    let pose = hand_pose(&MiningProgress::default());

    commands.entity(*camera).with_children(|camera| {
        camera
            .spawn((
                Name::new("First Person Viewmodel Camera"),
                Camera3d::default(),
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                Projection::from(PerspectiveProjection {
                    fov: VIEWMODEL_FOV.to_radians(),
                    ..default()
                }),
                Tonemapping::None,
                RenderLayers::layer(1),
                Transform::default(),
            ))
            .with_children(|viewmodel_camera| {
                // The hand is rendered on its own layer.  Give that layer a
                // small camera-local key light so the cube's faces remain
                // visibly three-dimensional without changing world lighting.
                viewmodel_camera.spawn((
                    Name::new("First Person Viewmodel Key Light"),
                    DirectionalLight {
                        illuminance: 700.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    RenderLayers::layer(1),
                    Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.7, 0.0)),
                ));
                viewmodel_camera
                    .spawn((
                        Name::new("First Person Hand Root"),
                        FirstPersonHandRoot,
                        RenderLayers::layer(1),
                        Transform::from_translation(pose.translation)
                            .with_rotation(pose.rotation)
                            .with_scale(pose.scale),
                        Visibility::Inherited,
                    ))
                    .with_children(|pivot| {
                        pivot.spawn((
                            Name::new("Steve Right Arm"),
                            Mesh3d(arm_mesh),
                            MeshMaterial3d(base_material),
                            RenderLayers::layer(1),
                        ));
                    });
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
    hand.0.scale = pose.scale;
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HandPose {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

fn hand_pose(mining: &MiningProgress) -> HandPose {
    if !mining.is_active() {
        return HandPose {
            translation: VIEWMODEL_HAND_TRANSLATION,
            rotation: Quat::from_euler(
                EulerRot::XYZ,
                VIEWMODEL_HAND_ROTATION.x,
                VIEWMODEL_HAND_ROTATION.y,
                VIEWMODEL_HAND_ROTATION.z,
            ),
            scale: VIEWMODEL_HAND_SCALE,
        };
    }

    let phase = (mining.elapsed() * 3.2).fract();
    let swing = (phase * PI).sin();
    let follow_through = (phase * PI * 2.0).sin();
    HandPose {
        translation: VIEWMODEL_HAND_TRANSLATION
            + Vec3::new(-0.025 * swing, 0.02 * swing, -0.04 * swing),
        rotation: Quat::from_euler(
            EulerRot::XYZ,
            VIEWMODEL_HAND_ROTATION.x + 0.30 * swing,
            VIEWMODEL_HAND_ROTATION.y + 0.16 * swing,
            VIEWMODEL_HAND_ROTATION.z + 0.12 * follow_through,
        ),
        scale: VIEWMODEL_HAND_SCALE,
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

fn arm_mesh() -> Mesh {
    let mut positions = Vec::with_capacity(48);
    let mut normals = Vec::with_capacity(48);
    let mut uvs = Vec::with_capacity(48);
    let mut indices = Vec::with_capacity(72);

    // Base layer: the complete 4x12x4 arm, including the visible hand.
    append_cuboid(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        ARM_WIDTH,
        ARM_HEIGHT,
        0.0,
        UvRect::pixels(88.0, 40.0, 8.0, 24.0),
    );
    // Outer layer: a slightly larger 4x8x4 shirt sleeve. It overlaps the
    // base mesh at the shoulder, so the first-person arm remains one
    // continuous model while retaining the classic Steve silhouette.
    append_cuboid(
        &mut positions,
        &mut normals,
        &mut uvs,
        &mut indices,
        ARM_WIDTH * 1.08,
        SLEEVE_HEIGHT,
        0.0,
        UvRect::pixels(72.0, 40.0, 8.0, 24.0),
    );

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

fn append_cuboid(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    width: f32,
    height: f32,
    top: f32,
    side_uv: UvRect,
) {
    let half_x = width * 0.5;
    let half_z = ARM_DEPTH * (width / ARM_WIDTH) * 0.5;
    let bottom = top - height;
    let cap_y = side_uv.min.y * 128.0 - 8.0;
    let faces = [
        (
            [
                [-half_x, bottom, half_z],
                [half_x, bottom, half_z],
                [half_x, top, half_z],
                [-half_x, top, half_z],
            ],
            [0.0, 0.0, 1.0],
            side_uv,
        ),
        (
            [
                [half_x, bottom, -half_z],
                [-half_x, bottom, -half_z],
                [-half_x, top, -half_z],
                [half_x, top, -half_z],
            ],
            [0.0, 0.0, -1.0],
            UvRect::pixels(
                side_uv.min.x + 16.0,
                side_uv.min.y,
                8.0,
                side_uv.max.y * 128.0 - side_uv.min.y * 128.0,
            ),
        ),
        (
            [
                [half_x, bottom, half_z],
                [half_x, bottom, -half_z],
                [half_x, top, -half_z],
                [half_x, top, half_z],
            ],
            [1.0, 0.0, 0.0],
            UvRect::pixels(
                side_uv.min.x - 8.0,
                side_uv.min.y,
                8.0,
                side_uv.max.y * 128.0 - side_uv.min.y * 128.0,
            ),
        ),
        (
            [
                [-half_x, bottom, -half_z],
                [-half_x, bottom, half_z],
                [-half_x, top, half_z],
                [-half_x, top, -half_z],
            ],
            [-1.0, 0.0, 0.0],
            UvRect::pixels(
                side_uv.min.x + 8.0,
                side_uv.min.y,
                8.0,
                side_uv.max.y * 128.0 - side_uv.min.y * 128.0,
            ),
        ),
        (
            [
                [-half_x, top, half_z],
                [half_x, top, half_z],
                [half_x, top, -half_z],
                [-half_x, top, -half_z],
            ],
            [0.0, 1.0, 0.0],
            UvRect::pixels(side_uv.min.x, cap_y, 8.0, 8.0),
        ),
        (
            [
                [-half_x, bottom, -half_z],
                [half_x, bottom, -half_z],
                [half_x, bottom, half_z],
                [-half_x, bottom, half_z],
            ],
            [0.0, -1.0, 0.0],
            UvRect::pixels(side_uv.min.x + 8.0, cap_y, 8.0, 8.0),
        ),
    ];

    let model_rotation = Quat::from_rotation_z(VIEWMODEL_MESH_ROTATION);
    for (face, normal, uv) in faces {
        let start = positions.len() as u32;
        positions.extend(face.map(|vertex| (model_rotation * Vec3::from_array(vertex)).to_array()));
        let oriented_normal = (model_rotation * Vec3::from_array(normal)).to_array();
        normals.extend([oriented_normal; 4]);
        uvs.extend(uv.corners());
        indices.extend([start, start + 1, start + 2, start + 2, start + 3, start]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_hand_uses_stable_resting_pose() {
        let pose = hand_pose(&MiningProgress::default());
        assert_eq!(pose.translation, VIEWMODEL_HAND_TRANSLATION);
        assert_eq!(
            pose.rotation,
            Quat::from_euler(
                EulerRot::XYZ,
                VIEWMODEL_HAND_ROTATION.x,
                VIEWMODEL_HAND_ROTATION.y,
                VIEWMODEL_HAND_ROTATION.z
            )
        );
        assert_eq!(pose.scale, VIEWMODEL_HAND_SCALE);
    }

    #[test]
    fn arm_contains_complete_textured_base_and_sleeve_cuboids() {
        let mesh = arm_mesh();
        assert_eq!(mesh.count_vertices(), 48);
        assert_eq!(mesh.indices().map(Indices::len), Some(72));
    }
}
