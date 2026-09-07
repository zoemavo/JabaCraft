//! Exact voxel selection using Amanatides-Woo style 3D grid traversal.

use bevy::prelude::*;

use crate::{
    block::{BlockFace, BlockId},
    chunk::ChunkStorage,
    coordinates::WorldBlockPos,
};

const AXIS_TIE_EPSILON: f32 = 1.0e-6;

/// First non-air voxel reached by a ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelRaycastHit {
    pub position: WorldBlockPos,
    pub block: BlockId,
    pub face: BlockFace,
    pub normal: IVec3,
    /// Last traversed air voxel, or `None` when the ray starts inside a block.
    pub previous_empty: Option<WorldBlockPos>,
    pub distance: f32,
}

/// Traverses the world grid without sampling small intervals along the ray.
/// An unloaded chunk terminates the query instead of selecting through unknown data.
pub fn voxel_raycast(
    storage: &ChunkStorage,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<VoxelRaycastHit> {
    raycast_dda(origin, direction, max_distance, |position| {
        storage.get_block(position)
    })
}

fn raycast_dda(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    mut block_at: impl FnMut(WorldBlockPos) -> Option<BlockId>,
) -> Option<VoxelRaycastHit> {
    if !origin.is_finite()
        || !direction.is_finite()
        || !max_distance.is_finite()
        || max_distance < 0.0
    {
        return None;
    }
    let direction = direction.try_normalize()?;
    let step = IVec3::new(
        axis_step(direction.x),
        axis_step(direction.y),
        axis_step(direction.z),
    );
    let mut cell = IVec3::new(
        initial_cell(origin.x, direction.x),
        initial_cell(origin.y, direction.y),
        initial_cell(origin.z, direction.z),
    );

    let first_position = world_position(cell);
    let first_block = block_at(first_position)?;
    if first_block != BlockId::AIR {
        let normal = initial_hit_normal(direction);
        return Some(VoxelRaycastHit {
            position: first_position,
            block: first_block,
            face: face_from_normal(normal),
            normal,
            previous_empty: None,
            distance: 0.0,
        });
    }

    let mut previous_empty = first_position;
    let mut t_max = Vec3::new(
        distance_to_boundary(origin.x, direction.x, cell.x, step.x),
        distance_to_boundary(origin.y, direction.y, cell.y, step.y),
        distance_to_boundary(origin.z, direction.z, cell.z, step.z),
    );
    let t_delta = Vec3::new(
        reciprocal_distance(direction.x),
        reciprocal_distance(direction.y),
        reciprocal_distance(direction.z),
    );

    loop {
        let distance = t_max.min_element();
        if distance > max_distance {
            return None;
        }

        let crosses_x = t_max.x <= distance + AXIS_TIE_EPSILON;
        let crosses_y = t_max.y <= distance + AXIS_TIE_EPSILON;
        let crosses_z = t_max.z <= distance + AXIS_TIE_EPSILON;
        let normal = if crosses_x {
            IVec3::new(-step.x, 0, 0)
        } else if crosses_y {
            IVec3::new(0, -step.y, 0)
        } else {
            IVec3::new(0, 0, -step.z)
        };

        if crosses_x {
            cell.x += step.x;
            t_max.x += t_delta.x;
        }
        if crosses_y {
            cell.y += step.y;
            t_max.y += t_delta.y;
        }
        if crosses_z {
            cell.z += step.z;
            t_max.z += t_delta.z;
        }
        let position = world_position(cell);
        let block = block_at(position)?;

        if block != BlockId::AIR {
            return Some(VoxelRaycastHit {
                position,
                block,
                face: face_from_normal(normal),
                normal,
                previous_empty: Some(previous_empty),
                distance,
            });
        }
        previous_empty = position;
    }
}

fn axis_step(direction: f32) -> i32 {
    if direction > 0.0 {
        1
    } else if direction < 0.0 {
        -1
    } else {
        0
    }
}

fn initial_cell(origin: f32, direction: f32) -> i32 {
    let floor = origin.floor();
    if direction < 0.0 && origin == floor {
        floor as i32 - 1
    } else {
        floor as i32
    }
}

fn distance_to_boundary(origin: f32, direction: f32, cell: i32, step: i32) -> f32 {
    if step == 0 {
        return f32::INFINITY;
    }
    let boundary = if step > 0 { cell + 1 } else { cell } as f32;
    ((boundary - origin) / direction).max(0.0)
}

fn reciprocal_distance(direction: f32) -> f32 {
    if direction == 0.0 {
        f32::INFINITY
    } else {
        direction.recip().abs()
    }
}

fn initial_hit_normal(direction: Vec3) -> IVec3 {
    let absolute = direction.abs();
    if absolute.x >= absolute.y && absolute.x >= absolute.z {
        IVec3::new(-axis_step(direction.x), 0, 0)
    } else if absolute.y >= absolute.z {
        IVec3::new(0, -axis_step(direction.y), 0)
    } else {
        IVec3::new(0, 0, -axis_step(direction.z))
    }
}

fn face_from_normal(normal: IVec3) -> BlockFace {
    match (normal.x, normal.y, normal.z) {
        (1, 0, 0) => BlockFace::East,
        (-1, 0, 0) => BlockFace::West,
        (0, 1, 0) => BlockFace::Top,
        (0, -1, 0) => BlockFace::Bottom,
        (0, 0, 1) => BlockFace::South,
        (0, 0, -1) => BlockFace::North,
        _ => unreachable!("DDA normals are always axis-aligned unit vectors"),
    }
}

fn world_position(cell: IVec3) -> WorldBlockPos {
    WorldBlockPos::new(cell.x, cell.y, cell.z)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn cast(
        blocks: &HashMap<WorldBlockPos, BlockId>,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
    ) -> Option<VoxelRaycastHit> {
        raycast_dda(origin, direction, max_distance, |position| {
            Some(*blocks.get(&position).unwrap_or(&BlockId::AIR))
        })
    }

    #[test]
    fn positive_x_hit_returns_block_face_and_previous_air() {
        let blocks = HashMap::from([(WorldBlockPos::new(3, 1, 1), BlockId::STONE)]);

        let hit = cast(&blocks, Vec3::new(0.5, 1.5, 1.5), Vec3::X, 5.0).unwrap();

        assert_eq!(hit.position, WorldBlockPos::new(3, 1, 1));
        assert_eq!(hit.block, BlockId::STONE);
        assert_eq!(hit.normal, IVec3::NEG_X);
        assert_eq!(hit.face, BlockFace::West);
        assert_eq!(hit.previous_empty, Some(WorldBlockPos::new(2, 1, 1)));
        assert!((hit.distance - 2.5).abs() < 0.0001);
    }

    #[test]
    fn negative_axis_traversal_handles_negative_world_coordinates() {
        let blocks = HashMap::from([(WorldBlockPos::new(-2, 0, 0), BlockId::DIRT)]);

        let hit = cast(&blocks, Vec3::new(0.5, 0.5, 0.5), Vec3::NEG_X, 5.0).unwrap();

        assert_eq!(hit.position, WorldBlockPos::new(-2, 0, 0));
        assert_eq!(hit.normal, IVec3::X);
        assert_eq!(hit.face, BlockFace::East);
        assert_eq!(hit.previous_empty, Some(WorldBlockPos::new(-1, 0, 0)));
        assert!((hit.distance - 1.5).abs() < 0.0001);
    }

    #[test]
    fn downward_hit_returns_top_face() {
        let blocks = HashMap::from([(WorldBlockPos::new(2, -3, 4), BlockId::GRASS)]);

        let hit = cast(&blocks, Vec3::new(2.5, 1.25, 4.5), Vec3::NEG_Y, 5.0).unwrap();

        assert_eq!(hit.position, WorldBlockPos::new(2, -3, 4));
        assert_eq!(hit.normal, IVec3::Y);
        assert_eq!(hit.face, BlockFace::Top);
        assert_eq!(hit.previous_empty, Some(WorldBlockPos::new(2, -2, 4)));
        assert!((hit.distance - 3.25).abs() < 0.0001);
    }

    #[test]
    fn exact_corner_crossing_skips_cells_only_touched_at_the_corner() {
        let blocks = HashMap::from([
            (WorldBlockPos::new(1, 0, 0), BlockId::STONE),
            (WorldBlockPos::new(1, 1, 1), BlockId::IRON_ORE),
        ]);

        let hit = cast(&blocks, Vec3::splat(0.5), Vec3::ONE, 5.0).unwrap();

        assert_eq!(hit.position, WorldBlockPos::new(1, 1, 1));
        assert_eq!(hit.block, BlockId::IRON_ORE);
        assert_eq!(hit.previous_empty, Some(WorldBlockPos::new(0, 0, 0)));
        assert_eq!(hit.normal, IVec3::NEG_X);
    }

    #[test]
    fn non_unit_direction_is_normalized_for_world_distance() {
        let blocks = HashMap::from([(WorldBlockPos::new(0, 0, 4), BlockId::WOOD)]);

        let hit = cast(&blocks, Vec3::splat(0.5), Vec3::new(0.0, 0.0, 20.0), 4.0).unwrap();

        assert_eq!(hit.position, WorldBlockPos::new(0, 0, 4));
        assert_eq!(hit.face, BlockFace::North);
        assert!((hit.distance - 3.5).abs() < 0.0001);
    }

    #[test]
    fn block_beyond_reach_is_not_returned() {
        let blocks = HashMap::from([(WorldBlockPos::new(6, 0, 0), BlockId::STONE)]);

        assert!(cast(&blocks, Vec3::splat(0.5), Vec3::X, 5.0).is_none());
    }

    #[test]
    fn start_inside_block_has_no_previous_empty_cell() {
        let blocks = HashMap::from([(WorldBlockPos::new(1, 2, 3), BlockId::COAL_ORE)]);

        let hit = cast(
            &blocks,
            Vec3::new(1.25, 2.75, 3.5),
            Vec3::new(1.0, 0.1, 0.0),
            5.0,
        )
        .unwrap();

        assert_eq!(hit.position, WorldBlockPos::new(1, 2, 3));
        assert_eq!(hit.previous_empty, None);
        assert_eq!(hit.distance, 0.0);
        assert_eq!(hit.normal, IVec3::NEG_X);
    }

    #[test]
    fn exact_boundary_uses_cell_in_forward_negative_direction() {
        let blocks = HashMap::from([(WorldBlockPos::new(0, 0, 0), BlockId::IRON_ORE)]);

        let hit = cast(&blocks, Vec3::new(1.0, 0.5, 0.5), Vec3::NEG_X, 1.0).unwrap();

        assert_eq!(hit.position, WorldBlockPos::new(0, 0, 0));
        assert_eq!(hit.face, BlockFace::East);
        assert_eq!(hit.distance, 0.0);
    }

    #[test]
    fn invalid_or_zero_ray_returns_no_hit() {
        let blocks = HashMap::new();

        assert!(cast(&blocks, Vec3::ZERO, Vec3::ZERO, 5.0).is_none());
        assert!(cast(&blocks, Vec3::ZERO, Vec3::X, -1.0).is_none());
        assert!(cast(&blocks, Vec3::NAN, Vec3::X, 5.0).is_none());
    }

    #[test]
    fn unloaded_cell_stops_traversal() {
        let hit = raycast_dda(Vec3::splat(0.5), Vec3::X, 5.0, |position| {
            if position.x == 1 {
                None
            } else if position.x == 2 {
                Some(BlockId::STONE)
            } else {
                Some(BlockId::AIR)
            }
        });

        assert!(hit.is_none());
    }
}
