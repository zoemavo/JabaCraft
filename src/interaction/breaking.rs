//! Progressive survival block breaking and direct inventory drops.

use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    block::{BlockId, BlockRegistry},
    chunk::ChunkStorage,
    coordinates::WorldBlockPos,
    inventory::PlayerInventory,
    item::{ItemId, ItemRegistry},
    player::Player,
    survival::{GameMode, Hunger},
};

use super::{CameraRaycast, InteractionSettings, MiningProgress};

#[derive(Debug, Default, Resource)]
pub(super) struct BlockBreakInput {
    target: Option<WorldBlockPos>,
    elapsed: f32,
    cursor_was_captured: bool,
    suppress_until_release: bool,
}

impl BlockBreakInput {
    fn reset_progress(&mut self) {
        self.target = None;
        self.elapsed = 0.0;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn break_selected_block(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    settings: Res<InteractionSettings>,
    mode: Res<GameMode>,
    registry: Res<BlockRegistry>,
    item_registry: Res<ItemRegistry>,
    mut storage: ResMut<ChunkStorage>,
    mut inventory: ResMut<PlayerInventory>,
    mut hunger: Single<&mut Hunger, With<Player>>,
    mut selected: ResMut<CameraRaycast>,
    mut input: ResMut<BlockBreakInput>,
    mut mining_progress: ResMut<MiningProgress>,
) {
    let cursor_captured = cursor.grab_mode != CursorGrabMode::None;
    let pressed = mouse_buttons.pressed(MouseButton::Left);
    if mouse_buttons.just_pressed(MouseButton::Left) && !input.cursor_was_captured {
        input.suppress_until_release = true;
    }
    input.cursor_was_captured = cursor_captured;
    if !pressed {
        input.suppress_until_release = false;
        input.reset_progress();
        mining_progress.reset();
        return;
    }
    if !cursor_captured || input.suppress_until_release {
        input.reset_progress();
        mining_progress.reset();
        return;
    }

    let Some(hit) = selected.0 else {
        input.reset_progress();
        mining_progress.reset();
        return;
    };
    let Some(block) = storage.get_block(hit.position) else {
        input.reset_progress();
        mining_progress.reset();
        return;
    };
    if block == BlockId::AIR || !registry.is_breakable(block) {
        input.reset_progress();
        mining_progress.reset();
        return;
    }

    if input.target != Some(hit.position) {
        input.target = Some(hit.position);
        input.elapsed = 0.0;
    }
    input.elapsed += time.delta_secs().max(0.0);
    let required = if *mode == GameMode::Creative {
        0.0
    } else {
        (registry.definition(block).hardness * settings.survival_break_time_multiplier)
            .max(settings.break_repeat_interval)
    };
    mining_progress.update(input.elapsed, required);
    if input.elapsed < required {
        return;
    }

    if let Some(broken) = take_breakable_block(&mut storage, &registry, hit.position) {
        if *mode == GameMode::Survival {
            if let Some(drop) = survival_drop(broken, hit.position, &item_registry) {
                let remainder = inventory.add_item(drop, 1, &item_registry);
                if remainder > 0 {
                    warn!("Inventory full; dropped {:?} could not be collected", drop);
                }
            }
            hunger.add_exhaustion(0.005);
        }
        debug!(
            "Broke {:?} at ({}, {}, {})",
            hit.block, hit.position.x, hit.position.y, hit.position.z
        );
        selected.0 = None;
        input.reset_progress();
        mining_progress.reset();
    }
}

#[cfg(test)]
fn try_break_block(
    storage: &mut ChunkStorage,
    registry: &BlockRegistry,
    position: WorldBlockPos,
) -> bool {
    take_breakable_block(storage, registry, position).is_some()
}

fn take_breakable_block(
    storage: &mut ChunkStorage,
    registry: &BlockRegistry,
    position: WorldBlockPos,
) -> Option<BlockId> {
    let block = storage.get_block(position)?;
    if block == BlockId::AIR || !registry.is_breakable(block) {
        return None;
    }

    storage
        .set_block(position, BlockId::AIR)
        .ok()
        .map(|_| block)
}

fn survival_drop(
    block: BlockId,
    position: WorldBlockPos,
    registry: &ItemRegistry,
) -> Option<ItemId> {
    match block {
        BlockId::GRASS => Some(ItemId::DIRT_BLOCK),
        BlockId::LEAVES => leaves_drop_apple(position).then_some(ItemId::APPLE),
        _ => registry.item_for_block(block),
    }
}

fn leaves_drop_apple(position: WorldBlockPos) -> bool {
    let mut hash = (position.x as u32).wrapping_mul(0x9e37_79b9)
        ^ (position.y as u32).wrapping_mul(0x85eb_ca6b)
        ^ (position.z as u32).wrapping_mul(0xc2b2_ae35);
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7feb_352d);
    hash ^= hash >> 15;
    hash.is_multiple_of(8)
}

#[cfg(test)]
mod tests {
    use crate::{chunk::Chunk, coordinates::ChunkPos};

    use super::*;

    #[test]
    fn breakable_boundary_block_becomes_air_and_dirties_both_chunks() {
        let center = ChunkPos::new(0, 0, 0);
        let east = ChunkPos::new(1, 0, 0);
        let position = WorldBlockPos::new(15, 5, 5);
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(center, Chunk::default());
        storage.insert_chunk(east, Chunk::default());
        storage.set_block(position, BlockId::STONE).unwrap();
        storage.get_chunk_mut(center).unwrap().mark_clean();
        storage.get_chunk_mut(east).unwrap().mark_clean();

        assert!(try_break_block(
            &mut storage,
            &BlockRegistry::default(),
            position
        ));

        assert_eq!(storage.get_block(position), Some(BlockId::AIR));
        assert!(storage.get_chunk(center).unwrap().is_dirty());
        assert!(storage.get_chunk(east).unwrap().is_dirty());
    }

    #[test]
    fn air_and_unbreakable_blocks_are_not_changed() {
        let chunk = ChunkPos::new(0, 0, 0);
        let air = WorldBlockPos::new(1, 1, 1);
        let water = WorldBlockPos::new(2, 1, 1);
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(chunk, Chunk::default());
        storage.set_block(water, BlockId::WATER).unwrap();
        storage.get_chunk_mut(chunk).unwrap().mark_clean();
        let registry = BlockRegistry::default();

        assert!(!try_break_block(&mut storage, &registry, air));
        assert!(!try_break_block(&mut storage, &registry, water));
        assert_eq!(storage.get_block(water), Some(BlockId::WATER));
        assert!(!storage.get_chunk(chunk).unwrap().is_dirty());
    }

    #[test]
    fn grass_drops_dirt_and_regular_blocks_drop_their_item() {
        let registry = ItemRegistry::default();
        let position = WorldBlockPos::new(3, 4, 5);

        assert_eq!(
            survival_drop(BlockId::GRASS, position, &registry),
            Some(ItemId::DIRT_BLOCK)
        );
        assert_eq!(
            survival_drop(BlockId::WOOD, position, &registry),
            Some(ItemId::WOOD_BLOCK)
        );
    }

    #[test]
    fn leaf_apple_drop_is_deterministic_and_uncommon() {
        let drops = (0..64)
            .filter(|&x| leaves_drop_apple(WorldBlockPos::new(x, 10, 3)))
            .count();

        assert!(drops > 0);
        assert!(drops < 20);
    }
}
