use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    time::{Duration, Instant},
};

use bevy::{
    light::NotShadowCaster,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};

use crate::{
    block::BlockRegistry,
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, ChunkStorage},
    coordinates::ChunkPos,
    debug::WorldProfiling,
    generation::{
        ChunkGenerationQueue, ChunkLifecycle, ChunkPriority, GenerationSettings, chunk_priority,
    },
    player::{LookState, Player},
};

use super::{
    ChunkMaterial, ChunkMeshingStats, MeshingSettings,
    voxel::{ChunkMeshes, build_chunk_mesh},
};

/// Identifies either render layer belonging to one chunk.
#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkMesh {
    pub position: ChunkPos,
}

#[derive(Debug, Default)]
struct RenderedPart {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
}

#[derive(Debug)]
struct RenderedChunk {
    opaque: RenderedPart,
    water: RenderedPart,
    revision: u64,
}

/// Rendering-only state that maps loaded chunk positions to Bevy objects.
///
/// No entity or asset handle is stored in [`ChunkStorage`] or [`crate::chunk::Chunk`].
#[derive(Debug, Default, Resource)]
pub struct ChunkRenderer {
    rendered: HashMap<ChunkPos, RenderedChunk>,
    last_cleanup_epoch: Option<u64>,
}

struct MeshingResult {
    position: ChunkPos,
    revision: u64,
    seed: u64,
    meshes: ChunkMeshes,
    meshing_elapsed: Duration,
}

/// Owns worker handles. Workers receive immutable owned snapshots and never
/// access ECS resources or Bevy asset collections.
#[derive(Default, Resource)]
pub(super) struct ChunkMeshingTasks {
    running: HashMap<ChunkPos, MeshingJob>,
    completed: HashMap<ChunkPos, MeshingResult>,
}

struct MeshingJob {
    revision: u64,
    seed: u64,
    task: Task<MeshingResult>,
}

#[derive(Default, Resource)]
pub(super) struct ChunkMeshingQueue {
    pending: BinaryHeap<Reverse<ChunkPriority>>,
    storage_epoch: Option<u64>,
    last_priority_center: Option<ChunkPos>,
    last_priority_forward: Vec3,
}

impl ChunkMeshingQueue {
    fn reprioritize(
        &mut self,
        positions: impl IntoIterator<Item = ChunkPos>,
        center: ChunkPos,
        view_forward: Vec3,
    ) {
        self.pending.clear();
        self.pending.extend(
            positions
                .into_iter()
                .map(|position| Reverse(chunk_priority(position, center, view_forward))),
        );
        self.last_priority_center = Some(center);
        self.last_priority_forward = view_forward.normalize_or(Vec3::NEG_Z);
    }

    fn reprioritize_pending(&mut self, center: ChunkPos, view_forward: Vec3) {
        let normalized_forward = view_forward.normalize_or(Vec3::NEG_Z);
        let direction_changed = self.last_priority_forward == Vec3::ZERO
            || self.last_priority_forward.dot(normalized_forward) < 0.996;
        if self.last_priority_center == Some(center) && !direction_changed {
            return;
        }
        let old_pending = std::mem::take(&mut self.pending);
        self.reprioritize(
            old_pending
                .into_iter()
                .map(|Reverse((_, _, _, position))| position),
            center,
            normalized_forward,
        );
    }

    fn invalidate(&mut self) {
        self.storage_epoch = None;
    }

    fn pop(&mut self) -> Option<ChunkPos> {
        self.pending
            .pop()
            .map(|Reverse((_, _, _, position))| position)
    }

    fn len(&self) -> usize {
        self.pending.len()
    }
}

impl ChunkRenderer {
    /// Opaque/cutout layer (water uses a separate entity).
    pub fn entity(&self, position: ChunkPos) -> Option<Entity> {
        self.rendered
            .get(&position)
            .and_then(|chunk| chunk.opaque.entity)
    }

    pub fn mesh(&self, position: ChunkPos) -> Option<&Handle<Mesh>> {
        self.rendered
            .get(&position)
            .and_then(|chunk| chunk.opaque.mesh.as_ref())
    }

    pub fn water_entity(&self, position: ChunkPos) -> Option<Entity> {
        self.rendered
            .get(&position)
            .and_then(|chunk| chunk.water.entity)
    }

    pub fn water_mesh(&self, position: ChunkPos) -> Option<&Handle<Mesh>> {
        self.rendered
            .get(&position)
            .and_then(|chunk| chunk.water.mesh.as_ref())
    }

