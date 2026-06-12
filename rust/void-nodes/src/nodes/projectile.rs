use godot::prelude::*;
use godot::classes::{Area3D, IArea3D, StaticBody3D};

use super::constants::{groups, methods, signals};

/// A projectile fired by the player or an enemy.
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Projectile {
    base: Base<Area3D>,

    #[export]
    speed: f32,
    #[export]
    damage: f32,
    #[export]
    lifetime: f32,
    /// If true, this projectile hurts the player on contact.
    #[export]
    is_enemy: bool,

    age: f32,
    direction: Vector3,
}

#[godot_api]
impl IArea3D for Projectile {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            speed: 50.0,
            damage: 10.0,
            lifetime: 3.0,
            is_enemy: false,
            age: 0.0,
            direction: Vector3::FORWARD,
        }
    }

    fn ready(&mut self) {
        if self.is_enemy {
            self.base_mut().add_to_group(groups::ENEMY_PROJECTILE);
        }

        // Match Portal/Lootbox pattern: enable monitoring, set layers, connect signal
        self.base_mut().set_monitoring(true);
        self.base_mut().set_collision_mask(1);  // Detect layer 1 (player + level geometry)
        self.base_mut().set_collision_layer(0); // Don't block anything

        let callable = self.base().callable(methods::ON_BODY_ENTERED);
        self.base_mut().connect(signals::BODY_ENTERED, &callable);
    }

    fn physics_process(&mut self, delta: f64) {
        let delta = delta as f32;
        self.age += delta;

        if self.age >= self.lifetime {
            self.base_mut().queue_free();
            return;
        }

        let movement = self.direction * self.speed * delta;
        let pos = self.base().get_global_position() + movement;
        self.base_mut().set_global_position(pos);
    }
}

#[godot_api]
impl Projectile {
    #[func]
    pub fn launch(&mut self, direction: Vector3, speed: f32, damage: f32) {
        self.direction = direction.normalized();
        self.speed = speed;
        self.damage = damage;
    }

    #[func]
    pub fn on_body_entered(&mut self, body: Gd<Node3D>) {
        // Level geometry stops every projectile, friendly or hostile.
        if body.clone().try_cast::<StaticBody3D>().is_ok() {
            self.base_mut().queue_free();
            return;
        }
        if !self.is_enemy {
            return;
        }
        // Only hurt the player
        if body.is_in_group(groups::PLAYER) && body.has_method(methods::TAKE_DAMAGE) {
            let mut body = body;
            body.call(methods::TAKE_DAMAGE, &[Variant::from(self.damage)]);
            self.base_mut().queue_free();
        }
    }
}
