//! HUD and user-interface systems.

mod crosshair;
mod first_person_hand;
mod inventory_panel;
mod item_icons;
mod survival_hud;

use bevy::prelude::*;

use crate::{
    inventory::{HOTBAR_SLOT_COUNT, InventoryState, PlayerInventory},
    item::ItemRegistry,
};

use item_icons::{ItemIconAssets, set_item_icon};

const UI_SCALE: f32 = 1.5;
const SLOT_SIZE: f32 = 40.0 * UI_SCALE;
const ICON_SIZE: f32 = 32.0 * UI_SCALE;
const HOTBAR_WIDTH: f32 = 364.0 * UI_SCALE;
const HOTBAR_HEIGHT: f32 = 44.0 * UI_SCALE;

/// Runtime switches for future HUD and menu presentation.
#[derive(Debug, Default, Resource)]
pub struct UiSettings {
    pub show_debug_overlay: bool,
}

#[derive(Component)]
struct HotbarIconView(usize);

#[derive(Component)]
struct HotbarSelectionView(usize);

#[derive(Component)]
struct HotbarCountView(usize);

#[derive(Component)]
struct HotbarSelectedName;

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
                    item_icons::load_item_icons,
                    spawn_hotbar,
                    inventory_panel::spawn_inventory_panel,
                    crosshair::spawn_crosshair,
                    survival_hud::spawn_survival_hud,
                    first_person_hand::spawn_first_person_hand,
                ),
            )
            .add_systems(Update, inventory_panel::handle_inventory_clicks)
            .add_systems(
                Last,
                (
                    sync_hotbar_ui,
                    inventory_panel::sync_inventory_panel,
                    inventory_panel::follow_cursor_stack,
                    inventory_panel::sync_item_tooltip,
                    crosshair::sync_crosshair_visibility,
                    survival_hud::sync_survival_hud,
                    first_person_hand::sync_first_person_hand,
                ),
            );
    }
}

fn spawn_hotbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            Name::new("Hotbar Root"),
            HotbarRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(18.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(100),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Selected Item Name"),
                HotbarSelectedName,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: Color::BLACK,
                },
                Node {
                    margin: UiRect::bottom(Val::Px(5.0)),
                    ..default()
                },
            ));
            root.spawn((
                Name::new("Hotbar"),
                ImageNode::new(asset_server.load("ui/faithful_hotbar.png")),
                Node {
                    position_type: PositionType::Relative,
                    width: Val::Px(HOTBAR_WIDTH),
                    height: Val::Px(HOTBAR_HEIGHT),
                    align_items: AlignItems::Center,
                    ..default()
                },
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
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((2.0 + index as f32 * 40.0) * UI_SCALE),
                top: Val::Px(2.0 * UI_SCALE),
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|slot| {
            slot.spawn((
                Name::new("Hotbar Selection"),
                HotbarSelectionView(index),
                ImageNode::new(Handle::default()),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-4.0 * UI_SCALE),
                    top: Val::Px(-3.0 * UI_SCALE),
                    width: Val::Px(48.0 * UI_SCALE),
                    height: Val::Px(46.0 * UI_SCALE),
                    ..default()
                },
                Visibility::Hidden,
            ));
            slot.spawn((
                Name::new("Item Icon"),
                HotbarIconView(index),
                ImageNode::default(),
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
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
    icon_assets: Res<ItemIconAssets>,
    registry: Res<ItemRegistry>,
    mut root: Single<&mut Node, With<HotbarRoot>>,
    mut selections: Query<
        (&HotbarSelectionView, &mut ImageNode, &mut Visibility),
        Without<HotbarIconView>,
    >,
    mut icons: Query<(&HotbarIconView, &mut ImageNode), Without<HotbarSelectionView>>,
    mut counts: Query<(&HotbarCountView, &mut Text), Without<HotbarSelectedName>>,
    mut selected_name: Single<&mut Text, (With<HotbarSelectedName>, Without<HotbarCountView>)>,
    asset_server: Res<AssetServer>,
) {
    root.display = if inventory_state.is_open() {
        Display::None
    } else {
        Display::Flex
    };

    if !inventory.is_changed() && !inventory_state.is_changed() {
        return;
    }

    for (view, mut image, mut visibility) in &mut selections {
        image.image = asset_server.load("ui/faithful_hotbar_selection.png");
        *visibility = if view.0 == inventory.selected_hotbar_index() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
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

    selected_name.0 = inventory
        .selected_stack()
        .map(|stack| registry.definition(stack.item()).name.to_owned())
        .unwrap_or_default();
}
