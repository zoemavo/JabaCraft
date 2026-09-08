use bevy::prelude::Resource;

use crate::item::{ItemId, ItemRegistry, ItemStack};

pub const HOTBAR_SLOT_COUNT: usize = 9;
pub const INVENTORY_SLOT_COUNT: usize = 27;
pub const TOTAL_SLOT_COUNT: usize = HOTBAR_SLOT_COUNT + INVENTORY_SLOT_COUNT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryClick {
    Left,
    Right,
}

/// All player-owned inventory slots plus the stack currently carried by the cursor.
#[derive(Clone, Debug, Resource)]
pub struct PlayerInventory {
    slots: [Option<ItemStack>; TOTAL_SLOT_COUNT],
    selected_hotbar: usize,
    cursor_held: Option<ItemStack>,
}

impl PlayerInventory {
    pub fn slots(&self) -> &[Option<ItemStack>; TOTAL_SLOT_COUNT] {
        &self.slots
    }

    pub fn hotbar_slots(&self) -> &[Option<ItemStack>] {
        &self.slots[..HOTBAR_SLOT_COUNT]
    }

    pub fn inventory_slots(&self) -> &[Option<ItemStack>] {
        &self.slots[HOTBAR_SLOT_COUNT..]
    }

    pub fn slot(&self, index: usize) -> Option<ItemStack> {
        self.slots.get(index).copied().flatten()
    }

    pub const fn selected_hotbar_index(&self) -> usize {
        self.selected_hotbar
    }

    pub fn selected_stack(&self) -> Option<ItemStack> {
        self.slot(self.selected_hotbar)
            .filter(|stack| !stack.is_empty())
    }

    pub const fn cursor_held_stack(&self) -> Option<ItemStack> {
        self.cursor_held
    }

    pub fn select_hotbar(&mut self, index: usize) -> bool {
        if index >= HOTBAR_SLOT_COUNT || index == self.selected_hotbar {
            return false;
        }
        self.selected_hotbar = index;
        true
    }

    pub fn cycle_hotbar(&mut self, offset: isize) {
        self.selected_hotbar = (self.selected_hotbar as isize + offset)
            .rem_euclid(HOTBAR_SLOT_COUNT as isize) as usize;
    }

    pub fn consume_selected_one(&mut self) -> bool {
        let Some(stack) = self.slots[self.selected_hotbar].as_mut() else {
            return false;
        };
        if stack.remove(1) == 0 {
            return false;
        }
        if stack.is_empty() {
            self.slots[self.selected_hotbar] = None;
        }
        true
    }

    /// Adds items to matching stacks first and then empty slots.
    ///
    /// Returns the amount that did not fit.
    pub fn add_item(&mut self, item: ItemId, mut count: u32, registry: &ItemRegistry) -> u32 {
        let stack_limit = registry.max_stack_size(item);

        for stack in self.slots.iter_mut().flatten() {
            if stack.item() == item && stack.max_stack_size() == stack_limit {
                count -= stack.add(count);
                if count == 0 {
                    return 0;
                }
            }
        }

        for slot in &mut self.slots {
            if slot.is_some() {
                continue;
            }
            let amount = count.min(stack_limit);
            *slot = Some(
                ItemStack::new(item, amount, stack_limit)
                    .expect("inventory insertion respects the registry stack limit"),
            );
            count -= amount;
            if count == 0 {
                return 0;
            }
        }

        count
    }

