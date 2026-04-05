//! Pure-data projectile fired by enemies toward the player.
//! No Godot dependency — position/damage/lifetime logic only.

use crate::newtypes::Damage;

/// Status after a tick: still in flight, or expired.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectileStatus {
    Flying,
    Expired,
}

/// A projectile's logical state: origin, direction, speed, damage, lifetime.
#[derive(Debug, Clone)]
pub struct EnemyProjectile {
    origin: [f32; 3],
    direction: [f32; 3],
    speed: f32,
    dmg: Damage,
    lifetime: f32,
    current_age: f32,
}

impl EnemyProjectile {
    pub fn new(
        origin: [f32; 3],
        direction: [f32; 3],
        speed: f32,
        damage: Damage,
        lifetime: f32,
    ) -> Self {
        Self {
            origin,
            direction,
            speed,
            dmg: damage,
            lifetime,
            current_age: 0.0,
        }
    }

    /// Advance the projectile by `delta` seconds. Returns `Expired` if past lifetime.
    pub fn tick(&mut self, delta: f32) -> ProjectileStatus {
        self.current_age += delta;
        if self.current_age >= self.lifetime {
            ProjectileStatus::Expired
        } else {
            ProjectileStatus::Flying
        }
    }

    /// Current position: origin + direction * speed * age.
    pub fn position(&self) -> [f32; 3] {
        [
            self.origin[0] + self.direction[0] * self.speed * self.current_age,
            self.origin[1] + self.direction[1] * self.speed * self.current_age,
            self.origin[2] + self.direction[2] * self.speed * self.current_age,
        ]
    }

    pub fn age(&self) -> f32 {
        self.current_age
    }

    pub fn damage(&self) -> Damage {
        self.dmg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_projectile_starts_at_origin() {
        let proj = EnemyProjectile::new(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            15.0,
            Damage::new(5.0),
            3.0,
        );
        assert_eq!(proj.position(), [0.0, 0.0, 0.0]);
        assert_eq!(proj.age(), 0.0);
        assert_eq!(proj.damage(), Damage::new(5.0));
    }

    #[test]
    fn tick_advances_age_and_stays_flying() {
        let mut proj = EnemyProjectile::new(
            [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 15.0, Damage::new(5.0), 3.0,
        );
        let status = proj.tick(0.5);
        assert_eq!(status, ProjectileStatus::Flying);
        assert!((proj.age() - 0.5).abs() < 0.001);
    }

    #[test]
    fn position_moves_along_direction() {
        let mut proj = EnemyProjectile::new(
            [1.0, 2.0, 3.0], [1.0, 0.0, 0.0], 10.0, Damage::new(1.0), 5.0,
        );
        proj.tick(1.0);
        let pos = proj.position();
        assert!((pos[0] - 11.0).abs() < 0.001);
        assert!((pos[1] - 2.0).abs() < 0.001);
        assert!((pos[2] - 3.0).abs() < 0.001);
    }

    #[test]
    fn expires_after_lifetime() {
        let mut proj = EnemyProjectile::new(
            [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 15.0, Damage::new(5.0), 2.0,
        );
        assert_eq!(proj.tick(1.9), ProjectileStatus::Flying);
        assert_eq!(proj.tick(0.2), ProjectileStatus::Expired);
    }

    #[test]
    fn damage_preserved_through_flight() {
        let mut proj = EnemyProjectile::new(
            [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 20.0, Damage::new(7.5), 5.0,
        );
        proj.tick(2.0);
        assert_eq!(proj.damage(), Damage::new(7.5));
    }
}
