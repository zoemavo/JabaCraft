//! Camera-only underwater appearance; terrain, lights, and HUD stay independent.

use bevy::{light::Skybox, prelude::*, render::view::ColorGrading};

use crate::{
    chunk::ChunkStorage,
    game_time::GameTime,
    player::{PlayerCamera, point_is_underwater},
};

use super::environment::distance_fog;

#[derive(Component, Default)]
pub(super) struct UnderwaterView {
    blend: f32,
}

pub(super) fn update_underwater_view(
    time: Res<Time>,
    game_time: Res<GameTime>,
    storage: Res<ChunkStorage>,
    mut cameras: Query<
        (
            &GlobalTransform,
            &mut UnderwaterView,
            &mut DistanceFog,
            &mut ColorGrading,
            &mut Skybox,
        ),
        With<PlayerCamera>,
    >,
) {
    for (transform, mut view, mut fog, mut grading, mut skybox) in &mut cameras {
        let submerged = point_is_underwater(transform.translation(), &storage);
        view.blend = advance_blend(view.blend, submerged, time.delta_secs());
        let blend = view.blend;
        *fog = underwater_fog(&game_time, blend);
        // Outdoor environment sets sky brightness once each Update. Apply this
        // multiplier afterwards so it never accumulates between frames.
        skybox.brightness *= 1.0 - 0.55 * blend;
        grading.global.exposure = -0.65 * blend;
        grading.global.temperature = -0.045 * blend;
        grading.global.tint = -0.015 * blend;
        grading.global.post_saturation = 1.0 - 0.12 * blend;
    }
}

fn advance_blend(current: f32, submerged: bool, dt: f32) -> f32 {
    let step = 5.0 * dt.max(0.0);
    if submerged {
        (current + step).min(1.0)
    } else {
        (current - step).max(0.0)
    }
}

fn underwater_fog(game_time: &GameTime, blend: f32) -> DistanceFog {
    let mut fog = distance_fog(game_time);
    fog.color = fog.color.mix(&Color::srgb(0.035, 0.16, 0.23), blend);
    fog.directional_light_color
        .set_alpha(fog.directional_light_color.alpha() * (1.0 - blend));
    if let FogFalloff::Linear { start, end } = &mut fog.falloff {
        *start *= 1.0 - blend;
        *end = *end * (1.0 - blend) + 14.0 * blend;
    }
    fog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfacing_fully_restores_current_outdoor_fog() {
        let time = GameTime::from_elapsed_days(0.7);
        let mut blend = 0.0;
        for _ in 0..30 {
            blend = advance_blend(blend, true, 1.0 / 60.0);
        }
        assert_eq!(blend, 1.0);
        assert!(matches!(
            underwater_fog(&time, blend).falloff,
            FogFalloff::Linear {
                start: 0.0,
                end: 14.0
            }
        ));
        for _ in 0..30 {
            blend = advance_blend(blend, false, 1.0 / 60.0);
        }
        assert_eq!(blend, 0.0);
        let restored = underwater_fog(&time, blend);
        let outdoor = distance_fog(&time);
        assert_eq!(restored.color, outdoor.color);
        match (restored.falloff, outdoor.falloff) {
            (FogFalloff::Linear { start: a, end: b }, FogFalloff::Linear { start: c, end: d }) => {
                assert_eq!((a, b), (c, d));
            }
            _ => panic!("expected linear outdoor fog"),
        }
    }

    #[test]
    fn camera_effect_enters_and_exits_using_camera_position() {
        use crate::{block::BlockId, chunk::Chunk, coordinates::ChunkPos};
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.25));
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(ChunkPos::new(0, 0, 0), Chunk::new(BlockId::WATER));
        app.insert_resource(time)
            .insert_resource(storage)
            .init_resource::<GameTime>()
            .add_systems(Update, update_underwater_view);
        let camera = app
            .world_mut()
            .spawn((
                PlayerCamera,
                GlobalTransform::from_translation(Vec3::splat(4.0)),
                UnderwaterView::default(),
                distance_fog(&GameTime::default()),
                ColorGrading::default(),
                Skybox::default(),
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .get::<ColorGrading>(camera)
                .unwrap()
                .global
                .exposure
                < 0.0
        );
        assert_eq!(
            app.world().get::<UnderwaterView>(camera).unwrap().blend,
            1.0
        );
        app.world_mut()
            .entity_mut(camera)
            .insert(GlobalTransform::from_translation(Vec3::new(4.0, 16.0, 4.0)));
        app.update();
        assert_eq!(
            app.world().get::<UnderwaterView>(camera).unwrap().blend,
            0.0
        );
        assert_eq!(
            app.world()
                .get::<ColorGrading>(camera)
                .unwrap()
                .global
                .exposure,
            0.0
        );
    }
}
