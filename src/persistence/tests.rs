use super::*;
use format::{DecodedSave, decode};
use std::fs;

fn sample() -> WorldSave {
    WorldSave {
        format_version: WORLD_SAVE_VERSION,
        world_name: "test-world".into(),
        seed: u64::MAX,
        game_time_elapsed_days: 42.625,
        player_position: [-17.5, 28.25, 100.0],
        player_rotation: PlayerRotation {
            yaw: 2.5,
            pitch: -0.5,
        },
    }
}

fn read_current(path: &std::path::Path) -> WorldSave {
    match storage::read_world_save(path).unwrap() {
        DecodedSave::Current(save) => save,
        DecodedSave::LegacyTime(_) => panic!("expected binary metadata"),
    }
}

#[test]
fn metadata_round_trips_every_field_and_has_a_fixed_wire_fixture() {
    let save = sample();
    match decode(&save.encode().unwrap()).unwrap() {
        DecodedSave::Current(decoded) => assert_eq!(decoded, save),
        _ => panic!("expected binary save"),
    }
    let fixture = WorldSave {
        format_version: 2,
        world_name: "a".into(),
        seed: 0,
        game_time_elapsed_days: 0.0,
        player_position: [0.0; 3],
        player_rotation: PlayerRotation {
            yaw: 0.0,
            pitch: 0.0,
        },
    };
    let mut expected = b"JABASAVE\x02\x00\x00\x00\x02\x01a\x00".to_vec();
    expected.extend([0; 28]);
    assert_eq!(fixture.encode().unwrap(), expected);
}

#[test]
fn truncated_future_trailing_and_invalid_numeric_saves_are_rejected() {
    let bytes = sample().encode().unwrap();
    for end in 0..bytes.len() {
        assert!(decode(&bytes[..end]).is_err(), "length {end}");
    }
    let mut future = bytes.clone();
    future[8] = 99;
    assert!(decode(&future).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing).is_err());
    assert!(decode(&vec![0; format::MAX_SAVE_BYTES + 1]).is_err());
    for bad in [f32::NAN, f32::INFINITY, 2_000_000.0] {
        let mut save = sample();
        save.player_position[0] = bad;
        assert!(save.encode().is_err());
        let mut raw = bytes[..12].to_vec();
        raw.extend(postcard::to_allocvec(&save).unwrap());
        assert!(decode(&raw).is_err());
    }
    let mut save = sample();
    save.game_time_elapsed_days = -1.0;
    assert!(save.validate().is_err());
    save = sample();
    save.player_rotation.pitch = 2.0;
    assert!(save.validate().is_err());
}

#[test]
fn slot_paths_are_isolated_and_cannot_escape_root() {
    for name in ["", "..", "../other", "a/b", "a\\b", "/tmp", "C:foo"] {
        assert!(
            PersistenceSettings {
                world_name: name.into(),
                ..default()
            }
            .world_save_path()
            .is_err()
        );
    }
    assert_eq!(
        PersistenceSettings::default().world_save_path().unwrap(),
        PathBuf::from("saves/default-world/world.save")
    );
}

#[test]
fn atomic_replacement_preserves_previous_save_on_failure_and_ignores_stale_temp() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("test-world/world.save");
    let mut save = sample();
    write_world_save(&path, &save).unwrap();
    assert_eq!(read_current(&path), save);
    let old = fs::read(&path).unwrap();
    save.player_position[0] = f32::NAN;
    assert!(write_world_save(&path, &save).is_err());
    assert_eq!(fs::read(&path).unwrap(), old);
    let stale = path.parent().unwrap().join(".world-interrupted.tmp");
    fs::write(&stale, b"incomplete data").unwrap();
    assert_eq!(read_current(&path), sample());
    save = sample();
    save.seed = 123;
    save.player_position[0] = 88.0;
    write_world_save(&path, &save).unwrap();
    assert_eq!(read_current(&path), save);
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 2);
    // Rename failure cleans up the new temp file and leaves other files intact.
    let directory_target = path.parent().unwrap().join("directory");
    fs::create_dir(&directory_target).unwrap();
    assert!(write_world_save(&directory_target, &save).is_err());
    assert_eq!(read_current(&path), save);
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 3);
}

#[derive(Resource)]
struct SeedAtSpawn(u64);

fn spawn_test_player(mut commands: Commands, generation: Res<GenerationSettings>) {
    commands.insert_resource(SeedAtSpawn(generation.seed));
    commands.spawn((
        Player,
        Transform::from_xyz(1.0, 20.0, 3.0),
        LookState::default(),
        Velocity(Vec3::splat(9.0)),
        Grounded(true),
        SurvivalTracker::new(20.0),
    ));
    commands.spawn((PlayerCamera, Transform::from_xyz(0.0, 0.72, 0.0)));
}

