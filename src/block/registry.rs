use bevy::prelude::Resource;

use super::{
    BUILTIN_BLOCK_COUNT, BlockDefinition, BlockFace, BlockId, FaceTextureIndices, TextureIndex,
};

/// Central source of block behaviour and rendering metadata.
#[derive(Debug, Resource)]
pub struct BlockRegistry {
    definitions: [BlockDefinition; BUILTIN_BLOCK_COUNT],
}

impl BlockRegistry {
    pub fn definition(&self, id: BlockId) -> &BlockDefinition {
        &self.definitions[id.as_index()]
    }

    pub fn definition_from_raw(&self, raw: u16) -> Option<&BlockDefinition> {
        BlockId::from_raw(raw).map(|id| self.definition(id))
    }

    pub fn is_solid(&self, id: BlockId) -> bool {
        self.definition(id).solid
    }

    pub fn is_transparent(&self, id: BlockId) -> bool {
        self.definition(id).transparent
    }

    pub fn is_breakable(&self, id: BlockId) -> bool {
        self.definition(id).breakable
    }

    pub fn texture_for(&self, id: BlockId, face: BlockFace) -> TextureIndex {
        self.definition(id).textures.texture_for(face)
    }

    pub fn debug_color(&self, id: BlockId) -> [f32; 4] {
        self.definition(id).debug_color
    }
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self {
            definitions: [
                definition(
                    "air",
                    false,
                    true,
                    false,
                    uniform(0),
                    0.0,
                    [0.0, 0.0, 0.0, 0.0],
                ),
                definition(
                    "grass",
                    true,
                    false,
                    true,
                    faces(1, 2, 3),
                    0.6,
                    [0.32, 0.68, 0.25, 1.0],
                ),
                definition(
                    "dirt",
                    true,
                    false,
                    true,
                    uniform(2),
                    0.5,
                    [0.45, 0.28, 0.14, 1.0],
                ),
                definition(
                    "stone",
                    true,
                    false,
                    true,
                    uniform(4),
                    1.5,
                    [0.48, 0.50, 0.52, 1.0],
                ),
                definition(
                    "sand",
                    true,
                    false,
                    true,
                    uniform(5),
                    0.5,
                    [0.82, 0.76, 0.49, 1.0],
                ),
                definition(
                    "wood",
                    true,
                    false,
                    true,
                    faces(6, 6, 7),
                    2.0,
                    [0.48, 0.30, 0.13, 1.0],
                ),
                definition(
                    "leaves",
                    true,
                    true,
                    true,
                    uniform(8),
                    0.2,
                    [0.20, 0.52, 0.16, 0.8],
                ),
                definition(
                    "water",
                    false,
                    true,
                    false,
                    uniform(9),
                    0.0,
                    [0.12, 0.42, 0.82, 0.65],
                ),
                definition(
                    "coal_ore",
                    true,
                    false,
                    true,
                    uniform(10),
                    3.0,
                    [0.20, 0.21, 0.22, 1.0],
                ),
                definition(
                    "iron_ore",
                    true,
                    false,
                    true,
                    uniform(11),
                    3.0,
                    [0.65, 0.48, 0.37, 1.0],
                ),
            ],
        }
    }
}

const fn definition(
    name: &'static str,
    solid: bool,
    transparent: bool,
    breakable: bool,
    textures: FaceTextureIndices,
    hardness: f32,
    debug_color: [f32; 4],
) -> BlockDefinition {
    BlockDefinition {
        name,
        solid,
        transparent,
        breakable,
        textures,
        hardness,
        debug_color,
    }
}

const fn uniform(texture: TextureIndex) -> FaceTextureIndices {
    FaceTextureIndices::uniform(texture)
}

const fn faces(top: TextureIndex, bottom: TextureIndex, sides: TextureIndex) -> FaceTextureIndices {
    FaceTextureIndices::top_bottom_sides(top, bottom, sides)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, mem::size_of};

    use super::*;

    #[test]
    fn block_id_is_compact() {
        assert_eq!(size_of::<BlockId>(), size_of::<u16>());
    }

    #[test]
    fn every_builtin_id_has_a_unique_definition() {
        let registry = BlockRegistry::default();
        let names = BlockId::ALL
            .into_iter()
            .map(|id| registry.definition(id).name)
            .collect::<HashSet<_>>();

        assert_eq!(names.len(), BUILTIN_BLOCK_COUNT);
    }

    #[test]
    fn air_is_not_solid_or_breakable() {
        let registry = BlockRegistry::default();

        assert!(!registry.is_solid(BlockId::AIR));
        assert!(!registry.is_breakable(BlockId::AIR));
        assert!(registry.is_transparent(BlockId::AIR));
    }

    #[test]
    fn invalid_raw_ids_are_rejected() {
        assert_eq!(
            BlockId::try_from(BUILTIN_BLOCK_COUNT as u16),
            Err(super::super::InvalidBlockId(BUILTIN_BLOCK_COUNT as u16))
        );
    }

    #[test]
    fn per_face_textures_are_read_through_the_registry() {
        let registry = BlockRegistry::default();

        assert_eq!(registry.texture_for(BlockId::GRASS, BlockFace::Top), 1);
        assert_eq!(registry.texture_for(BlockId::GRASS, BlockFace::Bottom), 2);
        assert_eq!(registry.texture_for(BlockId::GRASS, BlockFace::North), 3);
        assert_eq!(registry.texture_for(BlockId::WOOD, BlockFace::Top), 6);
        assert_eq!(registry.texture_for(BlockId::WOOD, BlockFace::East), 7);
    }
}