    pub fn revision(&self, position: ChunkPos) -> Option<u64> {
        self.rendered.get(&position).map(|chunk| chunk.revision)
    }

    pub fn contains(&self, position: ChunkPos) -> bool {
        self.rendered.contains_key(&position)
    }

    pub fn len(&self) -> usize {
        self.rendered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rendered.is_empty()
    }

    /// Total indexed geometry retained in main-world mesh assets.
    pub fn geometry_counts(&self, meshes: &Assets<Mesh>) -> (usize, usize) {
        self.rendered.values().fold((0, 0), |totals, chunk| {
            [&chunk.opaque, &chunk.water]
                .into_iter()
                .filter_map(|part| part.mesh.as_ref())
                .filter_map(|handle| meshes.get(handle))
                .fold(totals, |(vertices, indices), mesh| {
                    (
                        vertices + mesh.count_vertices(),
                        indices + mesh.indices().map_or(0, |indices| indices.len()),
                    )
                })
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn sync_chunk_renderer(
    mut commands: Commands,
    mut storage: ResMut<ChunkStorage>,
    registry: Res<BlockRegistry>,
    generation_settings: Res<GenerationSettings>,
    settings: Res<MeshingSettings>,
    material: Res<ChunkMaterial>,
    mut generation_queue: ResMut<ChunkGenerationQueue>,
    mut renderer: ResMut<ChunkRenderer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut tasks: ResMut<ChunkMeshingTasks>,
    mut priority_queue: ResMut<ChunkMeshingQueue>,
    players: Query<(&Transform, &LookState), With<Player>>,
    mut stats: ResMut<ChunkMeshingStats>,
    mut profiling: Option<ResMut<WorldProfiling>>,
) {
    let (center, view_forward) = players
        .single()
        .map(|(transform, look)| {
            (
                chunk_position_from_translation(transform.translation),
                Quat::from_rotation_y(look.yaw) * Quat::from_rotation_x(look.pitch) * Vec3::NEG_Z,
            )
        })
        .unwrap_or((ChunkPos::default(), Vec3::NEG_Z));

    let storage_epoch = storage.mesh_change_epoch();
    if renderer.last_cleanup_epoch != Some(storage_epoch) {
        despawn_unloaded_chunks(&mut commands, &storage, &mut renderer, &mut meshes);
        renderer.last_cleanup_epoch = Some(storage_epoch);
    }
    if cancel_obsolete_meshing(
        &mut tasks,
        &storage,
        &mut generation_queue,
        generation_settings.seed,
    ) {
        priority_queue.invalidate();
    }
    collect_completed_tasks(&mut tasks, profiling.as_deref_mut());

    let mut completed = tasks.completed.keys().copied().collect::<Vec<_>>();
    completed.sort_unstable_by_key(|position| chunk_priority(*position, center, view_forward));
    for position in completed
        .into_iter()
        .take(settings.max_mesh_uploads_per_frame)
    {
        let result = tasks
            .completed
            .remove(&position)
            .expect("completed position came from the result map");
        if !apply_meshing_result(
            result,
            &mut commands,
            &material,
            &mut meshes,
            &generation_settings,
            &mut storage,
            &mut generation_queue,
            &mut renderer,
        ) {
            priority_queue.invalidate();
        }
    }

    if priority_queue.storage_epoch != Some(storage_epoch) {
        let pending = storage.iter().filter_map(|(position, chunk)| {
            let can_mesh = matches!(
                generation_queue.state(position),
                Some(ChunkLifecycle::Generated | ChunkLifecycle::Ready)
            );
            (can_mesh
                && !tasks.running.contains_key(&position)
                && (!renderer.contains(position) || chunk.is_dirty()))
            .then_some(position)
        });
        priority_queue.reprioritize(pending, center, view_forward);
        priority_queue.storage_epoch = Some(storage_epoch);
    } else {
        priority_queue.reprioritize_pending(center, view_forward);
    }

    let available_slots = settings
        .max_meshing_tasks
        .saturating_sub(tasks.running.len() + tasks.completed.len());
    let start_count = available_slots.min(settings.start_budget_per_frame);
    let pool = AsyncComputeTaskPool::get();

    for _ in 0..start_count {
        let Some(position) = priority_queue.pop() else {
            break;
        };
        if !generation_queue.begin_meshing(position) {
            continue;
        }
        let revision = storage
            .get_chunk(position)
            .expect("meshing candidates must remain loaded during this system")
            .mesh_revision();
        let snapshot = storage.meshing_snapshot(position);
        let registry = registry.clone();
        let seed = generation_settings.seed;
        let task = pool.spawn(async move {
            let started = Instant::now();
            let meshes = build_chunk_mesh(position, &snapshot, &registry, seed);
            MeshingResult {
                position,
                revision,
                seed,
                meshes,
                meshing_elapsed: started.elapsed(),
            }
        });
        let previous = tasks.running.insert(
            position,
            MeshingJob {
                revision,
                seed,
                task,
            },
        );
        debug_assert!(
            previous.is_none(),
            "a chunk must have at most one mesh task"
        );
    }

    stats.queued = priority_queue.len();
    stats.running = tasks.running.len();
    stats.awaiting_upload = tasks.completed.len();
    stats.max_tasks = settings.max_meshing_tasks;
    stats.max_uploads_per_frame = settings.max_mesh_uploads_per_frame;
}

fn collect_completed_tasks(
    tasks: &mut ChunkMeshingTasks,
    mut profiling: Option<&mut WorldProfiling>,
) {
    let completed = tasks
        .running
        .iter_mut()
        .filter_map(|(&position, job)| check_ready(&mut job.task).map(|result| (position, result)))
        .collect::<Vec<_>>();
    for (position, result) in completed {
        tasks.running.remove(&position);
        if let Some(profiling) = profiling.as_deref_mut() {
            profiling.record_meshing(result.meshing_elapsed);
        }
        let previous = tasks.completed.insert(position, result);
        debug_assert!(previous.is_none(), "a chunk has one completed mesh result");
    }
}

fn cancel_obsolete_meshing(
    tasks: &mut ChunkMeshingTasks,
    storage: &ChunkStorage,
    queue: &mut ChunkGenerationQueue,
    seed: u64,
) -> bool {
    let mut obsolete = tasks
        .running
        .iter()
        .filter_map(|(&position, job)| {
            (!meshing_job_is_current(position, job.revision, job.seed, storage, queue, seed))
                .then_some(position)
        })
        .collect::<Vec<_>>();
    obsolete.extend(tasks.completed.iter().filter_map(|(&position, result)| {
        (!meshing_job_is_current(position, result.revision, result.seed, storage, queue, seed))
            .then_some(position)
    }));

    let had_obsolete = !obsolete.is_empty();
    for position in obsolete {
        tasks.running.remove(&position);
        tasks.completed.remove(&position);
        if queue.state(position) == Some(ChunkLifecycle::Meshing) {
            if storage.contains_chunk(position) {
                let transitioned = queue.retry_meshing(position);
                debug_assert!(transitioned, "cancelled mesh must return to Generated");
            } else {
                queue.cancel(position);
            }
        }
    }
    had_obsolete
}

fn meshing_job_is_current(
    position: ChunkPos,
    revision: u64,
    job_seed: u64,
    storage: &ChunkStorage,
    queue: &ChunkGenerationQueue,
    current_seed: u64,
) -> bool {
    job_seed == current_seed
        && queue.state(position) == Some(ChunkLifecycle::Meshing)
        && storage
            .get_chunk(position)
            .is_some_and(|chunk| chunk.mesh_revision() == revision)
}

#[allow(clippy::too_many_arguments)]
fn apply_meshing_result(
    result: MeshingResult,
    commands: &mut Commands,
    material: &ChunkMaterial,
    meshes: &mut Assets<Mesh>,
    settings: &GenerationSettings,
    storage: &mut ChunkStorage,
    queue: &mut ChunkGenerationQueue,
    renderer: &mut ChunkRenderer,
) -> bool {
    let current_revision = storage
        .get_chunk(result.position)
        .map(|chunk| chunk.mesh_revision());
    let is_current = result.seed == settings.seed
        && current_revision == Some(result.revision)
        && queue.state(result.position) == Some(ChunkLifecycle::Meshing);

    if !is_current {
        if queue.state(result.position) == Some(ChunkLifecycle::Meshing) {
            if current_revision.is_some() {
                let transitioned = queue.retry_meshing(result.position);
                debug_assert!(transitioned, "stale mesh must return to Generated");
            } else {
                queue.cancel(result.position);
            }
        }
        return false;
    }

    if let Some(rendered) = renderer.rendered.get_mut(&result.position) {
        apply_rebuilt_mesh(
            commands,
            material,
            meshes,
            result.position,
            rendered,
            result.meshes,
        );
        rendered.revision = result.revision;
    } else {
        let mut rendered = RenderedChunk {
            opaque: RenderedPart::default(),
            water: RenderedPart::default(),
            revision: result.revision,
        };
        apply_rebuilt_mesh(
            commands,
            material,
            meshes,
            result.position,
            &mut rendered,
            result.meshes,
        );
        renderer.rendered.insert(result.position, rendered);
    }

    storage
        .get_chunk_mut(result.position)
        .expect("validated mesh result must still have a loaded chunk")
        .mark_clean();
    let transitioned = queue.finish_meshing(result.position);
    debug_assert!(transitioned, "accepted mesh task must be in Meshing state");
    true
}

fn apply_rebuilt_mesh(
    commands: &mut Commands,
    material: &ChunkMaterial,
    meshes: &mut Assets<Mesh>,
    position: ChunkPos,
    rendered: &mut RenderedChunk,
    rebuilt_mesh: ChunkMeshes,
) {
    apply_part(
        commands,
        &material.0,
        meshes,
        position,
        &mut rendered.opaque,
        rebuilt_mesh.opaque,
        false,
    );
    apply_part(
        commands,
        &material.1,
        meshes,
        position,
        &mut rendered.water,
        rebuilt_mesh.water,
        true,
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_part<M: bevy::pbr::Material>(
    commands: &mut Commands,
    material: &Handle<M>,
    meshes: &mut Assets<Mesh>,
    position: ChunkPos,
    rendered: &mut RenderedPart,
    rebuilt_mesh: Mesh,
    water: bool,
) {
    if rebuilt_mesh.count_vertices() == 0 {
        release_part(commands, meshes, rendered);
        return;
    }

    if let (Some(entity), Some(mesh_handle)) = (rendered.entity, rendered.mesh.clone()) {
        if let Some(mut existing_mesh) = meshes.get_mut(&mesh_handle) {
            *existing_mesh = rebuilt_mesh;
        } else {
            let replacement = meshes.add(rebuilt_mesh);
            commands.entity(entity).insert(Mesh3d(replacement.clone()));
            rendered.mesh = Some(replacement);
        }
        return;
    }

    release_part(commands, meshes, rendered);
    let mesh = meshes.add(rebuilt_mesh);
    let entity = commands
        .spawn((
            Name::new(format!(
                "Voxel {} Chunk ({}, {}, {})",
                if water { "Water" } else { "Opaque" },
                position.x,
                position.y,
                position.z
            )),
            ChunkMesh { position },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(
                (position.x * CHUNK_WIDTH as i32) as f32,
                (position.y * CHUNK_HEIGHT as i32) as f32,
                (position.z * CHUNK_DEPTH as i32) as f32,
            ),
        ))
        .id();
    if water {
        commands.entity(entity).insert(NotShadowCaster);
    }
    rendered.entity = Some(entity);
    rendered.mesh = Some(mesh);
}

fn release_part(commands: &mut Commands, meshes: &mut Assets<Mesh>, rendered: &mut RenderedPart) {
    if let Some(entity) = rendered.entity.take() {
        commands.entity(entity).despawn();
    }
    if let Some(mesh) = rendered.mesh.take() {
        meshes.remove(mesh.id());
    }
}

fn release_render_objects(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    rendered: &mut RenderedChunk,
) {
    release_part(commands, meshes, &mut rendered.opaque);
    release_part(commands, meshes, &mut rendered.water);
}

fn despawn_unloaded_chunks(
    commands: &mut Commands,
    storage: &ChunkStorage,
    renderer: &mut ChunkRenderer,
    meshes: &mut Assets<Mesh>,
) {
    renderer.rendered.retain(|position, rendered| {
        if storage.contains_chunk(*position) {
            true
        } else {
            release_render_objects(commands, meshes, rendered);
            false
        }
    });
}

fn chunk_position_from_translation(translation: Vec3) -> ChunkPos {
    ChunkPos::new(
        (translation.x / CHUNK_WIDTH as f32).floor() as i32,
        (translation.y / CHUNK_HEIGHT as f32).floor() as i32,
        (translation.z / CHUNK_DEPTH as f32).floor() as i32,
    )
}

#[cfg(test)]
mod tests {
    use crate::{block::BlockId, chunk::Chunk, coordinates::WorldBlockPos};

    use super::*;

    fn renderer_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default())
            .init_resource::<Assets<Mesh>>()
            .init_resource::<ChunkStorage>()
            .init_resource::<ChunkGenerationQueue>()
            .init_resource::<ChunkMeshingQueue>()
            .init_resource::<ChunkMeshingTasks>()
            .init_resource::<ChunkRenderer>()
            .init_resource::<ChunkMeshingStats>()
            .insert_resource(BlockRegistry::default())
            .insert_resource(GenerationSettings::default())
            .insert_resource(MeshingSettings {
                max_meshing_tasks: usize::MAX,
                start_budget_per_frame: usize::MAX,
                max_mesh_uploads_per_frame: usize::MAX,
            })
            .insert_resource(ChunkMaterial(Handle::default(), Handle::default()))
            .add_systems(Update, sync_chunk_renderer);
        app
    }

    fn finish_meshing(app: &mut App) {
        for _ in 0..10_000 {
            app.update();
            let no_tasks = {
                let tasks = app.world().resource::<ChunkMeshingTasks>();
                tasks.running.is_empty() && tasks.completed.is_empty()
            };
            let all_loaded_ready = {
                let storage = app.world().resource::<ChunkStorage>();
                let queue = app.world().resource::<ChunkGenerationQueue>();
                storage
                    .iter()
                    .all(|(position, _)| queue.state(position) == Some(ChunkLifecycle::Ready))
            };
            if no_tasks && all_loaded_ready {
                return;
            }
            std::thread::yield_now();
        }
        panic!("meshing workers did not settle");
    }

    fn insert_chunk(app: &mut App, position: ChunkPos, fill: BlockId) {
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .insert_chunk(position, Chunk::new(fill));
        app.world_mut()
            .resource_mut::<ChunkGenerationQueue>()
            .register_generated(position);
    }

    #[test]
    fn mixed_chunk_layers_reuse_assets_and_release_independently() {
        let mut app = renderer_test_app();
        let position = ChunkPos::new(0, 0, 0);
        insert_chunk(&mut app, position, BlockId::AIR);
        let stone = WorldBlockPos::new(2, 2, 2);
        let water = WorldBlockPos::new(3, 2, 2);
        {
            let mut storage = app.world_mut().resource_mut::<ChunkStorage>();
            storage.set_block(stone, BlockId::STONE).unwrap();
            storage.set_block(water, BlockId::WATER).unwrap();
        }
        finish_meshing(&mut app);
        let renderer = app.world().resource::<ChunkRenderer>();
        let opaque_entity = renderer.entity(position).unwrap();
        let water_entity = renderer.water_entity(position).unwrap();
        let water_mesh = renderer.water_mesh(position).unwrap().id();
        assert_ne!(opaque_entity, water_entity);
        assert!(app.world().get::<NotShadowCaster>(water_entity).is_some());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(stone, BlockId::AIR)
            .unwrap();
        finish_meshing(&mut app);
        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.entity(position), None);
        assert_eq!(renderer.water_entity(position), Some(water_entity));
        assert_eq!(renderer.water_mesh(position).unwrap().id(), water_mesh);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(water, BlockId::AIR)
            .unwrap();
        finish_meshing(&mut app);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(water, BlockId::WATER)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(stone, BlockId::STONE)
            .unwrap();
        finish_meshing(&mut app);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .remove_chunk(position);
        finish_meshing(&mut app);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        let world = app.world_mut();
        assert_eq!(world.query::<&ChunkMesh>().iter(world).count(), 0);
    }

    #[test]
    fn water_neighbor_loading_and_unloading_rebuilds_boundary_faces() {
        let mut app = renderer_test_app();
        let left = ChunkPos::new(0, 0, 0);
        let right = ChunkPos::new(1, 0, 0);
        insert_chunk(&mut app, left, BlockId::WATER);
        finish_meshing(&mut app);
        let count = |app: &App| {
            let handle = app
                .world()
                .resource::<ChunkRenderer>()
                .water_mesh(left)
                .unwrap();
            app.world()
                .resource::<Assets<Mesh>>()
                .get(handle)
                .unwrap()
                .count_vertices()
        };
        let exposed = count(&app);
        insert_chunk(&mut app, right, BlockId::WATER);
        finish_meshing(&mut app);
        assert_eq!(exposed - count(&app), CHUNK_HEIGHT * CHUNK_DEPTH * 4);
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .remove_chunk(right);
        finish_meshing(&mut app);
        assert_eq!(count(&app), exposed);
    }

    #[test]
    fn full_4096_voxel_chunk_gets_exactly_one_entity_and_mesh_asset() {
        let mut app = renderer_test_app();
        let position = ChunkPos::new(0, 0, 0);
        insert_chunk(&mut app, position, BlockId::STONE);
        assert_eq!(CHUNK_WIDTH * CHUNK_HEIGHT * CHUNK_DEPTH, 4_096);

        finish_meshing(&mut app);

        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.len(), 1);
        assert!(renderer.entity(position).is_some());
        assert!(renderer.mesh(position).is_some());
        assert_eq!(renderer.revision(position), Some(1));
        assert_eq!(
            renderer.geometry_counts(app.world().resource::<Assets<Mesh>>()),
            (576, 864)
        );
        assert_eq!(
            app.world()
                .resource::<ChunkGenerationQueue>()
                .state(position),
            Some(ChunkLifecycle::Ready)
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        let entity_count = {
            let world = app.world_mut();
            let mut query = world.query::<&ChunkMesh>();
            query.iter(world).count()
        };
        assert_eq!(entity_count, 1);
    }

    #[test]
    fn repeated_dirty_rebuilds_reuse_mesh_asset_and_entity() {
        let mut app = renderer_test_app();
        let position = ChunkPos::new(0, 0, 0);
        insert_chunk(&mut app, position, BlockId::STONE);
        finish_meshing(&mut app);

        let (entity, mesh_id) = {
            let renderer = app.world().resource::<ChunkRenderer>();
            (
                renderer.entity(position).unwrap(),
                renderer.mesh(position).unwrap().id(),
            )
        };

        for (block, expected_revision) in
            [(BlockId::AIR, 2), (BlockId::DIRT, 3), (BlockId::SAND, 4)]
        {
            app.world_mut()
                .resource_mut::<ChunkStorage>()
                .set_block(WorldBlockPos::new(1, 1, 1), block)
                .unwrap();
            finish_meshing(&mut app);

            let renderer = app.world().resource::<ChunkRenderer>();
            assert_eq!(renderer.entity(position), Some(entity));
            assert_eq!(renderer.mesh(position).unwrap().id(), mesh_id);
            assert_eq!(renderer.revision(position), Some(expected_revision));
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        }
    }

    #[test]
    fn border_change_rebuilds_only_changed_chunk_and_face_neighbor() {
        let mut app = renderer_test_app();
        let changed = ChunkPos::new(0, 0, 0);
        let neighbor = ChunkPos::new(1, 0, 0);
        let unrelated = ChunkPos::new(3, 0, 0);
        for position in [changed, neighbor, unrelated] {
            insert_chunk(&mut app, position, BlockId::STONE);
        }
        finish_meshing(&mut app);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(WorldBlockPos::new(15, 1, 1), BlockId::AIR)
            .unwrap();
        finish_meshing(&mut app);

        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.revision(changed), Some(3));
        assert_eq!(renderer.revision(neighbor), Some(2));
        assert_eq!(renderer.revision(unrelated), Some(1));
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 3);
    }

    #[test]
    fn interior_change_rebuilds_only_its_own_chunk() {
        let mut app = renderer_test_app();
        let changed = ChunkPos::new(0, 0, 0);
        let neighbor = ChunkPos::new(1, 0, 0);
        insert_chunk(&mut app, changed, BlockId::STONE);
        insert_chunk(&mut app, neighbor, BlockId::STONE);
        finish_meshing(&mut app);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(WorldBlockPos::new(1, 1, 1), BlockId::AIR)
            .unwrap();
        finish_meshing(&mut app);

        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.revision(changed), Some(3));
        assert_eq!(renderer.revision(neighbor), Some(1));
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
    }

