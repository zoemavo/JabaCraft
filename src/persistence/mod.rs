//! Versioned world persistence with serialized background save jobs.
mod format;
mod storage;
#[cfg(test)]
mod tests;

use std::{collections::HashSet, io, path::PathBuf};

use bevy::{
    app::AppExit,
    prelude::*,
    tasks::{IoTaskPool, Task, futures_lite::future, poll_once},
};

use crate::{
    block::BlockId,
    chunk::{Chunk, ChunkStorage},
    coordinates::ChunkPos,
    game_time::GameTime,
    generation::GenerationSettings,
    player::{Grounded, LookState, Player, PlayerCamera, Velocity},
    survival::SurvivalTracker,
};

use format::{
    CHUNK_ARCHIVE_VERSION, ChunkArchive, DecodedSave, SavedChunk, invalid, valid_world_name,
};
pub use format::{PlayerRotation, WORLD_SAVE_VERSION, WorldSave};
pub use storage::write_world_save;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) struct PersistenceLoadSet;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) struct PersistenceSaveSet;

/// Request an immediate save. F5 sends the same message for the local player.
#[derive(Clone, Copy, Debug, Default, Message)]
pub struct ManualSave;

/// Read-only UI state for a small save indicator.
#[derive(Debug, Default, Resource)]
pub struct SaveStatus {
    pub saving: bool,
    pub last_error: Option<String>,
}

/// Each save slot is confined to one directory under the configured root.
#[derive(Debug, Resource)]
pub struct PersistenceSettings {
    pub world_name: String,
    pub save_directory: PathBuf,
    /// Set to zero to disable periodic saves. Manual and exit saves remain active.
    pub autosave_interval_seconds: f32,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            world_name: "default-world".into(),
            save_directory: PathBuf::from("saves"),
            autosave_interval_seconds: 60.0,
        }
    }
}

impl PersistenceSettings {
    pub fn world_directory(&self) -> io::Result<PathBuf> {
        if !valid_world_name(&self.world_name) {
            return Err(invalid("invalid save slot name"));
        }
        Ok(self.save_directory.join(&self.world_name))
    }

    pub fn world_save_path(&self) -> io::Result<PathBuf> {
        Ok(self.world_directory()?.join("world.save"))
    }

    fn chunk_save_path(&self) -> io::Result<PathBuf> {
        Ok(self.world_directory()?.join("chunks.save"))
    }
}

/// A failed load never grants permission to overwrite existing save files.
#[derive(Default, Resource)]
struct SaveSession {
    world_path: Option<PathBuf>,
    chunk_path: Option<PathBuf>,
    loaded: Option<WorldSave>,
    ready: bool,
}

#[derive(Default, Resource)]
struct SaveCoordinator {
    task: Option<Task<CompletedSave>>,
    pending: bool,
    elapsed: f32,
}

struct SaveSnapshot {
    world_path: PathBuf,
    chunk_path: PathBuf,
    world: WorldSave,
    chunks: Option<Vec<(ChunkPos, Vec<BlockId>)>>,
}

struct CompletedSave {
    result: Result<(), String>,
    chunks: Option<Vec<(ChunkPos, Vec<BlockId>)>>,
}

pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PersistenceSettings>()
            .init_resource::<SaveSession>()
            .init_resource::<SaveCoordinator>()
            .init_resource::<SaveStatus>()
            .add_message::<ManualSave>()
            .add_systems(
                OnEnter(crate::game::GameState::Loading),
                load_world.in_set(PersistenceLoadSet),
            )
            .add_systems(
                OnEnter(crate::game::GameState::Loading),
                restore_player.after(crate::scene::setup_scene),
            )
            .add_systems(Last, drive_saves.in_set(PersistenceSaveSet));
    }
}

