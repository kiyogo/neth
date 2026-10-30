use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, breeze_attack::BreezeAttackGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, revenge::RevengeGoal,
        swim::SwimGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
};

pub struct BreezeEntity {
    pub mob_entity: MobEntity,
    pub whirl_sound_ticks: AtomicI32,
}

impl BreezeEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::BREEZE_XP_REWARD;

    #[must_use]
    pub const fn can_be_leashed(&self) -> bool {
        false
    }

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let breeze = Self {
            mob_entity,
            whirl_sound_ticks: AtomicI32::new(rand::random_range(1..=80)),
        };
        let mob_arc = Arc::new(breeze);

        // Vanilla Breeze: MOVEMENT_SPEED = 0.63, MAX_HEALTH = 30.0, FOLLOW_RANGE = 24.0, ATTACK_DAMAGE = 3.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, f64::from(crate::entity::combat::BREEZE_MAX_HEALTH));
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::BREEZE_BASE_MOVEMENT_SPEED);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::FOLLOW_RANGE, crate::entity::combat::BREEZE_FOLLOW_RANGE);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, f64::from(crate::entity::combat::BREEZE_BASE_ATTACK_DAMAGE));
        mob_arc.mob_entity.living_entity.set_health(crate::entity::combat::BREEZE_MAX_HEALTH);

        let breeze_weak = Arc::downgrade(&mob_arc);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, BreezeAttackGoal::new(breeze_weak));
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(0.6)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
        };

        mob_arc
    }
}

impl Mob for BreezeEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn pre_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) -> bool {
        if let Some(src) = source {
            let src_type = src.get_entity().entity_type;
            if src_type == &EntityType::BREEZE {
                return false;
            }
            if src_type != &EntityType::BREEZE_WIND_CHARGE && src_type != &EntityType::WIND_CHARGE {
                if crate::entity::projectile::is_projectile(src_type) {
                    let world = self.get_entity().world.load();
                    world.play_sound_fine(
                        Sound::EntityBreezeDeflect,
                        SoundCategory::Hostile,
                        &self.get_entity().pos.load(),
                        1.0,
                        1.0,
                    );
                    let proj = src.get_entity();
                    let velo = proj.velocity.load();
                    proj.velocity.store(Vector3::new(-velo.x * 0.5, -velo.y * 0.5, -velo.z * 0.5));
                    let rotation = 170.0 + rand::random::<f32>() * 20.0;
                    proj.yaw.store(proj.yaw.load() + rotation);
                    proj.velocity_dirty.store(true, Ordering::Relaxed);
                    return false;
                }
            }
        }
        true
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = self.get_entity();
        let silent = entity.silent.load(Ordering::Relaxed);
        let world = entity.world.load();

        if !silent && rand::random_range(0..80) == 0 {
            let sound = if entity.on_ground.load(Ordering::Relaxed) {
                Sound::EntityBreezeIdleGround
            } else {
                Sound::EntityBreezeIdleAir
            };
            world.play_sound_fine(
                sound,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }

        // Vanilla Breeze whirl sound (Breeze.java:145-148, 190-194)
        let whirl_cd = self.whirl_sound_ticks.load(Ordering::Relaxed);
        if whirl_cd <= 0 {
            self.whirl_sound_ticks
                .store(rand::random_range(1..=80), Ordering::Relaxed);
            if !silent {
                let rand_pitch = rand::random_range(0.0..1.0);
                let rand_vol = rand::random_range(0.0..1.0);
                let pitch = crate::entity::combat::calculate_breeze_whirl_pitch(rand_pitch);
                let volume = crate::entity::combat::calculate_breeze_whirl_volume(rand_vol);
                world.play_sound_fine(
                    Sound::EntityBreezeWhirl,
                    SoundCategory::Hostile,
                    &entity.pos.load(),
                    volume,
                    pitch,
                );
            }
        } else {
            self.whirl_sound_ticks
                .store(whirl_cd - 1, Ordering::Relaxed);
        }
    }

    fn on_damage(&self, damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        if damage_type.id == DamageType::FALL.id {
            let fall_dist = self.mob_entity.living_entity.fall_distance.load();
            if crate::entity::combat::should_breeze_play_fall_sound(fall_dist) {
                let entity = self.get_entity();
                if !entity.silent.load(Ordering::Relaxed) {
                    entity.world.load().play_sound_fine(
                        Sound::EntityBreezeLand,
                        SoundCategory::Hostile,
                        &entity.pos.load(),
                        1.0,
                        1.0,
                    );
                }
            }
        }
    }

    fn as_breeze(&self) -> Option<&BreezeEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn can_be_leashed(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        crate::entity::combat::BREEZE_XP_REWARD
    }
}

impl CustomSound for BreezeEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBreezeHurt)
    }

    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBreezeDeath)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_breeze_fall_and_whirl_parity() {
        assert_eq!(crate::entity::combat::BREEZE_FALL_DISTANCE_SOUND_THRESHOLD, 3.0);
        assert!(crate::entity::combat::should_breeze_play_fall_sound(3.1));
        assert!(!crate::entity::combat::should_breeze_play_fall_sound(3.0));
        assert!((crate::entity::combat::calculate_breeze_whirl_pitch(0.0) - 0.7).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_breeze_whirl_pitch(1.0) - 1.1).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_breeze_whirl_volume(0.0) - 0.8).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_breeze_whirl_volume(1.0) - 1.0).abs() < 1e-6);
    }
}
