use std::f32::consts::FRAC_PI_2;

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use super::{
    Grounded, LookState, Noclip, Player, PlayerCamera, PlayerController, PlayerSettings, Velocity,
};

pub(super) fn capture_cursor_on_startup(
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    set_cursor_captured(&mut cursor, true);
}

pub(super) fn update_cursor_grab(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        set_cursor_captured(&mut cursor, false);
    } else if mouse_buttons.just_pressed(MouseButton::Left) {
        set_cursor_captured(&mut cursor, true);
    }
}

pub(super) fn collect_movement_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut players: Query<&mut PlayerController, With<Player>>,
) {
    for mut controller in &mut players {
        if cursor.grab_mode == CursorGrabMode::None {
            *controller = PlayerController::default();
            continue;
        }

        controller.movement = Vec2::new(
            key_axis(&keyboard, KeyCode::KeyA, KeyCode::KeyD),
            key_axis(&keyboard, KeyCode::KeyS, KeyCode::KeyW),
        )
        .normalize_or_zero();
        controller.sprinting =
            keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
        controller.jump_requested = keyboard.just_pressed(KeyCode::Space);
        controller.vertical_movement = if keyboard.pressed(KeyCode::Space) {
            1.0
        } else if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
            -1.0
        } else {
            0.0
        };
    }
}

pub(super) fn toggle_noclip(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut players: Query<(&mut Noclip, &mut Grounded), With<Player>>,
) {
    if !keyboard.just_pressed(KeyCode::F4) {
        return;
    }

    for (mut noclip, mut grounded) in &mut players {
        noclip.0 = !noclip.0;
        grounded.0 = false;
        info!(
            "Noclip mode {}",
            if noclip.0 { "enabled" } else { "disabled" }
        );
    }
}

pub(super) fn collect_look_input(
    mouse_motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    settings: Res<PlayerSettings>,
    mut players: Query<&mut LookState, With<Player>>,
) {
    if cursor.grab_mode == CursorGrabMode::None || mouse_motion.delta == Vec2::ZERO {
        return;
    }

    let pitch_limit = settings.pitch_limit.abs().min(FRAC_PI_2 - 0.001);
    for mut look in &mut players {
        look.yaw -= mouse_motion.delta.x * settings.mouse_sensitivity;
        look.pitch = (look.pitch - mouse_motion.delta.y * settings.mouse_sensitivity)
            .clamp(-pitch_limit, pitch_limit);
    }
}

pub(super) fn update_velocity(
    time: Res<Time>,
    settings: Res<PlayerSettings>,
    mut players: Query<
        (
            &PlayerController,
            &LookState,
            &Noclip,
            &mut Velocity,
            &mut Grounded,
        ),
        With<Player>,
    >,
) {
    let delta_seconds = time.delta_secs();
    for (controller, look, noclip, mut velocity, mut grounded) in &mut players {
        if noclip.0 {
            let target = noclip_target_velocity(controller, look, &settings);
            let rate = if target == Vec3::ZERO {
                settings.deceleration
            } else {
                settings.acceleration
            };
            velocity.0 = move_towards(velocity.0, target, rate.abs() * delta_seconds);
            grounded.0 = false;
            continue;
        }

        let target = walking_target_velocity(controller, look, &settings);
        let horizontal = Vec3::new(velocity.0.x, 0.0, velocity.0.z);
        let rate = if controller.movement == Vec2::ZERO {
            settings.deceleration
        } else {
            settings.acceleration
        };
        let next_horizontal = move_towards(horizontal, target, rate.abs() * delta_seconds);
        velocity.0.x = next_horizontal.x;
        velocity.0.z = next_horizontal.z;

        if controller.jump_requested && grounded.0 {
            velocity.0.y = jump_velocity(&settings);
            grounded.0 = false;
        } else {
            velocity.0.y = velocity_after_gravity(velocity.0.y, delta_seconds, &settings);
        }
    }
}

fn walking_target_velocity(
    controller: &PlayerController,
    look: &LookState,
    settings: &PlayerSettings,
) -> Vec3 {
    let speed = if controller.sprinting {
        settings.sprint_speed
    } else {
        settings.walk_speed
    }
    .max(0.0);
    let local_direction = Vec3::new(controller.movement.x, 0.0, -controller.movement.y);

    Quat::from_rotation_y(look.yaw) * local_direction * speed
}

fn noclip_target_velocity(
    controller: &PlayerController,
    look: &LookState,
    settings: &PlayerSettings,
) -> Vec3 {
    let yaw = Quat::from_rotation_y(look.yaw);
    let camera_rotation = yaw * Quat::from_rotation_x(look.pitch);
    let right = yaw * Vec3::X;
    let forward = camera_rotation * Vec3::NEG_Z;
    let direction = right * controller.movement.x
        + forward * controller.movement.y
        + Vec3::Y * controller.vertical_movement;

    direction.normalize_or_zero() * settings.noclip_speed.max(0.0)
}