fn load_world(
    settings: Res<PersistenceSettings>,
    mut game_time: ResMut<GameTime>,
    mut generation: ResMut<GenerationSettings>,
    mut chunks: ResMut<ChunkStorage>,
    mut session: ResMut<SaveSession>,
    mut status: ResMut<SaveStatus>,
    mut next: ResMut<NextState<crate::game::GameState>>,
) {
    *session = SaveSession::default();
    let result = (|| -> io::Result<()> {
        let world_path = settings.world_save_path()?;
        let chunk_path = settings.chunk_save_path()?;
        let (loaded, seed, elapsed_days) = match storage::read_world_save(&world_path) {
            Ok(DecodedSave::Current(save)) => {
                if save.world_name != settings.world_name {
                    return Err(invalid("save world name does not match slot"));
                }
                let seed = save.seed;
                let time = save.game_time_elapsed_days;
                (Some(save), seed, time)
            }
            Ok(DecodedSave::LegacyTime(time)) => {
                info!(
                    "Loaded legacy metadata; next save migrates {} to v2",
                    world_path.display()
                );
                (None, generation.seed, time)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                info!("Starting new world at {}", world_path.display());
                (None, generation.seed, game_time.elapsed_days())
            }
            Err(error) => return Err(error),
        };

        let saved_chunks = match storage::read_chunk_archive(&chunk_path) {
            Ok(archive) => validate_archive(archive, seed)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };

        generation.seed = seed;
        *game_time = GameTime::from_elapsed_days(elapsed_days);
        for (position, chunk) in saved_chunks {
            chunks.insert_saved_override(position, chunk);
        }
        session.world_path = Some(world_path);
        session.chunk_path = Some(chunk_path);
        session.loaded = loaded;
        Ok(())
    })();
    if let Err(error) = result {
        status.last_error = Some(format!("Cannot load world: {error}"));
        next.set(crate::game::GameState::MainMenu);
        error!("Cannot load world: {error}. Saving is disabled for this session to preserve it.");
    }
}

fn validate_archive(
    archive: ChunkArchive,
    expected_seed: u64,
) -> io::Result<Vec<(ChunkPos, Chunk)>> {
    if archive.seed != expected_seed {
        return Err(invalid("chunk archive seed does not match world metadata"));
    }
    let mut positions = HashSet::new();
    archive
        .chunks
        .into_iter()
        .map(|saved| {
            let position = ChunkPos::new(saved.position[0], saved.position[1], saved.position[2]);
            if !positions.insert(position) {
                return Err(invalid("duplicate chunk in archive"));
            }
            let blocks = saved
                .blocks
                .into_iter()
                .map(|raw| {
                    BlockId::from_raw(raw).ok_or_else(|| invalid("unknown block ID in archive"))
                })
                .collect::<io::Result<Vec<_>>>()?;
            let chunk = Chunk::from_saved_blocks(blocks)
                .map_err(|length| invalid(format!("chunk has {length} blocks")))?;
            Ok((position, chunk))
        })
        .collect()
}

#[allow(clippy::type_complexity)]
fn restore_player(
    mut session: ResMut<SaveSession>,
    mut players: Query<
        (
            &mut Transform,
            &mut LookState,
            &mut Velocity,
            &mut Grounded,
            &mut SurvivalTracker,
        ),
        (With<Player>, Without<PlayerCamera>),
    >,
    mut cameras: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    if session.world_path.is_none() {
        return;
    }
    let Ok((mut transform, mut look, mut velocity, mut grounded, mut tracker)) =
        players.single_mut()
    else {
        error!("Cannot restore player; saving remains disabled");
        return;
    };
    if let Some(save) = session.loaded.take() {
        transform.translation = Vec3::from_array(save.player_position);
        transform.rotation = Quat::from_rotation_y(save.player_rotation.yaw);
        look.yaw = save.player_rotation.yaw;
        look.pitch = save.player_rotation.pitch;
        velocity.0 = Vec3::ZERO;
        grounded.0 = false;
        *tracker = SurvivalTracker::new(transform.translation.y);
        for mut camera in &mut cameras {
            camera.rotation = Quat::from_rotation_x(look.pitch);
        }
    }
    session.ready = true;
}

#[allow(clippy::too_many_arguments)]
fn drive_saves(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    settings: Res<PersistenceSettings>,
    game_time: Res<GameTime>,
    generation: Res<GenerationSettings>,
    session: Res<SaveSession>,
    mut chunks: ResMut<ChunkStorage>,
    players: Query<(&Transform, &LookState), With<Player>>,
    mut manual: MessageReader<ManualSave>,
    mut exits: MessageReader<AppExit>,
    mut coordinator: ResMut<SaveCoordinator>,
    mut status: ResMut<SaveStatus>,
) {
    finish_ready_job(&mut coordinator, &mut chunks, &mut status);

    coordinator.elapsed += time.delta_secs().max(0.0);
    let interval = settings.autosave_interval_seconds;
    let autosave = interval.is_finite() && interval > 0.0 && coordinator.elapsed >= interval;
    if autosave {
        coordinator.elapsed %= interval;
    }
    let manual = manual.read().count() > 0 || keyboard.just_pressed(KeyCode::F5);
    let exiting = exits.read().count() > 0;
    let requested = autosave || manual || coordinator.pending || exiting;
    if !requested {
        return;
    }

    if coordinator.task.is_some() {
        if exiting {
            finish_blocking_job(&mut coordinator, &mut chunks, &mut status);
        } else {
            coordinator.pending = true;
            return;
        }
    }

    let Some(snapshot) = capture_snapshot(
        &settings,
        &game_time,
        &generation,
        &session,
        &chunks,
        &players,
    ) else {
        return;
    };
    coordinator.pending = false;
    status.saving = true;
    status.last_error = None;
    let task = IoTaskPool::get().spawn(save_snapshot(snapshot));
    if exiting {
        coordinator.task = Some(task);
        finish_blocking_job(&mut coordinator, &mut chunks, &mut status);
    } else {
        coordinator.task = Some(task);
    }
}

