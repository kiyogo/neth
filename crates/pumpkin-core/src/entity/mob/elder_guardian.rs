use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::{CGameEvent, GameEvent};

use pumpkin_util::math::vector3::Vector3;

use crate::entity::ai::control::guardian_move_control::GuardianMoveControl;
use crate::entity::ai::pathfinder::Navigator;
use crate::entity::ai::pathfinder::node::PathType;
use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, guardian_attack::GuardianAttackGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct ElderGuardianEntity {
    pub mob_entity: MobEntity,
    tick_count: AtomicI32,
    pub moving: AtomicBool,
}

impl ElderGuardianEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::ELDER_GUARDIAN_XP_REWARD;

    #[must_use]
    pub const fn can_be_leashed(&self) -> bool {
        false
    }

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
            tick_count: AtomicI32::new(0),
            moving: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(guardian);

        mob_arc.mob_entity.living_entity.entity.set_synced_data(
            tracked_data::elder_guardian::DATA_ID_MOVING,
            false,
        );
        mob_arc.mob_entity.living_entity.entity.set_synced_data(
            tracked_data::elder_guardian::DATA_ID_ATTACK_TARGET,
            VarInt(0),
        );

        // Vanilla Elder Guardian attributes: MAX_HEALTH = 80.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 8.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, f64::from(crate::entity::combat::ELDER_GUARDIAN_MAX_HEALTH));
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::ELDER_GUARDIAN_BASE_MOVEMENT_SPEED);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, f64::from(crate::entity::combat::ELDER_GUARDIAN_BASE_ATTACK_DAMAGE));
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::ELDER_GUARDIAN_MAX_HEALTH);
        mob_arc.mob_entity.persistence_required.store(true, Ordering::Relaxed);

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

            goal_selector.add_goal(4, Box::new(GuardianAttackGoal::new(true)));
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
                tracked_data::elder_guardian::DATA_ID_MOVING,
                moving,
            );
        }
    }
}

impl CustomSound for ElderGuardianEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityElderGuardianHurt
        } else {
            Sound::EntityElderGuardianHurtLand
        })
    }

    fn death_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityElderGuardianDeath
        } else {
            Sound::EntityElderGuardianDeathLand
        })
    }
}

impl Mob for ElderGuardianEntity {
    fn can_be_leashed(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        crate::entity::combat::ELDER_GUARDIAN_XP_REWARD
    }

    /// Elder guardians never drown.
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

    fn as_elder_guardian(&self) -> Option<&ElderGuardianEntity> {
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

        let ticks = self.tick_count.fetch_add(1, Ordering::Relaxed) + 1;
        if ticks % crate::entity::combat::ELDER_GUARDIAN_CURSE_INTERVAL_TICKS == 0 {
            let pos = entity.pos.load();
            let world = entity.world.load();
            let players = world.get_nearby_players(pos, crate::entity::combat::ELDER_GUARDIAN_CURSE_RADIUS);
            let packet = CGameEvent::new(
                GameEvent::PlayElderGuardianMobAppearance,
                if entity.silent.load(Ordering::Relaxed) { 0.0 } else { 1.0 },
            );

            for player in players {
                player.try_send_client_packet(&packet);
                player.living_entity.add_effect(crate::entity::combat::create_elder_guardian_curse_effect());
            }
        }

        // Ambient sound
        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let is_water = entity.touching_water.load(Ordering::Relaxed);
            let sound = if is_water {
                Sound::EntityElderGuardianAmbient
            } else {
                Sound::EntityElderGuardianAmbientLand
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
        ELDER_GUARDIAN_ATTACK_DURATION_TICKS, ELDER_GUARDIAN_BASE_ATTACK_DAMAGE,
        ELDER_GUARDIAN_BASE_MOVEMENT_SPEED, ELDER_GUARDIAN_CURSE_AMPLIFIER,
        ELDER_GUARDIAN_CURSE_DURATION_TICKS, ELDER_GUARDIAN_CURSE_INTERVAL_TICKS,
        ELDER_GUARDIAN_CURSE_RADIUS, ELDER_GUARDIAN_MAX_HEALTH, ELDER_GUARDIAN_XP_REWARD,
        calculate_guardian_laser_damage, create_elder_guardian_curse_effect,
        get_guardian_attack_duration,
    };

    #[test]
    fn test_elder_guardian_combat_and_curse_parity() {
        assert_eq!(ELDER_GUARDIAN_MAX_HEALTH, 80.0);
        assert_eq!(ELDER_GUARDIAN_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(ELDER_GUARDIAN_BASE_ATTACK_DAMAGE, 8.0);
        assert_eq!(ELDER_GUARDIAN_XP_REWARD, 10);
        assert_eq!(ELDER_GUARDIAN_ATTACK_DURATION_TICKS, 60);
        assert_eq!(get_guardian_attack_duration(true), 60);

        // Curse intervals & effects (ElderGuardian.java:23-28)
        assert_eq!(ELDER_GUARDIAN_CURSE_INTERVAL_TICKS, 1200);
        assert_eq!(ELDER_GUARDIAN_CURSE_RADIUS, 50.0);
        assert_eq!(ELDER_GUARDIAN_CURSE_DURATION_TICKS, 6000);
        assert_eq!(ELDER_GUARDIAN_CURSE_AMPLIFIER, 2);

        let effect = create_elder_guardian_curse_effect();
        assert_eq!(effect.effect_type, &pumpkin_data::effect::StatusEffect::MINING_FATIGUE);
        assert_eq!(effect.duration, 6000);
        assert_eq!(effect.amplifier, 2);
        assert!(!effect.ambient);
        assert!(effect.show_particles);
        assert!(effect.show_icon);

        // Elder guardian laser damage bonus (+2.0 over normal)
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Normal, true),
            3.0
        );
        assert_eq!(
            calculate_guardian_laser_damage(pumpkin_util::Difficulty::Hard, true),
            5.0
        );

        // Scale & curse display limit parity (ElderGuardian.java:20, 25)
        assert_eq!(crate::entity::combat::ELDER_GUARDIAN_SIZE_SCALE, 2.35);
        assert_eq!(
            crate::entity::combat::ELDER_GUARDIAN_CURSE_DISPLAY_LIMIT_TICKS,
            1200
        );
    }
}

