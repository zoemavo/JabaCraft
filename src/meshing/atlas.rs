use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::Image,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::block::TextureIndex;

pub(super) const ATLAS_COLUMNS: u32 = 4;
pub(super) const ATLAS_ROWS: u32 = 4;
pub(super) const TILE_SIZE: u32 = 16;

const ATLAS_WIDTH: u32 = ATLAS_COLUMNS * TILE_SIZE;
const ATLAS_HEIGHT: u32 = ATLAS_ROWS * TILE_SIZE;

/// Builds the original placeholder block atlas directly in memory.
pub(super) fn create_block_texture_atlas() -> Image {
    let mut pixels = Vec::with_capacity((ATLAS_WIDTH * ATLAS_HEIGHT * 4) as usize);

    for atlas_y in 0..ATLAS_HEIGHT {
        for atlas_x in 0..ATLAS_WIDTH {
            let tile_x = atlas_x / TILE_SIZE;
            let tile_y = atlas_y / TILE_SIZE;
            let texture = (tile_y * ATLAS_COLUMNS + tile_x) as TextureIndex;
            let local_x = atlas_x % TILE_SIZE;
            let local_y = atlas_y % TILE_SIZE;
            pixels.extend_from_slice(&placeholder_pixel(texture, local_x, local_y));
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: ATLAS_WIDTH,
            height: ATLAS_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
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

fn placeholder_pixel(texture: TextureIndex, x: u32, y: u32) -> [u8; 4] {
    let noise = pixel_noise(texture, x, y);
    match texture {
        // Air: never rendered.
        0 => [0, 0, 0, 0],
        // Grass top.
        1 => shade([74, 157, 52, 255], noise / 3),
        // Dirt and grass bottom.
        2 => shade([118, 76, 40, 255], noise / 2),
        // Grass side: green turf and irregular roots over dirt.
        3 => {
            if y < 4 || (y < 7 && (x + u32::from(pixel_noise(3, x, 0).unsigned_abs())) % 5 == 0) {
                shade([69, 148, 48, 255], noise / 3)
            } else {
                shade([116, 75, 39, 255], noise / 2)
            }
        }
        // Stone with subtle cracks.
        4 => {
            let base = if (x + y * 3) % 13 == 0 {
                [92, 96, 100, 255]
            } else {
                [126, 130, 134, 255]
            };
            shade(base, noise / 3)
        }
        // Sand with sparse darker grains.
        5 => {
            let base = if (x * 7 + y * 11) % 19 == 0 {
                [193, 175, 108, 255]
            } else {
                [220, 205, 139, 255]
            };
            shade(base, noise / 4)
        }
        // Wood end grain.
        6 => {
            let dx = x.abs_diff(TILE_SIZE / 2);
            let dy = y.abs_diff(TILE_SIZE / 2);
            let ring = (dx + dy) % 4 < 2;
            shade(
                if ring {
                    [151, 101, 48, 255]
                } else {
                    [112, 71, 34, 255]
                },
                noise / 4,
            )
        }
        // Wood bark / side grain.
        7 => {
            let stripe = (x + (y / 4)) % 5 < 2;
            shade(
                if stripe {
                    [104, 66, 31, 255]
                } else {
                    [139, 88, 39, 255]
                },
                noise / 4,
            )
        }
        // Leaves with small transparent gaps.
        8 => {
            if (x * 3 + y * 5) % 17 == 0 {
                [31, 91, 27, 0]
            } else {
                shade([47, 126, 42, 255], noise / 2)
            }
        }
        // Water placeholder.
        9 => shade(
            if y % 5 == 0 {
                [39, 111, 187, 190]
            } else {
                [48, 132, 211, 180]
            },
            noise / 5,
        ),
        // Coal ore over stone.
        10 => {
            if ore_spot(x, y, 3) {
                shade([35, 37, 40, 255], noise / 5)
            } else {
                shade([121, 125, 129, 255], noise / 3)
            }
        }
        // Iron ore over stone.
        11 => {
            if ore_spot(x, y, 7) {
                shade([174, 111, 72, 255], noise / 4)
            } else {
                shade([121, 125, 129, 255], noise / 3)
            }
        }
        // Reserved cells are conspicuous during development.
        _ => {
            if (x / 4 + y / 4) % 2 == 0 {
                [235, 40, 210, 255]
            } else {
                [35, 35, 35, 255]
            }
        }
    }
}

fn pixel_noise(texture: TextureIndex, x: u32, y: u32) -> i16 {
    let value = u32::from(texture)
        .wrapping_mul(1_103)
        .wrapping_add(x.wrapping_mul(1_741))
        .wrapping_add(y.wrapping_mul(2_659))
        .wrapping_add(x.wrapping_mul(y).wrapping_mul(97));
    ((value ^ (value >> 7) ^ (value << 9)) % 25) as i16 - 12
}

fn shade(mut color: [u8; 4], amount: i16) -> [u8; 4] {
    for channel in &mut color[..3] {
        *channel = (i16::from(*channel) + amount).clamp(0, 255) as u8;
    }
    color
}

fn ore_spot(x: u32, y: u32, salt: u32) -> bool {
    let value = x
        .wrapping_mul(13)
        .wrapping_add(y.wrapping_mul(29))
        .wrapping_add(x.wrapping_mul(y).wrapping_mul(3))
        .wrapping_add(salt);
    value % 23 < 5
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

        assert_eq!(grass_top[0], [16.5 / 64.0, 15.5 / 64.0]);
        assert_eq!(grass_top[2], [31.5 / 64.0, 0.5 / 64.0]);
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