fn test_app(root: &std::path::Path) -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        bevy::state::app::StatesPlugin,
    ))
    .insert_state(crate::game::GameState::Loading)
    .insert_resource(PersistenceSettings {
        world_name: "test-world".into(),
        save_directory: root.into(),
        autosave_interval_seconds: 30.0,
    })
    .init_resource::<GameTime>()
    .init_resource::<GenerationSettings>()
    .init_resource::<ChunkStorage>()
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<Time>()
    .add_message::<AppExit>()
    .add_plugins(PersistencePlugin)
    .add_systems(
        OnEnter(crate::game::GameState::Loading),
        spawn_test_player
            .after(PersistenceLoadSet)
            .before(restore_player),
    );
    app
}

fn trigger_exit(app: &mut App) {
    app.world_mut().write_message(AppExit::Success);
    app.update();
}

fn finish_background_save(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::ZERO);
    for _ in 0..10_000 {
        app.update();
        if !app.world().resource::<SaveStatus>().saving {
            return;
        }
        std::thread::yield_now();
    }
    panic!("background save did not finish");
}

#[test]
fn startup_restores_seed_before_spawn_and_pose_before_first_frame_then_saves_live_state() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("test-world/world.save");
    let save = sample();
    write_world_save(&path, &save).unwrap();
    let mut app = test_app(root.path());
    app.update();
    assert_eq!(app.world().resource::<SeedAtSpawn>().0, save.seed);
    assert_eq!(
        app.world().resource::<GameTime>().elapsed_days(),
        save.game_time_elapsed_days
    );
    let world = app.world_mut();
    let (transform, look, velocity, grounded) = world
        .query_filtered::<(&Transform, &LookState, &Velocity, &Grounded), With<Player>>()
        .single(world)
        .unwrap();
    assert_eq!(transform.translation.to_array(), save.player_position);
    assert_eq!(
        (look.yaw, look.pitch),
        (save.player_rotation.yaw, save.player_rotation.pitch)
    );
    assert_eq!(
        transform.rotation,
        Quat::from_rotation_y(save.player_rotation.yaw)
    );
    assert_eq!(velocity.0, Vec3::ZERO);
    assert!(!grounded.0);
    let camera = world
        .query_filtered::<&Transform, With<PlayerCamera>>()
        .single(world)
        .unwrap();
    assert_eq!(
        camera.rotation,
        Quat::from_rotation_x(save.player_rotation.pitch)
    );
    let mut query = world.query_filtered::<(&mut Transform, &mut LookState), With<Player>>();
    let (mut transform, mut look) = query.single_mut(world).unwrap();
    transform.translation = Vec3::new(-32.0, 5.0, 1.0);
    look.pitch = 0.25;
    trigger_exit(&mut app);
    let updated = read_current(&path);
    assert_eq!(updated.player_position, [-32.0, 5.0, 1.0]);
    assert_eq!(updated.player_rotation.pitch, 0.25);
    assert_eq!(updated.seed, save.seed);
    let mut reloaded = test_app(root.path());
    reloaded.update();
    let world = reloaded.world_mut();
    assert_eq!(
        world
            .query_filtered::<&Transform, With<Player>>()
            .single(world)
            .unwrap()
            .translation
            .to_array(),
        updated.player_position
    );
}

#[test]
fn legacy_time_migrates_and_new_world_autosaves() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("test-world/world.save");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"version=1\ngame_time_elapsed_days=12.5\n").unwrap();
    let mut app = test_app(root.path());
    app.update();
    assert_eq!(app.world().resource::<GameTime>().elapsed_days(), 12.5);
    assert!(fs::read(&path).unwrap().starts_with(b"version=1"));
    trigger_exit(&mut app);
    let migrated = read_current(&path);
    assert_eq!(migrated.game_time_elapsed_days, 12.5);
    assert_eq!(migrated.player_position, [1.0, 20.0, 3.0]);
    let fresh = tempfile::tempdir().unwrap();
    let mut app = test_app(fresh.path());
    app.update();
    let new_path = fresh.path().join("test-world/world.save");
    assert!(!new_path.exists());
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(30));
    app.update();
    finish_background_save(&mut app);
    assert_eq!(read_current(&new_path).world_name, "test-world");
}

