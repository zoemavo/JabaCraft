use bevy::prelude::*;

use crate::{
    inventory::InventoryState,
    player::Player,
    survival::{Health, Hunger},
};

const ICON_COUNT: usize = 10;
const ICON_SIZE: f32 = 27.0;
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
                ..default()
            },
        ))
        .with_children(|row| {
            for index in 0..ICON_COUNT {
                row.spawn((
                    Name::new(format!("{name} Icon {}", index + 1)),
                    Node {
                        position_type: PositionType::Relative,
                        width: Val::Px(ICON_SIZE),
                        height: Val::Px(ICON_SIZE),
                        ..default()
                    },
                ))
                .with_children(|icon| {
                    icon.spawn((
                        ImageNode::new(assets.load(empty_texture)),
                        Node {
                            position_type: PositionType::Absolute,
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
    stats: Single<(Ref<Health>, Ref<Hunger>), With<Player>>,
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
    if !health.is_changed() && !hunger.is_changed() && !inventory_state.is_changed() {
        return;
    }

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
}
