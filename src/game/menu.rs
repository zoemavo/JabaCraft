//! Title screen and save-slot selection. Disk work runs outside the frame loop.
use super::GameState;
use crate::{
    chunk::ChunkStorage,
    coordinates::ChunkPos,
    generation::GenerationSettings,
    persistence::{ManualSave, PersistenceSettings, SaveStatus},
    player::Player,
};
use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    tasks::{IoTaskPool, Task, futures_lite::future, poll_once},
};

#[derive(Default, PartialEq)]
enum Page {
    #[default]
    Title,
    Worlds,
    Create,
}
#[derive(Clone, Copy, PartialEq)]
enum Field {
    Name,
    Seed,
}
#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct MenuCamera;
#[derive(Component)]
struct LoadingRoot;
#[derive(Component)]
struct WorldList;
#[derive(Component, Clone)]
enum Action {
    Play,
    New,
    Quit,
    Back,
    Select(String),
    Load,
    Create,
    Focus(Field),
}

#[derive(Resource, Default)]
pub(super) struct Menu {
    page: Page,
    name: String,
    seed: String,
    focus: Option<Field>,
    worlds: Vec<String>,
    selected: Option<String>,
    scroll: f32,
    error: String,
    job: Option<Task<Result<JobResult, String>>>,
}
enum JobResult {
    Worlds(Vec<String>),
    Created(String, u64),
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Menu>()
        .add_systems(OnEnter(GameState::MainMenu), enter_menu)
        .add_systems(
            Update,
            (clicks, text_input, poll_job, draw_menu, scroll_worlds)
                .chain()
                .run_if(in_state(GameState::MainMenu)),
        )
        .add_systems(OnExit(GameState::MainMenu), leave_menu)
        .add_systems(OnEnter(GameState::Loading), loading_screen)
        .add_systems(Update, finish_loading.run_if(in_state(GameState::Loading)))
        .add_systems(OnExit(GameState::Loading), clear_loading)
        .add_systems(
            OnTransition {
                exited: GameState::Loading,
                entered: GameState::Playing,
            },
            initial_save,
        );
}

pub(super) fn enter_menu(mut commands: Commands, mut menu: ResMut<Menu>, status: Res<SaveStatus>) {
    commands.spawn((MenuCamera, Camera2d));
    menu.page = Page::Title;
    menu.focus = None;
    menu.error = status.last_error.clone().unwrap_or_default();
}

