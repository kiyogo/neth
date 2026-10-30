use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::boundingbox::EntityDimensions;

use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    living::LivingEntity,
    mob::{Mob, MobEntity, hoglin::throw_target},
};
use crate::world::World;

pub struct ZoglinEntity {
    pub mob_entity: MobEntity,
    pub is_baby: AtomicBool,
}

impl ZoglinEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::ZOGLIN_XP_REWARD;

    pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions {
        width: 0.75,
        height: 0.85,
        eye_height: 0.625,
    };

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::ZOGLIN_MAX_HEALTH);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::ZOGLIN_MOVEMENT_SPEED);
        mob_entity
            .living_entity
            .set_attribute_base(
                &Attributes::KNOCKBACK_RESISTANCE,
                crate::entity::combat::ZOGLIN_KNOCKBACK_RESISTANCE,
            );
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_KNOCKBACK, crate::entity::combat::ZOGLIN_ATTACK_KNOCKBACK);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, crate::entity::combat::ZOGLIN_ATTACK_DAMAGE);
        mob_entity
            .living_entity
            .health
            .store(crate::entity::combat::ZOGLIN_MAX_HEALTH as f32);

        let zoglin = Self {
            mob_entity,
            is_baby: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(zoglin);
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
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(0.4)));
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
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::PLAYER,
                    10,
                    true,
                    false,
                    Some(|target: &LivingEntity, _world: &World| {
                        crate::entity::combat::can_zoglin_target_entity(target.entity.entity_type)
                    }),
                )),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_baby(&self) -> bool {
        self.is_baby.load(Ordering::Relaxed)
    }

    /// Vanilla `Zoglin.setBaby`; growing up keeps the lowered attack damage, as in vanilla.
    pub fn set_baby(&self, baby: bool) {
        self.mob_entity
            .set_baby_flag(&self.is_baby, tracked_data::zoglin::DATA_BABY_ID, baby);
        let living = &self.mob_entity.living_entity;
        living.entity.entity_dimension.store(if baby {
            Self::BABY_DIMENSIONS
        } else {
            Entity::type_dimensions(living.entity.entity_type)
        });
        if baby {
            living.set_attribute_base(
                &Attributes::ATTACK_DAMAGE,
                crate::entity::combat::calculate_zoglin_attack_damage(true),
            );
        }
    }
}

impl CustomSound for ZoglinEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityZoglinDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityZoglinHurt)
    }
}

impl Mob for ZoglinEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_zoglin(&self) -> Option<&ZoglinEntity> {
        Some(self)
    }

    fn finalize_spawn(
        &self,
        _world: &Arc<World>,
        group_data: Option<crate::entity::mob::spawn::SpawnGroupData>,
    ) -> Option<crate::entity::mob::spawn::SpawnGroupData> {
        if rand::random::<f32>() < 0.2 {
            self.set_baby(true);
        }
        self.mob_entity.finalize_spawn_base();
        group_data
    }

    fn spawn_as_baby(&self) -> bool {
        self.set_baby(true);
        true
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("IsBaby", self.is_baby());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.set_baby(nbt.get_bool("IsBaby").unwrap_or(false));
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Ambient sound
        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if self.mob_entity.get_target().is_some() {
                Sound::EntityZoglinAngry
            } else {
                Sound::EntityZoglinAmbient
            };
            let rand_diff = rand::random::<f32>() - rand::random::<f32>();
            let pitch = crate::entity::combat::calculate_zoglin_sound_pitch(
                self.is_baby.load(Ordering::Relaxed),
                rand_diff,
            );
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                pitch,
            );
        }
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn modify_applied_attack_damage(&self, base_damage: f32) -> f32 {
        let rand_int = rand::random::<u32>();
        crate::entity::combat::calculate_hoglin_attack_damage(
            base_damage,
            self.is_baby.load(Ordering::Relaxed),
            rand_int,
        )
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        // Vanilla HoglinBase.hurtAndThrowTarget: babies don't throw.
        if self.is_baby() {
            return;
        }
        throw_target(&self.mob_entity.living_entity.entity, target);
    }

    fn on_blocked_by_shield(&self, defender: &dyn EntityBase, _fully_blocked: bool) {
        if !self.is_baby() {
            throw_target(&self.mob_entity.living_entity.entity, defender);
        }
    }

    /// Vanilla Zoglin.java:193: Zoglins can be leashed despite being hostile monsters.
    fn can_be_leashed(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoglin_combat_and_leash_parity() {
        assert_eq!(ZoglinEntity::XP_REWARD, 5);

        // Zoglins can be leashed (Zoglin.java:193)
        assert!(crate::entity::combat::is_mob_leashable(&EntityType::ZOGLIN));

        // Damage calculation formula
        assert_eq!(crate::entity::combat::calculate_hoglin_attack_damage(6.0, false, 0), 3.0);
        assert_eq!(crate::entity::combat::calculate_hoglin_attack_damage(6.0, false, 3), 6.0);
        assert_eq!(crate::entity::combat::calculate_hoglin_attack_damage(0.5, true, 3), 0.5);

        // Attributes & attack intervals parity (Zoglin.java:64-73, 127-128)
        assert_eq!(crate::entity::combat::ZOGLIN_MAX_HEALTH, 40.0);
        assert_eq!(crate::entity::combat::ZOGLIN_MOVEMENT_SPEED, 0.3);
        assert_eq!(crate::entity::combat::ZOGLIN_KNOCKBACK_RESISTANCE, 0.6);
        assert_eq!(crate::entity::combat::ZOGLIN_ATTACK_KNOCKBACK, 1.0);
        assert_eq!(crate::entity::combat::ZOGLIN_ATTACK_DAMAGE, 6.0);
        assert_eq!(crate::entity::combat::ZOGLIN_BABY_ATTACK_DAMAGE, 0.5);
        assert_eq!(crate::entity::combat::ZOGLIN_ADULT_ATTACK_INTERVAL, 40);
        assert_eq!(crate::entity::combat::ZOGLIN_BABY_ATTACK_INTERVAL, 15);
        assert_eq!(crate::entity::combat::ZOGLIN_ATTACK_DURATION, 200);
        assert_eq!(crate::entity::combat::calculate_zoglin_attack_interval(false), 40);
        assert_eq!(crate::entity::combat::calculate_zoglin_attack_interval(true), 15);
    }
}
