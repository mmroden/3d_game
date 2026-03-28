use godot::prelude::*;
use godot::classes::{CharacterBody3D, ICharacterBody3D};

/// 6DOF flight controller for the player ship.
/// Reads both keyboard and gamepad input, applies thrust and rotation.
#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct ShipController {
    base: Base<CharacterBody3D>,

    #[export]
    thrust_power: f32,
    #[export]
    rotation_speed: f32,
    #[export]
    damping: f32,

    linear_velocity: Vector3,
    angular_velocity: Vector3,
}

#[godot_api]
impl ICharacterBody3D for ShipController {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            base,
            thrust_power: 20.0,
            rotation_speed: 2.5,
            damping: 0.95,
            linear_velocity: Vector3::ZERO,
            angular_velocity: Vector3::ZERO,
        }
    }

    fn physics_process(&mut self, delta: f64) {
        let delta = delta as f32;
        let input = Input::singleton();

        // Gather movement input (keyboard + gamepad left stick + triggers)
        let forward = input.get_action_strength("move_forward") - input.get_action_strength("move_back");
        let strafe = input.get_action_strength("move_right") - input.get_action_strength("move_left");
        let vertical = input.get_action_strength("move_up") - input.get_action_strength("move_down");

        // Gather rotation input (keyboard + gamepad right stick + bumpers)
        let pitch = input.get_action_strength("look_down") - input.get_action_strength("look_up");
        let yaw = input.get_action_strength("look_left") - input.get_action_strength("look_right");
        let roll = input.get_action_strength("roll_right") - input.get_action_strength("roll_left");

        // Build thrust in local space
        let basis = self.base().get_transform().basis;
        let thrust = basis.col_c() * (-forward)  // -Z is forward in Godot
            + basis.col_a() * strafe
            + basis.col_b() * vertical;

        // Apply forces
        self.linear_velocity += thrust * self.thrust_power * delta;
        self.linear_velocity *= self.damping;

        self.angular_velocity += Vector3::new(pitch, yaw, roll) * self.rotation_speed * delta;
        self.angular_velocity *= self.damping;

        // Copy values out before mutably borrowing base
        let vel = self.linear_velocity;
        let rot = self.angular_velocity * delta;

        self.base_mut().set_velocity(vel);
        self.base_mut().move_and_slide();
        self.base_mut().rotate_x(rot.x);
        self.base_mut().rotate_y(rot.y);
        self.base_mut().rotate_z(rot.z);
    }
}
