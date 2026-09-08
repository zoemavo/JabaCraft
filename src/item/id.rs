use std::fmt;

/// Number of item types built into the base game.
pub const BUILTIN_ITEM_COUNT: usize = 13;

/// Compact item identity used by inventories and dropped-item data.
///
/// This is deliberately a distinct type from BlockId: items may represent
/// tools or resources that have no voxel counterpart.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ItemId(u16);

impl ItemId {
    pub const GRASS_BLOCK: Self = Self(0);
    pub const DIRT_BLOCK: Self = Self(1);
    pub const STONE_BLOCK: Self = Self(2);
    pub const SAND_BLOCK: Self = Self(3);
    pub const WOOD_BLOCK: Self = Self(4);
    pub const LEAVES_BLOCK: Self = Self(5);
    pub const WATER_BLOCK: Self = Self(6);
    pub const COAL_ORE_BLOCK: Self = Self(7);
    pub const IRON_ORE_BLOCK: Self = Self(8);
    pub const STICK: Self = Self(9);
    pub const APPLE: Self = Self(10);
    pub const OAK_PLANKS: Self = Self(11);
    pub const STONE_PICKAXE: Self = Self(12);

    pub const ALL: [Self; BUILTIN_ITEM_COUNT] = [
        Self::GRASS_BLOCK,
        Self::DIRT_BLOCK,
        Self::STONE_BLOCK,
        Self::SAND_BLOCK,
        Self::WOOD_BLOCK,
        Self::LEAVES_BLOCK,
        Self::WATER_BLOCK,
        Self::COAL_ORE_BLOCK,
        Self::IRON_ORE_BLOCK,
        Self::STICK,
        Self::APPLE,
        Self::OAK_PLANKS,
        Self::STONE_PICKAXE,
    ];

    pub const fn from_raw(raw: u16) -> Option<Self> {
        if raw < BUILTIN_ITEM_COUNT as u16 {
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

impl TryFrom<u16> for ItemId {
    type Error = InvalidItemId;

    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        Self::from_raw(raw).ok_or(InvalidItemId(raw))
    }
}

impl From<ItemId> for u16 {
    fn from(id: ItemId) -> Self {
        id.as_u16()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidItemId(pub u16);

impl fmt::Display for InvalidItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown item id {}", self.0)
    }
}

impl std::error::Error for InvalidItemId {}
