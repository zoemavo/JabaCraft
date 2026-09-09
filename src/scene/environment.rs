//! Lightweight, stylized outdoor rendering for the voxel world.

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    light::Skybox,
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
};

pub(crate) const SKY_COLOR: Color = Color::srgb(0.48, 0.69, 0.90);

const FOG_START: f32 = 58.0;
const FOG_END: f32 = 138.0;
const SKYBOX_FACE_SIZE: u32 = 64;
const SKYBOX_BRIGHTNESS: f32 = 900.0;

pub(super) fn spawn_environment(images: &mut Assets<Image>) -> Handle<Image> {
    images.add(create_stylized_skybox())
}

pub(super) fn skybox(image: Handle<Image>) -> Skybox {
    Skybox {
        image: Some(image),
        brightness: SKYBOX_BRIGHTNESS,
        ..default()
    }
}

pub(super) fn distance_fog() -> DistanceFog {
    DistanceFog {
        color: Color::srgba(0.52, 0.69, 0.86, 1.0),
        directional_light_color: Color::srgba(1.0, 0.88, 0.68, 0.28),
        directional_light_exponent: 24.0,
        // A cheap linear fade preserves crisp nearby voxels and blends the
        // circular eight-chunk streaming boundary into the sky.
        falloff: FogFalloff::Linear {
            start: FOG_START,
            end: FOG_END,
        },
    }
}

/// Generates a tiny cubemap once at startup. Its color follows the vertical
/// direction, producing a clean blue zenith and a pale horizon without a
/// texture asset, compute shaders, ray marching, or per-frame updates.
fn create_stylized_skybox() -> Image {
    let size = SKYBOX_FACE_SIZE;
    let mut data = Vec::with_capacity((size * size * 6 * 4) as usize);

    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = 2.0 * (x as f32 + 0.5) / size as f32 - 1.0;
                let v = 2.0 * (y as f32 + 0.5) / size as f32 - 1.0;
                let direction = cubemap_direction(face, u, v).normalize();
                data.extend_from_slice(&sky_pixel(direction.y));
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size * 6,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image
        .reinterpret_stacked_2d_as_array(6)
        .expect("procedural skybox contains six equally sized faces");
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::linear();
    image
}

fn cubemap_direction(face: u32, u: f32, v: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        5 => Vec3::new(-u, -v, -1.0),
        _ => unreachable!("a cubemap has exactly six faces"),
    }
}

fn sky_pixel(vertical: f32) -> [u8; 4] {
    const ZENITH: [f32; 3] = [48.0, 105.0, 184.0];
    const HORIZON: [f32; 3] = [146.0, 196.0, 235.0];
    const BELOW_HORIZON: [f32; 3] = [118.0, 164.0, 203.0];

    let (from, to, t) = if vertical >= 0.0 {
        (HORIZON, ZENITH, vertical.powf(0.62))
    } else {
        (HORIZON, BELOW_HORIZON, (-vertical).powf(0.45))
    };
    let mut pixel = [0_u8; 4];
    for channel in 0..3 {
        pixel[channel] = (from[channel] + (to[channel] - from[channel]) * t).round() as u8;
    }
    pixel[3] = 255;
    pixel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_skybox_is_a_small_six_face_cubemap() {
        let image = create_stylized_skybox();

        assert_eq!(image.width(), SKYBOX_FACE_SIZE);
        assert_eq!(image.height(), SKYBOX_FACE_SIZE);
        assert_eq!(image.texture_descriptor.array_layer_count(), 6);
        assert_eq!(
            image
                .texture_view_descriptor
                .as_ref()
                .and_then(|view| view.dimension),
            Some(TextureViewDimension::Cube)
        );
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some((SKYBOX_FACE_SIZE * SKYBOX_FACE_SIZE * 6 * 4) as usize)
        );
    }

    #[test]
    fn fog_starts_well_beyond_interaction_distance() {
        let fog = distance_fog();

        assert!(matches!(
            fog.falloff,
            FogFalloff::Linear { start, end }
                if start == FOG_START && end == FOG_END && start < end
        ));
    }
}
