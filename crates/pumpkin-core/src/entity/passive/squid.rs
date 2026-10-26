use crossbeam::atomic::AtomicCell;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::math::wrap_degrees;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    combat,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
};

pub struct SquidEntity {
    pub mob_entity: MobEntity,
    pub movement_vector: AtomicCell<Vector3<f64>>,
    pub tentacle_movement: AtomicCell<f32>,
    pub tentacle_speed: AtomicCell<f32>,
    pub rotate_speed: AtomicCell<f32>,
    pub air_supply: AtomicI32,
    pub flee_ticks: AtomicI32,
}

impl SquidEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let tentacle_speed = 1.0 / (rng.random::<f32>() + 1.0) * 0.2;
        let squid = Self {
            mob_entity,
            movement_vector: AtomicCell::new(Vector3::default()),
            tentacle_movement: AtomicCell::new(0.0),
            tentacle_speed: AtomicCell::new(tentacle_speed),
            rotate_speed: AtomicCell::new(0.0),
            air_supply: AtomicI32::new(300),
            flee_ticks: AtomicI32::new(0),
        };

        // Vanilla Squid attributes: MAX_HEALTH = 10.0
        {
            let mut attributes = squid
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 10.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
        }
        squid.mob_entity.living_entity.set_health(10.0);

        Arc::new(squid)
    }
}

impl Mob for SquidEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_squid(&self) -> Option<&SquidEntity> {
        Some(self)
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn can_breathe_underwater(&self) -> bool {
        true
    }

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    fn can_drown(&self) -> bool {
        false
    }

    fn dries_out_on_land(&self) -> bool {
        true
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        entity.move_entity(caller, entity.velocity.load());
        true
    }

    fn can_be_leashed(&self) -> bool {
        true
    }

    fn get_base_experience_reward(&self) -> u32 {
        combat::get_squid_xp_reward(rand::rng().random_range(0..3))
    }

