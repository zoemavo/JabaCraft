//! Toggleable F3 diagnostics and lightweight world-pipeline timings.

use std::{collections::HashSet, fmt::Write, mem::size_of, time::Duration};

use bevy::{camera::visibility::ViewVisibility, prelude::*};

use crate::{
    block::{BlockId, BlockRegistry},
    chunk::{CHUNK_VOLUME, ChunkStorage},
    coordinates::WorldBlockPos,
    game::GameState,
    generation::{BiomeSampler, ChunkStreamingStats, GenerationSettings},
    interaction::CameraRaycast,
    meshing::{ChunkMesh, ChunkMeshingStats, ChunkRenderer},
    player::{LookState, Player},
    ui::UiSettings,
};

const REFRESH_SECONDS: f32 = 0.25;
const MESH_VERTEX_BYTES: usize = 12 + 12 + 8 + 16 + 8;

#[derive(Clone, Copy, Debug, Default)]
pub struct TimingStats {
    samples: u64,
    total: Duration,
    last: Duration,
    max: Duration,
}

impl TimingStats {
    fn record(&mut self, elapsed: Duration) {
        self.samples = self.samples.saturating_add(1);
        self.total = self.total.saturating_add(elapsed);
        self.last = elapsed;
        self.max = self.max.max(elapsed);
    }

    pub fn samples(self) -> u64 {
        self.samples
    }

    pub fn average_ms(self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.total.as_secs_f64() * 1_000.0 / self.samples as f64
        }
    }

    pub fn last_ms(self) -> f64 {
        self.last.as_secs_f64() * 1_000.0
    }

    pub fn max_ms(self) -> f64 {
        self.max.as_secs_f64() * 1_000.0
    }
}

/// CPU timings reported by background workers and main-world chunk application.
#[derive(Debug, Default, Resource)]
pub struct WorldProfiling {
    pub terrain_generation: TimingStats,
    pub meshing: TimingStats,
    pub chunk_loading: TimingStats,
}

impl WorldProfiling {
    pub(crate) fn record_terrain_generation(&mut self, elapsed: Duration) {
        self.terrain_generation.record(elapsed);
    }

    pub(crate) fn record_meshing(&mut self, elapsed: Duration) {
        self.meshing.record(elapsed);
    }

    pub(crate) fn record_chunk_loading(&mut self, elapsed: Duration) {
        self.chunk_loading.record(elapsed);
    }
}

#[derive(Component)]
struct DebugOverlay;

#[derive(Resource, Default)]
struct FrameStats {
    elapsed: f32,
    frames: u32,
    fps: f32,
    frame_ms: f32,
    revision: u64,
}

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldProfiling>()
            .init_resource::<FrameStats>()
            .add_systems(
                PreUpdate,
                toggle_debug_overlay.run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                sample_frame_time
                    .run_if(in_state(GameState::Playing))
                    .run_if(debug_overlay_enabled),
            )
            .add_systems(
                Last,
                refresh_debug_overlay
                    .run_if(in_state(GameState::Playing))
                    .run_if(debug_overlay_enabled),
            )
            .add_systems(OnExit(GameState::Playing), disable_debug_overlay)
            .add_systems(OnEnter(GameState::Loading), reset_world_profiling);
    }
}

fn debug_overlay_enabled(settings: Res<UiSettings>) -> bool {
    settings.show_debug_overlay
}

fn toggle_debug_overlay(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut settings: ResMut<UiSettings>,
    roots: Query<Entity, With<DebugOverlay>>,
) {
    if !keyboard.just_pressed(KeyCode::F3) {
        return;
    }

    settings.show_debug_overlay = !settings.show_debug_overlay;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    if settings.show_debug_overlay {
        commands.spawn((
            Name::new("F3 Debug Overlay"),
            DebugOverlay,
            Text::new("Collecting diagnostics..."),
            TextFont {
                font: bevy::text::FontSource::Handle(asset_server.load("ui/menu/faithful.ttf")),
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextShadow {
                offset: Vec2::splat(1.0),
                color: Color::BLACK,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.04, 0.72)),
            GlobalZIndex(1_000),
        ));
    }
}

