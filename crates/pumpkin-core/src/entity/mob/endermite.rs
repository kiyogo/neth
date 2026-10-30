use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::combat::{
    ENDERMITE_BASE_ATTACK_DAMAGE, ENDERMITE_BASE_MOVEMENT_SPEED, ENDERMITE_LIFETIME_MAX,
    ENDERMITE_MAX_HEALTH, ENDERMITE_XP_REWARD, should_endermite_despawn,
};
use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, climb_on_top_of_powder_snow::ClimbOnTopOfPowderSnowGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal, revenge::RevengeGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct EndermiteEntity {
    pub mob_entity: MobEntity,
    pub lifetime: AtomicI32,
}

impl EndermiteEntity {
    pub const XP_REWARD: u32 = ENDERMITE_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let endermite = Self {
            mob_entity,
            lifetime: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(endermite);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Endermite: MAX_HEALTH = 8.0, MOVEMENT_SPEED = 0.25, ATTACK_DAMAGE = 2.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, f64::from(ENDERMITE_MAX_HEALTH));
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, ENDERMITE_BASE_MOVEMENT_SPEED);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, f64::from(ENDERMITE_BASE_ATTACK_DAMAGE));
        mob_arc.mob_entity.living_entity.set_health(ENDERMITE_MAX_HEALTH);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Goal 1: FloatGoal & ClimbOnTopOfPowderSnowGoal
            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(ClimbOnTopOfPowderSnowGoal));
            // Goal 2: MeleeAttackGoal(1.0, false)
            goal_selector.add_goal(2, Box::new(MeleeAttackGoal::new(1.0, false)));
            // Goal 3: WaterAvoidingRandomStrollGoal(1.0)
            goal_selector.add_goal(3, Box::new(WanderAroundGoal::new(1.0)));
            // Goal 7: LookAtPlayerGoal(8.0)
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            // Goal 8: RandomLookAroundGoal
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Target Goal 1: HurtByTargetGoal(alert_others=true)
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true).alerting_others()));
            // Target Goal 2: NearestAttackableTargetGoal(Player)
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }
}

impl CustomSound for EndermiteEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityEndermiteDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityEndermiteHurt)
    }
}

impl Mob for EndermiteEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_endermite(&self) -> Option<&EndermiteEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("Lifetime", self.lifetime.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(life) = nbt.get_int("Lifetime") {
            self.lifetime.store(life, Ordering::Relaxed);
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla yBodyRot = getYRot()
        let yaw = entity.yaw.load();
        entity.body_yaw.store(yaw);
        entity.head_yaw.store(yaw);

        // Vanilla aiStep: if (!this.isPersistenceRequired()) { this.life++; }
        let persistence = self.mob_entity.persistence_required.load(Ordering::Relaxed);
        if !persistence {
            let life = self.lifetime.fetch_add(1, Ordering::Relaxed) + 1;
            if crate::entity::combat::should_endermite_despawn(life, persistence) {
                entity.remove();
                return;
            }
        }

        if rand::random_range(0..100) == 0 && !entity.silent.load(Ordering::Relaxed) {
            entity.world.load().play_sound_fine(
                Sound::EntityEndermiteAmbient,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                (rand::random::<f32>() - rand::random::<f32>()).mul_add(0.2, 1.0),
            );
        }
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endermite_despawn_and_xp_parity() {
        assert_eq!(EndermiteEntity::XP_REWARD, ENDERMITE_XP_REWARD);
        assert_eq!(ENDERMITE_LIFETIME_MAX, 2400);
        assert_eq!(ENDERMITE_MAX_HEALTH, 8.0);
        assert_eq!(ENDERMITE_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(ENDERMITE_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(crate::entity::combat::ENDERMITE_STEP_SOUND_VOLUME, 0.15);
        assert!(should_endermite_despawn(2400, false));
        assert!(should_endermite_despawn(2500, false));
        assert!(!should_endermite_despawn(2399, false));
        assert!(!should_endermite_despawn(3000, true));
    }
}