    fn on_damage(&self, _damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        // Vanilla Squid.java:180: ink squirt only triggers when attacked by an entity
        if !combat::should_squid_squirt_ink(_source.is_some()) {
            return;
        }

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.silent.load(Ordering::Relaxed) {
            entity.world.load().play_sound_fine(
                Sound::EntitySquidSquirt,
                SoundCategory::Neutral,
                &entity.pos.load(),
                0.4,
                1.0,
            );
        }

        // Vanilla Squid.java:188-203: directional ink burst of 30 particles
        let mut rng = rand::rng();
        let pitch = entity.pitch.load();
        let yaw = entity.yaw.load();
        let pos = entity.pos.load() + combat::rotate_squid_vector(Vector3::new(0.0, -1.0, 0.0), pitch, yaw);
        let world = entity.world.load();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;

        for _ in 0..30 {
            let rx = (rng.random::<f64>() * 0.6 - 0.3) as f64;
            let rz = (rng.random::<f64>() * 0.6 - 0.3) as f64;
            let dir = combat::rotate_squid_vector(Vector3::new(rx, -1.0, rz), pitch, yaw);
            let offset_scale = combat::calculate_squid_ink_offset_scale(is_baby, rng.random::<f32>()) as f64;
            let dir_offset = dir * offset_scale;

            world.spawn_particle(
                pos + Vector3::new(0.0, 0.5, 0.0),
                Vector3::new(dir_offset.x as f32, dir_offset.y as f32, dir_offset.z as f32),
                0.1,
                0,
                Particle::SquidInk,
            );
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let in_water =
            entity.touching_water.load(Ordering::Relaxed) || entity.is_in_water();

        let mut rng = rand::rng();

        // 1. Squid Flee Goal (Vanilla Squid.java:240-298: SquidFleeGoal)
        let last_attacker_id = self
            .mob_entity
            .living_entity
            .last_attacker_id
            .load(Ordering::Relaxed);
        let mut is_fleeing = false;

        if in_water && last_attacker_id != 0 {
            let world = entity.world.load();
            if let Some(attacker) = world.get_entity_by_id(last_attacker_id) {
                let attacker_ent = attacker.get_entity();
                let attacker_pos = attacker_ent.pos.load();
                let dist_sqr = entity.pos.load().squared_distance_to_vec(&attacker_pos);
                if combat::should_squid_flee(in_water, attacker_ent.is_alive(), dist_sqr) {
                    let ticks = self.flee_ticks.fetch_add(1, Ordering::Relaxed) + 1;
                    let block_above = entity.block_pos.load().up();
                    let is_air_above = world.get_block(&block_above) == &pumpkin_data::Block::AIR;
                    let flee_movement = combat::calculate_squid_flee_movement(
                        entity.pos.load(),
                        attacker_pos,
                        is_air_above,
                    );
                    self.movement_vector.store(flee_movement);

                    if ticks % 10 == 5 {
                        world.spawn_particle(
                            entity.pos.load(),
                            Vector3::new(0.0, 0.0, 0.0),
                            0.0,
                            1,
                            Particle::Bubble,
                        );
                    }
                    is_fleeing = true;
                } else {
                    self.flee_ticks.store(0, Ordering::Relaxed);
                }
            } else {
                self.flee_ticks.store(0, Ordering::Relaxed);
            }
        } else {
            self.flee_ticks.store(0, Ordering::Relaxed);
        }

        // 2. Random movement vector selection (SquidRandomMovementGoal from vanilla Squid.java)
        let mut movement_vec = self.movement_vector.load();
        if !is_fleeing
            && (!in_water || movement_vec.length_squared() < 1e-5 || rng.random_range(0..50) == 0)
        {
            let angle = rng.random::<f32>() * std::f32::consts::TAU;
            movement_vec = Vector3::new(
                (angle.cos() * 0.2) as f64,
                (-0.1 + rng.random::<f32>() * 0.2) as f64,
                (angle.sin() * 0.2) as f64,
            );
            self.movement_vector.store(movement_vec);
        }

        // 3. Tentacle thrust cycle (Squid.aiStep from vanilla Squid.java)
        let mut tentacle_movement = self.tentacle_movement.load() + self.tentacle_speed.load();
        if tentacle_movement > std::f32::consts::TAU {
            tentacle_movement -= std::f32::consts::TAU;
            if rng.random_range(0..10) == 0 {
                let speed = 1.0 / (rng.random::<f32>() + 1.0) * 0.2;
                self.tentacle_speed.store(speed);
            }
        }
        self.tentacle_movement.store(tentacle_movement);

        if in_water {
            let mut rotate_speed = self.rotate_speed.load();
            if tentacle_movement < std::f32::consts::PI {
                let tentacle_scale = tentacle_movement / std::f32::consts::PI;
                if tentacle_scale > 0.75 {
                    entity.velocity.store(movement_vec);
                    rotate_speed = 1.0;
                } else {
                    rotate_speed *= 0.8;
                }
            } else {
                entity.velocity.store(entity.velocity.load() * 0.9);
                rotate_speed *= 0.99;
            }
            self.rotate_speed.store(rotate_speed);

            let vel = entity.velocity.load();
            let horizontal_dist = (vel.x * vel.x + vel.z * vel.z).sqrt();
            if horizontal_dist > 1e-4 {
                let target_yaw = (-vel.x.atan2(vel.z).to_degrees()) as f32;
                let cur_yaw = entity.yaw.load();
                let new_yaw = cur_yaw + wrap_degrees(target_yaw - cur_yaw) * 0.1;
                entity.yaw.store(new_yaw);
                entity.body_yaw.store(new_yaw);
                entity.head_yaw.store(new_yaw);
            }
            let target_pitch = (-horizontal_dist.atan2(vel.y).to_degrees()) as f32;
            let cur_pitch = entity.pitch.load();
            entity.pitch.store(cur_pitch + (target_pitch - cur_pitch) * 0.1);

            if !entity.silent.load(Ordering::Relaxed) && rng.random_range(0..120) == 0 {
                entity.world.load().play_sound_fine(
                    Sound::EntitySquidAmbient,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            }
        } else {
            let vel = entity.velocity.load();
            entity.velocity.store(Vector3::new(0.0, (vel.y - 0.08) * 0.98, 0.0));
            let cur_pitch = entity.pitch.load();
            entity.pitch.store(cur_pitch + (-90.0 - cur_pitch) * 0.02);
        }
    }
}

impl CustomSound for SquidEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySquidDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySquidHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_squid_ink_trigger() {
        assert!(combat::should_squid_squirt_ink(true));
        assert!(!combat::should_squid_squirt_ink(false));
    }
}
