//! Player input and rules for instant block breaking.

use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    block::{BlockId, BlockRegistry},
    chunk::ChunkStorage,
    coordinates::WorldBlockPos,
};

use super::{CameraRaycast, InteractionSettings, input::DebouncedButtonInput};

/// Debounces held left mouse input and remembers whether a click began while captured.
#[derive(Debug, Default, Resource)]
pub(super) struct BlockBreakInput(DebouncedButtonInput);

pub(super) fn break_selected_block(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    settings: Res<InteractionSettings>,
    registry: Res<BlockRegistry>,
    mut storage: ResMut<ChunkStorage>,
    mut selected: ResMut<CameraRaycast>,
    mut input: ResMut<BlockBreakInput>,
) {
    let cursor_captured = cursor.grab_mode != CursorGrabMode::None;
    let should_break = input.0.update(
        mouse_buttons.pressed(MouseButton::Left),
        mouse_buttons.just_pressed(MouseButton::Left),
        cursor_captured,
        time.delta_secs(),
        settings.break_repeat_interval,
    );
    if !should_break {
        return;
    }

    let Some(hit) = selected.0 else {
        return;
    };
    if try_break_block(&mut storage, &registry, hit.position) {
        debug!(
            "Broke {:?} at ({}, {}, {})",
            hit.block, hit.position.x, hit.position.y, hit.position.z
        );
        selected.0 = None;
    }
}

fn try_break_block(
    storage: &mut ChunkStorage,
    registry: &BlockRegistry,
    position: WorldBlockPos,
) -> bool {
    let Some(block) = storage.get_block(position) else {
        return false;
    };
    if block == BlockId::AIR || !registry.is_breakable(block) {
        return false;
    }

    storage.set_block(position, BlockId::AIR).is_ok()
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
}
