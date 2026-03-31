use godot::prelude::*;
use godot::classes::{Area3D, IArea3D};

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
            age: 0.0,
            direction: Vector3::FORWARD,
        }
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
}
