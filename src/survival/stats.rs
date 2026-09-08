use bevy::prelude::*;

pub const MAX_HEALTH: f32 = 20.0;
pub const MAX_FOOD_LEVEL: u8 = 20;
const EXHAUSTION_PER_POINT: f32 = 4.0;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub(super) current: f32,
}

impl Health {
    pub const fn current(self) -> f32 {
        self.current
    }

    pub fn damage(&mut self, amount: f32) -> f32 {
        let previous = self.current;
        self.current = (self.current - amount.max(0.0)).clamp(0.0, MAX_HEALTH);
        previous - self.current
    }

    pub fn heal(&mut self, amount: f32) -> f32 {
        let previous = self.current;
        self.current = (self.current + amount.max(0.0)).clamp(0.0, MAX_HEALTH);
        self.current - previous
    }

    pub fn is_dead(self) -> bool {
        self.current <= 0.0
    }

    pub fn restore(&mut self) {
        self.current = MAX_HEALTH;
    }
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: MAX_HEALTH,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hunger {
    pub(super) food_level: u8,
    pub(super) saturation: f32,
    pub(super) exhaustion: f32,
}

impl Hunger {
    pub const fn food_level(self) -> u8 {
        self.food_level
    }

    pub const fn saturation(self) -> f32 {
        self.saturation
    }

    pub fn is_full(self) -> bool {
        self.food_level >= MAX_FOOD_LEVEL
    }

    pub fn can_sprint(self) -> bool {
        self.food_level > 6
    }

    pub fn add_exhaustion(&mut self, amount: f32) {
        self.exhaustion += amount.max(0.0);
        while self.exhaustion >= EXHAUSTION_PER_POINT {
            self.exhaustion -= EXHAUSTION_PER_POINT;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.food_level = self.food_level.saturating_sub(1);
            }
        }
    }

    pub fn eat(&mut self, nutrition: u8, saturation: f32) -> bool {
        if self.is_full() || nutrition == 0 {
            return false;
        }
        self.food_level = self
            .food_level
            .saturating_add(nutrition)
            .min(MAX_FOOD_LEVEL);
        self.saturation = (self.saturation + saturation.max(0.0)).min(self.food_level as f32);
        true
    }

    pub fn restore(&mut self) {
        *self = Self::default();
    }
}

impl Default for Hunger {
    fn default() -> Self {
        Self {
            food_level: MAX_FOOD_LEVEL,
            saturation: 5.0,
            exhaustion: 0.0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct SurvivalTracker {
    pub(super) last_y: f32,
    pub(super) fall_distance: f32,
    pub(super) was_grounded: bool,
    pub(super) regeneration_elapsed: f32,
    pub(super) starvation_elapsed: f32,
}

impl SurvivalTracker {
    pub fn new(spawn_y: f32) -> Self {
        Self {
            last_y: spawn_y,
            fall_distance: 0.0,
            was_grounded: true,
            regeneration_elapsed: 0.0,
            starvation_elapsed: 0.0,
        }
    }

    pub(super) fn reset_at(&mut self, y: f32) {
        *self = Self::new(y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_and_healing_are_clamped_to_minecraft_health() {
        let mut health = Health::default();

        assert_eq!(health.damage(7.5), 7.5);
        assert_eq!(health.current(), 12.5);
        assert_eq!(health.heal(100.0), 7.5);
        assert_eq!(health.current(), MAX_HEALTH);
        health.damage(100.0);
        assert!(health.is_dead());
    }

    #[test]
    fn exhaustion_consumes_saturation_before_food() {
        let mut hunger = Hunger {
            food_level: 10,
            saturation: 1.0,
            exhaustion: 0.0,
        };

        hunger.add_exhaustion(4.0);
        assert_eq!(hunger.saturation(), 0.0);
        assert_eq!(hunger.food_level(), 10);
        hunger.add_exhaustion(4.0);
        assert_eq!(hunger.food_level(), 9);
    }

    #[test]
    fn food_is_capped_and_restores_saturation() {
        let mut hunger = Hunger {
            food_level: 18,
            saturation: 0.0,
            exhaustion: 0.0,
        };

        assert!(hunger.eat(4, 2.4));
        assert_eq!(hunger.food_level(), 20);
        assert_eq!(hunger.saturation(), 2.4);
        assert!(!hunger.eat(4, 2.4));
    }
}
