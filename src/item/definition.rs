use crate::block::BlockId;

/// Hunger restored by consuming an edible item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodProperties {
    pub nutrition: u8,
    pub saturation: f32,
}

/// Central metadata for an inventory item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemDefinition {
    pub name: &'static str,
    pub max_stack_size: u32,
    /// Voxel placed by this item, or None for non-block items.
    pub block: Option<BlockId>,
    pub food: Option<FoodProperties>,
    /// Temporary UI color used until dedicated item icons are available.
    pub debug_color: [f32; 4],
}
