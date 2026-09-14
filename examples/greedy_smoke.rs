//! GPU smoke test: temporary world, real PBR/shadow pipelines, automatic exit.
use bevy::{prelude::*, window::PresentMode};
use rustcraft::{
    app::{APP_TITLE, AppPlugin},
    crafting::CraftingPlugin,
    game::GamePlugin,
    game_time::GameTimePlugin,
    interaction::InteractionPlugin,
    inventory::InventoryPlugin,
    item::ItemPlugin,
    persistence::PersistencePlugin,
    player::PlayerPlugin,
    scene::ScenePlugin,
    survival::SurvivalPlugin,
    ui::UiPlugin,
    world::WorldPlugin,
};

fn main() {
    let temp = tempfile::tempdir().expect("temporary smoke-test save directory");
    App::new()
        .insert_resource(rustcraft::persistence::PersistenceSettings {
            world_name: "greedy-smoke".into(),
            save_directory: temp.path().into(),
            ..default()
        })
        .insert_resource(rustcraft::game::settings::ConfigPath(
            temp.path().join("settings.ron"),
        ))
        .add_systems(
            Startup,
            |mut next: ResMut<NextState<rustcraft::game::GameState>>| {
                next.set(rustcraft::game::GameState::Loading);
            },
        )
        .add_systems(Update, finish_smoke)
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: APP_TITLE.into(),
                        resolution: (1280, 720).into(),
                        present_mode: PresentMode::AutoVsync,
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins((
            AppPlugin,
            GamePlugin,
            GameTimePlugin,
            PlayerPlugin,
            WorldPlugin,
            ScenePlugin,
            ItemPlugin,
            InventoryPlugin,
            CraftingPlugin,
            SurvivalPlugin,
            InteractionPlugin,
            PersistencePlugin,
            UiPlugin,
        ))
        .run();
}

fn finish_smoke(
    time: Res<Time>,
    state: Res<State<rustcraft::game::GameState>>,
    chunks: Query<(), With<rustcraft::meshing::ChunkMesh>>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<bevy::app::AppExit>,
) {
    if *state.get() == rustcraft::game::GameState::Playing && chunks.iter().count() >= 10 {
        *frames += 1;
    }
    if *frames == 180 {
        info!(
            "GREEDY_GPU_SMOKE_OK: rendered {} chunk layers",
            chunks.iter().count()
        );
        exit.write(bevy::app::AppExit::Success);
    } else if time.elapsed_secs() > 60.0 {
        error!("GREEDY_GPU_SMOKE_TIMEOUT");
        exit.write(bevy::app::AppExit::error());
    }
}
