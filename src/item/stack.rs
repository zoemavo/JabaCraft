use std::fmt;

use super::ItemId;

/// A bounded quantity of one item type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemStack {
    item: ItemId,
    count: u32,
    max_stack_size: u32,
}

impl ItemStack {
    pub fn new(item: ItemId, count: u32, max_stack_size: u32) -> Result<Self, ItemStackError> {
        if max_stack_size == 0 {
            return Err(ItemStackError::ZeroMaxStackSize);
        }
        if count > max_stack_size {
            return Err(ItemStackError::CountExceedsMaximum {
                count,
                max_stack_size,
            });
        }

        Ok(Self {
            item,
            count,
            max_stack_size,
        })
    }

    pub const fn item(&self) -> ItemId {
        self.item
    }

    pub const fn count(&self) -> u32 {
        self.count
    }

    pub const fn max_stack_size(&self) -> u32 {
        self.max_stack_size
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub const fn remaining_capacity(&self) -> u32 {
        self.max_stack_size - self.count
    }

    /// Adds as many items as fit and returns the amount actually added.
    pub fn add(&mut self, amount: u32) -> u32 {
        let added = amount.min(self.remaining_capacity());
        self.count += added;
        added
    }

    /// Removes as many items as available and returns the amount removed.
    pub fn remove(&mut self, amount: u32) -> u32 {
        let removed = amount.min(self.count);
        self.count -= removed;
        removed
    }

    /// Moves matching items from source into this stack.
    pub fn merge(&mut self, source: &mut Self) -> u32 {
        if self.item != source.item || self.max_stack_size != source.max_stack_size {
            return 0;
        }

        let moved = source.count.min(self.remaining_capacity());
        self.count += moved;
        source.count -= moved;
        moved
    }

    /// Removes up to amount items into a new stack.
    pub fn split(&mut self, amount: u32) -> Option<Self> {
        let split_count = amount.min(self.count);
        if split_count == 0 {
            return None;
        }

        self.count -= split_count;
        Some(Self {
            item: self.item,
            count: split_count,
            max_stack_size: self.max_stack_size,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemStackError {
    ZeroMaxStackSize,
    CountExceedsMaximum { count: u32, max_stack_size: u32 },
}

impl fmt::Display for ItemStackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroMaxStackSize => write!(formatter, "max stack size must be greater than zero"),
            Self::CountExceedsMaximum {
                count,
                max_stack_size,
            } => write!(
                formatter,
                "item count {count} exceeds max stack size {max_stack_size}"
            ),
        }
    }
}

impl std::error::Error for ItemStackError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: ItemId, count: u32, max: u32) -> ItemStack {
        ItemStack::new(item, count, max).unwrap()
    }

    #[test]
    fn construction_enforces_stack_limit() {
        assert_eq!(
            ItemStack::new(ItemId::DIRT_BLOCK, 65, 64),
            Err(ItemStackError::CountExceedsMaximum {
                count: 65,
                max_stack_size: 64,
            })
        );
        assert_eq!(
            ItemStack::new(ItemId::DIRT_BLOCK, 0, 0),
            Err(ItemStackError::ZeroMaxStackSize)
        );
    }

    #[test]
    fn add_stops_at_maximum() {
        let mut stack = stack(ItemId::DIRT_BLOCK, 60, 64);

        assert_eq!(stack.add(10), 4);
        assert_eq!(stack.count(), 64);
        assert_eq!(stack.add(1), 0);
    }

    #[test]
    fn remove_stops_at_zero() {
        let mut stack = stack(ItemId::STONE_BLOCK, 3, 64);

        assert_eq!(stack.remove(10), 3);
        assert!(stack.is_empty());
        assert_eq!(stack.remove(1), 0);
    }

    #[test]
    fn merge_moves_only_available_capacity() {
        let mut target = stack(ItemId::SAND_BLOCK, 60, 64);
        let mut source = stack(ItemId::SAND_BLOCK, 10, 64);

        assert_eq!(target.merge(&mut source), 4);
        assert_eq!(target.count(), 64);
        assert_eq!(source.count(), 6);
    }

    #[test]
    fn merge_rejects_different_items_or_limits() {
        let mut target = stack(ItemId::WOOD_BLOCK, 1, 64);
        let mut other_item = stack(ItemId::STICK, 2, 64);
        let mut other_limit = stack(ItemId::WOOD_BLOCK, 2, 16);

        assert_eq!(target.merge(&mut other_item), 0);
        assert_eq!(target.merge(&mut other_limit), 0);
        assert_eq!(other_item.count(), 2);
        assert_eq!(other_limit.count(), 2);
    }

    #[test]
    fn split_preserves_item_and_limit() {
        let mut original = stack(ItemId::IRON_ORE_BLOCK, 12, 64);
        let split = original.split(5).unwrap();

        assert_eq!(original.count(), 7);
        assert_eq!(split.item(), ItemId::IRON_ORE_BLOCK);
        assert_eq!(split.count(), 5);
        assert_eq!(split.max_stack_size(), 64);
    }

    #[test]
    fn split_can_take_all_and_zero_is_a_noop() {
        let mut original = stack(ItemId::COAL_ORE_BLOCK, 3, 64);

        assert_eq!(original.split(0), None);
        assert_eq!(original.split(10).unwrap().count(), 3);
        assert!(original.is_empty());
        assert_eq!(original.split(1), None);
    }
}
