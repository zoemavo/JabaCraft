//! Coordinate types and lossless conversions for the voxel world.

use std::fmt;

pub const CHUNK_WIDTH: usize = 16;
pub const CHUNK_HEIGHT: usize = 16;
pub const CHUNK_DEPTH: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_WIDTH * CHUNK_HEIGHT * CHUNK_DEPTH;
pub const CHUNK_EDGE_LENGTH: usize = CHUNK_WIDTH;

const CHUNK_WIDTH_I32: i32 = CHUNK_WIDTH as i32;
const CHUNK_HEIGHT_I32: i32 = CHUNK_HEIGHT as i32;
const CHUNK_DEPTH_I32: i32 = CHUNK_DEPTH as i32;

/// Integer position of a block in the unbounded logical world grid.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorldBlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl WorldBlockPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Returns the chunk containing this world block.
    pub fn chunk_pos(self) -> ChunkPos {
        ChunkPos::new(
            self.x.div_euclid(CHUNK_WIDTH_I32),
            self.y.div_euclid(CHUNK_HEIGHT_I32),
            self.z.div_euclid(CHUNK_DEPTH_I32),
        )
    }

    /// Returns this block's non-negative coordinate inside its chunk.
    pub fn local_pos(self) -> LocalBlockPos {
        LocalBlockPos {
            x: self.x.rem_euclid(CHUNK_WIDTH_I32) as u8,
            y: self.y.rem_euclid(CHUNK_HEIGHT_I32) as u8,
            z: self.z.rem_euclid(CHUNK_DEPTH_I32) as u8,
        }
    }

    pub fn split(self) -> (ChunkPos, LocalBlockPos) {
        (self.chunk_pos(), self.local_pos())
    }
}

/// Integer position of a chunk in the world chunk grid.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Recombines a chunk coordinate and a valid local coordinate.
    pub fn world_block(self, local: LocalBlockPos) -> WorldBlockPos {
        WorldBlockPos::new(
            self.x * CHUNK_WIDTH_I32 + local.x as i32,
            self.y * CHUNK_HEIGHT_I32 + local.y as i32,
            self.z * CHUNK_DEPTH_I32 + local.z as i32,
        )
    }
}

/// Valid block coordinate inside one chunk.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalBlockPos {
    x: u8,
    y: u8,
    z: u8,
}

impl LocalBlockPos {
    pub fn new(x: usize, y: usize, z: usize) -> Result<Self, LocalBlockPosError> {
        Self::checked(x, y, z).ok_or(LocalBlockPosError { x, y, z })
    }

    pub const fn checked(x: usize, y: usize, z: usize) -> Option<Self> {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            Some(Self {
                x: x as u8,
                y: y as u8,
                z: z as u8,
            })
        } else {
            None
        }
    }

    pub const fn x(self) -> usize {
        self.x as usize
    }

    pub const fn y(self) -> usize {
        self.y as usize
    }

    pub const fn z(self) -> usize {
        self.z as usize
    }

    /// Dense chunk index with X fastest, then Z, then Y.
    pub const fn index(self) -> usize {
        self.x() + self.z() * CHUNK_WIDTH + self.y() * CHUNK_WIDTH * CHUNK_DEPTH
    }
}

impl From<WorldBlockPos> for ChunkPos {
    fn from(world: WorldBlockPos) -> Self {
        world.chunk_pos()
    }
}

impl From<WorldBlockPos> for LocalBlockPos {
    fn from(world: WorldBlockPos) -> Self {
        world.local_pos()
    }
}

impl From<(ChunkPos, LocalBlockPos)> for WorldBlockPos {
    fn from((chunk, local): (ChunkPos, LocalBlockPos)) -> Self {
        chunk.world_block(local)
    }
}

/// Coordinates supplied for a local block were outside the chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalBlockPosError {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl fmt::Display for LocalBlockPosError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "local block coordinate ({}, {}, {}) is outside a {}x{}x{} chunk",
            self.x, self.y, self.z, CHUNK_WIDTH, CHUNK_HEIGHT, CHUNK_DEPTH
        )
    }
}

impl std::error::Error for LocalBlockPosError {}

#[cfg(test)]
mod tests {
    use super::*;

    const AXIS_CASES: &[(i32, i32, usize)] = &[
        (-33, -3, 15),
        (-32, -2, 0),
        (-31, -2, 1),
        (-17, -2, 15),
        (-16, -1, 0),
        (-15, -1, 1),
        (-2, -1, 14),
        (-1, -1, 15),
        (0, 0, 0),
        (1, 0, 1),
        (14, 0, 14),
        (15, 0, 15),
        (16, 1, 0),
        (17, 1, 1),
        (31, 1, 15),
        (32, 2, 0),
        (33, 2, 1),
    ];

    #[test]
    fn every_axis_handles_positive_and_negative_chunk_boundaries() {
        for &(world, expected_chunk, expected_local) in AXIS_CASES {
            let x = WorldBlockPos::new(world, 0, 0);
            assert_eq!(x.chunk_pos().x, expected_chunk, "world x={world}");
            assert_eq!(x.local_pos().x(), expected_local, "world x={world}");

            let y = WorldBlockPos::new(0, world, 0);
            assert_eq!(y.chunk_pos().y, expected_chunk, "world y={world}");
            assert_eq!(y.local_pos().y(), expected_local, "world y={world}");

            let z = WorldBlockPos::new(0, 0, world);
            assert_eq!(z.chunk_pos().z, expected_chunk, "world z={world}");
            assert_eq!(z.local_pos().z(), expected_local, "world z={world}");
        }
    }

    #[test]
    fn minus_one_is_in_chunk_minus_one() {
        let world = WorldBlockPos::new(-1, -1, -1);

        assert_eq!(world.chunk_pos(), ChunkPos::new(-1, -1, -1));
        assert_eq!(world.local_pos(), LocalBlockPos::new(15, 15, 15).unwrap());
    }

    #[test]
    fn mixed_sign_coordinates_split_and_recombine() {
        let positions = [
            WorldBlockPos::new(-257, 42, 511),
            WorldBlockPos::new(257, -42, -511),
            WorldBlockPos::new(-16, 16, -1),
            WorldBlockPos::new(15, -17, 32),
            WorldBlockPos::new(0, 0, 0),
        ];

        for world in positions {
            let (chunk, local) = world.split();
            assert_eq!(chunk.world_block(local), world);
            assert_eq!(WorldBlockPos::from((chunk, local)), world);
        }
    }

    #[test]
    fn all_local_positions_round_trip_across_surrounding_chunks() {
        for chunk_y in -2..=2 {
            for chunk_z in -2..=2 {
                for chunk_x in -2..=2 {
                    let chunk = ChunkPos::new(chunk_x, chunk_y, chunk_z);
                    for y in 0..CHUNK_HEIGHT {
                        for z in 0..CHUNK_DEPTH {
                            for x in 0..CHUNK_WIDTH {
                                let local = LocalBlockPos::new(x, y, z).unwrap();
                                let world = chunk.world_block(local);
                                assert_eq!(world.split(), (chunk, local));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn local_positions_enforce_bounds_on_each_axis() {
        assert!(LocalBlockPos::new(15, 15, 15).is_ok());
        assert!(LocalBlockPos::new(16, 0, 0).is_err());
        assert!(LocalBlockPos::new(0, 16, 0).is_err());
        assert!(LocalBlockPos::new(0, 0, 16).is_err());
    }
}
