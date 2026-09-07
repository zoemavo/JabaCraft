use std::collections::{HashMap, HashSet};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};

use crate::{
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk, ChunkStorage},
    coordinates::ChunkPos,
    player::Player,
};

use super::{
    CaveSettings, ChunkGenerationQueue, ChunkLifecycle, GenerationSettings,
    terrain::generate_terrain_chunk,
};

#[derive(Clone, Copy, Debug)]
struct ChunkGenerationInput {
    position: ChunkPos,
    seed: u64,
    cave_settings: CaveSettings,
}

#[derive(Debug)]
struct GeneratedChunk {
    position: ChunkPos,
    seed: u64,
    cave_settings: CaveSettings,
    chunk: Chunk,
}

/// Owns task handles only; workers never receive this resource or access the ECS world.
#[derive(Default, Resource)]
pub(super) struct ChunkGenerationTasks {
    running: HashMap<ChunkPos, Task<GeneratedChunk>>,
}

pub(super) fn stream_chunks_around_player(
    players: Query<&Transform, With<Player>>,
    settings: Res<GenerationSettings>,
    cave_settings: Res<CaveSettings>,
    mut queue: ResMut<ChunkGenerationQueue>,
    mut tasks: ResMut<ChunkGenerationTasks>,
    mut storage: ResMut<ChunkStorage>,
) {
    let Ok(player_transform) = players.single() else {
        return;
    };

    let center = chunk_position_from_translation(player_transform.translation);
    let required = required_chunk_positions(center, &settings);
    let mut update = update_requests(&mut storage, &mut queue, &tasks, &required);

    for result in take_completed_tasks(&mut tasks) {
        if apply_generation_result(
            result,
            &required,
            &settings,
            &cave_settings,
            &mut storage,
            &mut queue,
        ) {
            update.applied += 1;
        } else {
            update.discarded += 1;
        }
    }

    queue.reprioritize(center);
    let available_slots = settings
        .max_concurrent_generation_jobs
        .saturating_sub(tasks.running.len());
    let pool = AsyncComputeTaskPool::get();

    for _ in 0..available_slots {
        let Some(position) = queue.take_next_generation() else {
            break;
        };
        let input = ChunkGenerationInput {
            position,
            seed: settings.seed,
            cave_settings: *cave_settings,
        };
        let task = pool.spawn(async move { generate_chunk(input) });
        let previous = tasks.running.insert(position, task);
        debug_assert!(previous.is_none(), "a chunk must have at most one task");
        update.started += 1;
    }

    if update.has_activity() {
        debug!(
            "Chunk streaming at ({}, {}, {}): requested {}, started {}, applied {}, discarded {}, unloaded {}, pending {}, running {}, stored {}",
            center.x,
            center.y,
            center.z,
            update.requested,
            update.started,
            update.applied,
            update.discarded,
            update.unloaded,
            queue.pending_count(),
            tasks.running.len(),
            storage.iter().count()
        );
    }
}

fn generate_chunk(input: ChunkGenerationInput) -> GeneratedChunk {
    GeneratedChunk {
        position: input.position,
        seed: input.seed,
        cave_settings: input.cave_settings,
        chunk: generate_terrain_chunk(input.position, input.seed, input.cave_settings),
    }
}

fn take_completed_tasks(tasks: &mut ChunkGenerationTasks) -> Vec<GeneratedChunk> {
    let completed = tasks
        .running
        .values_mut()
        .filter_map(check_ready)
        .collect::<Vec<_>>();
    for result in &completed {
        tasks.running.remove(&result.position);
    }
    completed
}

