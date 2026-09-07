//! Chunk coordinates and in-memory chunk storage.

use std::collections::HashMap;
use std::fmt;

use bevy::prelude::*;

use crate::block::BlockId;
use crate::coordinates::{ChunkPos, LocalBlockPos, WorldBlockPos};

pub use crate::coordinates::{
    CHUNK_DEPTH, CHUNK_EDGE_LENGTH, CHUNK_HEIGHT, CHUNK_VOLUME, CHUNK_WIDTH,
};

/// Dense block data belonging to one loaded chunk.
///
/// Rendering entities and meshes deliberately live outside this type.
#[derive(Clone, Debug)]
pub struct Chunk {
    blocks: Box<[BlockId]>,
    dirty: bool,
}

impl Chunk {
    /// Creates a chunk filled with one block type and awaiting its first mesh.
    pub fn new(fill: BlockId) -> Self {
        Self {
            blocks: vec![fill; CHUNK_VOLUME].into_boxed_slice(),
            dirty: true,
        }
    }

    /// Converts local coordinates to a dense-array index.
    ///
    /// X changes fastest, followed by Z, while Y selects horizontal layers.
    pub const fn xyz_to_index(x: usize, y: usize, z: usize) -> Option<usize> {
        match LocalBlockPos::checked(x, y, z) {
            Some(position) => Some(position.index()),
            None => None,
        }
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> Option<BlockId> {
        Self::xyz_to_index(x, y, z).map(|index| self.blocks[index])
    }

    pub fn get_local(&self, position: LocalBlockPos) -> BlockId {
        self.blocks[position.index()]
    }

    /// Replaces a block and returns its previous ID.
    ///
    /// The chunk becomes dirty only when the stored value actually changes.
    pub fn set_block(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        block: BlockId,
    ) -> Result<BlockId, ChunkBoundsError> {
        let index = Self::xyz_to_index(x, y, z).ok_or(ChunkBoundsError { x, y, z })?;
        let previous = self.blocks[index];

        if previous != block {
            self.blocks[index] = block;
            self.dirty = true;
        }

        Ok(previous)
    }

    pub fn set_local(&mut self, position: LocalBlockPos, block: BlockId) -> BlockId {
        let index = position.index();
        let previous = self.blocks[index];

        if previous != block {
            self.blocks[index] = block;
            self.dirty = true;
        }

        previous
    }

    pub fn blocks(&self) -> &[BlockId] {
        &self.blocks
    }

    /// Returns true when this chunk contains no voxel other than air.
    pub fn is_all_air(&self) -> bool {
        self.blocks.iter().all(|&block| block == BlockId::AIR)
    }

    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new(BlockId::AIR)
    }
}

/// Coordinates supplied to a chunk operation were outside its local bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkBoundsError {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl fmt::Display for ChunkBoundsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "block coordinate ({}, {}, {}) is outside a {}x{}x{} chunk",
            self.x, self.y, self.z, CHUNK_WIDTH, CHUNK_HEIGHT, CHUNK_DEPTH
        )
    }
}

impl std::error::Error for ChunkBoundsError {}

/// The authoritative runtime store for loaded chunks.
#[derive(Debug, Default, Resource)]
pub struct ChunkStorage {
    chunks: HashMap<ChunkPos, Chunk>,
}

impl ChunkStorage {
    /// Inserts a loaded chunk, returning the chunk previously stored at this position.
    pub fn insert_chunk(&mut self, position: ChunkPos, chunk: Chunk) -> Option<Chunk> {
        let previous = self.chunks.insert(position, chunk);
        self.mark_face_neighbors_dirty(position);
        previous
    }

    /// Removes and returns a loaded chunk.
    pub fn remove_chunk(&mut self, position: ChunkPos) -> Option<Chunk> {
        let removed = self.chunks.remove(&position);
        if removed.is_some() {
            self.mark_face_neighbors_dirty(position);
        }
        removed
    }