    pub fn item_count(&self, item: ItemId) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|stack| stack.item() == item)
            .map(ItemStack::count)
            .sum()
    }

    /// Removes up to `count` items from inventory slots and returns the amount removed.
    /// Cursor-held items are intentionally excluded from crafting and consumption.
    pub fn remove_item(&mut self, item: ItemId, mut count: u32) -> u32 {
        let requested = count;
        for slot in &mut self.slots {
            let Some(stack) = slot.as_mut().filter(|stack| stack.item() == item) else {
                continue;
            };
            count -= stack.remove(count);
            if stack.is_empty() {
                *slot = None;
            }
            if count == 0 {
                break;
            }
        }
        requested - count
    }

    pub fn clear(&mut self) {
        self.slots.fill(None);
        self.cursor_held = None;
    }

    /// Applies a mouse action without involving UI entities or Bevy pointer state.
    pub fn click_slot(&mut self, index: usize, click: InventoryClick) -> bool {
        if index >= TOTAL_SLOT_COUNT {
            return false;
        }

        match click {
            InventoryClick::Left => self.left_click_slot(index),
            InventoryClick::Right => self.right_click_slot(index),
        }
    }

    /// Returns a carried stack to matching slots first, then to an empty slot.
    ///
    /// If every compatible slot is full, the stack remains carried so items are
    /// never discarded when the inventory closes.
    pub fn stow_held_stack(&mut self) {
        let Some(mut held) = self.cursor_held.take() else {
            return;
        };

        for slot in self.slots.iter_mut().flatten() {
            if slot.item() == held.item() && slot.max_stack_size() == held.max_stack_size() {
                slot.merge(&mut held);
                if held.is_empty() {
                    return;
                }
            }
        }

        if let Some(empty) = self.slots.iter_mut().find(|slot| slot.is_none()) {
            *empty = Some(held);
        } else {
            self.cursor_held = Some(held);
        }
    }

    fn left_click_slot(&mut self, index: usize) -> bool {
        let Some(mut held) = self.cursor_held.take() else {
            self.cursor_held = self.slots[index].take();
            return self.cursor_held.is_some();
        };

        let Some(mut target) = self.slots[index].take() else {
            self.slots[index] = Some(held);
            return true;
        };

        if target.item() == held.item() && target.max_stack_size() == held.max_stack_size() {
            let moved = target.merge(&mut held);
            self.slots[index] = Some(target);
            if !held.is_empty() {
                self.cursor_held = Some(held);
            }
            moved > 0
        } else {
            self.slots[index] = Some(held);
            self.cursor_held = Some(target);
            true
        }
    }

    fn right_click_slot(&mut self, index: usize) -> bool {
        let Some(mut held) = self.cursor_held.take() else {
            let Some(mut target) = self.slots[index].take() else {
                return false;
            };
            let take_count = target.count().div_ceil(2);
            self.cursor_held = target.split(take_count);
            if !target.is_empty() {
                self.slots[index] = Some(target);
            }
            return true;
        };

        let Some(mut target) = self.slots[index].take() else {
            let one = held
                .split(1)
                .expect("a cursor-held stack is always non-empty");
            self.slots[index] = Some(one);
            if !held.is_empty() {
                self.cursor_held = Some(held);
            }
            return true;
        };

        if target.item() == held.item() && target.max_stack_size() == held.max_stack_size() {
            let added = target.add(1);
            held.remove(added);
            self.slots[index] = Some(target);
            if !held.is_empty() {
                self.cursor_held = Some(held);
            }
            added == 1
        } else {
            self.slots[index] = Some(target);
            self.cursor_held = Some(held);
            false
        }
    }
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self {
            slots: [None; TOTAL_SLOT_COUNT],
            selected_hotbar: 0,
            cursor_held: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    const STACK_LIMIT: u32 = 64;

    fn block_stack(item: ItemId, count: u32) -> ItemStack {
        ItemStack::new(item, count, STACK_LIMIT).unwrap()
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            slots: [None; TOTAL_SLOT_COUNT],
            selected_hotbar: 0,
            cursor_held: None,
        }
    }

    fn counts(inventory: &PlayerInventory) -> HashMap<ItemId, u32> {
        let mut result = HashMap::new();
        for stack in inventory
            .slots
            .iter()
            .flatten()
            .chain(inventory.cursor_held.iter())
        {
            *result.entry(stack.item()).or_default() += stack.count();
        }
        result
    }

    #[test]
    fn default_inventory_has_nine_hotbar_and_twenty_seven_storage_slots() {
        let inventory = PlayerInventory::default();

        assert_eq!(inventory.hotbar_slots().len(), HOTBAR_SLOT_COUNT);
        assert_eq!(inventory.inventory_slots().len(), INVENTORY_SLOT_COUNT);
        assert_eq!(inventory.slots().len(), TOTAL_SLOT_COUNT);
        assert_eq!(inventory.selected_stack(), None);
    }

    #[test]
    fn hotbar_selection_wraps_and_rejects_storage_indices() {
        let mut inventory = PlayerInventory::default();

        assert!(inventory.select_hotbar(8));
        inventory.cycle_hotbar(1);
        assert_eq!(inventory.selected_hotbar_index(), 0);
        inventory.cycle_hotbar(-1);
        assert_eq!(inventory.selected_hotbar_index(), 8);
        assert!(!inventory.select_hotbar(HOTBAR_SLOT_COUNT));
    }

    #[test]
    fn left_click_takes_and_places_a_whole_stack() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::DIRT_BLOCK, 12));

        assert!(inventory.click_slot(0, InventoryClick::Left));
        assert_eq!(inventory.slot(0), None);
        assert_eq!(inventory.cursor_held_stack().unwrap().count(), 12);
        assert!(inventory.click_slot(5, InventoryClick::Left));
        assert_eq!(inventory.slot(5).unwrap().count(), 12);
        assert_eq!(inventory.cursor_held_stack(), None);
    }

    #[test]
    fn left_click_merges_matching_stacks_without_exceeding_limit() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::STONE_BLOCK, 60));
        inventory.slots[1] = Some(block_stack(ItemId::STONE_BLOCK, 10));
        inventory.click_slot(1, InventoryClick::Left);

        assert!(inventory.click_slot(0, InventoryClick::Left));
        assert_eq!(inventory.slot(0).unwrap().count(), 64);
        assert_eq!(inventory.cursor_held_stack().unwrap().count(), 6);
    }

    #[test]
    fn left_click_swaps_different_items() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::DIRT_BLOCK, 3));
        inventory.slots[1] = Some(block_stack(ItemId::WOOD_BLOCK, 7));
        inventory.click_slot(0, InventoryClick::Left);

        assert!(inventory.click_slot(1, InventoryClick::Left));
        assert_eq!(inventory.slot(1).unwrap().item(), ItemId::DIRT_BLOCK);
        assert_eq!(
            inventory.cursor_held_stack().unwrap().item(),
            ItemId::WOOD_BLOCK
        );
    }

    #[test]
    fn right_click_takes_rounded_up_half() {
        let mut inventory = empty_inventory();
        inventory.slots[2] = Some(block_stack(ItemId::SAND_BLOCK, 5));

        assert!(inventory.click_slot(2, InventoryClick::Right));
        assert_eq!(inventory.slot(2).unwrap().count(), 2);
        assert_eq!(inventory.cursor_held_stack().unwrap().count(), 3);
    }

    #[test]
    fn right_click_places_exactly_one_item() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::LEAVES_BLOCK, 3));
        inventory.click_slot(0, InventoryClick::Left);

        assert!(inventory.click_slot(4, InventoryClick::Right));
        assert_eq!(inventory.slot(4).unwrap().count(), 1);
        assert_eq!(inventory.cursor_held_stack().unwrap().count(), 2);
        assert!(inventory.click_slot(4, InventoryClick::Right));
        assert_eq!(inventory.slot(4).unwrap().count(), 2);
        assert_eq!(inventory.cursor_held_stack().unwrap().count(), 1);
    }

    #[test]
    fn incompatible_right_click_is_a_noop() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::DIRT_BLOCK, 2));
        inventory.slots[1] = Some(block_stack(ItemId::STONE_BLOCK, 2));
        inventory.click_slot(0, InventoryClick::Left);
        let before = counts(&inventory);

        assert!(!inventory.click_slot(1, InventoryClick::Right));
        assert_eq!(counts(&inventory), before);
        assert_eq!(inventory.slot(1).unwrap().item(), ItemId::STONE_BLOCK);
    }

    #[test]
    fn every_mouse_operation_conserves_all_items() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::DIRT_BLOCK, 20));
        inventory.slots[1] = Some(block_stack(ItemId::STONE_BLOCK, 12));
        let expected = counts(&inventory);
        let actions = [
            (0, InventoryClick::Left),
            (8, InventoryClick::Right),
            (9, InventoryClick::Left),
            (1, InventoryClick::Left),
            (18, InventoryClick::Right),
            (2, InventoryClick::Right),
            (30, InventoryClick::Left),
            (30, InventoryClick::Right),
        ];

        for (slot, click) in actions {
            inventory.click_slot(slot, click);
            assert_eq!(counts(&inventory), expected);
        }
        inventory.stow_held_stack();
        assert_eq!(counts(&inventory), expected);
    }

    #[test]
    fn invalid_slot_never_changes_inventory() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::DIRT_BLOCK, 3));
        let expected = counts(&inventory);

        assert!(!inventory.click_slot(TOTAL_SLOT_COUNT, InventoryClick::Left));
        assert_eq!(counts(&inventory), expected);
    }

    #[test]
    fn picked_up_items_merge_and_spill_into_empty_slots() {
        let registry = ItemRegistry::default();
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::APPLE, 63));

        assert_eq!(inventory.add_item(ItemId::APPLE, 3, &registry), 0);
        assert_eq!(inventory.slot(0).unwrap().count(), 64);
        assert_eq!(inventory.slot(1).unwrap().count(), 2);
    }

    #[test]
    fn full_inventory_returns_items_that_do_not_fit() {
        let registry = ItemRegistry::default();
        let mut inventory = empty_inventory();
        inventory
            .slots
            .fill(Some(block_stack(ItemId::DIRT_BLOCK, 64)));

        assert_eq!(inventory.add_item(ItemId::APPLE, 2, &registry), 2);
    }

    #[test]
    fn counting_and_removing_items_spans_stacks_without_touching_cursor() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(block_stack(ItemId::WOOD_BLOCK, 40));
        inventory.slots[3] = Some(block_stack(ItemId::WOOD_BLOCK, 30));
        inventory.cursor_held = Some(block_stack(ItemId::WOOD_BLOCK, 7));

        assert_eq!(inventory.item_count(ItemId::WOOD_BLOCK), 70);
        assert_eq!(inventory.remove_item(ItemId::WOOD_BLOCK, 50), 50);
        assert_eq!(inventory.item_count(ItemId::WOOD_BLOCK), 20);
        assert_eq!(inventory.cursor_held_stack().unwrap().count(), 7);
    }
}
