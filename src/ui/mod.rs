//! HUD and user-interface systems.

mod crosshair;
mod inventory_panel;

use bevy::prelude::*;

use crate::{
    inventory::{HOTBAR_SLOT_COUNT, InventoryState, PlayerInventory},
    item::ItemRegistry,
};

const SLOT_SIZE: f32 = 58.0;
const ICON_SIZE: f32 = 38.0;
const SELECTED_COLOR: Color = Color::srgb(1.0, 0.82, 0.2);
const UNSELECTED_COLOR: Color = Color::srgba(0.72, 0.74, 0.78, 0.9);
const EMPTY_ICON_COLOR: Color = Color::srgb(0.11, 0.12, 0.14);

/// Runtime switches for future HUD and menu presentation.
#[derive(Debug, Default, Resource)]
pub struct UiSettings {
    pub show_debug_overlay: bool,
}

#[derive(Component)]
struct HotbarSlotView(usize);

#[derive(Component)]
struct HotbarIconView(usize);

#[derive(Component)]
struct HotbarCountView(usize);

#[derive(Component)]
struct HotbarRoot;

/// Owns the HUD, menus, and UI-driven game-state transitions.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiSettings>()
            .add_systems(
                Startup,
                (
                    spawn_hotbar,
                    inventory_panel::spawn_inventory_panel,
                    crosshair::spawn_crosshair,
                ),
            )
            .add_systems(Update, inventory_panel::handle_inventory_clicks)
            .add_systems(
                Last,
                (
                    sync_hotbar_ui,
                    inventory_panel::sync_inventory_panel,
                    inventory_panel::follow_cursor_stack,
                    crosshair::sync_crosshair_visibility,
                ),
            );
    }
}

fn spawn_hotbar(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Hotbar Root"),
            HotbarRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(18.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(100),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Hotbar"),
                Node {
                    height: Val::Px(SLOT_SIZE + 12.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    column_gap: Val::Px(5.0),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.035, 0.04, 0.05, 0.82)),
            ))
            .with_children(|bar| {
                for index in 0..HOTBAR_SLOT_COUNT {
                    spawn_hotbar_slot(bar, index);
                }
            });
        });
}

fn spawn_hotbar_slot(parent: &mut ChildSpawnerCommands, index: usize) {
    parent
        .spawn((
            Name::new(format!("Hotbar Slot {}", index + 1)),
            HotbarSlotView(index),
            Node {
                position_type: PositionType::Relative,
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.11, 0.13, 0.94)),
            BorderColor::all(UNSELECTED_COLOR),
            Outline::new(Val::Px(2.0), Val::Px(1.0), Color::NONE),
        ))
        .with_children(|slot| {
            slot.spawn((
                Name::new("Block Color Placeholder"),
                HotbarIconView(index),
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(EMPTY_ICON_COLOR),
                BorderColor::all(Color::srgba(0.02, 0.02, 0.025, 0.9)),
            ));
            slot.spawn((
                Name::new("Slot Number"),
                Text::new((index + 1).to_string()),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.92, 0.96, 0.9)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(3.0),
                    top: Val::Px(1.0),
                    ..default()
                },
            ));
            slot.spawn((
                Name::new("Item Count"),
                HotbarCountView(index),
                Text::new(""),
                TextFont {
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
                    right: Val::Px(3.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
            ));
        });
}

fn sync_hotbar_ui(
    inventory: Res<PlayerInventory>,
    inventory_state: Res<InventoryState>,
    registry: Res<ItemRegistry>,
    mut root: Single<&mut Node, With<HotbarRoot>>,
    mut slots: Query<(&HotbarSlotView, &mut BorderColor, &mut Outline)>,
    mut icons: Query<(&HotbarIconView, &mut BackgroundColor)>,
    mut counts: Query<(&HotbarCountView, &mut Text)>,
) {
    root.display = if inventory_state.is_open() {
        Display::None
    } else {
        Display::Flex
    };

    if !inventory.is_changed() && !inventory_state.is_changed() {
        return;
    }

    for (view, mut border, mut outline) in &mut slots {
        let selected = view.0 == inventory.selected_hotbar_index();
        *border = BorderColor::all(if selected {
            SELECTED_COLOR
        } else {
            UNSELECTED_COLOR
        });
        outline.color = if selected {
            SELECTED_COLOR
        } else {
            Color::NONE
        };
    }

    for (view, mut background) in &mut icons {
        background.0 = inventory
            .slot(view.0)
            .map(|stack| {
                let [red, green, blue, alpha] = registry.debug_color(stack.item());
                Color::srgba(red, green, blue, alpha)
            })
            .unwrap_or(EMPTY_ICON_COLOR);
    }

    for (view, mut text) in &mut counts {
        text.0 = inventory
            .slot(view.0)
            .map(|stack| stack.count().to_string())
            .unwrap_or_default();
    }
}
