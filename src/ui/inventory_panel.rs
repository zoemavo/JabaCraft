use bevy::{prelude::*, window::PrimaryWindow};

use crate::inventory::{
    HOTBAR_SLOT_COUNT, INVENTORY_SLOT_COUNT, InventoryClick, InventoryState, PlayerInventory,
};

use super::item_icons::{ItemIconAssets, set_item_icon};

const UI_SCALE: f32 = 1.5;
const SLOT_SIZE: f32 = 36.0 * UI_SCALE;
const ICON_SIZE: f32 = 32.0 * UI_SCALE;
const GRID_WIDTH: f32 = SLOT_SIZE * HOTBAR_SLOT_COUNT as f32;
const PANEL_WIDTH: f32 = 352.0 * UI_SCALE;
const PANEL_HEIGHT: f32 = 187.0 * UI_SCALE;
const GRID_LEFT: f32 = 14.0 * UI_SCALE;
const STORAGE_TOP: f32 = 21.0 * UI_SCALE;
const HOTBAR_TOP: f32 = 139.0 * UI_SCALE;
const HOVER_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.32);
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

#[derive(Component)]
pub(super) struct CursorHeldIcon;

#[derive(Component)]
pub(super) struct ItemTooltip;

#[derive(Component)]
pub(super) struct ItemTooltipText;

pub(super) fn spawn_inventory_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
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
                    ImageNode::new(asset_server.load("ui/faithful_inventory.png")),
                    Node {
                        position_type: PositionType::Relative,
                        width: Val::Px(PANEL_WIDTH),
                        height: Val::Px(PANEL_HEIGHT),
                        ..default()
                    },
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Name::new("Inventory Title"),
                        Text::new("Inventory"),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.24, 0.24, 0.24)),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(GRID_LEFT),
                            top: Val::Px(5.0 * UI_SCALE),
                            ..default()
                        },
                    ));
                    spawn_slot_grid(
                        panel,
                        "Inventory Storage Grid",
                        HOTBAR_SLOT_COUNT,
                        INVENTORY_SLOT_COUNT,
                        STORAGE_TOP,
                    );
                    spawn_slot_grid(
                        panel,
                        "Inventory Hotbar Grid",
                        0,
                        HOTBAR_SLOT_COUNT,
                        HOTBAR_TOP,
                    );
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
                Name::new("Cursor Held Item Icon"),
                CursorHeldIcon,
                ImageNode::default(),
                Node {
                    width: Val::Px(36.0),
                    height: Val::Px(36.0),
                    ..default()
                },
            ));
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

    commands
        .spawn((
            Name::new("Item Tooltip"),
            ItemTooltip,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.01, 0.08, 0.94)),
            BorderColor::all(Color::srgb(0.25, 0.05, 0.5)),
            GlobalZIndex(400),
        ))
        .with_child((
            ItemTooltipText,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(17.0),
                ..default()
            },
            TextColor(Color::WHITE),
            TextShadow {
                offset: Vec2::splat(1.0),
                color: Color::BLACK,
            },
        ));
}

fn spawn_slot_grid(
    parent: &mut ChildSpawnerCommands,
    name: &'static str,
    start: usize,
    count: usize,
    top: f32,
) {
    parent
        .spawn((
            Name::new(name),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(GRID_LEFT),
                top: Val::Px(top),
                width: Val::Px(GRID_WIDTH),
                flex_wrap: FlexWrap::Wrap,
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
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|slot| {
            slot.spawn((
                Name::new("Inventory Item Icon"),
                InventorySlotIcon(index),
                ImageNode::default(),
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    ..default()
                },
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
                    right: Val::Px(2.0),
                    bottom: Val::Px(1.0),
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
    icon_assets: Res<ItemIconAssets>,
    mut panel: Single<&mut Node, With<InventoryPanelRoot>>,
    mut slots: Query<(&InventorySlotView, &Interaction, &mut BackgroundColor)>,
    mut icons: Query<(&InventorySlotIcon, &mut ImageNode)>,
    mut counts: Query<(&InventorySlotCount, &mut Text)>,
) {
    panel.display = if state.is_open() {
        Display::Flex
    } else {
        Display::None
    };
    for (_, interaction, mut background) in &mut slots {
        background.0 = if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            HOVER_COLOR
        } else {
            Color::NONE
        };
    }
    if !state.is_changed() && !inventory.is_changed() {
        return;
    }
    for (view, mut image) in &mut icons {
        set_item_icon(
            &mut image,
            &icon_assets,
            inventory.slot(view.0).map(|stack| stack.item()),
        );
    }
    for (view, mut text) in &mut counts {
        text.0 = inventory
            .slot(view.0)
            .map(|stack| stack.count().to_string())
            .unwrap_or_default();
    }
}

pub(super) fn sync_item_tooltip(
    state: Res<InventoryState>,
    inventory: Res<PlayerInventory>,
    registry: Res<crate::item::ItemRegistry>,
    window: Single<&Window, With<PrimaryWindow>>,
    slots: Query<(&Interaction, &InventorySlotView)>,
    mut tooltip: Single<&mut Node, With<ItemTooltip>>,
    mut text: Single<&mut Text, With<ItemTooltipText>>,
) {
    let hovered_item = state.is_open().then(|| {
        slots
            .iter()
            .find(|(interaction, _)| matches!(**interaction, Interaction::Hovered))
            .and_then(|(_, slot)| inventory.slot(slot.0))
            .map(|stack| stack.item())
    });
    let Some(Some(item)) = hovered_item else {
        tooltip.display = Display::None;
        text.0.clear();
        return;
    };

    tooltip.display = Display::Flex;
    if let Some(position) = window.cursor_position() {
        tooltip.left = Val::Px(position.x + 14.0);
        tooltip.top = Val::Px(position.y - 32.0);
    }
    text.0 = registry.definition(item).name.to_owned();
}

pub(super) fn follow_cursor_stack(
    state: Res<InventoryState>,
    inventory: Res<PlayerInventory>,
    icon_assets: Res<ItemIconAssets>,
    window: Single<&Window, With<PrimaryWindow>>,
    held: Single<(&mut Node, &mut BackgroundColor), With<CursorHeldView>>,
    mut icon: Single<&mut ImageNode, With<CursorHeldIcon>>,
    mut count: Single<&mut Text, With<CursorHeldCount>>,
) {
    let (mut node, mut background) = held.into_inner();
    let Some(stack) = inventory.cursor_held_stack().filter(|_| state.is_open()) else {
        node.display = Display::None;
        set_item_icon(&mut icon, &icon_assets, None);
        count.0.clear();
        return;
    };

    node.display = Display::Flex;
    if let Some(position) = window.cursor_position() {
        node.left = Val::Px(position.x + 14.0);
        node.top = Val::Px(position.y + 14.0);
    }
    background.0 = EMPTY_COLOR;
    set_item_icon(&mut icon, &icon_assets, Some(stack.item()));
    count.0 = stack.count().to_string();
}