fn capture_snapshot(
    settings: &PersistenceSettings,
    game_time: &GameTime,
    generation: &GenerationSettings,
    session: &SaveSession,
    chunks: &ChunkStorage,
    players: &Query<(&Transform, &LookState), With<Player>>,
) -> Option<SaveSnapshot> {
    if !session.ready {
        return None;
    }
    let (Some(world_path), Some(chunk_path)) = (&session.world_path, &session.chunk_path) else {
        return None;
    };
    if settings.world_save_path().ok().as_ref() != Some(world_path)
        || settings.chunk_save_path().ok().as_ref() != Some(chunk_path)
    {
        error!("Save slot changed during play; restart to switch worlds");
        return None;
    }
    let Ok((transform, look)) = players.single() else {
        return None;
    };
    Some(SaveSnapshot {
        world_path: world_path.clone(),
        chunk_path: chunk_path.clone(),
        world: WorldSave {
            format_version: WORLD_SAVE_VERSION,
            world_name: settings.world_name.clone(),
            seed: generation.seed,
            game_time_elapsed_days: game_time.elapsed_days(),
            player_position: transform.translation.to_array(),
            player_rotation: PlayerRotation {
                yaw: look.yaw,
                pitch: look.pitch,
            },
        },
        chunks: chunks.has_unsaved_chunks().then(|| chunks.save_snapshot()),
    })
}

async fn save_snapshot(snapshot: SaveSnapshot) -> CompletedSave {
    let chunk_result = snapshot.chunks.as_ref().map_or(Ok(()), |chunks| {
        let archive = ChunkArchive {
            version: CHUNK_ARCHIVE_VERSION,
            seed: snapshot.world.seed,
            chunks: chunks
                .iter()
                .map(|(position, blocks)| SavedChunk {
                    position: [position.x, position.y, position.z],
                    blocks: blocks.iter().map(|block| block.as_u16()).collect(),
                })
                .collect(),
        };
        storage::write_chunk_archive(&snapshot.chunk_path, &archive)
    });
    let result = chunk_result
        .and_then(|()| write_world_save(&snapshot.world_path, &snapshot.world))
        .map_err(|error| error.to_string());
    CompletedSave {
        result,
        chunks: snapshot.chunks,
    }
}

fn finish_ready_job(
    coordinator: &mut SaveCoordinator,
    chunks: &mut ChunkStorage,
    status: &mut SaveStatus,
) {
    let completed = coordinator
        .task
        .as_mut()
        .and_then(|task| future::block_on(poll_once(task)));
    if let Some(completed) = completed {
        coordinator.task = None;
        apply_completion(completed, chunks, status);
    }
}

fn finish_blocking_job(
    coordinator: &mut SaveCoordinator,
    chunks: &mut ChunkStorage,
    status: &mut SaveStatus,
) {
    if let Some(task) = coordinator.task.take() {
        apply_completion(future::block_on(task), chunks, status);
    }
}

fn apply_completion(completed: CompletedSave, chunks: &mut ChunkStorage, status: &mut SaveStatus) {
    status.saving = false;
    match completed.result {
        Ok(()) => {
            if let Some(snapshot) = &completed.chunks {
                chunks.acknowledge_saved(snapshot);
            }
            status.last_error = None;
            info!("World saved");
        }
        Err(error) => {
            error!("Save failed: {error}");
            status.last_error = Some(error);
        }
    }
}

/// Only called after the coordinator has completed the explicit departure save.
pub(crate) fn reset_session(world: &mut World) {
    world.insert_resource(SaveSession::default());
    world.insert_resource(SaveCoordinator::default());
    world.insert_resource(SaveStatus::default());
}