    #[test]
    fn unloaded_chunk_despawns_entity_and_removes_mesh_asset() {
        let mut app = renderer_test_app();
        let position = ChunkPos::new(-1, 0, 2);
        insert_chunk(&mut app, position, BlockId::STONE);
        finish_meshing(&mut app);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .remove_chunk(position);
        finish_meshing(&mut app);

        assert!(app.world().resource::<ChunkRenderer>().is_empty());
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        let entity_count = {
            let world = app.world_mut();
            let mut query = world.query::<&ChunkMesh>();
            query.iter(world).count()
        };
        assert_eq!(entity_count, 0);
    }

    #[test]
    fn empty_chunk_is_tracked_without_gpu_objects_and_can_transition_both_ways() {
        let mut app = renderer_test_app();
        let position = ChunkPos::new(0, 1, 0);
        insert_chunk(&mut app, position, BlockId::AIR);

        finish_meshing(&mut app);

        let renderer = app.world().resource::<ChunkRenderer>();
        assert!(renderer.contains(position));
        assert_eq!(renderer.revision(position), Some(1));
        assert_eq!(renderer.entity(position), None);
        assert_eq!(renderer.mesh(position), None);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(WorldBlockPos::new(1, 17, 1), BlockId::STONE)
            .unwrap();
        finish_meshing(&mut app);
        assert!(
            app.world()
                .resource::<ChunkRenderer>()
                .entity(position)
                .is_some()
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(WorldBlockPos::new(1, 17, 1), BlockId::AIR)
            .unwrap();
        finish_meshing(&mut app);
        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.revision(position), Some(3));
        assert_eq!(renderer.entity(position), None);
        assert_eq!(renderer.mesh(position), None);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    }

