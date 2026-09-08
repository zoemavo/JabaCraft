use bevy::prelude::*;

use crate::{interaction::MiningProgress, inventory::InventoryState};

const CROSSHAIR_LENGTH: f32 = 18.0;
const CROSSHAIR_THICKNESS: f32 = 2.0;
const CROSSHAIR_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.92);

#[derive(Component)]
pub(super) struct CrosshairRoot;

#[derive(Component)]
pub(super) struct CrosshairProgressTrack;

#[derive(Component)]
pub(super) struct CrosshairProgressFill;

pub(super) fn spawn_crosshair(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Crosshair"),
            CrosshairRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            UiTransform::default(),
            GlobalZIndex(90),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Crosshair Horizontal"),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(CROSSHAIR_LENGTH),
                    height: Val::Px(CROSSHAIR_THICKNESS),
                    ..default()
                },
                BackgroundColor(CROSSHAIR_COLOR),
            ));
            root.spawn((
                Name::new("Crosshair Vertical"),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(CROSSHAIR_THICKNESS),
                    height: Val::Px(CROSSHAIR_LENGTH),
                    ..default()
                },
                BackgroundColor(CROSSHAIR_COLOR),
            ));
            root.spawn((
                Name::new("Mining Progress Track"),
                CrosshairProgressTrack,
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    width: Val::Px(42.0),
                    height: Val::Px(5.0),
                    margin: UiRect {
                        left: Val::Px(-21.0),
                        top: Val::Px(16.0),
                        ..default()
                    },
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            ))
            .with_child((
                Name::new("Mining Progress Fill"),
                CrosshairProgressFill,
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
            ));
        });
}

#[allow(clippy::type_complexity)]
pub(super) fn sync_crosshair(
    state: Res<InventoryState>,
    mining: Res<MiningProgress>,
    crosshair: Single<
        (&mut Node, &mut UiTransform),
        (
            With<CrosshairRoot>,
            Without<CrosshairProgressTrack>,
            Without<CrosshairProgressFill>,
        ),
    >,
    mut track: Single<
        &mut Node,
        (
            With<CrosshairProgressTrack>,
            Without<CrosshairRoot>,
            Without<CrosshairProgressFill>,
        ),
    >,
    mut fill: Single<
        &mut Node,
        (
            With<CrosshairProgressFill>,
            Without<CrosshairRoot>,
            Without<CrosshairProgressTrack>,
        ),
    >,
) {
    let (mut crosshair_node, mut crosshair_transform) = crosshair.into_inner();
    crosshair_node.display = if state.is_open() {
        Display::None
    } else {
        Display::Flex
    };
    track.display = if mining.is_active() && !state.is_open() {
        Display::Flex
    } else {
        Display::None
    };
    fill.width = Val::Percent(mining.normalized() * 100.0);
    crosshair_transform.scale = Vec2::splat(crosshair_scale(mining.normalized()));
}

fn crosshair_scale(progress: f32) -> f32 {
    1.0 + (progress.clamp(0.0, 1.0) * std::f32::consts::PI).sin() * 0.16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mining_animation_expands_then_returns_to_normal_at_completion() {
        assert_eq!(crosshair_scale(0.0), 1.0);
        assert!(crosshair_scale(0.5) > 1.1);
        assert!((crosshair_scale(1.0) - 1.0).abs() < 0.0001);
    }
}
