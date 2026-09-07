use bevy::{prelude::*, window::PresentMode};
use rustcraft::{
    app::{APP_TITLE, AppPlugin},
    game::GamePlugin,
    interaction::InteractionPlugin,
    inventory::InventoryPlugin,
    item::ItemPlugin,
    persistence::PersistencePlugin,
    player::PlayerPlugin,
    scene::ScenePlugin,
    ui::UiPlugin,
    world::WorldPlugin,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: APP_TITLE.into(),
                resolution: (1280, 720).into(),
                present_mode: PresentMode::AutoVsync,
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            AppPlugin,
            GamePlugin,
            PlayerPlugin,
            WorldPlugin,
            ScenePlugin,
            ItemPlugin,
            InventoryPlugin,
            InteractionPlugin,
            PersistencePlugin,
            UiPlugin,
        ))
        .run();
}
