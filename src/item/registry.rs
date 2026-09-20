use bevy::prelude::Resource;

use crate::block::BlockId;

use super::{
    BLOCK_ITEM_MAX_STACK_SIZE, BUILTIN_ITEM_COUNT, FoodProperties, HarvestTier, ItemDefinition,
    ItemId, ItemStack, ItemStackError, ToolDefinition, ToolProperties, ToolType,
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

    pub fn tool(&self, id: ItemId) -> Option<ToolDefinition> {
        self.definition(id).tool
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
                    tool: None,
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
                    tool: None,
                    debug_color: [0.82, 0.05, 0.04, 1.0],
                },
                ItemDefinition {
                    name: "Oak Planks",
                    max_stack_size: 64,
                    block: None,
                    food: None,
                    tool: None,
                    debug_color: [0.64, 0.45, 0.24, 1.0],
                },
                tool_item(
                    "Stone Pickaxe",
                    ToolType::Pickaxe,
                    HarvestTier::Stone,
                    131,
                    4.0,
                    3.0,
                    1.2,
                    STONE_TOOL_COLOR,
                ),
                tool_item(
                    "Wooden Pickaxe",
                    ToolType::Pickaxe,
                    HarvestTier::Wood,
                    59,
                    2.0,
                    2.0,
                    1.2,
                    WOODEN_TOOL_COLOR,
                ),
                tool_item(
                    "Iron Pickaxe",
                    ToolType::Pickaxe,
                    HarvestTier::Iron,
                    250,
                    6.0,
                    4.0,
                    1.2,
                    IRON_TOOL_COLOR,
                ),
                tool_item(
                    "Wooden Axe",
                    ToolType::Axe,
                    HarvestTier::Wood,
                    59,
                    2.0,
                    7.0,
                    0.8,
                    WOODEN_TOOL_COLOR,
                ),
                tool_item(
                    "Stone Axe",
                    ToolType::Axe,
                    HarvestTier::Stone,
                    131,
                    4.0,
                    9.0,
                    0.8,
                    STONE_TOOL_COLOR,
                ),
                tool_item(
                    "Iron Axe",
                    ToolType::Axe,
                    HarvestTier::Iron,
                    250,
                    6.0,
                    9.0,
                    0.9,
                    IRON_TOOL_COLOR,
                ),
                tool_item(
                    "Wooden Shovel",
                    ToolType::Shovel,
                    HarvestTier::Wood,
                    59,
                    2.0,
                    2.5,
                    1.0,
                    WOODEN_TOOL_COLOR,
                ),
                tool_item(
                    "Stone Shovel",
                    ToolType::Shovel,
                    HarvestTier::Stone,
                    131,
                    4.0,
                    3.5,
                    1.0,
                    STONE_TOOL_COLOR,
                ),
                tool_item(
                    "Iron Shovel",
                    ToolType::Shovel,
                    HarvestTier::Iron,
                    250,
                    6.0,
                    4.5,
                    1.0,
                    IRON_TOOL_COLOR,
                ),
                tool_item(
                    "Wooden Sword",
                    ToolType::Sword,
                    HarvestTier::Wood,
                    59,
                    1.5,
                    4.0,
                    1.6,
                    WOODEN_TOOL_COLOR,
                ),
                tool_item(
                    "Stone Sword",
                    ToolType::Sword,
                    HarvestTier::Stone,
                    131,
                    1.5,
                    5.0,
                    1.6,
                    STONE_TOOL_COLOR,
                ),
                tool_item(
                    "Iron Sword",
                    ToolType::Sword,
                    HarvestTier::Iron,
                    250,
                    1.5,
                    6.0,
                    1.6,
                    IRON_TOOL_COLOR,
                ),
            ],
        }
    }
}

const WOODEN_TOOL_COLOR: [f32; 4] = [0.56, 0.38, 0.19, 1.0];
const STONE_TOOL_COLOR: [f32; 4] = [0.43, 0.43, 0.43, 1.0];
const IRON_TOOL_COLOR: [f32; 4] = [0.78, 0.79, 0.74, 1.0];

