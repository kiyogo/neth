use crate::entity::ai::control::move_control::{MoveControl, Operation};
use crate::entity::ai::control::{Control, MoveControlTrait};
use crate::entity::mob::Mob;
use pumpkin_data::attributes::Attributes;
use pumpkin_util::math::lerp;
use pumpkin_util::math::vector3::Vector3;
use std::sync::atomic::Ordering;

/// Vanilla `Guardian.GuardianMoveControl`: swimming motion with sinusoidal pulses and 3D navigation.
#[derive(Default)]
pub struct GuardianMoveControl {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
    operation: Operation,
}

impl Control for GuardianMoveControl {}

impl MoveControlTrait for GuardianMoveControl {
    fn tick(&mut self, mob: &dyn Mob) {
        let mob_entity = mob.get_mob_entity();
        let living = &mob_entity.living_entity;
        let entity = &living.entity;

        let is_done = mob_entity
            .navigator
            .try_lock()
            .is_ok_and(|navigator| navigator.is_done());

        if self.operation == Operation::MoveTo && !is_done {
            let pos = entity.pos.load();
            let delta = Vector3::new(self.wanted_x - pos.x, self.wanted_y - pos.y, self.wanted_z - pos.z);
            let length = delta.length();
            if length < 1e-4 {
                living.movement_input.store(Vector3::default());
                return;
            }

            let xd = delta.x / length;
            let yd = delta.y / length;
            let zd = delta.z / length;

            let y_rot_d = (delta.z.atan2(delta.x).to_degrees() as f32) - 90.0;
            let current_yaw = entity.yaw.load();
            let new_yaw = MoveControl::rotlerp(current_yaw, y_rot_d, 90.0);
            entity.yaw.store(new_yaw);
            entity.body_yaw.store(new_yaw);
            entity.head_yaw.store(new_yaw);

            let movement_speed = living.get_attribute_value(&Attributes::MOVEMENT_SPEED);
            let target_speed = (self.speed_modifier * movement_speed) as f32;
            let current_speed = living.movement_input.load().z as f32;
            let new_speed = lerp(0.125, current_speed as f64, target_speed as f64) as f32;
            living.movement_input.store(Vector3::new(0.0, 0.0, new_speed as f64));

            let age = entity.age.load(Ordering::Relaxed) as f64;
            let id = entity.entity_id as f64;
            let push = ((age + id) * 0.5).sin() * 0.05;
            let yaw_rad = f64::from(new_yaw.to_radians());
            let cos = yaw_rad.cos();
            let sin = yaw_rad.sin();
            let y_push = ((age + id) * 0.75).sin() * 0.05;

            let mut vel = entity.velocity.load();
            vel.x += push * cos;
            vel.y += y_push * (sin + cos) * 0.25 + f64::from(new_speed) * yd * 0.1;
            vel.z += push * sin;
            entity.velocity.store(vel);

            let new_look_x = pos.x + xd * 2.0;
            let new_look_y = entity.get_eye_y() + yd / length;
            let new_look_z = pos.z + zd * 2.0;
            if let Ok(mut look_control) = mob_entity.look_control.try_lock() {
                look_control.look_at_with_range(new_look_x, new_look_y, new_look_z, 10.0, 40.0);
            }
        } else {
            living.movement_input.store(Vector3::default());
        }
    }

    fn set_wanted_position(&mut self, x: f64, y: f64, z: f64, speed_modifier: f64) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.speed_modifier = speed_modifier;
        self.operation = Operation::MoveTo;
    }

    fn strafe(&mut self, _forward: f32, _right: f32) {}

    fn has_wanted(&self) -> bool {
        self.operation == Operation::MoveTo
    }
}