    pub fn get_chunk(&self, position: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&position)
    }

    pub fn get_chunk_mut(&mut self, position: ChunkPos) -> Option<&mut Chunk> {
        self.chunks.get_mut(&position)
    }

    pub fn contains_chunk(&self, position: ChunkPos) -> bool {
        self.chunks.contains_key(&position)
    }

    /// Iterates over loaded chunk positions and their voxel data.
    pub fn iter(&self) -> impl Iterator<Item = (ChunkPos, &Chunk)> {
        self.chunks
            .iter()
            .map(|(&position, chunk)| (position, chunk))
    }

    /// Reads a block through world coordinates, or returns `None` if its chunk is unloaded.
    pub fn get_block(&self, world_position: WorldBlockPos) -> Option<BlockId> {
        let (chunk_position, local_position) = world_position.split();
        self.get_chunk(chunk_position)
            .map(|chunk| chunk.get_local(local_position))
    }

    /// Replaces a block through world coordinates and returns its previous ID.
    ///
    /// A changed block dirties its own chunk. If it lies on one or more chunk
    /// faces, every loaded chunk sharing one of those faces is dirtied as well.
    pub fn set_block(
        &mut self,
        world_position: WorldBlockPos,
        block: BlockId,
    ) -> Result<BlockId, ChunkNotLoadedError> {
        let (chunk_position, local_position) = world_position.split();
        let chunk = self
            .get_chunk_mut(chunk_position)
            .ok_or(ChunkNotLoadedError { chunk_position })?;
        let previous = chunk.set_local(local_position, block);

        if previous != block {
            self.mark_border_neighbors_dirty(chunk_position, local_position);
        }

        Ok(previous)
    }

    fn mark_border_neighbors_dirty(
        &mut self,
        chunk_position: ChunkPos,
        local_position: LocalBlockPos,
    ) {
        if local_position.x() == 0 {
            self.mark_chunk_dirty(ChunkPos::new(
                chunk_position.x - 1,
                chunk_position.y,
                chunk_position.z,
            ));
        }
        if local_position.x() == CHUNK_WIDTH - 1 {
            self.mark_chunk_dirty(ChunkPos::new(
                chunk_position.x + 1,
                chunk_position.y,
                chunk_position.z,
            ));
        }
        if local_position.y() == 0 {
            self.mark_chunk_dirty(ChunkPos::new(
                chunk_position.x,
                chunk_position.y - 1,
                chunk_position.z,
            ));
        }
        if local_position.y() == CHUNK_HEIGHT - 1 {
            self.mark_chunk_dirty(ChunkPos::new(
                chunk_position.x,
                chunk_position.y + 1,
                chunk_position.z,
            ));
        }
        if local_position.z() == 0 {
            self.mark_chunk_dirty(ChunkPos::new(
                chunk_position.x,
                chunk_position.y,
                chunk_position.z - 1,
            ));
        }
        if local_position.z() == CHUNK_DEPTH - 1 {
            self.mark_chunk_dirty(ChunkPos::new(
                chunk_position.x,
                chunk_position.y,
                chunk_position.z + 1,
            ));
        }
    }

    fn mark_face_neighbors_dirty(&mut self, position: ChunkPos) {
        for neighbor in face_neighbors(position) {
            self.mark_chunk_dirty(neighbor);
        }
    }

    fn mark_chunk_dirty(&mut self, position: ChunkPos) {
        if let Some(chunk) = self.get_chunk_mut(position) {
            chunk.mark_dirty();
        }
    }
}

fn face_neighbors(position: ChunkPos) -> [ChunkPos; 6] {
    [
        ChunkPos::new(position.x - 1, position.y, position.z),
        ChunkPos::new(position.x + 1, position.y, position.z),
        ChunkPos::new(position.x, position.y - 1, position.z),
        ChunkPos::new(position.x, position.y + 1, position.z),
        ChunkPos::new(position.x, position.y, position.z - 1),
        ChunkPos::new(position.x, position.y, position.z + 1),
    ]
}

/// A world block operation addressed a chunk that is not currently loaded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkNotLoadedError {
    pub chunk_position: ChunkPos,
}

impl fmt::Display for ChunkNotLoadedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let position = self.chunk_position;
        write!(
            formatter,
            "chunk ({}, {}, {}) is not loaded",
            position.x, position.y, position.z
        )
    }
}

impl std::error::Error for ChunkNotLoadedError {}