pub(super) fn label(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    value: &str,
    size: f32,
) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font: bevy::text::FontSource::Handle(assets.load("ui/menu/faithful.ttf")),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow {
            offset: Vec2::splat(2.0),
            color: Color::BLACK,
        },
    ));
}
fn button(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    title: &str,
    action: Action,
    selected: bool,
) {
    parent
        .spawn((
            Button,
            action,
            ImageNode::new(assets.load(if selected {
                "ui/menu/button_highlighted.png"
            } else {
                "ui/menu/button.png"
            })),
            Node {
                width: Val::Px(400.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|row| label(row, assets, title, 20.0));
}
fn draw_menu(
    mut commands: Commands,
    menu: Res<Menu>,
    assets: Res<AssetServer>,
    roots: Query<Entity, With<MenuRoot>>,
) {
    if !menu.is_changed() {
        return;
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            ImageNode {
                image: assets.load("ui/menu/background.png"),
                color: Color::srgb(0.3, 0.3, 0.3),
                ..default()
            },
        ))
        .with_children(|root| {
            label(
                root,
                &assets,
                match menu.page {
                    Page::Title => "JabaCraft",
                    Page::Worlds => "Select World",
                    Page::Create => "Create New World",
                },
                42.0,
            );
            root.spawn(Node {
                height: Val::Px(24.0),
                ..default()
            });
            match menu.page {
                Page::Title => {
                    button(root, &assets, "Play", Action::Play, false);
                    button(root, &assets, "Create World", Action::New, false);
                    button(root, &assets, "Quit", Action::Quit, false);
                }
                Page::Worlds => {
                    root.spawn((
                        WorldList,
                        ScrollPosition(Vec2::new(0.0, menu.scroll)),
                        Node {
                            width: Val::Px(440.0),
                            height: Val::Px(260.0),
                            overflow: Overflow::scroll_y(),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        if menu.worlds.is_empty() {
                            label(list, &assets, "No saved worlds", 20.0);
                        }
                        for name in &menu.worlds {
                            button(
                                list,
                                &assets,
                                name,
                                Action::Select(name.clone()),
                                menu.selected.as_ref() == Some(name),
                            );
                        }
                    });
                    button(root, &assets, "Load", Action::Load, false);
                    button(root, &assets, "Cancel", Action::Back, false);
                }
                Page::Create => {
                    for (field, title, value) in [
                        (Field::Name, "World name", &menu.name),
                        (Field::Seed, "Seed (optional)", &menu.seed),
                    ] {
                        label(root, &assets, title, 18.0);
                        root.spawn((
                            Button,
                            Action::Focus(field),
                            Node {
                                width: Val::Px(400.0),
                                height: Val::Px(42.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                overflow: Overflow::clip(),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BorderColor::all(if menu.focus == Some(field) {
                                Color::WHITE
                            } else {
                                Color::srgb(0.5, 0.5, 0.5)
                            }),
                            BackgroundColor(Color::BLACK),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new(format!(
                                    "{}{}",
                                    value,
                                    if menu.focus == Some(field) { "_" } else { "" }
                                )),
                                TextFont {
                                    font: bevy::text::FontSource::Handle(
                                        assets.load("ui/menu/faithful.ttf"),
                                    ),
                                    font_size: FontSize::Px(20.0),
                                    ..default()
                                },
                            ));
                        });
                    }
                    label(root, &assets, "Name: letters, numbers, - and _", 16.0);
                    button(root, &assets, "Create", Action::Create, false);
                    button(root, &assets, "Cancel", Action::Back, false);
                }
            }
            if menu.job.is_some() {
                label(root, &assets, "Please wait...", 18.0);
            }
            if !menu.error.is_empty() {
                label(root, &assets, &menu.error, 17.0);
            }
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn clicks(
    mut menu: ResMut<Menu>,
    mut interactions: Query<(&Interaction, &Action, Option<&mut ImageNode>), Changed<Interaction>>,
    assets: Res<AssetServer>,
    mut settings: ResMut<PersistenceSettings>,
    mut status: ResMut<SaveStatus>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action, mut image) in &mut interactions {
        if let Some(ref mut image) = image {
            image.image = assets.load(if *interaction != Interaction::None || matches!(action, Action::Select(name) if menu.selected.as_ref() == Some(name)) {
                "ui/menu/button_highlighted.png"
            } else {
                "ui/menu/button.png"
            });
        }
        if *interaction != Interaction::Pressed || menu.job.is_some() {
            continue;
        }
        menu.error.clear();
        match action {
            Action::Quit => {
                exit.write(AppExit::Success);
            }
            Action::Back => {
                menu.page = Page::Title;
                menu.focus = None;
            }
            Action::New => {
                menu.page = Page::Create;
                menu.focus = Some(Field::Name);
            }
            Action::Focus(field) => menu.focus = Some(*field),
            Action::Select(name) => menu.selected = Some(name.clone()),
            Action::Play => {
                menu.page = Page::Worlds;
                menu.scroll = 0.0;
                let root = settings.save_directory.clone();
                menu.job = Some(IoTaskPool::get().spawn(async move {
                    let entries = match std::fs::read_dir(root) {
                        Ok(entries) => entries,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            return Ok(JobResult::Worlds(Vec::new()));
                        }
                        Err(e) => return Err(e.to_string()),
                    };
                    let mut worlds = Vec::new();
                    for entry in entries {
                        let entry = entry.map_err(|e| e.to_string())?;
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if valid_name(&name) && entry.path().join("world.save").is_file() {
                            worlds.push(name);
                        }
                    }
                    worlds.sort();
                    Ok(JobResult::Worlds(worlds))
                }));
            }
            Action::Load => {
                if let Some(name) = &menu.selected {
                    settings.world_name = name.clone();
                    status.last_error = None;
                    next.set(GameState::Loading);
                } else {
                    menu.error = "Select a world first".into();
                }
            }
            Action::Create => {
                let name = menu.name.trim().to_owned();
                if !valid_name(&name) {
                    menu.error = "Enter a valid world name".into();
                    continue;
                }
                let seed = if menu.seed.trim().is_empty() {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64
                } else {
                    match menu.seed.trim().parse::<u64>() {
                        Ok(seed) => seed,
                        Err(_) => {
                            menu.error = "Seed must be an unsigned 64-bit number".into();
                            continue;
                        }
                    }
                };
                let root = settings.save_directory.clone();
                menu.job = Some(IoTaskPool::get().spawn(async move {
                    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                    std::fs::create_dir(root.join(&name)).map_err(|e| {
                        if e.kind() == std::io::ErrorKind::AlreadyExists {
                            "A world with this name already exists".into()
                        } else {
                            e.to_string()
                        }
                    })?;
                    Ok(JobResult::Created(name, seed))
                }));
            }
        }
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}
fn text_input(mut input: MessageReader<KeyboardInput>, mut menu: ResMut<Menu>) {
    for event in input.read() {
        if event.state != ButtonState::Pressed || menu.page != Page::Create || menu.job.is_some() {
            continue;
        }
        if event.logical_key == Key::Tab {
            menu.focus = Some(if menu.focus == Some(Field::Name) {
                Field::Seed
            } else {
                Field::Name
            });
            continue;
        }
        let field = menu.focus;
        let value = match field {
            Some(Field::Name) => &mut menu.name,
            Some(Field::Seed) => &mut menu.seed,
            None => continue,
        };
        match &event.logical_key {
            Key::Backspace => {
                value.pop();
            }
            _ => {
                if let Some(text) = &event.text {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if value.len() + c.len_utf8() <= 128 {
                            value.push(c);
                        }
                    }
                }
            }
        }
    }
}
fn poll_job(
    mut menu: ResMut<Menu>,
    mut settings: ResMut<PersistenceSettings>,
    mut generation: ResMut<GenerationSettings>,
    mut status: ResMut<SaveStatus>,
    mut next: ResMut<NextState<GameState>>,
) {
    let result = menu
        .bypass_change_detection()
        .job
        .as_mut()
        .and_then(|job| future::block_on(poll_once(job)));
    if let Some(result) = result {
        menu.job = None;
        match result {
            Ok(JobResult::Worlds(worlds)) => {
                menu.worlds = worlds;
                menu.selected = None;
            }
            Ok(JobResult::Created(name, seed)) => {
                settings.world_name = name;
                generation.seed = seed;
                status.last_error = None;
                next.set(GameState::Loading);
            }
            Err(error) => menu.error = error,
        }
    }
}
fn leave_menu(
    mut commands: Commands,
    roots: Query<Entity, Or<(With<MenuRoot>, With<MenuCamera>)>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
fn loading_screen(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        LoadingRoot,
        Camera2d,
        Camera {
            order: 10,
            ..default()
        },
    ));
    commands
        .spawn((
            LoadingRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.10, 0.08)),
        ))
        .with_children(|root| label(root, &assets, "Loading world...", 28.0));
}
fn finish_loading(
    chunks: Res<ChunkStorage>,
    player: Query<&Transform, With<Player>>,
    status: Res<SaveStatus>,
    settings: Res<GenerationSettings>,
    mut next: ResMut<NextState<GameState>>,
) {
    if status.last_error.is_some() {
        return;
    }
    let Ok(player) = player.single() else { return };
    let p = player.translation.floor().as_ivec3();
    let c = IVec3::new(p.x.div_euclid(16), p.y.div_euclid(16), p.z.div_euclid(16));
    let radius = settings.render_distance_chunks.clamp(0, 1);
    if (-radius..=radius).all(|x| {
        (-radius..=radius).all(|z| {
            if x * x + z * z > settings.render_distance_chunks.pow(2) {
                return true;
            }
            (-1..=0).all(|y| {
                let cy = c.y + y;
                cy < settings.world_min_y.div_euclid(16)
                    || cy > (settings.world_max_y - 1).div_euclid(16)
                    || chunks.contains_chunk(ChunkPos::new(c.x + x, cy, c.z + z))
            })
        })
    }) {
        next.set(GameState::Playing);
    }
}
fn clear_loading(mut commands: Commands, roots: Query<Entity, With<LoadingRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
fn initial_save(mut save: MessageWriter<ManualSave>) {
    save.write(ManualSave);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slot_names_preserve_persistence_path_contract() {
        for valid in ["World", "my-world_12"] {
            assert!(valid_name(valid));
        }
        for invalid in ["", "..", "a/b", "../test"] {
            assert!(!valid_name(invalid));
        }
    }
}

fn scroll_worlds(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut lists: Query<&mut ScrollPosition, With<WorldList>>,
    mut menu: ResMut<Menu>,
) {
    let delta: f32 = wheel
        .read()
        .map(|event| {
            event.y
                * if event.unit == bevy::input::mouse::MouseScrollUnit::Line {
                    40.0
                } else {
                    1.0
                }
        })
        .sum();
    for mut scroll in &mut lists {
        scroll.0.y =
            (scroll.0.y - delta).clamp(0.0, (menu.worlds.len() as f32 * 46.0 - 260.0).max(0.0));
        menu.bypass_change_detection().scroll = scroll.0.y;
    }
}
