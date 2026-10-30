use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::{damage::DamageType, sound::{Sound, SoundCategory}, tracked_data};
use pumpkin_protocol::codec::var_int::VarInt;

use pumpkin_util::math::vector3::Vector3;

use crate::entity::ai::control::guardian_move_control::GuardianMoveControl;
use crate::entity::ai::pathfinder::Navigator;
use crate::entity::ai::pathfinder::node::PathType;
use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct GuardianEntity {
    pub mob_entity: MobEntity,
    pub moving: AtomicBool,
}

impl GuardianEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        *mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(GuardianMoveControl::default());

        let mut navigator = Navigator::water_bound(false);
        navigator.set_pathfinding_malus(PathType::Water, 0.0);
        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;

        let guardian = Self {
            mob_entity,
            moving: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(guardian);

        mob_arc.mob_entity.living_entity.entity.set_synced_data(
            tracked_data::guardian::DATA_ID_MOVING,
            false,
        );
        mob_arc.mob_entity.living_entity.entity.set_synced_data(
            tracked_data::guardian::DATA_ID_ATTACK_TARGET,
            VarInt(0),
        );

        // Vanilla Guardian attributes: MAX_HEALTH = 30.0, MOVEMENT_SPEED = 0.5, ATTACK_DAMAGE = 6.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = f64::from(crate::entity::combat::GUARDIAN_MAX_HEALTH);
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = crate::entity::combat::GUARDIAN_BASE_MOVEMENT_SPEED;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = f64::from(crate::entity::combat::GUARDIAN_BASE_ATTACK_DAMAGE);
                damage.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::GUARDIAN_MAX_HEALTH);

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

            goal_selector.add_goal(4, Box::new(crate::entity::ai::goal::guardian_attack::GuardianAttackGoal::new(false)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::GUARDIAN, 12.0),
            );
            goal_selector.add_goal(9, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SQUID, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::GLOW_SQUID, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::AXOLOTL, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_moving(&self) -> bool {
        self.moving.load(Ordering::Relaxed)
    }

    pub fn set_moving(&self, moving: bool) {
        let old = self.moving.swap(moving, Ordering::Relaxed);
        if old != moving {
            self.mob_entity.living_entity.entity.set_synced_data(
                tracked_data::guardian::DATA_ID_MOVING,
                moving,
            );
        }
    }
}

impl CustomSound for GuardianEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityGuardianHurt
        } else {
            Sound::EntityGuardianHurtLand
        })
    }

    fn death_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityGuardianDeath
        } else {
            Sound::EntityGuardianDeathLand
        })
    }
}

impl Mob for GuardianEntity {
    fn get_base_experience_reward(&self) -> u32 {
        crate::entity::combat::GUARDIAN_XP_REWARD
    }

    /// Guardians never drown.
    fn can_breathe_underwater(&self) -> bool {
        true
    }

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    fn mob_travel_in_water(&self, caller: &dyn EntityBase) -> bool {
        let living = &self.mob_entity.living_entity;
        let entity = &living.entity;
        if !entity.touching_water.load(Ordering::Relaxed) {
            return false;
        }
        let input = living.movement_input.load();
        entity.update_velocity_from_input(Vector3::new(input.x, input.y, input.z), 0.1);
        entity.move_entity(caller, entity.velocity.load());
        let mut vel = entity.velocity.load() * 0.9;
        if self.mob_entity.get_target().is_none() {
            vel.y -= 0.005;
        }
        entity.velocity.store(vel);
        true
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_guardian(&self) -> Option<&GuardianEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let is_water = entity.touching_water.load(Ordering::Relaxed);
            let sound = if is_water {
                Sound::EntityGuardianAmbient
            } else {
                Sound::EntityGuardianAmbientLand
            };
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn on_damage(&self, damage_type: DamageType, source: Option<&dyn EntityBase>) {
        let direct_living = source.and_then(|s| s.get_living_entity());
        if crate::entity::combat::should_guardian_retaliate_thorns(
            self.is_moving(),
            &damage_type,
            direct_living.is_some(),
        ) {
            if let Some(living) = direct_living {
                let guardian_base = self as &dyn EntityBase;
                let _ = living.damage_with_context(
                    guardian_base,
                    crate::entity::combat::GUARDIAN_THORNS_DAMAGE,
                    DamageType::THORNS,
                    None,
                    Some(guardian_base),
                    Some(guardian_base),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::entity::combat::{
        GUARDIAN_ATTACK_DURATION_TICKS, GUARDIAN_BASE_ATTACK_DAMAGE,
        GUARDIAN_BASE_MOVEMENT_SPEED, GUARDIAN_LASER_WARMUP_TICKS, GUARDIAN_MAX_HEALTH,
        GUARDIAN_THORNS_DAMAGE, GUARDIAN_XP_REWARD, calculate_guardian_laser_damage,
        get_guardian_attack_duration, should_guardian_retaliate_thorns,
    };

    #[test]
    fn test_guardian_combat_and_attributes_parity() {
        assert_eq!(GUARDIAN_MAX_HEALTH, 30.0);
        assert_eq!(GUARDIAN_BASE_MOVEMENT_SPEED, 0.5);
        assert_eq!(GUARDIAN_BASE_ATTACK_DAMAGE, 6.0);
        assert_eq!(GUARDIAN_XP_REWARD, 10);
        assert_eq!(GUARDIAN_LASER_WARMUP_TICKS, 10);
        assert_eq!(GUARDIAN_ATTACK_DURATION_TICKS, 80);
        assert_eq!(get_guardian_attack_duration(false), 80);
        assert_eq!(GUARDIAN_THORNS_DAMAGE, 2.0);

        // Thorns trigger only when stationary and hit by living entity
        assert!(should_guardian_retaliate_thorns(
            false,
            &pumpkin_data::damage::DamageType::PLAYER_ATTACK,
            true
        ));
        assert!(!should_guardian_retaliate_thorns(
            true,
            &pumpkin_data::damage::DamageType::PLAYER_ATTACK,
            true
        ));

        // Laser damage on normal vs hard difficulty
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Normal, false),
            1.0
        );
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Hard, false),
            3.0
        );
    }
}