#[allow(clippy::too_many_arguments)]
const fn tool_item(
    name: &'static str,
    tool_type: ToolType,
    harvest_tier: HarvestTier,
    durability: u32,
    mining_speed: f32,
    attack_damage: f32,
    attack_speed: f32,
    debug_color: [f32; 4],
) -> ItemDefinition {
    ItemDefinition {
        name,
        max_stack_size: 1,
        block: None,
        food: None,
        tool: Some(ToolDefinition {
            tool_type,
            durability,
            properties: ToolProperties {
                mining_speed,
                attack_damage,
                attack_speed,
                harvest_tier,
            },
        }),
        debug_color,
    }
}

const fn block_item(name: &'static str, block: BlockId, debug_color: [f32; 4]) -> ItemDefinition {
    ItemDefinition {
        name,
        max_stack_size: BLOCK_ITEM_MAX_STACK_SIZE,
        block: Some(block),
        food: None,
        tool: None,
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

    #[test]
    fn stone_pickaxe_has_complete_tool_metadata() {
        let registry = ItemRegistry::default();
        let definition = registry.definition(ItemId::STONE_PICKAXE);

        assert_eq!(definition.max_stack_size, 1);
        assert_eq!(definition.block, None);
        assert_eq!(
            definition.tool,
            Some(ToolDefinition {
                tool_type: ToolType::Pickaxe,
                durability: 131,
                properties: ToolProperties {
                    mining_speed: 4.0,
                    attack_damage: 3.0,
                    attack_speed: 1.2,
                    harvest_tier: HarvestTier::Stone,
                },
            })
        );
    }

    #[test]
    fn all_tool_and_weapon_ids_are_registered_by_category() {
        let registry = ItemRegistry::default();
        let expected = [
            (ItemId::WOODEN_PICKAXE, ToolType::Pickaxe),
            (ItemId::STONE_PICKAXE, ToolType::Pickaxe),
            (ItemId::IRON_PICKAXE, ToolType::Pickaxe),
            (ItemId::WOODEN_AXE, ToolType::Axe),
            (ItemId::STONE_AXE, ToolType::Axe),
            (ItemId::IRON_AXE, ToolType::Axe),
            (ItemId::WOODEN_SHOVEL, ToolType::Shovel),
            (ItemId::STONE_SHOVEL, ToolType::Shovel),
            (ItemId::IRON_SHOVEL, ToolType::Shovel),
            (ItemId::WOODEN_SWORD, ToolType::Sword),
            (ItemId::STONE_SWORD, ToolType::Sword),
            (ItemId::IRON_SWORD, ToolType::Sword),
        ];

        for (item, tool_type) in expected {
            let definition = registry.definition(item);
            let tool = definition.tool.expect("expected a tool definition");
            assert_eq!(tool.tool_type, tool_type);
            assert_eq!(definition.max_stack_size, 1);
            assert!(tool.durability > 0);
            assert!(tool.properties.mining_speed > 0.0);
            assert!(tool.properties.attack_damage > 0.0);
            assert!(tool.properties.attack_speed > 0.0);
        }
    }

    #[test]
    fn material_tiers_increase_core_tool_properties() {
        let registry = ItemRegistry::default();
        let wood = registry.tool(ItemId::WOODEN_PICKAXE).unwrap();
        let stone = registry.tool(ItemId::STONE_PICKAXE).unwrap();
        let iron = registry.tool(ItemId::IRON_PICKAXE).unwrap();

        assert!(wood.durability < stone.durability && stone.durability < iron.durability);
        assert!(
            wood.properties.mining_speed < stone.properties.mining_speed
                && stone.properties.mining_speed < iron.properties.mining_speed
        );
        assert!(
            wood.properties.harvest_tier < stone.properties.harvest_tier
                && stone.properties.harvest_tier < iron.properties.harvest_tier
        );
        assert!((stone.attack_cooldown_seconds() - (1.0 / 1.2)).abs() < f32::EPSILON);
    }
}
