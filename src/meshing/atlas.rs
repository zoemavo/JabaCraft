use bevy::{
    asset::RenderAssetUsages,
    image::{
        CompressedImageFormats, ImageFilterMode, ImageSampler, ImageSamplerDescriptor, ImageType,
    },
    prelude::Image,
};

use crate::block::TextureIndex;

pub(super) const ATLAS_COLUMNS: u32 = 4;
pub(super) const ATLAS_ROWS: u32 = 4;
pub(super) const TILE_SIZE: u32 = 32;
pub(super) const TILE_GUTTER: u32 = 16;
pub(super) const ATLAS_CELL_SIZE: u32 = TILE_SIZE + TILE_GUTTER * 2;
const MIP_LEVEL_COUNT: u32 = 5;

const SOURCE_ATLAS_WIDTH: u32 = ATLAS_COLUMNS * TILE_SIZE;
pub(super) const ATLAS_WIDTH: u32 = ATLAS_COLUMNS * ATLAS_CELL_SIZE;
const ATLAS_HEIGHT: u32 = ATLAS_ROWS * ATLAS_CELL_SIZE;

/// Loads the built-in Faithful 32x block atlas.
pub(super) fn create_block_texture_atlas() -> Image {
    let mut image = Image::from_buffer(
        include_bytes!("../../assets/textures/faithful_32x_atlas.png"),
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .expect("embedded Faithful block atlas must be a valid PNG");
    let source = image
        .data
        .take()
        .expect("decoded Faithful atlas must retain its pixels");
    assert_eq!(
        source.len(),
        (SOURCE_ATLAS_WIDTH * SOURCE_ATLAS_WIDTH * 4) as usize,
        "embedded Faithful atlas must decode to RGBA8"
    );

    image.data = Some(build_padded_mip_chain(&source));
    image.texture_descriptor.size.width = ATLAS_WIDTH;
    image.texture_descriptor.size.height = ATLAS_HEIGHT;
    image.texture_descriptor.mip_level_count = MIP_LEVEL_COUNT;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        // Preserve crisp pixel art up close, but blend minified texels and mip
        // levels so shallow ground planes do not turn into colored moire bands.
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        lod_max_clamp: (MIP_LEVEL_COUNT - 1) as f32,
        ..ImageSamplerDescriptor::default()
    });
    image
}

/// Each tile gets a wide replicated gutter at every mip. Hardware filtering can
/// therefore never pull sand/water/etc. into a neighboring material's sample.
fn build_padded_mip_chain(source: &[u8]) -> Vec<u8> {
    let total_bytes = (0..MIP_LEVEL_COUNT)
        .map(|level| {
            let width = ATLAS_WIDTH >> level;
            (width * width * 4) as usize
        })
        .sum();
    let mut output = Vec::with_capacity(total_bytes);

    for level in 0..MIP_LEVEL_COUNT {
        let scale = 1_u32 << level;
        let inner_size = TILE_SIZE / scale;
        let gutter = TILE_GUTTER / scale;
        let cell_size = inner_size + gutter * 2;
        let atlas_width = cell_size * ATLAS_COLUMNS;
        let mut pixels = vec![0; (atlas_width * atlas_width * 4) as usize];

        for tile_y in 0..ATLAS_ROWS {
            for tile_x in 0..ATLAS_COLUMNS {
                for cell_y in 0..cell_size {
                    for cell_x in 0..cell_size {
                        let inner_x = cell_x.saturating_sub(gutter).min(inner_size - 1);
                        let inner_y = cell_y.saturating_sub(gutter).min(inner_size - 1);
                        let rgba = downsample_source_pixel(
                            source,
                            tile_x * TILE_SIZE + inner_x * scale,
                            tile_y * TILE_SIZE + inner_y * scale,
                            scale,
                        );
                        let x = tile_x * cell_size + cell_x;
                        let y = tile_y * cell_size + cell_y;
                        let offset = ((y * atlas_width + x) * 4) as usize;
                        pixels[offset..offset + 4].copy_from_slice(&rgba);
                    }
                }
            }
        }
        output.extend_from_slice(&pixels);
    }
    output
}

fn downsample_source_pixel(source: &[u8], x: u32, y: u32, scale: u32) -> [u8; 4] {
    let mut sum = [0_u32; 4];
    for offset_y in 0..scale {
        for offset_x in 0..scale {
            let offset = (((y + offset_y) * SOURCE_ATLAS_WIDTH + x + offset_x) * 4) as usize;
            for channel in 0..4 {
                sum[channel] += u32::from(source[offset + channel]);
            }
        }
    }
    let samples = scale * scale;
    sum.map(|channel| ((channel + samples / 2) / samples) as u8)
}