/// Provides chunk storage; streaming systems will be added later.
pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkStorage>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_produce_the_expected_storage_size() {
        let chunk = Chunk::default();

        assert_eq!(CHUNK_VOLUME, 16 * 16 * 16);
        assert_eq!(chunk.blocks().len(), CHUNK_VOLUME);
        assert!(chunk.blocks().iter().all(|block| *block == BlockId::AIR));
        assert!(chunk.is_all_air());
    }

    #[test]
    fn xyz_indices_cover_the_dense_storage_exactly_once() {
        let mut visited = vec![false; CHUNK_VOLUME];

        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_DEPTH {
                for x in 0..CHUNK_WIDTH {
                    let index = Chunk::xyz_to_index(x, y, z).expect("coordinate must be valid");
                    assert!(!visited[index], "duplicate index {index}");
                    visited[index] = true;
                }
            }
        }

        assert!(visited.into_iter().all(|was_visited| was_visited));
        assert_eq!(Chunk::xyz_to_index(0, 0, 0), Some(0));
        assert_eq!(
            Chunk::xyz_to_index(CHUNK_WIDTH - 1, CHUNK_HEIGHT - 1, CHUNK_DEPTH - 1),
            Some(CHUNK_VOLUME - 1)
        );
    }

    #[test]
    fn get_and_set_block_use_local_coordinates() {
        let mut chunk = Chunk::default();
        chunk.mark_clean();

        assert_eq!(chunk.get_block(3, 7, 11), Some(BlockId::AIR));
        assert_eq!(chunk.set_block(3, 7, 11, BlockId::STONE), Ok(BlockId::AIR));
        assert_eq!(chunk.get_block(3, 7, 11), Some(BlockId::STONE));
        assert!(chunk.is_dirty());
        assert!(!chunk.is_all_air());
    }

    #[test]
    fn unchanged_blocks_do_not_dirty_a_clean_chunk() {
        let mut chunk = Chunk::new(BlockId::DIRT);
        chunk.mark_clean();

        assert_eq!(chunk.set_block(1, 2, 3, BlockId::DIRT), Ok(BlockId::DIRT));
        assert!(!chunk.is_dirty());
    }

    #[test]
    fn out_of_bounds_coordinates_are_rejected_without_mutation() {
        let mut chunk = Chunk::default();
        chunk.mark_clean();

        assert_eq!(chunk.get_block(CHUNK_WIDTH, 0, 0), None);
        assert_eq!(chunk.get_block(0, CHUNK_HEIGHT, 0), None);
        assert_eq!(chunk.get_block(0, 0, CHUNK_DEPTH), None);
        assert_eq!(
            chunk.set_block(CHUNK_WIDTH, 0, 0, BlockId::STONE),
            Err(ChunkBoundsError {
                x: CHUNK_WIDTH,
                y: 0,
                z: 0,
            })
        );
        assert!(!chunk.is_dirty());
        assert!(chunk.blocks().iter().all(|block| *block == BlockId::AIR));
    }

    #[test]
    fn storage_inserts_reads_mutates_and_removes_chunks() {
        let position = ChunkPos::new(3, -2, 7);
        let mut storage = ChunkStorage::default();

        assert!(!storage.contains_chunk(position));
        assert!(
            storage
                .insert_chunk(position, Chunk::new(BlockId::DIRT))
                .is_none()
        );
        assert!(storage.contains_chunk(position));
        assert_eq!(
            storage
                .get_chunk(position)
                .unwrap()
                .get_local(LocalBlockPos::default()),
            BlockId::DIRT
        );

        storage.get_chunk_mut(position).unwrap().mark_clean();
        assert!(!storage.get_chunk(position).unwrap().is_dirty());

        let removed = storage
            .remove_chunk(position)
            .expect("chunk must be removed");
        assert!(!storage.contains_chunk(position));
        assert_eq!(removed.get_local(LocalBlockPos::default()), BlockId::DIRT);
        assert!(storage.remove_chunk(position).is_none());
    }

    #[test]
    fn world_block_access_handles_negative_coordinates() {
        let negative_chunk = ChunkPos::new(-1, -1, -1);
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(negative_chunk, Chunk::default());
        storage.get_chunk_mut(negative_chunk).unwrap().mark_clean();

        let world_position = WorldBlockPos::new(-1, -1, -1);
        assert_eq!(storage.get_block(world_position), Some(BlockId::AIR));
        assert_eq!(
            storage.set_block(world_position, BlockId::STONE),
            Ok(BlockId::AIR)
        );
        assert_eq!(storage.get_block(world_position), Some(BlockId::STONE));
        assert_eq!(
            storage
                .get_chunk(negative_chunk)
                .unwrap()
                .get_local(LocalBlockPos::new(15, 15, 15).unwrap()),
            BlockId::STONE
        );
        assert!(storage.get_chunk(negative_chunk).unwrap().is_dirty());
    }

    #[test]
    fn missing_chunks_are_reported_without_being_created() {
        let mut storage = ChunkStorage::default();
        let world_position = WorldBlockPos::new(-1, 20, 40);
        let expected_chunk = ChunkPos::new(-1, 1, 2);

        assert_eq!(storage.get_block(world_position), None);
        assert_eq!(
            storage.set_block(world_position, BlockId::STONE),
            Err(ChunkNotLoadedError {
                chunk_position: expected_chunk,
            })
        );
        assert!(!storage.contains_chunk(expected_chunk));
    }

    #[test]
    fn changing_each_chunk_face_dirties_its_loaded_neighbor() {
        let center = ChunkPos::new(0, 0, 0);
        let neighbors = [
            ChunkPos::new(-1, 0, 0),
            ChunkPos::new(1, 0, 0),
            ChunkPos::new(0, -1, 0),
            ChunkPos::new(0, 1, 0),
            ChunkPos::new(0, 0, -1),
            ChunkPos::new(0, 0, 1),
        ];
        let cases = [
            (WorldBlockPos::new(0, 8, 8), neighbors[0]),
            (WorldBlockPos::new(15, 8, 8), neighbors[1]),
            (WorldBlockPos::new(8, 0, 8), neighbors[2]),
            (WorldBlockPos::new(8, 15, 8), neighbors[3]),
            (WorldBlockPos::new(8, 8, 0), neighbors[4]),
            (WorldBlockPos::new(8, 8, 15), neighbors[5]),
        ];

        for (world_position, expected_dirty_neighbor) in cases {
            let mut storage = ChunkStorage::default();
            storage.insert_chunk(center, Chunk::default());
            for neighbor in neighbors {
                storage.insert_chunk(neighbor, Chunk::default());
            }
            for chunk in storage.chunks.values_mut() {
                chunk.mark_clean();
            }

            storage.set_block(world_position, BlockId::STONE).unwrap();

            assert!(storage.get_chunk(center).unwrap().is_dirty());
            for neighbor in neighbors {
                assert_eq!(
                    storage.get_chunk(neighbor).unwrap().is_dirty(),
                    neighbor == expected_dirty_neighbor,
                    "unexpected dirty state for {neighbor:?} after changing {world_position:?}"
                );
            }
        }
    }

    #[test]
    fn chunk_corner_dirties_three_face_neighbors_but_not_diagonals() {
        let center = ChunkPos::new(0, 0, 0);
        let face_neighbors = [
            ChunkPos::new(-1, 0, 0),
            ChunkPos::new(0, -1, 0),
            ChunkPos::new(0, 0, -1),
        ];
        let diagonal = ChunkPos::new(-1, -1, -1);
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(center, Chunk::default());
        for neighbor in face_neighbors {
            storage.insert_chunk(neighbor, Chunk::default());
        }
        storage.insert_chunk(diagonal, Chunk::default());
        for chunk in storage.chunks.values_mut() {
            chunk.mark_clean();
        }

        storage
            .set_block(WorldBlockPos::new(0, 0, 0), BlockId::STONE)
            .unwrap();

        assert!(storage.get_chunk(center).unwrap().is_dirty());
        for neighbor in face_neighbors {
            assert!(storage.get_chunk(neighbor).unwrap().is_dirty());
        }
        assert!(!storage.get_chunk(diagonal).unwrap().is_dirty());
    }

    #[test]
    fn interior_or_unchanged_blocks_do_not_dirty_neighbors() {
        let center = ChunkPos::new(0, 0, 0);
        let neighbor = ChunkPos::new(-1, 0, 0);
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(center, Chunk::default());
        storage.insert_chunk(neighbor, Chunk::default());
        for chunk in storage.chunks.values_mut() {
            chunk.mark_clean();
        }

        storage
            .set_block(WorldBlockPos::new(8, 8, 8), BlockId::STONE)
            .unwrap();
        assert!(!storage.get_chunk(neighbor).unwrap().is_dirty());

        storage.get_chunk_mut(center).unwrap().mark_clean();
        storage
            .set_block(WorldBlockPos::new(0, 8, 8), BlockId::AIR)
            .unwrap();
        assert!(!storage.get_chunk(center).unwrap().is_dirty());
        assert!(!storage.get_chunk(neighbor).unwrap().is_dirty());
    }

    #[test]
    fn loading_a_chunk_dirties_only_loaded_face_neighbors() {
        let center = ChunkPos::new(0, 0, 0);
        let neighbors = face_neighbors(center);
        let diagonal = ChunkPos::new(1, 1, 0);
        let mut storage = ChunkStorage::default();

        for position in neighbors {
            storage.insert_chunk(position, Chunk::default());
        }
        storage.insert_chunk(diagonal, Chunk::default());
        for chunk in storage.chunks.values_mut() {
            chunk.mark_clean();
        }

        storage.insert_chunk(center, Chunk::default());

        for position in neighbors {
            assert!(storage.get_chunk(position).unwrap().is_dirty());
        }
        assert!(!storage.get_chunk(diagonal).unwrap().is_dirty());
    }

    #[test]
    fn unloading_a_chunk_dirties_only_loaded_face_neighbors() {
        let center = ChunkPos::new(-3, 2, 7);
        let neighbors = face_neighbors(center);
        let diagonal = ChunkPos::new(center.x + 1, center.y + 1, center.z);
        let mut storage = ChunkStorage::default();

        storage.insert_chunk(center, Chunk::default());
        for position in neighbors {
            storage.insert_chunk(position, Chunk::default());
        }
        storage.insert_chunk(diagonal, Chunk::default());
        for chunk in storage.chunks.values_mut() {
            chunk.mark_clean();
        }

        storage
            .remove_chunk(center)
            .expect("center chunk must exist");

        for position in neighbors {
            assert!(storage.get_chunk(position).unwrap().is_dirty());
        }
        assert!(!storage.get_chunk(diagonal).unwrap().is_dirty());
    }
}
