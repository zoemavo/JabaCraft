use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::{interaction::MiningProgress, inventory::InventoryState};

const HAND_WIDTH: f32 = 96.0;
const HAND_HEIGHT: f32 = 288.0;
const IDLE_ROTATION: f32 = -0.34;

#[derive(Component)]
pub(super) struct FirstPersonHandRoot;

#[derive(Component)]
pub(super) struct FirstPersonHandSprite;

pub(super) fn spawn_first_person_hand(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            Name::new("First Person Hand Root"),
            FirstPersonHandRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(240.0),
                height: Val::Px(360.0),
                overflow: Overflow::clip(),
                ..default()
            },
            GlobalZIndex(80),
        ))
        .with_child((
            Name::new("Steve Right Arm"),
            FirstPersonHandSprite,
            ImageNode::new(assets.load("ui/steve_hand.png")),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(-8.0),
                bottom: Val::Px(-92.0),
                width: Val::Px(HAND_WIDTH),
                height: Val::Px(HAND_HEIGHT),
                ..default()
            },
            UiTransform {
                rotation: Rot2::radians(IDLE_ROTATION),
                ..default()
            },
        ));
}

pub(super) fn sync_first_person_hand(
    inventory_state: Res<InventoryState>,
    mining: Res<MiningProgress>,
    mut root: Single<&mut Node, With<FirstPersonHandRoot>>,
    mut hand: Single<&mut UiTransform, With<FirstPersonHandSprite>>,
) {
    root.display = if inventory_state.is_open() {
        Display::None
    } else {
        Display::Flex
    };

    let pose = hand_pose(&mining);
    hand.translation = Val2::px(pose.translation.x, pose.translation.y);
    hand.rotation = Rot2::radians(pose.rotation);
    hand.scale = Vec2::splat(pose.scale);
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HandPose {
    translation: Vec2,
    rotation: f32,
    scale: f32,
}

fn hand_pose(mining: &MiningProgress) -> HandPose {
    if !mining.is_active() {
        return HandPose {
            translation: Vec2::ZERO,
            rotation: IDLE_ROTATION,
            scale: 1.0,
        };
    }

    // The arm repeats a Minecraft-like swing while also moving farther into
    // the strike as the current block approaches its breaking threshold.
    let swing = (mining.elapsed() * TAU * 1.8).sin().abs();
    let completion = mining.normalized();
    HandPose {
        translation: Vec2::new(-72.0 * swing - 10.0 * completion, 28.0 * swing),
        rotation: IDLE_ROTATION - 0.72 * swing,
        scale: 1.0 + 0.06 * swing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_hand_uses_stable_resting_pose() {
        assert_eq!(
            hand_pose(&MiningProgress::default()),
            HandPose {
                translation: Vec2::ZERO,
                rotation: IDLE_ROTATION,
                scale: 1.0,
            }
        );
    }
}
