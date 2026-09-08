//! Placement of the currently selected block next to a raycast hit.

use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    block::{BlockId, BlockRegistry},
    chunk::ChunkStorage,
    coordinates::WorldBlockPos,
    generation::GenerationSettings,
    inventory::PlayerInventory,
    item::ItemRegistry,
    player::{Player, PlayerCollider},
};

use super::{CameraRaycast, InteractionSettings, VoxelRaycastHit, input::DebouncedButtonInput};

#[derive(Debug, Default, Resource)]
pub(super) struct BlockPlaceInput(DebouncedButtonInput);

pub(super) fn place_selected_block(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    settings: Res<InteractionSettings>,
    generation: Res<GenerationSettings>,
    registry: Res<BlockRegistry>,
    item_registry: Res<ItemRegistry>,
    player: Single<(&Transform, &PlayerCollider), With<Player>>,
    raycast: Res<CameraRaycast>,
    mut storage: ResMut<ChunkStorage>,
    mut inventory: ResMut<PlayerInventory>,
    mut input: ResMut<BlockPlaceInput>,
) {
    let cursor_captured = cursor.grab_mode != CursorGrabMode::None;
    let should_place = input.0.update(
        mouse_buttons.pressed(MouseButton::Right),
        mouse_buttons.just_pressed(MouseButton::Right),
        cursor_captured,
        time.delta_secs(),
        settings.place_repeat_interval,
    );
    if !should_place {
        return;
    }

    let Some(hit) = raycast.0 else {
        return;
    };
    let Some(position) = adjacent_position(hit) else {
        return;
    };
    let (player_transform, player_collider) = player.into_inner();
    let placed_block = inventory
        .selected_stack()
        .and_then(|stack| item_registry.block_for(stack.item()));
    if try_place_selected_block(
        &mut storage,
        &registry,
        &item_registry,
        &generation,
        player_transform.translation,
        player_collider.half_extents,
        position,
        &mut inventory,
    ) {
        debug!(
            "Placed {:?} at ({}, {}, {})",
            placed_block, position.x, position.y, position.z
        );
    }
}

fn adjacent_position(hit: VoxelRaycastHit) -> Option<WorldBlockPos> {
    let normal = hit.normal;
    if ![
        IVec3::X,
        IVec3::NEG_X,
        IVec3::Y,
        IVec3::NEG_Y,
        IVec3::Z,
        IVec3::NEG_Z,
    ]
    .contains(&normal)
    {
        return None;
    }

    Some(WorldBlockPos::new(
        hit.position.x.checked_add(normal.x)?,
        hit.position.y.checked_add(normal.y)?,
        hit.position.z.checked_add(normal.z)?,
    ))
}

fn try_place_block(
    storage: &mut ChunkStorage,
    registry: &BlockRegistry,
    generation: &GenerationSettings,
    player_center: Vec3,
    player_half_extents: Vec3,
    position: WorldBlockPos,
    block: BlockId,
) -> bool {
    if block == BlockId::AIR
        || position.y < generation.world_min_y
        || position.y >= generation.world_max_y
        || block_intersects_aabb(position, player_center, player_half_extents)
    {
        return false;
    }

    let Some(existing) = storage.get_block(position) else {
        return false;
    };
    if registry.is_solid(existing) {
        return false;
    }

    storage.set_block(position, block).is_ok()
}

fn try_place_selected_block(
    storage: &mut ChunkStorage,
    registry: &BlockRegistry,
    item_registry: &ItemRegistry,
    generation: &GenerationSettings,
    player_center: Vec3,
    player_half_extents: Vec3,
    position: WorldBlockPos,
    inventory: &mut PlayerInventory,
) -> bool {
    let Some(stack) = inventory.selected_stack() else {
        return false;
    };
    let Some(block) = item_registry.block_for(stack.item()) else {
        return false;
    };
    if !try_place_block(
        storage,
        registry,
        generation,
        player_center,
        player_half_extents,
        position,
        block,
    ) {
        return false;
    }

    let consumed = inventory.consume_selected_one();
    debug_assert!(consumed, "a successfully placed stack must be consumable");
    true
}

fn block_intersects_aabb(block: WorldBlockPos, center: Vec3, half_extents: Vec3) -> bool {
    if !center.is_finite() || !half_extents.is_finite() {
        return true;
    }

    let half_extents = half_extents.abs();
    let player_min = center - half_extents;
    let player_max = center + half_extents;
    let block_min = Vec3::new(block.x as f32, block.y as f32, block.z as f32);
    let block_max = block_min + Vec3::ONE;

    player_min.x < block_max.x
        && player_max.x > block_min.x
        && player_min.y < block_max.y
        && player_max.y > block_min.y
        && player_min.z < block_max.z
        && player_max.z > block_min.z
}

