use bevy::prelude::*;

use crate::persistence::SaveStatus;

#[derive(Component)]
pub(super) struct SaveIndicator;

pub(super) fn spawn_save_indicator(mut commands: Commands) {
    commands.spawn((
        Name::new("Save Indicator"),
        SaveIndicator,
        Text::new("Saving..."),
        TextFont {
            font_size: FontSize::Px(17.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
        TextShadow {
            offset: Vec2::splat(1.5),
            color: Color::BLACK,
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(18.0),
            right: Val::Px(20.0),
            ..default()
        },
        GlobalZIndex(200),
        Visibility::Hidden,
    ));
}

pub(super) fn sync_save_indicator(
    status: Res<SaveStatus>,
    mut indicator: Single<&mut Visibility, With<SaveIndicator>>,
) {
    if !status.is_changed() {
        return;
    }
    **indicator = if status.saving {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indicator_tracks_background_save_state() {
        let mut app = App::new();
        app.init_resource::<SaveStatus>()
            .add_systems(Startup, spawn_save_indicator)
            .add_systems(Last, sync_save_indicator);
        app.update();

        let world = app.world_mut();
        let visibility = world
            .query_filtered::<&Visibility, With<SaveIndicator>>()
            .single(world)
            .unwrap();
        assert_eq!(*visibility, Visibility::Hidden);

        world.resource_mut::<SaveStatus>().saving = true;
        app.update();
        let world = app.world_mut();
        let visibility = world
            .query_filtered::<&Visibility, With<SaveIndicator>>()
            .single(world)
            .unwrap();
        assert_eq!(*visibility, Visibility::Visible);
    }
}
