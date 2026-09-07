use std::fmt;

/// Number of block types built into the base game.
pub const BUILTIN_BLOCK_COUNT: usize = 10;

/// Compact block identity stored for every voxel.
///
/// The private field guarantees that every `BlockId` indexes a definition in
/// the built-in registry. A `u16` leaves room for future registered block types
/// while using only two bytes per voxel.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(u16);

impl BlockId {
    pub const AIR: Self = Self(0);
    pub const GRASS: Self = Self(1);
    pub const DIRT: Self = Self(2);
    pub const STONE: Self = Self(3);
    pub const SAND: Self = Self(4);
    pub const WOOD: Self = Self(5);
    pub const LEAVES: Self = Self(6);
    pub const WATER: Self = Self(7);
    pub const COAL_ORE: Self = Self(8);
    pub const IRON_ORE: Self = Self(9);

    pub const ALL: [Self; BUILTIN_BLOCK_COUNT] = [
        Self::AIR,
        Self::GRASS,
        Self::DIRT,
        Self::STONE,
        Self::SAND,
        Self::WOOD,
        Self::LEAVES,
        Self::WATER,
        Self::COAL_ORE,
        Self::IRON_ORE,
    ];

    pub const fn from_raw(raw: u16) -> Option<Self> {
        if raw < BUILTIN_BLOCK_COUNT as u16 {
            Some(Self(raw))
        } else {
            None
        }
    }

    pub const fn as_u16(self) -> u16 {
        self.0
    }

    pub(crate) const fn as_index(self) -> usize {
        self.0 as usize
    }
}

impl TryFrom<u16> for BlockId {
    type Error = InvalidBlockId;

    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        Self::from_raw(raw).ok_or(InvalidBlockId(raw))
    }
}

impl From<BlockId> for u16 {
    fn from(id: BlockId) -> Self {
        id.as_u16()
    }
}

/// Error returned when persisted or networked data contains an unknown ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidBlockId(pub u16);

impl fmt::Display for InvalidBlockId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown block id {}", self.0)
    }
}

impl std::error::Error for InvalidBlockId {}