fn move_towards(current: Vec3, target: Vec3, max_delta: f32) -> Vec3 {
    let delta = target - current;
    let distance = delta.length();
    if distance <= max_delta || distance == 0.0 {
        target
    } else {
        current + delta / distance * max_delta.max(0.0)
    }
}

fn jump_velocity(settings: &PlayerSettings) -> f32 {
    settings.jump_speed.max(0.0)
}

fn velocity_after_gravity(current: f32, delta_seconds: f32, settings: &PlayerSettings) -> f32 {
    (current - settings.gravity.abs() * delta_seconds.max(0.0))
        .max(-settings.terminal_velocity.abs())
}

pub(super) fn sync_view_transforms(
    player: Single<(&LookState, &mut Transform), (With<Player>, Without<PlayerCamera>)>,
    mut camera: Single<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    let (look, mut player_transform) = player.into_inner();
    player_transform.rotation = Quat::from_rotation_y(look.yaw);
    camera.rotation = Quat::from_rotation_x(look.pitch);
}

fn key_axis(keyboard: &ButtonInput<KeyCode>, negative: KeyCode, positive: KeyCode) -> f32 {
    let negative = if keyboard.pressed(negative) { 1.0 } else { 0.0 };
    let positive = if keyboard.pressed(positive) { 1.0 } else { 0.0 };
    positive - negative
}

fn set_cursor_captured(cursor: &mut CursorOptions, captured: bool) {
    let next_mode = if captured {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };

    if cursor.grab_mode == next_mode && cursor.visible == !captured {
        return;
    }

    cursor.grab_mode = next_mode;
    cursor.visible = !captured;
    info!(
        "Mouse cursor {}",
        if captured { "captured" } else { "released" }
    );
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;

    fn assert_vec3_close(actual: Vec3, expected: Vec3) {
        assert!(
            actual.distance(expected) < 0.0001,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn jump_uses_configured_upward_speed() {
        let settings = PlayerSettings {
            jump_speed: 9.5,
            ..Default::default()
        };

        assert_eq!(jump_velocity(&settings), 9.5);
    }

    #[test]
    fn gravity_accelerates_falling_and_has_a_terminal_speed() {
        let settings = PlayerSettings {
            gravity: 20.0,
            terminal_velocity: 37.0,
            ..Default::default()
        };

        assert_eq!(velocity_after_gravity(0.0, 0.25, &settings), -5.0);
        assert_eq!(velocity_after_gravity(-36.0, 1.0, &settings), -37.0);
    }

    #[test]
    fn acceleration_is_independent_of_frame_rate() {
        let target = Vec3::new(10.0, 0.0, 0.0);
        let acceleration = 6.0;
        let simulate = |frames: usize| {
            let mut velocity = Vec3::ZERO;
            for _ in 0..frames {
                velocity = move_towards(velocity, target, acceleration * (1.0 / frames as f32));
            }
            velocity
        };

        assert_vec3_close(simulate(60), Vec3::new(6.0, 0.0, 0.0));
        assert_vec3_close(simulate(144), Vec3::new(6.0, 0.0, 0.0));
        assert_vec3_close(simulate(60), simulate(144));
    }

    #[test]
    fn deceleration_reduces_velocity_toward_zero() {
        let velocity = move_towards(Vec3::new(5.0, 0.0, 0.0), Vec3::ZERO, 3.0);

        assert_vec3_close(velocity, Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn walking_direction_uses_camera_yaw_and_sprint_speed() {
        let settings = PlayerSettings::default();
        let controller = PlayerController {
            movement: Vec2::Y,
            sprinting: true,
            ..Default::default()
        };
        let look = LookState {
            yaw: FRAC_PI_2,
            pitch: 0.7,
        };

        let target = walking_target_velocity(&controller, &look, &settings);

        assert_vec3_close(target, Vec3::new(-settings.sprint_speed, 0.0, 0.0));
    }

    #[test]
    fn noclip_forward_follows_camera_pitch_and_supports_vertical_input() {
        let settings = PlayerSettings::default();
        let look = LookState {
            yaw: 0.0,
            pitch: FRAC_PI_2,
        };
        let forward = PlayerController {
            movement: Vec2::Y,
            ..Default::default()
        };
        let ascending = PlayerController {
            vertical_movement: 1.0,
            ..Default::default()
        };

        assert_vec3_close(
            noclip_target_velocity(&forward, &look, &settings),
            Vec3::Y * settings.noclip_speed,
        );
        assert_vec3_close(
            noclip_target_velocity(&ascending, &LookState::default(), &settings),
            Vec3::Y * settings.noclip_speed,
        );
    }
}
