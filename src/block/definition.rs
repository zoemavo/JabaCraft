/// Index into a future block-texture atlas.
pub type TextureIndex = u16;

/// One face of an axis-aligned voxel block.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlockFace {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

/// Texture-atlas indices for all six block faces.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FaceTextureIndices {
    pub top: TextureIndex,
    pub bottom: TextureIndex,
    pub north: TextureIndex,
    pub south: TextureIndex,
    pub east: TextureIndex,
    pub west: TextureIndex,
}

impl FaceTextureIndices {
    pub const fn uniform(texture: TextureIndex) -> Self {
        Self {
            top: texture,
            bottom: texture,
            north: texture,
            south: texture,
            east: texture,
            west: texture,
        }
    }

    pub const fn top_bottom_sides(
        top: TextureIndex,
        bottom: TextureIndex,
        sides: TextureIndex,
    ) -> Self {
        Self {
            top,
            bottom,
            north: sides,
            south: sides,
            east: sides,
            west: sides,
        }
    }

    pub const fn texture_for(self, face: BlockFace) -> TextureIndex {
        match face {
            BlockFace::Top => self.top,
            BlockFace::Bottom => self.bottom,
            BlockFace::North => self.north,
            BlockFace::South => self.south,
            BlockFace::East => self.east,
            BlockFace::West => self.west,
        }
    }
}

/// Central metadata describing the behaviour and rendering of a block type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockDefinition {
    pub name: &'static str,
    pub solid: bool,
    pub transparent: bool,
    pub breakable: bool,
    pub textures: FaceTextureIndices,
    pub hardness: f32,
    /// Temporary vertex color used until the texture atlas is introduced.
    pub debug_color: [f32; 4],
}
