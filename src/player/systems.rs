use std::f32::consts::FRAC_PI_2;

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use super::{
    Grounded, LookState, Noclip, Player, PlayerCamera, PlayerController, PlayerInWater,
    PlayerSettings, Velocity,
};
use crate::{
    inventory::InventoryState,
    survival::{GameMode, Hunger},
};

pub(super) fn capture_cursor_on_startup(
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    set_cursor_captured(&mut cursor, true);
}

pub(super) fn update_cursor_grab(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    inventory_state: Res<InventoryState>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if inventory_state.is_open() {
        set_cursor_captured(&mut cursor, false);
    } else if inventory_state.is_changed() {
        set_cursor_captured(&mut cursor, true);
    } else if keyboard.just_pressed(KeyCode::Escape) {
        set_cursor_captured(&mut cursor, false);
    } else if mouse_buttons.just_pressed(MouseButton::Left) {
        set_cursor_captured(&mut cursor, true);
    }
}

pub(super) fn collect_movement_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    inventory_state: Res<InventoryState>,
    mut players: Query<(&mut PlayerController, Option<&Hunger>), With<Player>>,
) {
    for (mut controller, hunger) in &mut players {
        if inventory_state.is_open() || cursor.grab_mode == CursorGrabMode::None {
            *controller = PlayerController::default();
            continue;
        }

        controller.movement = Vec2::new(
            key_axis(&keyboard, KeyCode::KeyA, KeyCode::KeyD),
            key_axis(&keyboard, KeyCode::KeyS, KeyCode::KeyW),
        )
        .normalize_or_zero();
        let sprint_pressed =
            keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
        controller.sprinting = sprint_pressed && hunger.is_none_or(|hunger| hunger.can_sprint());
        // Keeping Space held queues another jump as soon as collision marks
        // the player grounded, matching Minecraft's repeated jumping.
        controller.jump_requested = jump_requested(&keyboard);
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
    inventory_state: Res<InventoryState>,
    mode: Res<GameMode>,
    mut players: Query<(&mut Noclip, &mut Grounded), With<Player>>,
) {
    if *mode == GameMode::Survival {
        for (mut noclip, mut grounded) in &mut players {
            if noclip.0 {
                noclip.0 = false;
                grounded.0 = false;
            }
        }
        return;
    }
    if inventory_state.is_open() || !keyboard.just_pressed(KeyCode::F4) {
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
    inventory_state: Res<InventoryState>,
    settings: Res<PlayerSettings>,
    mut players: Query<&mut LookState, With<Player>>,
) {
    if inventory_state.is_open()
        || cursor.grab_mode == CursorGrabMode::None
        || mouse_motion.delta == Vec2::ZERO
    {
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
            &PlayerInWater,
            &mut Velocity,
            &mut Grounded,
        ),
        With<Player>,
    >,
) {
    let delta_seconds = time.delta_secs();
    for (controller, look, noclip, water, mut velocity, mut grounded) in &mut players {
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

        if water.is_in_water() {
            velocity.0 = swimming_velocity(velocity.0, controller, look, delta_seconds, &settings);
            continue;
        }

        let target = walking_target_velocity(controller, look, &settings);
        let horizontal = Vec3::new(velocity.0.x, 0.0, velocity.0.z);
        let rate = if controller.movement == Vec2::ZERO {
            settings.deceleration
        } else {
            settings.acceleration
        };
        let next_horizontal = clamp_horizontal_speed(
            move_towards(horizontal, target, rate.abs() * delta_seconds),
            horizontal_speed_limit(controller, &settings),
        );
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

fn swimming_velocity(
    current: Vec3,
    controller: &PlayerController,
    look: &LookState,
    delta_seconds: f32,
    settings: &PlayerSettings,
) -> Vec3 {
    let dt = delta_seconds.max(0.0);
    let speed = settings.swim_speed.max(0.0);
    let direction = Vec3::new(controller.movement.x, 0.0, -controller.movement.y);
    let target = Quat::from_rotation_y(look.yaw) * direction.normalize_or_zero() * speed;
    let mut next = clamp_horizontal_speed(
        move_towards(
            Vec3::new(current.x, 0.0, current.z),
            target,
            settings.water_acceleration.abs() * dt,
        ),
        speed,
    );
    let rise = settings.swim_up_speed.abs();
    let sink = settings.water_terminal_velocity.abs();
    let vertical = current.y.clamp(-sink, rise);
    next.y = if controller.jump_requested {
        (vertical + settings.water_acceleration.abs() * dt).min(rise)
    } else {
        (vertical - settings.water_gravity.abs() * dt).max(-sink)
    };
    next
}

fn walking_target_velocity(
    controller: &PlayerController,
    look: &LookState,
    settings: &PlayerSettings,
) -> Vec3 {
    let speed = horizontal_speed_limit(controller, settings);
    let local_direction = Vec3::new(controller.movement.x, 0.0, -controller.movement.y);

    Quat::from_rotation_y(look.yaw) * local_direction * speed
}

fn horizontal_speed_limit(controller: &PlayerController, settings: &PlayerSettings) -> f32 {
    if controller.sprinting && controller.jump_requested && controller.movement != Vec2::ZERO {
        settings.bunny_hop_speed.max(settings.sprint_speed)
    } else if controller.sprinting {
        settings.sprint_speed
    } else {
        settings.walk_speed
    }
    .max(0.0)
}

fn clamp_horizontal_speed(velocity: Vec3, limit: f32) -> Vec3 {
    let limit = limit.max(0.0);
    let speed = velocity.length();
    if speed > limit && speed > 0.0 {
        velocity * (limit / speed)
    } else {
        velocity
    }
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

fn jump_requested(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::Space)
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
    fn swimming_slows_fast_entry_and_ignores_sprint_bunny_hop() {
        let settings = PlayerSettings::default();
        let controller = PlayerController {
            movement: Vec2::Y,
            sprinting: true,
            jump_requested: true,
            ..default()
        };
        let next = swimming_velocity(
            Vec3::new(20.0, -50.0, 0.0),
            &controller,
            &LookState::default(),
            1.0 / 64.0,
            &settings,
        );
        assert!(Vec2::new(next.x, next.z).length() <= settings.swim_speed + 0.0001);
        assert!(next.y >= -settings.water_terminal_velocity);
        assert!(settings.water_gravity < settings.gravity);
    }

    #[test]
    fn held_space_swims_up_without_ground_and_release_sinks_slowly() {
        let settings = PlayerSettings::default();
        let mut controller = PlayerController {
            jump_requested: true,
            ..default()
        };
        let mut velocity = Vec3::ZERO;
        for _ in 0..128 {
            velocity = swimming_velocity(
                velocity,
                &controller,
                &LookState::default(),
                1.0 / 64.0,
                &settings,
            );
        }
        assert_eq!(velocity.y, settings.swim_up_speed);
        controller.jump_requested = false;
        for _ in 0..256 {
            velocity = swimming_velocity(
                velocity,
                &controller,
                &LookState::default(),
                1.0 / 64.0,
                &settings,
            );
        }
        assert_eq!(velocity.y, -settings.water_terminal_velocity);
    }

    #[test]
    fn held_space_keeps_requesting_jumps_after_the_initial_press() {
        let mut keyboard = ButtonInput::default();
        keyboard.press(KeyCode::Space);
        keyboard.clear_just_pressed(KeyCode::Space);

        assert!(!keyboard.just_pressed(KeyCode::Space));
        assert!(jump_requested(&keyboard));

        keyboard.release(KeyCode::Space);
        assert!(!jump_requested(&keyboard));
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
    fn sprint_bunny_hop_has_a_fun_but_bounded_speed() {
        let settings = PlayerSettings::default();
        let controller = PlayerController {
            movement: Vec2::Y,
            sprinting: true,
            jump_requested: true,
            ..Default::default()
        };

        let target = walking_target_velocity(&controller, &LookState::default(), &settings);
        let excessive = target.normalize_or_zero() * 100.0;

        assert_eq!(target.length(), settings.bunny_hop_speed);
        assert!(settings.bunny_hop_speed > settings.sprint_speed);
        assert!(settings.bunny_hop_speed < settings.sprint_speed * 1.5);
        assert_vec3_close(
            clamp_horizontal_speed(excessive, settings.bunny_hop_speed),
            target,
        );
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
