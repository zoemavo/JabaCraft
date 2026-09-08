use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    generation::GenerationSettings,
    inventory::{InventoryState, PlayerInventory},
    item::ItemRegistry,
    player::{Grounded, Noclip, Player, PlayerController, Velocity},
};

use super::{
    EatingProgress, GameMode, Health, Hunger, RespawnPoint, SurvivalSettings, SurvivalTracker,
};

#[allow(clippy::type_complexity)]
pub(super) fn update_survival_stats(
    time: Res<Time>,
    mode: Res<GameMode>,
    settings: Res<SurvivalSettings>,
    generation: Res<GenerationSettings>,
    respawn: Res<RespawnPoint>,
    player: Single<
        (
            &mut Transform,
            &mut Velocity,
            &Grounded,
            &Noclip,
            &PlayerController,
            &mut Health,
            &mut Hunger,
            &mut SurvivalTracker,
        ),
        With<Player>,
    >,
) {
    let (
        mut transform,
        mut velocity,
        grounded,
        noclip,
        controller,
        mut health,
        mut hunger,
        mut tracker,
    ) = player.into_inner();
    let delta_seconds = time.delta_secs().max(0.0);

    if *mode == GameMode::Creative || noclip.0 {
        tracker.reset_at(transform.translation.y);
        return;
    }

    update_fall_damage(
        transform.translation.y,
        grounded.0,
        &mut health,
        &mut tracker,
        &settings,
    );

    let horizontal_distance = Vec2::new(velocity.0.x, velocity.0.z).length() * delta_seconds;
    if controller.sprinting && horizontal_distance > 0.0 {
        hunger.add_exhaustion(horizontal_distance * settings.sprint_exhaustion_per_block);
    }
    if tracker.was_grounded && !grounded.0 && controller.jump_requested {
        hunger.add_exhaustion(if controller.sprinting {
            settings.sprint_jump_exhaustion
        } else {
            settings.jump_exhaustion
        });
    }

    update_regeneration_and_starvation(
        delta_seconds,
        &mut health,
        &mut hunger,
        &mut tracker,
        &settings,
    );

    tracker.was_grounded = grounded.0;
    tracker.last_y = transform.translation.y;

    if transform.translation.y < generation.world_min_y as f32 - 16.0 {
        health.damage(Health::default().current());
    }
    if health.is_dead() {
        respawn_player(
            &mut transform,
            &mut velocity,
            &mut health,
            &mut hunger,
            &mut tracker,
            *respawn,
        );
    }
}

fn update_fall_damage(
    current_y: f32,
    grounded: bool,
    health: &mut Health,
    tracker: &mut SurvivalTracker,
    settings: &SurvivalSettings,
) {
    if grounded {
        if !tracker.was_grounded {
            health.damage(fall_damage(
                tracker.fall_distance,
                settings.safe_fall_distance,
            ));
        }
        tracker.fall_distance = 0.0;
    } else if current_y < tracker.last_y {
        tracker.fall_distance += tracker.last_y - current_y;
    } else if current_y > tracker.last_y {
        tracker.fall_distance = 0.0;
    }
}

fn update_regeneration_and_starvation(
    delta_seconds: f32,
    health: &mut Health,
    hunger: &mut Hunger,
    tracker: &mut SurvivalTracker,
    settings: &SurvivalSettings,
) {
    if health.current() < super::MAX_HEALTH
        && hunger.food_level() >= settings.regeneration_food_level
    {
        tracker.regeneration_elapsed += delta_seconds;
        if tracker.regeneration_elapsed >= settings.regeneration_interval_seconds {
            tracker.regeneration_elapsed -= settings.regeneration_interval_seconds;
            health.heal(1.0);
            hunger.add_exhaustion(settings.regeneration_exhaustion);
        }
    } else {
        tracker.regeneration_elapsed = 0.0;
    }

    if hunger.food_level() == 0 && health.current() > 1.0 {
        tracker.starvation_elapsed += delta_seconds;
        if tracker.starvation_elapsed >= settings.starvation_interval_seconds {
            tracker.starvation_elapsed -= settings.starvation_interval_seconds;
            health.damage(1.0);
        }
    } else {
        tracker.starvation_elapsed = 0.0;
    }
}

fn fall_damage(distance: f32, safe_distance: f32) -> f32 {
    (distance - safe_distance.max(0.0)).ceil().max(0.0)
}

fn respawn_player(
    transform: &mut Transform,
    velocity: &mut Velocity,
    health: &mut Health,
    hunger: &mut Hunger,
    tracker: &mut SurvivalTracker,
    respawn: RespawnPoint,
) {
    transform.translation = respawn.0;
    velocity.0 = Vec3::ZERO;
    health.restore();
    hunger.restore();
    tracker.reset_at(respawn.0.y);
    info!("Player died and respawned");
}

#[allow(clippy::too_many_arguments)]
pub(super) fn eat_selected_food(
    time: Res<Time>,
    mode: Res<GameMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    inventory_state: Res<InventoryState>,
    item_registry: Res<ItemRegistry>,
    mut inventory: ResMut<PlayerInventory>,
    mut hunger: Single<&mut Hunger, With<Player>>,
    mut progress: ResMut<EatingProgress>,
) {
    if *mode != GameMode::Survival
        || inventory_state.is_open()
        || cursor.grab_mode == CursorGrabMode::None
        || !mouse.pressed(MouseButton::Right)
        || hunger.is_full()
    {
        progress.reset();
        return;
    }

    let Some(item) = inventory.selected_stack().map(|stack| stack.item()) else {
        progress.reset();
        return;
    };
    let Some(food) = item_registry.food(item) else {
        progress.reset();
        return;
    };

    if progress.item != Some(item) {
        progress.item = Some(item);
        progress.elapsed = 0.0;
    }
    progress.elapsed += time.delta_secs().max(0.0);
    if progress.elapsed < progress.required_seconds {
        return;
    }

    if inventory.consume_selected_one() && hunger.eat(food.nutrition, food.saturation) {
        debug!("Ate {}", item_registry.definition(item).name);
    }
    progress.reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_three_blocks_of_falling_are_safe() {
        assert_eq!(fall_damage(3.0, 3.0), 0.0);
        assert_eq!(fall_damage(3.01, 3.0), 1.0);
        assert_eq!(fall_damage(7.0, 3.0), 4.0);
    }

    #[test]
    fn starvation_on_normal_difficulty_stops_at_half_a_heart() {
        let settings = SurvivalSettings::default();
        let mut health = Health { current: 1.0 };
        let mut hunger = Hunger {
            food_level: 0,
            saturation: 0.0,
            exhaustion: 0.0,
        };
        let mut tracker = SurvivalTracker::new(0.0);

        update_regeneration_and_starvation(
            100.0,
            &mut health,
            &mut hunger,
            &mut tracker,
            &settings,
        );

        assert_eq!(health.current(), 1.0);
    }

    #[test]
    fn well_fed_player_regenerates_and_builds_exhaustion() {
        let settings = SurvivalSettings::default();
        let mut health = Health { current: 10.0 };
        let mut hunger = Hunger::default();
        let mut tracker = SurvivalTracker::new(0.0);

        update_regeneration_and_starvation(
            settings.regeneration_interval_seconds,
            &mut health,
            &mut hunger,
            &mut tracker,
            &settings,
        );

        assert_eq!(health.current(), 11.0);
        assert!(hunger.saturation() < 5.0);
    }
}
