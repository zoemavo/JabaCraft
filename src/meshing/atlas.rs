use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::Image,
};

use crate::block::TextureIndex;

pub(super) const ATLAS_COLUMNS: u32 = 4;
pub(super) const ATLAS_ROWS: u32 = 4;
pub(super) const TILE_SIZE: u32 = 32;

const ATLAS_WIDTH: u32 = ATLAS_COLUMNS * TILE_SIZE;
const ATLAS_HEIGHT: u32 = ATLAS_ROWS * TILE_SIZE;

/// Loads the built-in Faithful 32x block atlas.
pub(super) fn create_block_texture_atlas() -> Image {
    Image::from_buffer(
        include_bytes!("../../assets/textures/faithful_32x_atlas.png"),
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .expect("embedded Faithful block atlas must be a valid PNG")
}

/// Returns UVs inset by half a texel so filtering cannot sample an adjacent tile.
pub(super) fn atlas_uvs(texture: TextureIndex) -> [[f32; 2]; 4] {
    debug_assert!(u32::from(texture) < ATLAS_COLUMNS * ATLAS_ROWS);

    let tile_x = u32::from(texture) % ATLAS_COLUMNS;
    let tile_y = u32::from(texture) / ATLAS_COLUMNS;
    let half_texel = 0.5;
    let u_min = (tile_x * TILE_SIZE) as f32 + half_texel;
    let u_max = ((tile_x + 1) * TILE_SIZE) as f32 - half_texel;
    let v_min = (tile_y * TILE_SIZE) as f32 + half_texel;
    let v_max = ((tile_y + 1) * TILE_SIZE) as f32 - half_texel;
    let u_min = u_min / ATLAS_WIDTH as f32;
    let u_max = u_max / ATLAS_WIDTH as f32;
    let v_min = v_min / ATLAS_HEIGHT as f32;
    let v_max = v_max / ATLAS_HEIGHT as f32;

    [
        [u_min, v_max],
        [u_min, v_min],
        [u_max, v_min],
        [u_max, v_max],
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn atlas_has_expected_dimensions_and_nearest_sampling() {
        let image = create_block_texture_atlas();

        assert_eq!(image.width(), ATLAS_WIDTH);
        assert_eq!(image.height(), ATLAS_HEIGHT);
        assert_eq!(image.sampler, ImageSampler::nearest());
        assert_eq!(
            image.data.as_ref().unwrap().len(),
            (ATLAS_WIDTH * ATLAS_HEIGHT * 4) as usize
        );
    }

    #[test]
    fn visible_block_textures_are_distinct() {
        let image = create_block_texture_atlas();
        let data = image.data.as_ref().unwrap();
        let textures = [1_u16, 2, 3, 4, 5, 6, 7, 8, 10, 11];
        let tiles = textures
            .into_iter()
            .map(|texture| tile_bytes(data, texture))
            .collect::<HashSet<_>>();

        assert_eq!(tiles.len(), textures.len());
    }

    #[test]
    fn uv_coordinates_stay_half_a_texel_inside_their_cell() {
        let grass_top = atlas_uvs(1);

        assert_eq!(grass_top[0], [32.5 / 128.0, 31.5 / 128.0]);
        assert_eq!(grass_top[2], [63.5 / 128.0, 0.5 / 128.0]);
        assert!(
            grass_top
                .into_iter()
                .flatten()
                .all(|value| (0.0..=1.0).contains(&value))
        );
    }

    fn tile_bytes(data: &[u8], texture: TextureIndex) -> Vec<u8> {
        let tile_x = u32::from(texture) % ATLAS_COLUMNS;
        let tile_y = u32::from(texture) / ATLAS_COLUMNS;
        let mut bytes = Vec::with_capacity((TILE_SIZE * TILE_SIZE * 4) as usize);

        for y in 0..TILE_SIZE {
            let atlas_y = tile_y * TILE_SIZE + y;
            let atlas_x = tile_x * TILE_SIZE;
            let start = ((atlas_y * ATLAS_WIDTH + atlas_x) * 4) as usize;
            let end = start + (TILE_SIZE * 4) as usize;
            bytes.extend_from_slice(&data[start..end]);
        }

        bytes
    }
}
