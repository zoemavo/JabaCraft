//! Axis-aligned player collision against the authoritative voxel storage.

use bevy::prelude::*;

use crate::{block::BlockRegistry, chunk::ChunkStorage, coordinates::WorldBlockPos};

use super::{Grounded, Noclip, Player, PlayerCollider, Velocity};

/// Keeping every substep below half a voxel prevents crossing a one-block wall
/// between collision queries at normal player speeds.
const MAX_COLLISION_STEP: f32 = 0.45;
const CONTACT_EPSILON: f32 = 0.0001;
const GROUND_PROBE_DISTANCE: f32 = 0.002;

pub(super) fn move_players_with_voxel_collisions(
    time: Res<Time>,
    storage: Res<ChunkStorage>,
    registry: Res<BlockRegistry>,
    mut players: Query<
        (
            &PlayerCollider,
            &Noclip,
            &mut Transform,
            &mut Velocity,
            &mut Grounded,
        ),
        With<Player>,
    >,
) {
    let delta_seconds = time.delta_secs();
    for (collider, noclip, mut transform, mut velocity, mut grounded) in &mut players {
        if noclip.0 {
            transform.translation += velocity.0 * delta_seconds.max(0.0);
            grounded.0 = false;
            continue;
        }

        let previous_grounded = grounded.0;
        let result = move_aabb(
            transform.translation,
            collider.half_extents,
            &mut velocity.0,
            delta_seconds,
            &storage,
            &registry,
        );

        transform.translation = result.position;
        grounded.0 = result.grounded
            || (result.waiting_for_world && previous_grounded && velocity.0.y <= 0.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MovementResult {
    position: Vec3,
    grounded: bool,
    waiting_for_world: bool,
}

fn move_aabb(
    mut position: Vec3,
    half_extents: Vec3,
    velocity: &mut Vec3,
    delta_seconds: f32,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
) -> MovementResult {
    let half_extents = half_extents.abs().max(Vec3::splat(0.005));
    let displacement = *velocity * delta_seconds.max(0.0);
    let step_count = (displacement.abs().max_element() / MAX_COLLISION_STEP)
        .ceil()
        .max(1.0) as usize;
    let step = displacement / step_count as f32;
    let mut grounded = false;
    let mut waiting_for_world = false;

    for _ in 0..step_count {
        for axis in Axis::ALL {
            let amount = axis.component(step);
            if amount == 0.0 {
                continue;
            }

            match move_axis(&mut position, half_extents, amount, axis, storage, registry) {
                AxisMove::Free => {}
                AxisMove::Solid => {
                    axis.set_component(velocity, 0.0);
                    grounded |= axis == Axis::Y && amount < 0.0;
                }
                AxisMove::Unloaded => {
                    axis.set_component(velocity, 0.0);
                    waiting_for_world = true;
                }
            }
        }
    }

    if !grounded && velocity.y <= 0.0 {
        let probe_position = position - Vec3::Y * GROUND_PROBE_DISTANCE;
        match collision_at(probe_position, half_extents, storage, registry) {
            CollisionAt::Solid(_) => grounded = true,
            CollisionAt::Unloaded => waiting_for_world = true,
            CollisionAt::Free => {}
        }
    }

    MovementResult {
        position,
        grounded,
        waiting_for_world,
    }
}

fn move_axis(
    position: &mut Vec3,
    half_extents: Vec3,
    amount: f32,
    axis: Axis,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
) -> AxisMove {
    let previous = axis.component(*position);
    axis.set_component(position, previous + amount);

    match collision_at(*position, half_extents, storage, registry) {
        CollisionAt::Free => AxisMove::Free,
        CollisionAt::Unloaded => {
            axis.set_component(position, previous);
            AxisMove::Unloaded
        }
        CollisionAt::Solid(blocks) => {
            let half_extent = axis.component(half_extents);
            let resolved = if amount > 0.0 {
                blocks
                    .iter()
                    .map(|block| axis.block_component(*block) as f32 - half_extent)
                    .fold(f32::INFINITY, f32::min)
            } else {
                blocks
                    .iter()
                    .map(|block| axis.block_component(*block) as f32 + 1.0 + half_extent)
                    .fold(f32::NEG_INFINITY, f32::max)
            };
            axis.set_component(position, resolved);
            AxisMove::Solid
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AxisMove {
    Free,
    Solid,
    Unloaded,
}

#[derive(Debug, Eq, PartialEq)]
enum CollisionAt {
    Free,
    Solid(Vec<WorldBlockPos>),
    Unloaded,
}

fn collision_at(
    center: Vec3,
    half_extents: Vec3,
    storage: &ChunkStorage,
    registry: &BlockRegistry,
) -> CollisionAt {
    let Some(bounds) = BlockBounds::from_aabb(center, half_extents) else {
        return CollisionAt::Free;
    };
    let mut solid_blocks = Vec::new();

    for y in bounds.min.y..=bounds.max.y {
        for z in bounds.min.z..=bounds.max.z {
            for x in bounds.min.x..=bounds.max.x {
                let position = WorldBlockPos::new(x, y, z);
                let Some(block) = storage.get_block(position) else {
                    return CollisionAt::Unloaded;
                };
                if registry.is_solid(block) {
                    solid_blocks.push(position);
                }
            }
        }
    }

    if solid_blocks.is_empty() {
        CollisionAt::Free
    } else {
        CollisionAt::Solid(solid_blocks)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BlockBounds {
    min: WorldBlockPos,
    max: WorldBlockPos,
}

impl BlockBounds {
    fn from_aabb(center: Vec3, half_extents: Vec3) -> Option<Self> {
        if !center.is_finite() || !half_extents.is_finite() {
            return None;
        }

        let min = center - half_extents + Vec3::splat(CONTACT_EPSILON);
        let max = center + half_extents - Vec3::splat(CONTACT_EPSILON);
        Some(Self {
            min: WorldBlockPos::new(
                min.x.floor() as i32,
                min.y.floor() as i32,
                min.z.floor() as i32,
            ),
            max: WorldBlockPos::new(
                max.x.floor() as i32,
                max.y.floor() as i32,
                max.z.floor() as i32,
            ),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    fn component(self, vector: Vec3) -> f32 {
        match self {
            Self::X => vector.x,
            Self::Y => vector.y,
            Self::Z => vector.z,
        }
    }

    fn set_component(self, vector: &mut Vec3, value: f32) {
        match self {
            Self::X => vector.x = value,
            Self::Y => vector.y = value,
            Self::Z => vector.z = value,
        }
    }

    fn block_component(self, position: WorldBlockPos) -> i32 {
        match self {
            Self::X => position.x,
            Self::Y => position.y,
            Self::Z => position.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{block::BlockId, chunk::Chunk, coordinates::ChunkPos};

    use super::*;

    const PLAYER_HALF_EXTENTS: Vec3 = Vec3::new(0.3, 0.9, 0.3);

    fn storage_with_origin_chunk() -> ChunkStorage {
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(ChunkPos::default(), Chunk::default());
        storage
    }

    fn put(storage: &mut ChunkStorage, x: i32, y: i32, z: i32) {
        storage
            .set_block(WorldBlockPos::new(x, y, z), BlockId::STONE)
            .unwrap();
    }

    #[test]
    fn standing_player_detects_support_without_scanning_the_chunk() {
        let mut storage = storage_with_origin_chunk();
        put(&mut storage, 0, 0, 0);
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::ZERO;

        let result = move_aabb(
            Vec3::new(0.5, 1.9, 0.5),
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            1.0 / 60.0,
            &storage,
            &registry,
        );

        assert!(result.grounded);
        assert_eq!(result.position.y, 1.9);
    }

    #[test]
    fn falling_player_lands_on_a_block() {
        let mut storage = storage_with_origin_chunk();
        put(&mut storage, 0, 0, 0);
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::new(0.0, -30.0, 0.0);

        let result = move_aabb(
            Vec3::new(0.5, 8.0, 0.5),
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            0.5,
            &storage,
            &registry,
        );

        assert!(result.grounded);
        assert!((result.position.y - 1.9).abs() < 0.0001);
        assert_eq!(velocity.y, 0.0);
    }

    #[test]
    fn player_cannot_tunnel_through_a_wall() {
        let mut storage = storage_with_origin_chunk();
        put(&mut storage, 2, 1, 0);
        put(&mut storage, 2, 2, 0);
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::new(30.0, 0.0, 0.0);

        let result = move_aabb(
            Vec3::new(0.5, 1.9, 0.5),
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            0.5,
            &storage,
            &registry,
        );

        assert!((result.position.x - 1.7).abs() < 0.0001);
        assert_eq!(velocity.x, 0.0);
    }

    #[test]
    fn jumping_player_hits_a_ceiling() {
        let mut storage = storage_with_origin_chunk();
        put(&mut storage, 0, 3, 0);
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::new(0.0, 12.0, 0.0);

        let result = move_aabb(
            Vec3::new(0.5, 1.9, 0.5),
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            0.25,
            &storage,
            &registry,
        );

        assert!((result.position.y - 2.1).abs() < 0.0001);
        assert_eq!(velocity.y, 0.0);
        assert!(!result.grounded);
    }

    #[test]
    fn horizontal_collision_still_allows_sliding_on_other_axis() {
        let mut storage = storage_with_origin_chunk();
        put(&mut storage, 2, 1, 0);
        put(&mut storage, 2, 2, 0);
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::new(6.0, 0.0, 2.0);

        let result = move_aabb(
            Vec3::new(1.5, 1.9, 0.5),
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            0.25,
            &storage,
            &registry,
        );

        assert!((result.position.x - 1.7).abs() < 0.0001);
        assert!(result.position.z > 0.5);
        assert_eq!(velocity.x, 0.0);
        assert_eq!(velocity.z, 2.0);
    }

    #[test]
    fn empty_loaded_space_allows_falling() {
        let storage = storage_with_origin_chunk();
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::new(0.0, -4.0, 0.0);

        let result = move_aabb(
            Vec3::new(0.5, 8.0, 0.5),
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            0.25,
            &storage,
            &registry,
        );

        assert!((result.position.y - 7.0).abs() < 0.0001);
        assert_eq!(result.position.x, 0.5);
        assert_eq!(result.position.z, 0.5);
        assert!(!result.grounded);
    }

    #[test]
    fn collision_uses_negative_world_and_chunk_coordinates() {
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(ChunkPos::new(-1, 0, -1), Chunk::default());
        put(&mut storage, -1, 0, -1);
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::new(0.0, -20.0, 0.0);

        let result = move_aabb(
            Vec3::new(-0.5, 6.0, -0.5),
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            0.5,
            &storage,
            &registry,
        );

        assert!(result.grounded);
        assert!((result.position.y - 1.9).abs() < 0.0001);
        assert_eq!(velocity.y, 0.0);
    }

    #[test]
    fn unloaded_space_blocks_motion_until_voxels_arrive() {
        let storage = ChunkStorage::default();
        let registry = BlockRegistry::default();
        let mut velocity = Vec3::new(2.0, -2.0, 0.0);
        let start = Vec3::new(0.5, 1.9, 0.5);

        let result = move_aabb(
            start,
            PLAYER_HALF_EXTENTS,
            &mut velocity,
            0.25,
            &storage,
            &registry,
        );

        assert_eq!(result.position, start);
        assert!(result.waiting_for_world);
        assert_eq!(velocity, Vec3::ZERO);
    }

    #[test]
    fn broad_phase_uses_only_blocks_overlapping_the_aabb() {
        let bounds =
            BlockBounds::from_aabb(Vec3::new(4.5, 10.9, -2.5), PLAYER_HALF_EXTENTS).unwrap();

        assert_eq!(bounds.min, WorldBlockPos::new(4, 10, -3));
        assert_eq!(bounds.max, WorldBlockPos::new(4, 11, -3));
    }
}
