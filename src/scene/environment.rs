//! Lightweight, stylized outdoor rendering for the voxel world.

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, Skybox},
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
};

use crate::game_time::{DAYLIGHT_FRACTION, GameTime, GameTimeUpdateSet};

pub(crate) const SKY_COLOR: Color = Color::srgb(0.48, 0.69, 0.90);

const FOG_START: f32 = 48.0;
const FOG_END: f32 = 128.0;
const SKYBOX_FACE_SIZE: u32 = 64;
const SKY_UPDATE_STEP: f32 = 1.0 / 720.0;

#[derive(Component)]
pub(super) struct Sun;

#[derive(Resource)]
struct EnvironmentSky(Handle<Image>);

/// A subtle palette tint, independent of the sun and ambient light intensity.
#[derive(Clone, Copy, Debug, Resource)]
pub(crate) struct WorldLightTint(pub [f32; 3]);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) struct EnvironmentUpdateSet;

pub(super) fn install_environment(app: &mut App) {
    let lighting = lighting_at(GameTime::default().day_fraction());
    app.insert_resource(GlobalAmbientLight {
        color: color(lighting.ambient_color),
        brightness: lighting.ambient_brightness,
        affects_lightmapped_meshes: true,
    })
    .insert_resource(DirectionalLightShadowMap { size: 1024 })
    .insert_resource(WorldLightTint(lighting.world_tint))
    .add_systems(
        Update,
        update_environment
            .after(GameTimeUpdateSet)
            .in_set(EnvironmentUpdateSet),
    );
}

pub(super) fn spawn_environment(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    game_time: &GameTime,
) -> Handle<Image> {
    let lighting = lighting_at(game_time.day_fraction());
    let image = images.add(create_stylized_skybox(lighting));
    commands.insert_resource(EnvironmentSky(image.clone()));
    commands.spawn((
        Name::new("Sun"),
        Sun,
        DirectionalLight {
            color: color(lighting.sun_color),
            illuminance: lighting.sun_illuminance,
            // Ambient fill keeps these filtered shadows low contrast. Limit
            // their range to avoid rendering the entire streamed world twice.
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            first_cascade_far_bound: 20.0,
            maximum_distance: 80.0,
            overlap_proportion: 0.2,
            ..default()
        }
        .build(),
        sun_transform(lighting.sun_direction),
    ));
    image
}

pub(super) fn skybox(image: Handle<Image>, game_time: &GameTime) -> Skybox {
    let lighting = lighting_at(game_time.day_fraction());
    Skybox {
        image: Some(image),
        brightness: lighting.sky_brightness,
        ..default()
    }
}

