use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    inventory::{
        HOTBAR_SLOT_COUNT, INVENTORY_SLOT_COUNT, InventoryClick, InventoryState, PlayerInventory,
    },
    item::ItemRegistry,
};

const SLOT_SIZE: f32 = 54.0;
const ICON_SIZE: f32 = 36.0;
const SLOT_GAP: f32 = 5.0;
const GRID_WIDTH: f32 =
    SLOT_SIZE * HOTBAR_SLOT_COUNT as f32 + SLOT_GAP * (HOTBAR_SLOT_COUNT - 1) as f32;
const SELECTED_COLOR: Color = Color::srgb(1.0, 0.82, 0.2);
const SLOT_BORDER: Color = Color::srgba(0.58, 0.61, 0.67, 0.95);
const EMPTY_COLOR: Color = Color::srgb(0.10, 0.11, 0.13);

#[derive(Component)]
pub(super) struct InventoryPanelRoot;

#[derive(Component)]
pub(super) struct InventorySlotView(usize);

#[derive(Component)]
pub(super) struct InventorySlotIcon(usize);

#[derive(Component)]
pub(super) struct InventorySlotCount(usize);

#[derive(Component)]
pub(super) struct CursorHeldView;

#[derive(Component)]
pub(super) struct CursorHeldCount;

pub(super) fn spawn_inventory_panel(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Inventory Overlay"),
            InventoryPanelRoot,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.46)),
            GlobalZIndex(200),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Name::new("Inventory Panel"),
                    Node {
                        width: Val::Px(GRID_WIDTH + 36.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(13.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.055, 0.06, 0.075, 0.98)),
                    BorderColor::all(Color::srgba(0.42, 0.45, 0.52, 1.0)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Name::new("Inventory Title"),
                        Text::new("INVENTORY"),
                        TextFont {
                            font_size: FontSize::Px(22.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    spawn_slot_grid(
                        panel,
                        "Inventory Storage Grid",
                        HOTBAR_SLOT_COUNT,
                        INVENTORY_SLOT_COUNT,
                    );
                    panel.spawn((
                        Name::new("Hotbar Label"),
                        Text::new("HOTBAR"),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgba(0.8, 0.82, 0.87, 1.0)),
                    ));
                    spawn_slot_grid(panel, "Inventory Hotbar Grid", 0, HOTBAR_SLOT_COUNT);
                    panel.spawn((
                        Name::new("Inventory Help"),
                        Text::new("LMB: take / place    RMB: half / one    E or Esc: close"),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgba(0.72, 0.75, 0.8, 1.0)),
                    ));
                });
        });

    commands
        .spawn((
            Name::new("Cursor Held Stack"),
            CursorHeldView,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                width: Val::Px(46.0),
                height: Val::Px(46.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(EMPTY_COLOR),
            BorderColor::all(Color::WHITE),
            GlobalZIndex(300),
        ))
        .with_children(|held| {
            held.spawn((
                Name::new("Cursor Held Count"),
                CursorHeldCount,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(16.0),
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
                    bottom: Val::Px(1.0),
                    ..default()
                },
            ));
        });
}

fn spawn_slot_grid(
    parent: &mut ChildSpawnerCommands,
    name: &'static str,
    start: usize,
    count: usize,
) {
    parent
        .spawn((
            Name::new(name),
            Node {
                width: Val::Px(GRID_WIDTH),
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(SLOT_GAP),
                row_gap: Val::Px(SLOT_GAP),
                ..default()
            },
        ))
        .with_children(|grid| {
            for index in start..start + count {
                spawn_inventory_slot(grid, index);
            }
        });
}

fn spawn_inventory_slot(parent: &mut ChildSpawnerCommands, index: usize) {
    parent
        .spawn((
            Name::new(format!("Inventory Slot {}", index)),
            Button,
            InventorySlotView(index),
            Node {
                position_type: PositionType::Relative,
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.11, 0.14, 1.0)),
            BorderColor::all(SLOT_BORDER),
        ))
        .with_children(|slot| {
            slot.spawn((
                Name::new("Inventory Item Placeholder"),
                InventorySlotIcon(index),
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(EMPTY_COLOR),
                BorderColor::all(Color::srgba(0.02, 0.02, 0.025, 0.9)),
            ));
            slot.spawn((
                Name::new("Inventory Item Count"),
                InventorySlotCount(index),
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

pub(super) fn handle_inventory_clicks(
    state: Res<InventoryState>,
    mouse: Res<ButtonInput<MouseButton>>,
    slots: Query<(&Interaction, &InventorySlotView)>,
    mut inventory: ResMut<PlayerInventory>,
) {
    if !state.is_open() {
        return;
    }

    let click = if mouse.just_pressed(MouseButton::Left) {
        Some(InventoryClick::Left)
    } else if mouse.just_pressed(MouseButton::Right) {
        Some(InventoryClick::Right)
    } else {
        None
    };
    let Some(click) = click else {
        return;
    };

    let hovered = slots
        .iter()
        .find(|(interaction, _)| {
            matches!(**interaction, Interaction::Pressed | Interaction::Hovered)
        })
        .map(|(_, slot)| slot.0);
    if let Some(index) = hovered {
        inventory.click_slot(index, click);
    }
}

pub(super) fn sync_inventory_panel(
    state: Res<InventoryState>,
    inventory: Res<PlayerInventory>,
    registry: Res<ItemRegistry>,
    mut panel: Single<&mut Node, With<InventoryPanelRoot>>,
    mut slots: Query<(&InventorySlotView, &mut BorderColor)>,
    mut icons: Query<(&InventorySlotIcon, &mut BackgroundColor)>,
    mut counts: Query<(&InventorySlotCount, &mut Text)>,
) {
    panel.display = if state.is_open() {
        Display::Flex
    } else {
        Display::None
    };
    if !state.is_changed() && !inventory.is_changed() {
        return;
    }

    for (view, mut border) in &mut slots {
        let selected = view.0 < HOTBAR_SLOT_COUNT && view.0 == inventory.selected_hotbar_index();
        *border = BorderColor::all(if selected {
            SELECTED_COLOR
        } else {
            SLOT_BORDER
        });
    }
    for (view, mut background) in &mut icons {
        background.0 = inventory
            .slot(view.0)
            .map(|stack| item_color(&registry, stack.item()))
            .unwrap_or(EMPTY_COLOR);
    }
    for (view, mut text) in &mut counts {
        text.0 = inventory
            .slot(view.0)
            .map(|stack| stack.count().to_string())
            .unwrap_or_default();
    }
}

pub(super) fn follow_cursor_stack(
    state: Res<InventoryState>,
    inventory: Res<PlayerInventory>,
    registry: Res<ItemRegistry>,
    window: Single<&Window, With<PrimaryWindow>>,
    held: Single<(&mut Node, &mut BackgroundColor), With<CursorHeldView>>,
    mut count: Single<&mut Text, With<CursorHeldCount>>,
) {
    let (mut node, mut background) = held.into_inner();
    let Some(stack) = inventory.cursor_held_stack().filter(|_| state.is_open()) else {
        node.display = Display::None;
        count.0.clear();
        return;
    };

    node.display = Display::Flex;
    if let Some(position) = window.cursor_position() {
        node.left = Val::Px(position.x + 14.0);
        node.top = Val::Px(position.y + 14.0);
    }
    background.0 = item_color(&registry, stack.item());
    count.0 = stack.count().to_string();
}

fn item_color(registry: &ItemRegistry, item: crate::item::ItemId) -> Color {
    let [red, green, blue, alpha] = registry.debug_color(item);
    Color::srgba(red, green, blue, alpha)
}
