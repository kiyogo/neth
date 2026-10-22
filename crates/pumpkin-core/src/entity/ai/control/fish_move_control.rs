use crate::entity::ai::control::move_control::{MoveControl, Operation};
use crate::entity::ai::control::{Control, MoveControlTrait};
use crate::entity::mob::Mob;
use pumpkin_data::attributes::Attributes;
use pumpkin_util::math::lerp;
use pumpkin_util::math::vector3::Vector3;

/// Vanilla `AbstractFish.FishMoveControl`: moves fish smoothly in 3D water.
#[derive(Default)]
pub struct FishMoveControl {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
    operation: Operation,
}

impl Control for FishMoveControl {}

impl MoveControlTrait for FishMoveControl {
    fn tick(&mut self, mob: &dyn Mob) {
        let mob_entity = mob.get_mob_entity();
        let living = &mob_entity.living_entity;
        let entity = &living.entity;

        if entity.is_submerged_in_water() {
            let mut velocity = entity.velocity.load();
            velocity.y += 0.005;
            entity.velocity.store(velocity);
        }

        let is_done = mob_entity
            .navigator
            .try_lock()
            .is_ok_and(|navigator| navigator.is_done());

        if self.operation == Operation::MoveTo && !is_done {
            let movement_speed = living.get_attribute_value(&Attributes::MOVEMENT_SPEED);
            let target_speed = (self.speed_modifier * movement_speed) as f32;
            let current_speed = living.movement_input.load().z as f32;
            let new_speed = lerp(0.125, current_speed as f64, target_speed as f64) as f32;
            living.movement_input.store(Vector3::new(0.0, 0.0, new_speed as f64));

            let pos = entity.pos.load();
            let xd = self.wanted_x - pos.x;
            let yd = self.wanted_y - pos.y;
            let zd = self.wanted_z - pos.z;

            if yd != 0.0 {
                let dd = (xd * xd + yd * yd + zd * zd).sqrt();
                if dd > 0.0 {
                    let mut vel = entity.velocity.load();
                    vel.y += f64::from(new_speed) * (yd / dd) * 0.1;
                    entity.velocity.store(vel);
                }
            }

            if xd != 0.0 || zd != 0.0 {
                let y_rot_d = (zd.atan2(xd).to_degrees() as f32) - 90.0;
                let current_yaw = entity.yaw.load();
                let new_yaw = MoveControl::rotlerp(current_yaw, y_rot_d, 90.0);
                entity.yaw.store(new_yaw);
                entity.body_yaw.store(new_yaw);
                entity.head_yaw.store(new_yaw);
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