fn apply_generation_result(
    result: GeneratedChunk,
    required: &HashSet<ChunkPos>,
    settings: &GenerationSettings,
    cave_settings: &CaveSettings,
    storage: &mut ChunkStorage,
    queue: &mut ChunkGenerationQueue,
) -> bool {
    let still_expected = required.contains(&result.position)
        && result.seed == settings.seed
        && result.cave_settings == *cave_settings
        && queue.state(result.position) == Some(ChunkLifecycle::Generating)
        && !storage.contains_chunk(result.position);

    if still_expected {
        storage.insert_chunk(result.position, result.chunk);
        let transitioned = queue.finish_generation(result.position);
        debug_assert!(transitioned, "completed task must be in Generating state");
        return true;
    }

    // Never let an obsolete worker overwrite newer data. Requeue only if the
    // position is still wanted and no other producer supplied it.
    queue.cancel(result.position);
    if storage.contains_chunk(result.position) {
        queue.register_generated(result.position);
    } else if required.contains(&result.position) {
        queue.request(result.position);
    }
    false
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StreamingUpdate {
    requested: usize,
    started: usize,
    applied: usize,
    discarded: usize,
    unloaded: usize,
}

impl StreamingUpdate {
    fn has_activity(self) -> bool {
        self != Self::default()
    }
}

fn update_requests(
    storage: &mut ChunkStorage,
    queue: &mut ChunkGenerationQueue,
    tasks: &ChunkGenerationTasks,
    required: &HashSet<ChunkPos>,
) -> StreamingUpdate {
    let tracked = queue
        .positions()
        .chain(storage.iter().map(|(position, _)| position))
        .collect::<HashSet<_>>();
    let mut update = StreamingUpdate::default();

    for position in tracked {
        if required.contains(&position) {
            continue;
        }

        // Keep an in-flight state until its worker finishes. This prevents a
        // second task if the player leaves and quickly re-enters the area.
        if queue.state(position) != Some(ChunkLifecycle::Generating) {
            queue.cancel(position);
        }
        if storage.remove_chunk(position).is_some() {
            update.unloaded += 1;
        }
    }

    for &position in required {
        if storage.contains_chunk(position) {
            queue.register_generated(position);
        } else if queue.state(position) == Some(ChunkLifecycle::Generating)
            && !tasks.running.contains_key(&position)
        {
            // Recover from an externally dropped task instead of leaving the
            // lifecycle permanently stuck in Generating.
            queue.cancel(position);
            if queue.request(position) {
                update.requested += 1;
            }
        } else if queue.request(position) {
            update.requested += 1;
        }
    }

    update
}

fn required_chunk_positions(center: ChunkPos, settings: &GenerationSettings) -> HashSet<ChunkPos> {
    let radius = settings.render_distance_chunks.max(0);
    let Some((min_chunk_y, max_chunk_y)) = vertical_chunk_bounds(settings) else {
        return HashSet::new();
    };
    let radius_squared = i64::from(radius).pow(2);
    let mut required = HashSet::new();

    for chunk_y in min_chunk_y..=max_chunk_y {
        for offset_z in -radius..=radius {
            for offset_x in -radius..=radius {
                let horizontal_distance_squared =
                    i64::from(offset_x).pow(2) + i64::from(offset_z).pow(2);
                if horizontal_distance_squared <= radius_squared {
                    required.insert(ChunkPos::new(
                        center.x + offset_x,
                        chunk_y,
                        center.z + offset_z,
                    ));
                }
            }
        }
    }

    required
}

fn vertical_chunk_bounds(settings: &GenerationSettings) -> Option<(i32, i32)> {
    if settings.world_min_y >= settings.world_max_y {
        return None;
    }

    let chunk_height = CHUNK_HEIGHT as i32;
    let min_chunk_y = settings.world_min_y.div_euclid(chunk_height);
    let max_chunk_y = (settings.world_max_y - 1).div_euclid(chunk_height);
    Some((min_chunk_y, max_chunk_y))
}

fn chunk_position_from_translation(translation: Vec3) -> ChunkPos {
    ChunkPos::new(
        world_coordinate_to_chunk(translation.x, CHUNK_WIDTH),
        world_coordinate_to_chunk(translation.y, CHUNK_HEIGHT),
        world_coordinate_to_chunk(translation.z, CHUNK_DEPTH),
    )
}

fn world_coordinate_to_chunk(coordinate: f32, chunk_size: usize) -> i32 {
    (coordinate / chunk_size as f32).floor() as i32
}

#[cfg(test)]
mod tests {
    use crate::{block::BlockId, coordinates::WorldBlockPos};

    use super::*;

    fn settings(
        radius: i32,
        world_min_y: i32,
        world_max_y: i32,
        concurrent: usize,
    ) -> GenerationSettings {
        GenerationSettings {
            seed: 42,
            render_distance_chunks: radius,
            world_min_y,
            world_max_y,
            max_concurrent_generation_jobs: concurrent,
        }
    }

    fn begin_generation(queue: &mut ChunkGenerationQueue, position: ChunkPos) {
        assert!(queue.request(position));
        queue.reprioritize(position);
        assert_eq!(queue.take_next_generation(), Some(position));
    }

    fn generation_input(
        position: ChunkPos,
        seed: u64,
        cave_settings: CaveSettings,
    ) -> ChunkGenerationInput {
        ChunkGenerationInput {
            position,
            seed,
            cave_settings,
        }
    }

    #[test]
    fn default_streaming_window_has_requested_radius_and_bounded_concurrency() {
        let settings = GenerationSettings::default();
        let center = ChunkPos::new(4, 2, -3);
        let required = required_chunk_positions(center, &settings);

        assert_eq!(settings.render_distance_chunks, 8);
        assert_eq!(settings.max_concurrent_generation_jobs, 4);
        assert_eq!(settings.world_min_y, -64);
        assert_eq!(settings.world_max_y, 192);
        assert_eq!(required.len(), 197 * 16);
        assert!(required.contains(&ChunkPos::new(12, -4, -3)));
        assert!(required.contains(&ChunkPos::new(12, 0, -3)));
        assert!(required.contains(&ChunkPos::new(12, 11, -3)));
        assert!(!required.contains(&ChunkPos::new(12, 2, 5)));
        assert!(!required.contains(&ChunkPos::new(4, 12, -3)));
    }

    #[test]
    fn world_bounds_convert_to_inclusive_vertical_chunk_range() {
        let default_settings = GenerationSettings::default();
        assert_eq!(vertical_chunk_bounds(&default_settings), Some((-4, 11)));

        let unaligned = settings(0, -63, 193, 1);
        assert_eq!(vertical_chunk_bounds(&unaligned), Some((-4, 12)));
    }

    #[test]
    fn invalid_vertical_world_bounds_request_no_chunks() {
        let required = required_chunk_positions(ChunkPos::default(), &settings(8, 192, -64, 4));

        assert!(required.is_empty());
    }

    #[test]
    fn negative_world_positions_map_to_negative_chunks() {
        assert_eq!(
            chunk_position_from_translation(Vec3::new(-0.01, -0.01, -16.01)),
            ChunkPos::new(-1, -1, -2)
        );
        assert_eq!(
            chunk_position_from_translation(Vec3::new(15.99, 16.0, 31.99)),
            ChunkPos::new(0, 1, 1)
        );
    }

    #[test]
    fn required_positions_use_horizontal_distance_not_a_square() {
        let center = ChunkPos::new(-5, 0, 7);
        let required = required_chunk_positions(center, &settings(2, 0, 16, 1));

        assert_eq!(required.len(), 13);
        assert!(required.contains(&ChunkPos::new(-3, 0, 7)));
        assert!(required.contains(&ChunkPos::new(-4, 0, 8)));
        assert!(!required.contains(&ChunkPos::new(-3, 0, 9)));
    }

    #[test]
    fn worker_input_and_output_preserve_position_and_seed() {
        let input = generation_input(ChunkPos::new(-3, 0, 5), 987, CaveSettings::default());

        let result = generate_chunk(input);

        assert_eq!(result.position, input.position);
        assert_eq!(result.seed, input.seed);
        assert_eq!(result.cave_settings, input.cave_settings);
        assert!(!result.chunk.is_all_air());
    }

    #[test]
    fn completed_result_is_applied_only_in_main_world_step() {
        let position = ChunkPos::default();
        let settings = settings(0, 0, 16, 1);
        let required = HashSet::from([position]);
        let mut storage = ChunkStorage::default();
        let mut queue = ChunkGenerationQueue::default();
        let cave_settings = CaveSettings::default();
        begin_generation(&mut queue, position);
        let result = generate_chunk(generation_input(position, settings.seed, cave_settings));

        assert!(apply_generation_result(
            result,
            &required,
            &settings,
            &cave_settings,
            &mut storage,
            &mut queue
        ));
        assert!(storage.contains_chunk(position));
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Generated));
    }

    #[test]
    fn result_is_discarded_when_chunk_is_no_longer_required() {
        let position = ChunkPos::new(8, 0, 8);
        let settings = settings(0, 0, 16, 1);
        let mut storage = ChunkStorage::default();
        let mut queue = ChunkGenerationQueue::default();
        let cave_settings = CaveSettings::default();
        begin_generation(&mut queue, position);
        let result = generate_chunk(generation_input(position, settings.seed, cave_settings));

        assert!(!apply_generation_result(
            result,
            &HashSet::new(),
            &settings,
            &cave_settings,
            &mut storage,
            &mut queue
        ));
        assert!(!storage.contains_chunk(position));
        assert_eq!(queue.state(position), None);
    }

    #[test]
    fn stale_seed_result_is_requeued_without_overwriting_data() {
        let position = ChunkPos::default();
        let settings = settings(0, 0, 16, 1);
        let required = HashSet::from([position]);
        let mut storage = ChunkStorage::default();
        let mut queue = ChunkGenerationQueue::default();
        let cave_settings = CaveSettings::default();
        begin_generation(&mut queue, position);
        let stale = generate_chunk(generation_input(position, settings.seed + 1, cave_settings));

        assert!(!apply_generation_result(
            stale,
            &required,
            &settings,
            &cave_settings,
            &mut storage,
            &mut queue
        ));
        assert!(!storage.contains_chunk(position));
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Requested));
    }

    #[test]
    fn existing_chunk_is_never_overwritten_by_worker_result() {
        let position = ChunkPos::default();
        let settings = settings(0, 0, 16, 1);
        let required = HashSet::from([position]);
        let mut storage = ChunkStorage::default();
        let mut queue = ChunkGenerationQueue::default();
        let cave_settings = CaveSettings::default();
        begin_generation(&mut queue, position);
        storage.insert_chunk(position, Chunk::new(BlockId::IRON_ORE));
        let result = generate_chunk(generation_input(position, settings.seed, cave_settings));

        assert!(!apply_generation_result(
            result,
            &required,
            &settings,
            &cave_settings,
            &mut storage,
            &mut queue
        ));
        assert_eq!(
            storage.get_block(WorldBlockPos::new(0, 0, 0)),
            Some(BlockId::IRON_ORE)
        );
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Generated));
    }

    #[test]
    fn stale_cave_settings_result_is_requeued() {
        let position = ChunkPos::default();
        let settings = settings(0, 0, 16, 1);
        let cave_settings = CaveSettings::default();
        let required = HashSet::from([position]);
        let mut storage = ChunkStorage::default();
        let mut queue = ChunkGenerationQueue::default();
        begin_generation(&mut queue, position);
        let stale = generate_chunk(generation_input(
            position,
            settings.seed,
            CaveSettings {
                threshold: cave_settings.threshold + 0.1,
                ..cave_settings
            },
        ));

        assert!(!apply_generation_result(
            stale,
            &required,
            &settings,
            &cave_settings,
            &mut storage,
            &mut queue
        ));
        assert!(!storage.contains_chunk(position));
        assert_eq!(queue.state(position), Some(ChunkLifecycle::Requested));
    }

    #[test]
    fn leaving_window_preserves_generating_state_for_inflight_result() {
        let position = ChunkPos::new(5, 0, 5);
        let required = HashSet::new();
        let mut storage = ChunkStorage::default();
        let mut queue = ChunkGenerationQueue::default();
        let tasks = ChunkGenerationTasks::default();
        begin_generation(&mut queue, position);

        update_requests(&mut storage, &mut queue, &tasks, &required);

        assert_eq!(queue.state(position), Some(ChunkLifecycle::Generating));
        assert_eq!(queue.generating_count(), 1);
    }
}
