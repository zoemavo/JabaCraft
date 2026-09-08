use crate::block::BlockId;

/// Hunger restored by consuming an edible item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodProperties {
    pub nutrition: u8,
    pub saturation: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolType {
    Pickaxe,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolProperties {
    pub tool_type: ToolType,
    pub mining_speed: f32,
}

/// Central metadata for an inventory item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemDefinition {
    pub name: &'static str,
    pub max_stack_size: u32,
    /// Voxel placed by this item, or None for non-block items.
    pub block: Option<BlockId>,
    pub food: Option<FoodProperties>,
    pub tool: Option<ToolProperties>,
    /// Lightweight world-drop tint and fallback presentation color.
    pub debug_color: [f32; 4],
}
