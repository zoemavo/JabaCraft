use bevy::prelude::*;

use crate::inventory::InventoryState;

const CROSSHAIR_LENGTH: f32 = 18.0;
const CROSSHAIR_THICKNESS: f32 = 2.0;
const CROSSHAIR_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.92);

#[derive(Component)]
pub(super) struct CrosshairRoot;

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
        });
}

pub(super) fn sync_crosshair_visibility(
    state: Res<InventoryState>,
    mut crosshair: Single<&mut Node, With<CrosshairRoot>>,
) {
    if !state.is_changed() {
        return;
    }
    crosshair.display = if state.is_open() {
        Display::None
    } else {
        Display::Flex
    };
}