#[cfg(test)]
mod tests {
    use crate::{block::BlockFace, chunk::Chunk, coordinates::ChunkPos, item::ItemId};

    use super::*;

    const PLAYER_HALF_EXTENTS: Vec3 = Vec3::new(0.3, 0.9, 0.3);

    fn settings() -> GenerationSettings {
        GenerationSettings {
            world_min_y: -64,
            world_max_y: 192,
            ..default()
        }
    }

    #[test]
    fn placement_position_uses_hit_normal() {
        let hit = VoxelRaycastHit {
            position: WorldBlockPos::new(-2, 4, 7),
            block: BlockId::STONE,
            face: BlockFace::East,
            normal: IVec3::X,
            previous_empty: Some(WorldBlockPos::new(-1, 4, 7)),
            distance: 2.0,
        };

        assert_eq!(adjacent_position(hit), Some(WorldBlockPos::new(-1, 4, 7)));
    }

    #[test]
    fn dirt_placement_at_chunk_boundary_dirties_both_chunks() {
        let west = ChunkPos::new(0, 0, 0);
        let east = ChunkPos::new(1, 0, 0);
        let position = WorldBlockPos::new(16, 5, 5);
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(west, Chunk::default());
        storage.insert_chunk(east, Chunk::default());
        storage.get_chunk_mut(west).unwrap().mark_clean();
        storage.get_chunk_mut(east).unwrap().mark_clean();

        assert!(try_place_block(
            &mut storage,
            &BlockRegistry::default(),
            &settings(),
            Vec3::new(10.0, 10.0, 10.0),
            PLAYER_HALF_EXTENTS,
            position,
            BlockId::DIRT,
        ));

        assert_eq!(storage.get_block(position), Some(BlockId::DIRT));
        assert!(storage.get_chunk(west).unwrap().is_dirty());
        assert!(storage.get_chunk(east).unwrap().is_dirty());
    }

    #[test]
    fn placement_uses_selected_hotbar_item_and_consumes_one() {
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(ChunkPos::default(), Chunk::default());
        let position = WorldBlockPos::new(3, 3, 3);
        let mut inventory = PlayerInventory::default();
        let item_registry = ItemRegistry::default();
        inventory.add_item(ItemId::STONE_BLOCK, 2, &item_registry);
        let previous_count = inventory.selected_stack().unwrap().count();

        assert!(try_place_selected_block(
            &mut storage,
            &BlockRegistry::default(),
            &item_registry,
            &settings(),
            Vec3::new(10.0, 10.0, 10.0),
            PLAYER_HALF_EXTENTS,
            position,
            &mut inventory,
        ));

        assert_eq!(storage.get_block(position), Some(BlockId::STONE));
        assert_eq!(
            inventory.selected_stack().unwrap().count(),
            previous_count - 1
        );
    }

    #[test]
    fn placement_cannot_overlap_player_collider() {
        let chunk = ChunkPos::new(0, 0, 0);
        let position = WorldBlockPos::new(0, 1, 0);
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(chunk, Chunk::default());

        assert!(!try_place_block(
            &mut storage,
            &BlockRegistry::default(),
            &settings(),
            Vec3::new(0.5, 1.9, 0.5),
            PLAYER_HALF_EXTENTS,
            position,
            BlockId::DIRT,
        ));
        assert_eq!(storage.get_block(position), Some(BlockId::AIR));
    }

    #[test]
    fn placement_rejects_solid_and_out_of_bounds_cells() {
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(ChunkPos::default(), Chunk::default());
        storage.insert_chunk(ChunkPos::new(0, -5, 0), Chunk::default());
        storage.insert_chunk(ChunkPos::new(0, 12, 0), Chunk::default());
        storage
            .set_block(WorldBlockPos::new(2, 2, 2), BlockId::STONE)
            .unwrap();
        let registry = BlockRegistry::default();
        let generation = settings();
        let player = Vec3::new(10.0, 10.0, 10.0);

        for position in [
            WorldBlockPos::new(2, 2, 2),
            WorldBlockPos::new(0, -65, 0),
            WorldBlockPos::new(0, 192, 0),
        ] {
            assert!(!try_place_block(
                &mut storage,
                &registry,
                &generation,
                player,
                PLAYER_HALF_EXTENTS,
                position,
                BlockId::DIRT,
            ));
        }
    }
}
