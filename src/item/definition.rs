use crate::block::BlockId;

/// Hunger restored by consuming an edible item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodProperties {
    pub nutrition: u8,
    pub saturation: f32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ToolType {
    Pickaxe,
    Axe,
    Shovel,
    Sword,
}

impl ToolType {
    /// Whether this category receives its configured mining speed for a block.
    pub const fn is_effective_against(self, block: BlockId) -> bool {
        match self {
            Self::Pickaxe => matches!(
                block,
                BlockId::STONE | BlockId::COAL_ORE | BlockId::IRON_ORE
            ),
            Self::Axe => matches!(block, BlockId::WOOD),
            Self::Shovel => matches!(block, BlockId::GRASS | BlockId::DIRT | BlockId::SAND),
            Self::Sword => matches!(block, BlockId::LEAVES),
        }
    }
}

/// Material tier used when deciding whether a tool may harvest a block drop.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HarvestTier {
    Wood,
    Stone,
    Iron,
}

/// Tunable behavior shared by mining and future melee combat systems.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolProperties {
    pub mining_speed: f32,
    pub attack_damage: f32,
    /// Fully charged attacks per second.
    pub attack_speed: f32,
    pub harvest_tier: HarvestTier,
}

/// Complete immutable definition of an inventory tool or weapon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolDefinition {
    pub tool_type: ToolType,
    pub durability: u32,
    pub properties: ToolProperties,
}

impl ToolDefinition {
    pub const fn is_effective_against(self, block: BlockId) -> bool {
        self.tool_type.is_effective_against(block)
    }

    pub fn mining_speed_for(self, block: BlockId) -> f32 {
        if self.is_effective_against(block) {
            self.properties.mining_speed.max(1.0)
        } else {
            1.0
        }
    }

    pub fn can_harvest(self, block: BlockId) -> bool {
        required_harvest_tier(block).is_none_or(|required| {
            self.is_effective_against(block) && self.properties.harvest_tier >= required
        })
    }

    pub fn attack_cooldown_seconds(self) -> f32 {
        self.properties.attack_speed.max(f32::EPSILON).recip()
    }

    pub const fn block_requires_tool(block: BlockId) -> bool {
        required_harvest_tier(block).is_some()
    }
}

const fn required_harvest_tier(block: BlockId) -> Option<HarvestTier> {
    match block {
        BlockId::STONE | BlockId::COAL_ORE => Some(HarvestTier::Wood),
        BlockId::IRON_ORE => Some(HarvestTier::Stone),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WOODEN_PICKAXE: ToolDefinition = ToolDefinition {
        tool_type: ToolType::Pickaxe,
        durability: 59,
        properties: ToolProperties {
            mining_speed: 2.0,
            attack_damage: 2.0,
            attack_speed: 1.2,
            harvest_tier: HarvestTier::Wood,
        },
    };
    const STONE_PICKAXE: ToolDefinition = ToolDefinition {
        properties: ToolProperties {
            harvest_tier: HarvestTier::Stone,
            ..WOODEN_PICKAXE.properties
        },
        ..WOODEN_PICKAXE
    };

    #[test]
    fn tool_categories_define_effective_blocks_in_one_api() {
        assert!(ToolType::Pickaxe.is_effective_against(BlockId::STONE));
        assert!(ToolType::Axe.is_effective_against(BlockId::WOOD));
        assert!(ToolType::Shovel.is_effective_against(BlockId::DIRT));
        assert!(ToolType::Sword.is_effective_against(BlockId::LEAVES));
        assert!(!ToolType::Axe.is_effective_against(BlockId::STONE));
    }

    #[test]
    fn harvest_tier_controls_ore_drops() {
        assert!(WOODEN_PICKAXE.can_harvest(BlockId::COAL_ORE));
        assert!(!WOODEN_PICKAXE.can_harvest(BlockId::IRON_ORE));
        assert!(STONE_PICKAXE.can_harvest(BlockId::IRON_ORE));
    }

    #[test]
    fn ineffective_tools_use_hand_mining_speed() {
        assert_eq!(WOODEN_PICKAXE.mining_speed_for(BlockId::STONE), 2.0);
        assert_eq!(WOODEN_PICKAXE.mining_speed_for(BlockId::WOOD), 1.0);
    }
}

/// Central metadata for an inventory item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemDefinition {
    pub name: &'static str,
    pub max_stack_size: u32,
    /// Voxel placed by this item, or None for non-block items.
    pub block: Option<BlockId>,
    pub food: Option<FoodProperties>,
    pub tool: Option<ToolDefinition>,
    /// Lightweight world-drop tint and fallback presentation color.
    pub debug_color: [f32; 4],
}
