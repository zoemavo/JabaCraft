use bevy::prelude::*;

use crate::{
    inventory::InventoryState,
    player::Player,
    survival::{Health, Hunger},
};

const ICON_COUNT: usize = 10;
const ICON_SIZE: f32 = 27.0;
const ICON_STEP: f32 = 24.0;
const ROW_EDGE_PADDING: f32 = 1.5;
const HOTBAR_TOP_OFFSET: f32 = 90.0;
const HUD_WIDTH: f32 = 546.0;

#[derive(Component)]
pub(super) struct SurvivalHudRoot;

#[derive(Clone, Copy)]
enum VitalKind {
    Health,
    Food,
}

#[derive(Component)]
pub(super) struct VitalFill {
    kind: VitalKind,
    index: usize,
}

pub(super) fn spawn_survival_hud(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            Name::new("Survival HUD"),
            SurvivalHudRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(HOTBAR_TOP_OFFSET),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(99),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Survival Vitals"),
                Node {
                    width: Val::Px(HUD_WIDTH),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                },
            ))
            .with_children(|vitals| {
                spawn_vital_row(
                    vitals,
                    &assets,
                    "Health",
                    VitalKind::Health,
                    "ui/survival/heart_empty.png",
                    FlexDirection::Row,
                );
                spawn_vital_row(
                    vitals,
                    &assets,
                    "Hunger",
                    VitalKind::Food,
                    "ui/survival/food_empty.png",
                    FlexDirection::RowReverse,
                );
            });
        });
}

fn spawn_vital_row(
    parent: &mut ChildSpawnerCommands,
    assets: &AssetServer,
    name: &'static str,
    kind: VitalKind,
    empty_texture: &'static str,
    direction: FlexDirection,
) {
    parent
        .spawn((
            Name::new(format!("{name} Bar")),
            Node {
                flex_direction: direction,
                padding: UiRect::horizontal(Val::Px(ROW_EDGE_PADDING)),
                ..default()
            },
        ))
        .with_children(|row| {
            for index in 0..ICON_COUNT {
                row.spawn((
                    Name::new(format!("{name} Icon {}", index + 1)),
                    Node {
                        position_type: PositionType::Relative,
                        width: Val::Px(ICON_STEP),
                        height: Val::Px(ICON_SIZE),
                        ..default()
                    },
                ))
                .with_children(|icon| {
                    icon.spawn((
                        ImageNode::new(assets.load(empty_texture)),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-(ICON_SIZE - ICON_STEP) * 0.5),
                            width: Val::Px(ICON_SIZE),
                            height: Val::Px(ICON_SIZE),
                            ..default()
                        },
                    ));
                    icon.spawn((
                        VitalFill { kind, index },
                        ImageNode::default(),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-(ICON_SIZE - ICON_STEP) * 0.5),
                            width: Val::Px(ICON_SIZE),
                            height: Val::Px(ICON_SIZE),
                            ..default()
                        },
                        Visibility::Hidden,
                    ));
                });
            }
        });
}

pub(super) fn sync_survival_hud(
    inventory_state: Res<InventoryState>,
    stats: Single<(&Health, &Hunger), With<Player>>,
    assets: Res<AssetServer>,
    mut root: Single<&mut Node, With<SurvivalHudRoot>>,
    mut fills: Query<(&VitalFill, &mut ImageNode, &mut Visibility)>,
) {
    root.display = if inventory_state.is_open() {
        Display::None
    } else {
        Display::Flex
    };

    let (health, hunger) = stats.into_inner();

    for (fill, mut image, mut visibility) in &mut fills {
        let value = match fill.kind {
            VitalKind::Health => health.current(),
            VitalKind::Food => f32::from(hunger.food_level()),
        };
        let state = icon_fill(value, fill.index);
        let texture = match (fill.kind, state) {
            (_, IconFill::Empty) => {
                *visibility = Visibility::Hidden;
                continue;
            }
            (VitalKind::Health, IconFill::Half) => "ui/survival/heart_half.png",
            (VitalKind::Health, IconFill::Full) => "ui/survival/heart_full.png",
            (VitalKind::Food, IconFill::Half) => "ui/survival/food_half.png",
            (VitalKind::Food, IconFill::Full) => "ui/survival/food_full.png",
        };
        image.image = assets.load(texture);
        *visibility = Visibility::Visible;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IconFill {
    Empty,
    Half,
    Full,
}

fn icon_fill(value: f32, index: usize) -> IconFill {
    let start = index as f32 * 2.0;
    if value >= start + 2.0 {
        IconFill::Full
    } else if value > start {
        IconFill::Half
    } else {
        IconFill::Empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_points_fill_ten_icons() {
        assert!((0..10).all(|index| icon_fill(20.0, index) == IconFill::Full));
    }

    #[test]
    fn odd_points_render_a_half_icon() {
        assert_eq!(icon_fill(5.0, 0), IconFill::Full);
        assert_eq!(icon_fill(5.0, 1), IconFill::Full);
        assert_eq!(icon_fill(5.0, 2), IconFill::Half);
        assert_eq!(icon_fill(5.0, 3), IconFill::Empty);
    }

    #[test]
    fn damaged_health_and_consumed_food_change_visible_icon_states() {
        let mut health = Health::default();
        let mut hunger = Hunger::default();

        health.damage(3.0);
        hunger.add_exhaustion(24.0);

        assert_eq!(health.current(), 17.0);
        assert_eq!(icon_fill(health.current(), 8), IconFill::Half);
        assert_eq!(icon_fill(health.current(), 9), IconFill::Empty);
        assert_eq!(hunger.food_level(), 19);
        assert_eq!(icon_fill(f32::from(hunger.food_level()), 9), IconFill::Half);
    }

    #[test]
    fn vital_rows_fit_inside_the_hotbar_width() {
        let row_width = ICON_COUNT as f32 * ICON_STEP + ROW_EDGE_PADDING * 2.0;

        assert!(row_width * 2.0 < HUD_WIDTH);
        assert_eq!(HUD_WIDTH, 364.0 * 1.5);
    }
}