fn disable_debug_overlay(
    mut commands: Commands,
    mut settings: ResMut<UiSettings>,
    roots: Query<Entity, With<DebugOverlay>>,
) {
    settings.show_debug_overlay = false;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn reset_world_profiling(mut profiling: ResMut<WorldProfiling>) {
    *profiling = WorldProfiling::default();
}

fn sample_frame_time(time: Res<Time<Real>>, mut stats: ResMut<FrameStats>) {
    let delta = time.delta_secs();
    if !delta.is_finite() || delta <= 0.0 {
        return;
    }
    stats.elapsed += delta;
    stats.frames = stats.frames.saturating_add(1);
    if stats.elapsed >= REFRESH_SECONDS {
        stats.frame_ms = stats.elapsed * 1_000.0 / stats.frames as f32;
        stats.fps = stats.frames as f32 / stats.elapsed;
        stats.elapsed = 0.0;
        stats.frames = 0;
        stats.revision = stats.revision.wrapping_add(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_debug_overlay(
    frame: Res<FrameStats>,
    profiling: Res<WorldProfiling>,
    streaming: Res<ChunkStreamingStats>,
    meshing: Res<ChunkMeshingStats>,
    generation: Res<GenerationSettings>,
    storage: Res<ChunkStorage>,
    renderer: Res<ChunkRenderer>,
    meshes: Res<Assets<Mesh>>,
    registry: Res<BlockRegistry>,
    raycast: Res<CameraRaycast>,
    player: Single<(&Transform, &LookState), With<Player>>,
    visible_parts: Query<(&ChunkMesh, &ViewVisibility)>,
    mut text: Single<&mut Text, With<DebugOverlay>>,
    mut displayed_revision: Local<u64>,
) {
    if frame.revision == *displayed_revision {
        return;
    }
    *displayed_revision = frame.revision;

    let (transform, look) = *player;
    let position = transform.translation;
    let block_position = WorldBlockPos::new(
        position.x.floor() as i32,
        position.y.floor() as i32,
        position.z.floor() as i32,
    );
    let chunk = block_position.chunk_pos();
    let forward = Quat::from_rotation_y(look.yaw) * Quat::from_rotation_x(look.pitch) * Vec3::NEG_Z;
    let visible_chunks = visible_parts
        .iter()
        .filter_map(|(mesh, visible)| visible.get().then_some(mesh.position))
        .collect::<HashSet<_>>()
        .len();
    let (vertex_count, index_count) = renderer.geometry_counts(&meshes);
    let triangle_count = index_count / 3;
    let voxel_bytes = storage.iter().count() * CHUNK_VOLUME * size_of::<BlockId>();
    let mesh_bytes = vertex_count * MESH_VERTEX_BYTES + index_count * size_of::<u32>();
    let biome = BiomeSampler::new(generation.seed)
        .sample(i64::from(block_position.x), i64::from(block_position.z))
        .biome;
    let target = raycast.0.map_or_else(
        || "none".to_string(),
        |hit| {
            format!(
                "{} @ {}, {}, {} ({:.2}m)",
                registry.definition(hit.block).name,
                hit.position.x,
                hit.position.y,
                hit.position.z,
                hit.distance
            )
        },
    );

    let mut output = String::with_capacity(1_024);
    let _ = writeln!(output, "JabaCraft debug (F3)");
    let _ = writeln!(
        output,
        "FPS: {:.0} | frame: {:.2} ms",
        frame.fps, frame.frame_ms
    );
    let _ = writeln!(
        output,
        "XYZ: {:.2} / {:.2} / {:.2}",
        position.x, position.y, position.z
    );
    let _ = writeln!(output, "ChunkPos: {} / {} / {}", chunk.x, chunk.y, chunk.z);
    let _ = writeln!(
        output,
        "Facing: {} | look: [{:+.2}, {:+.2}, {:+.2}] | yaw {:.1} pitch {:.1}",
        cardinal_direction(forward),
        forward.x,
        forward.y,
        forward.z,
        look.yaw.to_degrees(),
        look.pitch.to_degrees()
    );
    let _ = writeln!(output, "Biome: {biome:?} | Target: {target}");
    let _ = writeln!(
        output,
        "Chunks: {} loaded | {} visible | {} renderer entries",
        storage.iter().count(),
        visible_chunks,
        renderer.len()
    );
    let _ = writeln!(
        output,
        "Generation: {} queued | {} running / {} max",
        streaming.queued, streaming.running, generation.max_generation_tasks
    );
    let _ = writeln!(
        output,
        "Meshing: {} queued | {} running / {} max | {} awaiting upload ({} per frame)",
        meshing.queued,
        meshing.running,
        meshing.max_tasks,
        meshing.awaiting_upload,
        meshing.max_uploads_per_frame
    );
    let _ = writeln!(
        output,
        "Mesh: {} vertices | {} triangles | {} assets",
        vertex_count,
        triangle_count,
        meshes.len()
    );
    let _ = writeln!(
        output,
        "Memory est.: voxels {} | CPU meshes {}",
        format_bytes(voxel_bytes),
        format_bytes(mesh_bytes)
    );
    timing_line(&mut output, "Terrain", profiling.terrain_generation);
    timing_line(&mut output, "Meshing", profiling.meshing);
    timing_line(&mut output, "Chunk load", profiling.chunk_loading);

    text.0 = output;
}

fn cardinal_direction(forward: Vec3) -> &'static str {
    if forward.x.abs() > forward.z.abs() {
        if forward.x >= 0.0 {
            "East (+X)"
        } else {
            "West (-X)"
        }
    } else if forward.z >= 0.0 {
        "South (+Z)"
    } else {
        "North (-Z)"
    }
}

fn timing_line(output: &mut String, label: &str, timing: TimingStats) {
    let _ = writeln!(
        output,
        "{label}: avg {:.2} | last {:.2} | max {:.2} ms (n={})",
        timing.average_ms(),
        timing.last_ms(),
        timing.max_ms(),
        timing.samples()
    );
}

fn format_bytes(bytes: usize) -> String {
    const MIB: f64 = 1_048_576.0;
    const KIB: f64 = 1_024.0;
    if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else {
        format!("{:.1} KiB", bytes as f64 / KIB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_stats_report_average_last_and_max() {
        let mut timing = TimingStats::default();
        timing.record(Duration::from_millis(2));
        timing.record(Duration::from_millis(6));

        assert_eq!(timing.samples(), 2);
        assert_eq!(timing.average_ms(), 4.0);
        assert_eq!(timing.last_ms(), 6.0);
        assert_eq!(timing.max_ms(), 6.0);
    }

    #[test]
    fn horizontal_view_maps_to_minecraft_cardinal_directions() {
        assert_eq!(cardinal_direction(Vec3::NEG_Z), "North (-Z)");
        assert_eq!(cardinal_direction(Vec3::Z), "South (+Z)");
        assert_eq!(cardinal_direction(Vec3::X), "East (+X)");
        assert_eq!(cardinal_direction(Vec3::NEG_X), "West (-X)");
    }
}
