//! Pause controls and save-before-leaving flow.
use super::{
    GameState,
    menu::label,
    settings::{ConfigWriter, UserSettings},
};
use crate::{
    inventory::{InventoryInputSet, InventoryState},
    persistence::{ManualSave, PersistenceSaveSet, SaveStatus},
};
use bevy::{
    app::AppExit,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

#[derive(Clone, Copy, Debug)]
enum Destination {
    Menu,
    Quit,
}
#[derive(Resource, Default, Debug)]
pub(super) struct Pause {
    settings: bool,
    waiting: bool,
    destination: Option<Destination>,
    error: String,
}
#[derive(Component)]
struct PauseRoot;
#[derive(Component, Clone, Copy)]
enum Action {
    Resume,
    Settings,
    Save,
    Menu,
    Quit,
    Back,
    Distance(i32),
    Sensitivity(f32),
    Fov(f32),
    Volume(f32),
    Vsync,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Pause>()
        .add_systems(
            PreUpdate,
            escape
                .after(bevy::input::InputSystems)
                .before(InventoryInputSet::Toggle),
        )
        .add_systems(OnEnter(GameState::Paused), enter)
        .add_systems(
            Update,
            (clicks, draw).chain().run_if(in_state(GameState::Paused)),
        )
        .add_systems(
            Last,
            departure
                .after(PersistenceSaveSet)
                .run_if(in_state(GameState::Paused)),
        )
        .add_systems(OnExit(GameState::Paused), close)
        .add_systems(
            OnEnter(GameState::MainMenu),
            clear_session.before(super::menu::enter_menu),
        );
}
fn escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    inventory: Res<InventoryState>,
    mut pause: ResMut<Pause>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    match state.get() {
        GameState::Playing if !inventory.is_open() => next.set(GameState::Paused),
        GameState::Paused if !pause.waiting => {
            if pause.settings {
                pause.settings = false;
            } else {
                next.set(GameState::Playing);
            }
        }
        _ => (),
    }
}
fn enter(mut pause: ResMut<Pause>, mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>) {
    *pause = Pause::default();
    cursor.grab_mode = CursorGrabMode::None;
    cursor.visible = true;
}
fn button(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    title: &str,
    action: Action,
    width: f32,
) {
    parent
        .spawn((
            Button,
            action,
            ImageNode::new(assets.load("ui/menu/button.png")),
            Node {
                width: Val::Px(width),
                height: Val::Px(40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|row| label(row, assets, title, 20.0));
}
fn option(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    title: &str,
    down: Action,
    up: Action,
) {
    parent
        .spawn(Node {
            width: Val::Px(500.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            button(row, assets, "-", down, 40.0);
            label(row, assets, title, 18.0);
            button(row, assets, "+", up, 40.0);
        });
}
fn draw(
    mut commands: Commands,
    pause: Res<Pause>,
    settings: Res<UserSettings>,
    status: Res<SaveStatus>,
    config: Res<ConfigWriter>,
    assets: Res<AssetServer>,
    roots: Query<Entity, With<PauseRoot>>,
    mut last_draw: Local<String>,
) {
    let key = format!(
        "{:?}{:?}{}{:?}",
        *pause, *settings, status.saving, config.error
    );
    if *last_draw == key && !roots.is_empty() {
        return;
    }
    *last_draw = key;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    commands
        .spawn((
            PauseRoot,
            GlobalZIndex(1000),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        ))
        .with_children(|root| {
            label(
                root,
                &assets,
                if pause.settings {
                    "Settings"
                } else {
                    "Game paused"
                },
                30.0,
            );
            if pause.settings {
                option(
                    root,
                    &assets,
                    &format!("Render distance: {} chunks", settings.render_distance),
                    Action::Distance(-1),
                    Action::Distance(1),
                );
                option(
                    root,
                    &assets,
                    &format!(
                        "Mouse sensitivity: {:.0}%",
                        settings.mouse_sensitivity / 0.0025 * 100.0
                    ),
                    Action::Sensitivity(-0.00025),
                    Action::Sensitivity(0.00025),
                );
                option(
                    root,
                    &assets,
                    &format!("FOV: {:.0}", settings.fov),
                    Action::Fov(-5.0),
                    Action::Fov(5.0),
                );
                option(
                    root,
                    &assets,
                    &format!("Master volume: {:.0}%", settings.master_volume * 100.0),
                    Action::Volume(-0.05),
                    Action::Volume(0.05),
                );
                button(
                    root,
                    &assets,
                    if settings.vsync {
                        "VSync: ON"
                    } else {
                        "VSync: OFF"
                    },
                    Action::Vsync,
                    400.0,
                );
                button(root, &assets, "Done", Action::Back, 400.0);
            } else {
                for (name, action) in [
                    ("Resume", Action::Resume),
                    ("Settings", Action::Settings),
                    ("Save", Action::Save),
                    ("Main Menu", Action::Menu),
                    ("Quit", Action::Quit),
                ] {
                    button(root, &assets, name, action, 400.0);
                }
            }
            if pause.waiting || status.saving {
                label(root, &assets, "Saving...", 18.0);
            }
            if !pause.error.is_empty() {
                label(root, &assets, &pause.error, 16.0);
            }
            if let Some(error) = &config.error {
                label(root, &assets, error, 16.0);
            }
        });
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn clicks(
    mut interactions: Query<(&Interaction, &Action, &mut ImageNode), Changed<Interaction>>,
    assets: Res<AssetServer>,
    mut pause: ResMut<Pause>,
    mut settings: ResMut<UserSettings>,
    mut next: ResMut<NextState<GameState>>,
    mut save: MessageWriter<ManualSave>,
) {
    for (interaction, action, mut image) in &mut interactions {
        image.image = assets.load(if *interaction == Interaction::None {
            "ui/menu/button.png"
        } else {
            "ui/menu/button_highlighted.png"
        });
        if *interaction != Interaction::Pressed || pause.waiting {
            continue;
        }
        match *action {
            Action::Resume => next.set(GameState::Playing),
            Action::Settings => pause.settings = true,
            Action::Back => pause.settings = false,
            Action::Save | Action::Menu | Action::Quit => {
                pause.destination = match action {
                    Action::Menu => Some(Destination::Menu),
                    Action::Quit => Some(Destination::Quit),
                    _ => None,
                };
                pause.waiting = true;
                pause.error.clear();
                save.write(ManualSave);
            }
            Action::Distance(delta) => {
                settings.render_distance = (settings.render_distance + delta).clamp(2, 16)
            }
            Action::Sensitivity(delta) => {
                settings.mouse_sensitivity =
                    (settings.mouse_sensitivity + delta).clamp(0.0005, 0.01)
            }
            Action::Fov(delta) => settings.fov = (settings.fov + delta).clamp(40.0, 110.0),
            Action::Volume(delta) => {
                settings.master_volume = (settings.master_volume + delta).clamp(0.0, 1.0)
            }
            Action::Vsync => settings.vsync = !settings.vsync,
        }
    }
}
pub(super) fn departure(
    mut pause: ResMut<Pause>,
    status: Res<SaveStatus>,
    mut next: ResMut<NextState<GameState>>,
    mut exits: MessageWriter<AppExit>,
) {
    if !pause.waiting || status.saving {
        return;
    }
    pause.waiting = false;
    if let Some(error) = &status.last_error {
        pause.error = format!("Save failed: {error}");
        pause.destination = None;
        return;
    }
    match pause.destination.take() {
        Some(Destination::Menu) => next.set(GameState::MainMenu),
        Some(Destination::Quit) => {
            exits.write(AppExit::Success);
        }
        None => (),
    }
}
fn close(mut commands: Commands, roots: Query<Entity, With<PauseRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
fn clear_session(world: &mut World) {
    // MainMenu is also the initial state, when there is no session to dispose of.
    if world
        .query_filtered::<Entity, With<crate::player::Player>>()
        .iter(world)
        .next()
        .is_none()
    {
        return;
    }
    let roots: Vec<Entity> = world
        .query_filtered::<Entity, (
            Without<ChildOf>,
            Or<(
                With<Node>,
                With<crate::player::Player>,
                With<DirectionalLight>,
                With<crate::item::DroppedItem>,
            )>,
        )>()
        .iter(world)
        .collect();
    for entity in roots {
        world.despawn(entity);
    }
    world.insert_resource(crate::chunk::ChunkStorage::default());
    crate::generation::reset_session(world);
    crate::meshing::reset_session(world);
    crate::persistence::reset_session(world);
    world.insert_resource(crate::game_time::GameTime::default());
    world.insert_resource(crate::inventory::InventoryState::default());
    world.insert_resource(crate::inventory::PlayerInventory::default());
    world.insert_resource(crate::interaction::MiningProgress::default());
    world.insert_resource(crate::interaction::CameraRaycast::default());
    // ChunkRenderer removes its old entities and mesh assets when it sees empty storage.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn departure_waits_and_refuses_to_leave_after_save_error() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .insert_state(GameState::Paused)
            .insert_resource(Pause {
                waiting: true,
                destination: Some(Destination::Menu),
                ..default()
            })
            .insert_resource(SaveStatus {
                saving: true,
                ..default()
            })
            .add_message::<AppExit>()
            .add_systems(Update, departure);
        app.update();
        assert!(app.world().resource::<Pause>().waiting);
        {
            let mut status = app.world_mut().resource_mut::<SaveStatus>();
            status.saving = false;
            status.last_error = Some("disk full".into());
        }
        app.update();
        assert!(!app.world().resource::<Pause>().waiting);
        assert!(app.world().resource::<Pause>().error.contains("disk full"));
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Paused
        );
        app.world_mut().resource_mut::<SaveStatus>().last_error = None;
        {
            let mut pause = app.world_mut().resource_mut::<Pause>();
            pause.waiting = true;
            pause.destination = Some(Destination::Menu);
        }
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::MainMenu
        );
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;
    #[test]
    fn returning_to_menu_discards_session_data_but_keeps_preferences() {
        let mut world = World::new();
        world.insert_resource(UserSettings {
            render_distance: 12,
            ..default()
        });
        let player = world
            .spawn((crate::player::Player, Transform::default()))
            .id();
        let hud = world.spawn(Node::default()).id();
        let mut chunks = crate::chunk::ChunkStorage::default();
        chunks.insert_chunk(
            crate::coordinates::ChunkPos::new(0, 0, 0),
            crate::chunk::Chunk::default(),
        );
        world.insert_resource(chunks);
        clear_session(&mut world);
        assert!(world.get_entity(player).is_err());
        assert!(world.get_entity(hud).is_err());
        assert_eq!(
            world
                .resource::<crate::chunk::ChunkStorage>()
                .iter()
                .count(),
            0
        );
        assert_eq!(world.resource::<UserSettings>().render_distance, 12);
        assert!(!world.resource::<SaveStatus>().saving);
    }
}
