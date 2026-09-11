use std::collections::{HashMap, HashSet};

use bevy::{light::NotShadowCaster, prelude::*};

use crate::{
    block::BlockRegistry,
    chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, ChunkStorage},
    coordinates::ChunkPos,
    generation::{ChunkGenerationQueue, ChunkLifecycle, GenerationSettings},
};

use super::{
    ChunkMaterial, MeshingSettings,
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
}

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
) {
    despawn_unloaded_chunks(&mut commands, &storage, &mut renderer, &mut meshes);

    let mut pending = storage
        .iter()
        .filter_map(|(position, chunk)| {
            let can_mesh = matches!(
                generation_queue.state(position),
                Some(ChunkLifecycle::Generated | ChunkLifecycle::Ready)
            );
            (can_mesh && (!renderer.contains(position) || chunk.is_dirty())).then_some(position)
        })
        .collect::<Vec<_>>();
    pending.sort_unstable_by_key(distance_from_origin_squared);

    for position in pending.into_iter().take(settings.rebuild_budget_per_frame) {
        if !generation_queue.begin_meshing(position) {
            continue;
        }
        let rebuilt_mesh =
            build_chunk_mesh(position, &storage, &registry, generation_settings.seed);

        if let Some(rendered) = renderer.rendered.get_mut(&position) {
            apply_rebuilt_mesh(
                &mut commands,
                &material,
                &mut meshes,
                position,
                rendered,
                rebuilt_mesh,
            );
            rendered.revision = rendered.revision.saturating_add(1);
        } else {
            let mut rendered = RenderedChunk {
                opaque: RenderedPart::default(),
                water: RenderedPart::default(),
                revision: 1,
            };
            apply_rebuilt_mesh(
                &mut commands,
                &material,
                &mut meshes,
                position,
                &mut rendered,
                rebuilt_mesh,
            );
            renderer.rendered.insert(position, rendered);
        }

        if let Some(chunk) = storage.get_chunk_mut(position) {
            chunk.mark_clean();
        }
        let transitioned = generation_queue.finish_meshing(position);
        debug_assert!(transitioned, "meshing job must be in Meshing state");
    }
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
fn apply_part(
    commands: &mut Commands,
    material: &Handle<StandardMaterial>,
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
    let loaded = storage
        .iter()
        .map(|(position, _)| position)
        .collect::<HashSet<_>>();
    renderer.rendered.retain(|position, rendered| {
        if loaded.contains(position) {
            true
        } else {
            release_render_objects(commands, meshes, rendered);
            false
        }
    });
}

fn distance_from_origin_squared(position: &ChunkPos) -> i64 {
    i64::from(position.x).pow(2) + i64::from(position.y).pow(2) + i64::from(position.z).pow(2)
}

#[cfg(test)]
mod tests {
    use crate::{block::BlockId, chunk::Chunk, coordinates::WorldBlockPos};

    use super::*;

    fn renderer_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<ChunkStorage>()
            .init_resource::<ChunkGenerationQueue>()
            .init_resource::<ChunkRenderer>()
            .insert_resource(BlockRegistry::default())
            .insert_resource(GenerationSettings::default())
            .insert_resource(MeshingSettings {
                rebuild_budget_per_frame: usize::MAX,
            })
            .insert_resource(ChunkMaterial(Handle::default(), Handle::default()))
            .add_systems(Update, sync_chunk_renderer);
        app
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
        app.update();
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
        app.update();
        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.entity(position), None);
        assert_eq!(renderer.water_entity(position), Some(water_entity));
        assert_eq!(renderer.water_mesh(position).unwrap().id(), water_mesh);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(water, BlockId::AIR)
            .unwrap();
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(water, BlockId::WATER)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(stone, BlockId::STONE)
            .unwrap();
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .remove_chunk(position);
        app.update();
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
        app.update();
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
        app.update();
        assert_eq!(exposed - count(&app), CHUNK_HEIGHT * CHUNK_DEPTH * 4);
        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .remove_chunk(right);
        app.update();
        assert_eq!(count(&app), exposed);
    }

    #[test]
    fn loaded_chunk_gets_exactly_one_entity_and_mesh_asset() {
        let mut app = renderer_test_app();
        let position = ChunkPos::new(0, 0, 0);
        insert_chunk(&mut app, position, BlockId::STONE);

        app.update();

        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.len(), 1);
        assert!(renderer.entity(position).is_some());
        assert!(renderer.mesh(position).is_some());
        assert_eq!(renderer.revision(position), Some(1));
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
        app.update();

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
            app.update();

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
        app.update();

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(WorldBlockPos::new(15, 1, 1), BlockId::AIR)
            .unwrap();
        app.update();

        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.revision(changed), Some(2));
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
        app.update();

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .set_block(WorldBlockPos::new(1, 1, 1), BlockId::AIR)
            .unwrap();
        app.update();

        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.revision(changed), Some(2));
        assert_eq!(renderer.revision(neighbor), Some(1));
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
    }

    #[test]
    fn unloaded_chunk_despawns_entity_and_removes_mesh_asset() {
        let mut app = renderer_test_app();
        let position = ChunkPos::new(-1, 0, 2);
        insert_chunk(&mut app, position, BlockId::STONE);
        app.update();

        app.world_mut()
            .resource_mut::<ChunkStorage>()
            .remove_chunk(position);
        app.update();

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

        app.update();
        app.update();

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
        app.update();
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
        app.update();
        let renderer = app.world().resource::<ChunkRenderer>();
        assert_eq!(renderer.revision(position), Some(3));
        assert_eq!(renderer.entity(position), None);
        assert_eq!(renderer.mesh(position), None);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
    }
}
