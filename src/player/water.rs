use bevy::prelude::*;

use crate::{block::BlockId, chunk::ChunkStorage, coordinates::WorldBlockPos};

use super::{Player, PlayerCollider, PlayerInWater};

pub(super) fn update_player_water(
    storage: Res<ChunkStorage>,
    mut players: Query<(&Transform, &PlayerCollider, &mut PlayerInWater), With<Player>>,
) {
    for (transform, collider, mut state) in &mut players {
        *state = water_overlap(transform.translation, collider.half_extents, &storage);
    }
}

pub(crate) fn point_is_underwater(point: Vec3, storage: &ChunkStorage) -> bool {
    point.is_finite()
        && storage.get_block(WorldBlockPos::new(
            point.x.floor() as i32,
            point.y.floor() as i32,
            point.z.floor() as i32,
        )) == Some(BlockId::WATER)
}

fn water_overlap(center: Vec3, half: Vec3, storage: &ChunkStorage) -> PlayerInWater {
    if !center.is_finite() || !half.is_finite() || half.min_element() <= 0.0 {
        return PlayerInWater::default();
    }
    let min = center - half;
    let max = center + half;
    let mut volume = 0.0;
    // Exclusive upper bounds avoid counting contact with the surface as immersion.
    // Normal player dimensions touch only a handful of cells, including corners
    // and negative/vertical chunk boundaries.
    for y in min.y.floor() as i32..max.y.ceil() as i32 {
        for z in min.z.floor() as i32..max.z.ceil() as i32 {
            for x in min.x.floor() as i32..max.x.ceil() as i32 {
                if storage.get_block(WorldBlockPos::new(x, y, z)) != Some(BlockId::WATER) {
                    continue;
                }
                let cell_min = Vec3::new(x as f32, y as f32, z as f32);
                let overlap = (max.min(cell_min + Vec3::ONE) - min.max(cell_min)).max(Vec3::ZERO);
                volume += overlap.x * overlap.y * overlap.z;
            }
        }
    }
    PlayerInWater {
        submerged_fraction: (volume / (8.0 * half.x * half.y * half.z)).clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{chunk::Chunk, coordinates::ChunkPos};

    #[test]
    fn body_detects_dry_partial_and_full_immersion_at_negative_chunk_seam() {
        let mut storage = ChunkStorage::default();
        for x in [-1, 0] {
            storage.insert_chunk(ChunkPos::new(x, -1, 0), Chunk::new(BlockId::WATER));
        }
        let half = Vec3::new(0.3, 0.9, 0.3);
        assert!(!water_overlap(Vec3::new(0.0, 0.9, 0.5), half, &storage).is_in_water());
        let partial = water_overlap(Vec3::new(0.0, 0.0, 0.5), half, &storage);
        assert!((partial.submerged_fraction - 0.5).abs() < 0.0001);
        assert!(!partial.is_fully_submerged());
        assert!(water_overlap(Vec3::new(0.0, -1.0, 0.5), half, &storage).is_fully_submerged());
        assert!(!point_is_underwater(Vec3::new(0.0, 0.0, 0.5), &storage));
        assert!(point_is_underwater(Vec3::new(-0.01, -0.01, 0.5), &storage));
    }

    #[test]
    fn missing_chunks_and_non_water_blocks_are_dry() {
        let mut storage = ChunkStorage::default();
        let center = Vec3::splat(4.0);
        assert!(!water_overlap(center, Vec3::splat(0.3), &storage).is_in_water());
        storage.insert_chunk(ChunkPos::new(0, 0, 0), Chunk::new(BlockId::LEAVES));
        assert!(!water_overlap(center, Vec3::splat(0.3), &storage).is_in_water());
    }

    #[test]
    fn water_state_drives_physics_and_clears_when_water_is_removed() {
        use super::super::{
            Grounded, LookState, Noclip, PlayerController, PlayerSettings, Velocity, systems,
        };
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(0.1));
        let mut storage = ChunkStorage::default();
        storage.insert_chunk(ChunkPos::new(0, 0, 0), Chunk::new(BlockId::WATER));
        app.insert_resource(time)
            .insert_resource(storage)
            .init_resource::<PlayerSettings>()
            .add_systems(
                Update,
                (update_player_water, systems::update_velocity).chain(),
            );
        let player = app
            .world_mut()
            .spawn((
                Player,
                Transform::from_translation(Vec3::splat(4.0)),
                PlayerCollider::from_dimensions(0.6, 1.8),
                PlayerController::default(),
                LookState::default(),
                Noclip::default(),
                Velocity(Vec3::new(10.0, -40.0, 0.0)),
                Grounded(false),
            ))
            .id();
        app.update();
        assert!(
            app.world()
                .get::<PlayerInWater>(player)
                .unwrap()
                .is_fully_submerged()
        );
        assert_eq!(app.world().get::<Velocity>(player).unwrap().0.y, -2.5);
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .insert_chunk(ChunkPos::new(0, 0, 0), Chunk::new(BlockId::AIR));
        app.update();
        assert!(
            !app.world()
                .get::<PlayerInWater>(player)
                .unwrap()
                .is_in_water()
        );
        assert!(app.world().get::<Velocity>(player).unwrap().0.y < -4.0);
    }
}