pub(super) fn distance_fog(game_time: &GameTime) -> DistanceFog {
    let lighting = lighting_at(game_time.day_fraction());
    DistanceFog {
        color: color(lighting.fog_color),
        directional_light_color: Color::srgba(
            lighting.sun_color[0],
            lighting.sun_color[1],
            lighting.sun_color[2],
            lighting.daylight * 0.06,
        ),
        directional_light_exponent: 24.0,
        // A cheap linear fade preserves crisp nearby voxels and blends the
        // circular eight-chunk streaming boundary into the sky.
        falloff: FogFalloff::Linear {
            start: FOG_START,
            end: FOG_END,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn update_environment(
    game_time: Res<GameTime>,
    sky_resource: Option<Res<EnvironmentSky>>,
    mut images: ResMut<Assets<Image>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut world_tint: ResMut<WorldLightTint>,
    mut clear_color: ResMut<ClearColor>,
    mut sun: Single<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut cameras: Query<(&mut Skybox, &mut DistanceFog)>,
    mut last_sky_update: Local<Option<f32>>,
) {
    let fraction = game_time.day_fraction();
    let lighting = lighting_at(fraction);

    sun.0.color = color(lighting.sun_color);
    sun.0.illuminance = lighting.sun_illuminance;
    *sun.1 = sun_transform(lighting.sun_direction);

    ambient.color = color(lighting.ambient_color);
    ambient.brightness = lighting.ambient_brightness;
    world_tint.0 = lighting.world_tint;
    clear_color.0 = color(lighting.fog_color);

    for (mut skybox, mut fog) in &mut cameras {
        skybox.brightness = lighting.sky_brightness;
        fog.color = color(lighting.fog_color);
        fog.directional_light_color = Color::srgba(
            lighting.sun_color[0],
            lighting.sun_color[1],
            lighting.sun_color[2],
            lighting.daylight * 0.06,
        );
    }

    let should_refresh_sky = last_sky_update.is_none_or(|last| {
        let distance = (fraction - last).abs();
        distance.min(1.0 - distance) >= SKY_UPDATE_STEP
    });
    if should_refresh_sky {
        if let Some(mut image) = sky_resource
            .as_deref()
            .and_then(|sky| images.get_mut(&sky.0))
        {
            image.data = Some(stylized_skybox_pixels(lighting));
        }
        *last_sky_update = Some(fraction);
    }
}

/// Generates a tiny cubemap at startup and refreshes it at discrete clock steps.
/// Its color follows the vertical
/// direction, producing a clean blue zenith and a pale horizon without a
/// texture asset, compute shaders, ray marching, or per-frame updates.
fn create_stylized_skybox(lighting: LightingState) -> Image {
    let size = SKYBOX_FACE_SIZE;
    let data = stylized_skybox_pixels(lighting);

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

fn stylized_skybox_pixels(lighting: LightingState) -> Vec<u8> {
    let size = SKYBOX_FACE_SIZE;
    let mut data = Vec::with_capacity((size * size * 6 * 4) as usize);
    for face in 0..6 {
        for y in 0..size {
            for x in 0..size {
                let u = 2.0 * (x as f32 + 0.5) / size as f32 - 1.0;
                let v = 2.0 * (y as f32 + 0.5) / size as f32 - 1.0;
                let direction = cubemap_direction(face, u, v).normalize();
                data.extend_from_slice(&sky_pixel(direction, lighting));
            }
        }
    }
    data
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

fn sky_pixel(direction: Vec3, lighting: LightingState) -> [u8; 4] {
    let vertical = direction.y;
    let palette = lighting.sky;
    let (from, to, t) = if vertical >= 0.0 {
        (palette.horizon, palette.zenith, vertical.powf(0.62))
    } else {
        (
            palette.horizon,
            palette.below_horizon,
            (-vertical).powf(0.45),
        )
    };
    let mut rgb = mix3(from, to, t);
    let sun_dot = direction.dot(lighting.sun_direction);
    let sun_glow = smoothstep(0.94, 0.995, sun_dot) * lighting.daylight * 0.10;
    let sun_disc = smoothstep(0.995, 0.9985, sun_dot) * lighting.daylight;
    rgb = mix3(rgb, [1.0, 0.78, 0.40], sun_glow);
    rgb = mix3(rgb, [1.0, 0.94, 0.72], sun_disc);

    let mut pixel = [0_u8; 4];
    for channel in 0..3 {
        pixel[channel] = (rgb[channel] * 255.0).round() as u8;
    }
    pixel[3] = 255;
    pixel
}

#[derive(Clone, Copy, Debug)]
struct SkyPalette {
    zenith: [f32; 3],
    horizon: [f32; 3],
    below_horizon: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
struct LightingState {
    daylight: f32,
    sun_direction: Vec3,
    sun_color: [f32; 3],
    sun_illuminance: f32,
    ambient_color: [f32; 3],
    ambient_brightness: f32,
    sky: SkyPalette,
    sky_brightness: f32,
    fog_color: [f32; 3],
    world_tint: [f32; 3],
}

fn lighting_at(day_fraction: f32) -> LightingState {
    const DAY_SKY: SkyPalette = SkyPalette {
        zenith: [0.188, 0.412, 0.722],
        horizon: [0.573, 0.769, 0.922],
        below_horizon: [0.463, 0.643, 0.796],
    };
    const SUNSET_SKY: SkyPalette = SkyPalette {
        zenith: [0.12, 0.12, 0.32],
        horizon: [0.76, 0.53, 0.43],
        below_horizon: [0.36, 0.30, 0.36],
    };
    const NIGHT_SKY: SkyPalette = SkyPalette {
        zenith: [0.025, 0.045, 0.120],
        horizon: [0.100, 0.130, 0.240],
        below_horizon: [0.040, 0.060, 0.130],
    };

    let day_fraction = day_fraction.rem_euclid(1.0);
    let night_fraction = 1.0 - DAYLIGHT_FRACTION;
    let angle = if day_fraction < DAYLIGHT_FRACTION {
        std::f32::consts::PI * day_fraction / DAYLIGHT_FRACTION
    } else {
        std::f32::consts::PI
            + std::f32::consts::PI * (day_fraction - DAYLIGHT_FRACTION) / night_fraction
    };
    let sun_height = angle.sin();
    // A wide horizon blend keeps dawn and dusk gradual instead of snapping
    // between the day and night palettes in a few frames.
    let daylight = smoothstep(-0.38, 0.38, sun_height);
    let twilight = (1.0 - sun_height.abs() / 0.52).clamp(0.0, 1.0);
    let sunset_mix = twilight * (1.0 - daylight * 0.35);
    let sky = mix_palette(
        mix_palette(NIGHT_SKY, DAY_SKY, daylight),
        SUNSET_SKY,
        sunset_mix,
    );
    let sun_direction =
        Vec3::new(-angle.cos() * 0.88, sun_height, angle.cos() * 0.36).normalize_or_zero();
    let sun_color = mix3([1.0, 0.48, 0.24], [1.0, 0.94, 0.82], daylight);

    LightingState {
        daylight,
        sun_direction,
        sun_color,
        // A modest key light over broad ambient fill gives readable voxel
        // shapes without inky shadows or glaring sun-facing surfaces.
        sun_illuminance: 1_200.0 * smoothstep(0.0, 0.35, sun_height),
        ambient_color: mix3([0.48, 0.56, 0.72], [0.82, 0.87, 0.95], daylight),
        ambient_brightness: lerp(260.0, 2_400.0, daylight),
        sky,
        sky_brightness: lerp(220.0, 900.0, daylight),
        fog_color: sky.horizon,
        // Intensity already follows the clock through the actual lights;
        // avoid multiplying a second night-time blackout into the material.
        world_tint: mix3([0.88, 0.92, 1.0], [1.0, 1.0, 1.0], daylight),
    }
}

fn sun_transform(direction: Vec3) -> Transform {
    let up = if direction.dot(Vec3::Y).abs() > 0.98 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    Transform::from_translation(direction * 100.0).looking_at(Vec3::ZERO, up)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t.clamp(0.0, 1.0)
}

fn mix3(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|channel| lerp(from[channel], to[channel], t))
}

fn mix_palette(from: SkyPalette, to: SkyPalette, t: f32) -> SkyPalette {
    SkyPalette {
        zenith: mix3(from.zenith, to.zenith, t),
        horizon: mix3(from.horizon, to.horizon, t),
        below_horizon: mix3(from.below_horizon, to.below_horizon, t),
    }
}

fn color(rgb: [f32; 3]) -> Color {
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_skybox_is_a_small_six_face_cubemap() {
        let image = create_stylized_skybox(lighting_at(0.25));

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
        let fog = distance_fog(&GameTime::default());

        assert!(matches!(
            fog.falloff,
            FogFalloff::Linear { start, end }
                if start == FOG_START && end == FOG_END && start < end
        ));
    }

    #[test]
    fn noon_is_bright_and_midnight_is_dark() {
        let noon = lighting_at(DAYLIGHT_FRACTION * 0.5);
        let midnight = lighting_at(DAYLIGHT_FRACTION + (1.0 - DAYLIGHT_FRACTION) * 0.5);

        assert!(noon.sun_direction.y > 0.99);
        assert!(midnight.sun_direction.y < -0.99);
        assert!(noon.sun_illuminance > midnight.sun_illuminance);
        assert!(noon.ambient_brightness > midnight.ambient_brightness);
        assert!(noon.sky_brightness > midnight.sky_brightness);
        assert!(noon.world_tint[0] > midnight.world_tint[0]);
    }

    #[test]
    fn sunset_changes_smoothly_across_the_day_night_boundary() {
        let before = lighting_at(DAYLIGHT_FRACTION - 0.001);
        let after = lighting_at(DAYLIGHT_FRACTION + 0.001);

        // These samples are about 1.44 real seconds apart on either side of
        // sunset, so even the faster night arc must remain visually gradual.
        assert!((before.sky_brightness - after.sky_brightness).abs() < 25.0);
        assert!((before.world_tint[2] - after.world_tint[2]).abs() < 0.02);
        assert!((before.fog_color[0] - after.fog_color[0]).abs() < 0.02);
    }

    #[test]
    fn fog_matches_the_sky_horizon_throughout_the_cycle() {
        for step in 0..=96 {
            let lighting = lighting_at(step as f32 / 96.0);
            assert_eq!(lighting.fog_color, lighting.sky.horizon);
            assert!(lighting.sun_illuminance.is_finite());
            if lighting.sun_direction.y <= 0.0 {
                assert_eq!(lighting.sun_illuminance, 0.0);
            }
        }
    }
}