    #[test]
    fn stale_worker_job_is_cancelled_and_remeshed_at_latest_revision() {
        let mut app = renderer_test_app();
        let position = ChunkPos::default();
        insert_chunk(&mut app, position, BlockId::STONE);

        // The first update may only schedule CPU work. Mesh assets are owned
        // by the main world and are not created inside the worker.
        app.update();
        assert_eq!(
            app.world()
                .resource::<ChunkGenerationQueue>()
                .state(position),
            Some(ChunkLifecycle::Meshing)
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(WorldBlockPos::new(1, 1, 1), BlockId::AIR)
            .unwrap();
        finish_meshing(&mut app);

        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.revision(position), Some(2));
        assert_eq!(
            app.world()
                .resource::<ChunkGenerationQueue>()
                .state(position),
            Some(ChunkLifecycle::Ready)
        );
    }

    #[test]
    fn meshing_priority_prefers_near_then_forward_chunks() {
        let center = ChunkPos::default();
        let near_behind = ChunkPos::new(0, 0, 1);
        let ahead = ChunkPos::new(0, 0, -2);
        let behind = ChunkPos::new(0, 0, 2);
        let mut queue = ChunkMeshingQueue::default();
        queue.reprioritize([behind, ahead, near_behind], center, Vec3::NEG_Z);

        assert_eq!(queue.pop(), Some(near_behind));
        assert_eq!(queue.pop(), Some(ahead));
        assert_eq!(queue.pop(), Some(behind));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn mesh_upload_budget_limits_ready_chunks_applied_per_frame() {
        let mut app = renderer_test_app();
        *app.world_mut().resource_mut::<MeshingSettings>() = MeshingSettings {
            max_meshing_tasks: 3,
            start_budget_per_frame: 3,
            max_mesh_uploads_per_frame: 1,
        };
        for x in 0..3 {
            insert_chunk(&mut app, ChunkPos::new(x, 0, 0), BlockId::STONE);
        }

        for _ in 0..10_000 {
            let before = app.world().resource::<ChunkRenderer>().len();
            app.update();
            let after = app.world().resource::<ChunkRenderer>().len();
            assert!(after.saturating_sub(before) <= 1);
            let tasks = app.world().resource::<ChunkMeshingTasks>();
            assert!(tasks.running.len() + tasks.completed.len() <= 3);
            if after == 3 && tasks.running.is_empty() && tasks.completed.is_empty() {
                return;
            }
            std::thread::yield_now();
        }
        panic!("upload-budget test did not finish");
    }

    #[test]
    fn unloading_cancels_an_inflight_meshing_job() {
        let mut app = renderer_test_app();
        let position = ChunkPos::default();
        insert_chunk(&mut app, position, BlockId::STONE);
        app.update();
        assert!(
            app.world()
                .resource::<ChunkMeshingTasks>()
                .running
                .contains_key(&position)
        );

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .remove_chunk(position);
        app.update();

        let tasks = app.world().resource::<ChunkMeshingTasks>();
        assert!(!tasks.running.contains_key(&position));
        assert!(!tasks.completed.contains_key(&position));
        assert_eq!(
            app.world()
                .resource::<ChunkGenerationQueue>()
                .state(position),
            None
        );
    }
}
