//! Lightweight wireframe feedback for the block selected by the camera raycast.

use bevy::prelude::*;

use crate::coordinates::WorldBlockPos;

use super::CameraRaycast;

const BLOCK_SIZE: f32 = 1.0;
const OUTLINE_PADDING: f32 = 0.004;
const OUTLINE_COLOR: Color = Color::srgb(1.0, 0.86, 0.18);

const CUBE_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (1, 3),
    (3, 2),
    (2, 0),
    (4, 5),
    (5, 7),
    (7, 6),
    (6, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// Keeps block-selection lines independent from other debug gizmos.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub(super) struct BlockOutlineGizmos;

pub(super) fn configure_block_outline(mut configs: ResMut<GizmoConfigStore>) {
    let (config, _) = configs.config_mut::<BlockOutlineGizmos>();
    config.line.width = 3.0;
    config.line.perspective = false;
    config.depth_bias = -0.001;
}

/// Draws into Bevy's reusable gizmo buffers. No gameplay entity or material cube is spawned.
pub(super) fn draw_block_outline(
    selected: Res<CameraRaycast>,
    mut gizmos: Gizmos<BlockOutlineGizmos>,
) {
    let Some(hit) = selected.0 else {
        return;
    };

    let corners = outline_corners(hit.position, OUTLINE_PADDING);
    for (start, end) in CUBE_EDGES {
        gizmos.line(corners[start], corners[end], OUTLINE_COLOR);
    }
}

fn outline_corners(position: WorldBlockPos, padding: f32) -> [Vec3; 8] {
    let min =
        Vec3::new(position.x as f32, position.y as f32, position.z as f32) - Vec3::splat(padding);
    let max = min + Vec3::splat(BLOCK_SIZE + padding * 2.0);

    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, max.y, max.z),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_encloses_selected_block_with_padding() {
        let corners = outline_corners(WorldBlockPos::new(-2, 3, 4), OUTLINE_PADDING);

        assert!((corners[0] - Vec3::new(-2.004, 2.996, 3.996)).length() < 1.0e-6);
        assert!((corners[7] - Vec3::new(-0.996, 4.004, 5.004)).length() < 1.0e-6);
    }

    #[test]
    fn cube_has_twelve_unique_edges() {
        let mut edges = CUBE_EDGES;
        for (start, end) in &mut edges {
            if *start > *end {
                std::mem::swap(start, end);
            }
        }
        edges.sort_unstable();

        assert!(edges.windows(2).all(|pair| pair[0] != pair[1]));
    }
}