#[test]
fn corrupt_future_or_wrong_world_save_is_never_overwritten() {
    let mut future = sample().encode().unwrap();
    future[8] = 99;
    let mut wrong = sample();
    wrong.world_name = "different-world".into();
    for bytes in [b"corrupt".to_vec(), future, wrong.encode().unwrap()] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test-world/world.save");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &bytes).unwrap();
        let mut app = test_app(root.path());
        app.update();
        assert!(!app.world().resource::<SaveSession>().ready);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(31));
        app.update();
        trigger_exit(&mut app);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn manual_save_persists_an_unloaded_dirty_chunk_and_restores_it_as_an_override() {
    let root = tempfile::tempdir().unwrap();
    let mut app = test_app(root.path());
    app.update();

    let position = ChunkPos::new(2, 0, -1);
    let world_block = crate::coordinates::WorldBlockPos::new(32, 4, -16);
    let chunks = &mut app.world_mut().resource_mut::<ChunkStorage>();
    chunks.insert_chunk(position, Chunk::default());
    chunks.set_block(world_block, BlockId::WOOD).unwrap();
    chunks.remove_chunk(position);

    app.world_mut().write_message(ManualSave);
    app.update();
    assert!(app.world().resource::<SaveStatus>().saving);
    finish_background_save(&mut app);

    let archive_path = root.path().join("test-world/chunks.save");
    let archive = storage::read_chunk_archive(&archive_path).unwrap();
    assert_eq!(
        archive.seed,
        app.world().resource::<GenerationSettings>().seed
    );
    assert_eq!(archive.chunks.len(), 1);
    assert_eq!(archive.chunks[0].position, [2, 0, -1]);

    let mut reloaded = test_app(root.path());
    reloaded.update();
    let chunks = &mut reloaded.world_mut().resource_mut::<ChunkStorage>();
    chunks.insert_chunk(position, Chunk::new(BlockId::STONE));
    assert_eq!(chunks.get_block(world_block), Some(BlockId::WOOD));
}

#[test]
fn chunk_archive_rejects_wrong_seed_duplicate_positions_and_unknown_blocks() {
    let root = tempfile::tempdir().unwrap();
    let world_path = root.path().join("test-world/world.save");
    let chunk_path = root.path().join("test-world/chunks.save");
    let save = sample();
    write_world_save(&world_path, &save).unwrap();

    let stored = SavedChunk {
        position: [0, 0, 0],
        blocks: vec![BlockId::AIR.as_u16(); crate::chunk::CHUNK_VOLUME],
    };
    for archive in [
        ChunkArchive {
            version: CHUNK_ARCHIVE_VERSION,
            seed: save.seed.wrapping_sub(1),
            chunks: vec![stored.clone()],
        },
        ChunkArchive {
            version: CHUNK_ARCHIVE_VERSION,
            seed: save.seed,
            chunks: vec![stored.clone(), stored.clone()],
        },
        ChunkArchive {
            version: CHUNK_ARCHIVE_VERSION,
            seed: save.seed,
            chunks: vec![SavedChunk {
                blocks: vec![u16::MAX; crate::chunk::CHUNK_VOLUME],
                ..stored.clone()
            }],
        },
    ] {
        storage::write_chunk_archive(&chunk_path, &archive).unwrap();
        let original_metadata = fs::read(&world_path).unwrap();
        let original_chunks = fs::read(&chunk_path).unwrap();
        let mut app = test_app(root.path());
        app.update();
        assert!(!app.world().resource::<SaveSession>().ready);
        trigger_exit(&mut app);
        assert_eq!(fs::read(&world_path).unwrap(), original_metadata);
        assert_eq!(fs::read(&chunk_path).unwrap(), original_chunks);
    }
}

#[test]
fn overlapping_manual_requests_queue_one_follow_up_without_replacing_the_active_job() {
    let root = tempfile::tempdir().unwrap();
    let mut app = test_app(root.path());
    app.update();

    let task = IoTaskPool::get().spawn(std::future::pending::<CompletedSave>());
    {
        let mut coordinator = app.world_mut().resource_mut::<SaveCoordinator>();
        coordinator.task = Some(task);
    }
    app.world_mut().resource_mut::<SaveStatus>().saving = true;
    app.world_mut().write_message(ManualSave);
    app.world_mut().write_message(ManualSave);
    app.update();

    let coordinator = app.world().resource::<SaveCoordinator>();
    assert!(coordinator.task.is_some());
    assert!(coordinator.pending);
    assert!(app.world().resource::<SaveStatus>().saving);
}

#[test]
fn main_menu_and_quit_do_not_open_or_write_a_world() {
    let root = tempfile::tempdir().unwrap();
    let mut app = test_app(root.path());
    app.insert_state(crate::game::GameState::MainMenu);
    app.update();
    assert!(!app.world().resource::<SaveSession>().ready);
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<Player>>()
            .iter(app.world())
            .next()
            .is_none()
    );
    trigger_exit(&mut app);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn choosing_a_slot_loads_only_on_loading_transition() {
    let root = tempfile::tempdir().unwrap();
    let save = sample();
    write_world_save(&root.path().join("test-world/world.save"), &save).unwrap();
    let mut app = test_app(root.path());
    app.insert_state(crate::game::GameState::MainMenu);
    app.update();
    assert!(!app.world().resource::<SaveSession>().ready);
    app.world_mut()
        .resource_mut::<NextState<crate::game::GameState>>()
        .set(crate::game::GameState::Loading);
    app.update();
    assert!(app.world().resource::<SaveSession>().ready);
    assert_eq!(app.world().resource::<GenerationSettings>().seed, save.seed);
    assert_eq!(
        app.world().resource::<GameTime>().elapsed_days(),
        save.game_time_elapsed_days
    );
}
