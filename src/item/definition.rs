use crate::block::BlockId;

/// Central metadata for an inventory item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemDefinition {
    pub name: &'static str,
    pub max_stack_size: u32,
    /// Voxel placed by this item, or None for non-block items.
    pub block: Option<BlockId>,
    /// Temporary UI color used until dedicated item icons are available.
    pub debug_color: [f32; 4],
}
