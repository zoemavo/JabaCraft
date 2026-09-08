use bevy::prelude::Resource;

use crate::block::BlockId;

use super::{
    BLOCK_ITEM_MAX_STACK_SIZE, BUILTIN_ITEM_COUNT, FoodProperties, ItemDefinition, ItemId,
    ItemStack, ItemStackError,
};

/// Central source of item metadata and block-item mappings.
#[derive(Debug, Resource)]
pub struct ItemRegistry {
    definitions: [ItemDefinition; BUILTIN_ITEM_COUNT],
}

impl ItemRegistry {
    pub fn definition(&self, id: ItemId) -> &ItemDefinition {
        &self.definitions[id.as_index()]
    }

    pub fn definition_from_raw(&self, raw: u16) -> Option<&ItemDefinition> {
        ItemId::from_raw(raw).map(|id| self.definition(id))
    }

    pub fn block_for(&self, id: ItemId) -> Option<BlockId> {
        self.definition(id).block
    }

    pub fn item_for_block(&self, block: BlockId) -> Option<ItemId> {
        ItemId::ALL
            .into_iter()
            .find(|&item| self.definition(item).block == Some(block))
    }

    pub fn food(&self, id: ItemId) -> Option<FoodProperties> {
        self.definition(id).food
    }

    pub fn max_stack_size(&self, id: ItemId) -> u32 {
        self.definition(id).max_stack_size
    }

    pub fn debug_color(&self, id: ItemId) -> [f32; 4] {
        self.definition(id).debug_color
    }

    pub fn create_stack(&self, item: ItemId, count: u32) -> Result<ItemStack, ItemStackError> {
        ItemStack::new(item, count, self.max_stack_size(item))
    }
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self {
            definitions: [
                block_item("Grass Block", BlockId::GRASS, [0.32, 0.68, 0.25, 1.0]),
                block_item("Dirt", BlockId::DIRT, [0.45, 0.28, 0.14, 1.0]),
                block_item("Stone", BlockId::STONE, [0.48, 0.50, 0.52, 1.0]),
                block_item("Sand", BlockId::SAND, [0.82, 0.76, 0.49, 1.0]),
                block_item("Oak Log", BlockId::WOOD, [0.48, 0.30, 0.13, 1.0]),
                block_item("Oak Leaves", BlockId::LEAVES, [0.20, 0.52, 0.16, 0.8]),
                block_item("Water", BlockId::WATER, [0.12, 0.42, 0.82, 0.65]),
                block_item("Coal Ore", BlockId::COAL_ORE, [0.20, 0.21, 0.22, 1.0]),
                block_item("Iron Ore", BlockId::IRON_ORE, [0.65, 0.48, 0.37, 1.0]),
                ItemDefinition {
                    name: "Stick",
                    max_stack_size: 64,
                    block: None,
                    food: None,
                    debug_color: [0.55, 0.34, 0.15, 1.0],
                },
                ItemDefinition {
                    name: "Apple",
                    max_stack_size: 64,
                    block: None,
                    food: Some(FoodProperties {
                        nutrition: 4,
                        saturation: 2.4,
                    }),
                    debug_color: [0.82, 0.05, 0.04, 1.0],
                },
            ],
        }
    }
}

const fn block_item(name: &'static str, block: BlockId, debug_color: [f32; 4]) -> ItemDefinition {
    ItemDefinition {
        name,
        max_stack_size: BLOCK_ITEM_MAX_STACK_SIZE,
        block: Some(block),
        food: None,
        debug_color,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, mem::size_of};

    use super::*;

    #[test]
    fn item_id_is_compact_and_distinct_from_block_id() {
        assert_eq!(size_of::<ItemId>(), size_of::<u16>());
        assert_ne!(
            std::any::TypeId::of::<ItemId>(),
            std::any::TypeId::of::<BlockId>()
        );
    }

    #[test]
    fn every_builtin_item_has_a_unique_definition() {
        let registry = ItemRegistry::default();
        let names = ItemId::ALL
            .into_iter()
            .map(|id| registry.definition(id).name)
            .collect::<HashSet<_>>();

        assert_eq!(names.len(), BUILTIN_ITEM_COUNT);
    }

    #[test]
    fn block_items_map_to_blocks_but_regular_items_do_not() {
        let registry = ItemRegistry::default();

        assert_eq!(registry.block_for(ItemId::DIRT_BLOCK), Some(BlockId::DIRT));
        assert_eq!(registry.block_for(ItemId::STICK), None);
        assert_eq!(
            registry.item_for_block(BlockId::DIRT),
            Some(ItemId::DIRT_BLOCK)
        );
    }

    #[test]
    fn apple_restores_four_hunger_points() {
        let food = ItemRegistry::default().food(ItemId::APPLE).unwrap();

        assert_eq!(food.nutrition, 4);
        assert_eq!(food.saturation, 2.4);
    }

    #[test]
    fn registry_creates_stack_with_definition_limit() {
        let registry = ItemRegistry::default();
        let stack = registry.create_stack(ItemId::STONE_BLOCK, 12).unwrap();

        assert_eq!(stack.item(), ItemId::STONE_BLOCK);
        assert_eq!(stack.count(), 12);
        assert_eq!(stack.max_stack_size(), BLOCK_ITEM_MAX_STACK_SIZE);
    }
}