/// Returns UVs inset by half a texel so filtering cannot sample an adjacent tile.
pub(super) fn atlas_uvs(texture: TextureIndex) -> [[f32; 2]; 4] {
    debug_assert!(u32::from(texture) < ATLAS_COLUMNS * ATLAS_ROWS);

    let tile_x = u32::from(texture) % ATLAS_COLUMNS;
    let tile_y = u32::from(texture) / ATLAS_COLUMNS;
    let half_texel = 0.5;
    let u_min = (tile_x * ATLAS_CELL_SIZE + TILE_GUTTER) as f32 + half_texel;
    let u_max = (tile_x * ATLAS_CELL_SIZE + TILE_GUTTER + TILE_SIZE) as f32 - half_texel;
    let v_min = (tile_y * ATLAS_CELL_SIZE + TILE_GUTTER) as f32 + half_texel;
    let v_max = (tile_y * ATLAS_CELL_SIZE + TILE_GUTTER + TILE_SIZE) as f32 - half_texel;
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
    fn atlas_has_padded_mips_and_minification_filtering() {
        let image = create_block_texture_atlas();

        assert_eq!(image.width(), ATLAS_WIDTH);
        assert_eq!(image.height(), ATLAS_HEIGHT);
        assert_eq!(image.texture_descriptor.mip_level_count, MIP_LEVEL_COUNT);
        let ImageSampler::Descriptor(sampler) = &image.sampler else {
            panic!("atlas must use its explicit minification sampler");
        };
        assert_eq!(sampler.mag_filter, ImageFilterMode::Nearest);
        assert_eq!(sampler.min_filter, ImageFilterMode::Linear);
        assert_eq!(sampler.mipmap_filter, ImageFilterMode::Linear);
        assert_eq!(
            image.data.as_ref().unwrap().len(),
            (0..MIP_LEVEL_COUNT)
                .map(|level| {
                    let width = ATLAS_WIDTH >> level;
                    (width * width * 4) as usize
                })
                .sum::<usize>()
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
    fn every_mip_replicates_tile_edges_into_its_gutter() {
        let image = create_block_texture_atlas();
        let data = image.data.as_ref().unwrap();
        let mut level_offset = 0;

        for level in 0..MIP_LEVEL_COUNT {
            let width = ATLAS_WIDTH >> level;
            let cell = ATLAS_CELL_SIZE >> level;
            let gutter = TILE_GUTTER >> level;
            let inner = TILE_SIZE >> level;
            for tile_y in 0..ATLAS_ROWS {
                for tile_x in 0..ATLAS_COLUMNS {
                    let x = tile_x * cell;
                    let y = tile_y * cell;
                    let sample_y = y + gutter;
                    let sample_x = x + gutter;
                    assert_eq!(
                        mip_pixel(data, level_offset, width, x, sample_y),
                        mip_pixel(data, level_offset, width, sample_x, sample_y)
                    );
                    assert_eq!(
                        mip_pixel(data, level_offset, width, x + cell - 1, sample_y),
                        mip_pixel(data, level_offset, width, sample_x + inner - 1, sample_y)
                    );
                    assert_eq!(
                        mip_pixel(data, level_offset, width, sample_x, y),
                        mip_pixel(data, level_offset, width, sample_x, sample_y)
                    );
                    assert_eq!(
                        mip_pixel(data, level_offset, width, sample_x, y + cell - 1),
                        mip_pixel(data, level_offset, width, sample_x, sample_y + inner - 1)
                    );
                }
            }
            level_offset += (width * width * 4) as usize;
        }
    }

    #[test]
    fn uv_coordinates_stay_half_a_texel_inside_their_cell() {
        let grass_top = atlas_uvs(1);

        assert_eq!(grass_top[0], [80.5 / 256.0, 47.5 / 256.0]);
        assert_eq!(grass_top[2], [111.5 / 256.0, 16.5 / 256.0]);
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
            let atlas_y = tile_y * ATLAS_CELL_SIZE + TILE_GUTTER + y;
            let atlas_x = tile_x * ATLAS_CELL_SIZE + TILE_GUTTER;
            let start = ((atlas_y * ATLAS_WIDTH + atlas_x) * 4) as usize;
            let end = start + (TILE_SIZE * 4) as usize;
            bytes.extend_from_slice(&data[start..end]);
        }

        bytes
    }

    fn mip_pixel(data: &[u8], offset: usize, width: u32, x: u32, y: u32) -> &[u8] {
        let start = offset + ((y * width + x) * 4) as usize;
        &data[start..start + 4]
    }
}
