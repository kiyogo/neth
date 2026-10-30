use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};
use uuid::Uuid;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::{damage::DamageType, entity::EntityType, sound::Sound};

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
};

pub struct VexEntity {
    pub mob_entity: MobEntity,
    pub has_limited_life: AtomicBool,
    pub limited_life_ticks: AtomicI32,
    pub is_charging: AtomicBool,
    pub owner_uuid: Mutex<Option<Uuid>>,
}

impl VexEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::VEX_XP_REWARD;
    pub fn new(entity: Entity) -> Arc<Self> {
        entity.synched_data.define(pumpkin_data::tracked_data::vex::DATA_FLAGS_ID, 0u8);
        let mob_entity = MobEntity::new(entity);
        let vex = Self {
            mob_entity,
            has_limited_life: AtomicBool::new(false),
            limited_life_ticks: AtomicI32::new(0),
            is_charging: AtomicBool::new(false),
            owner_uuid: Mutex::new(None),
        };
        let mob_arc = Arc::new(vex);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Vex: populateDefaultEquipmentSlots -> Iron Sword in mainhand with 0.0 drop chance
        {
            let mut equipment = mob_arc
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equipment.put(
                &pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
                pumpkin_data::item_stack::ItemStack::new(1, &pumpkin_data::item::Item::IRON_SWORD),
            );
        }

        // Vanilla Vex: MAX_HEALTH = 14.0, ATTACK_DAMAGE = 4.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::VEX_MAX_HEALTH as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, crate::entity::combat::VEX_BASE_ATTACK_DAMAGE as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::VEX_MAX_HEALTH);

        // Vanilla Vex: noPhysics = true (flies through solid blocks), hasNoGravity = true
        let entity_ref = mob_arc.get_entity();
        entity_ref.no_physics.store(true, Ordering::Relaxed);
        entity_ref.set_has_no_gravity(true);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
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
        };

        mob_arc
    }

    pub fn set_limited_life(&self, life_ticks: i32) {
        self.has_limited_life.store(true, Ordering::Relaxed);
        self.limited_life_ticks.store(life_ticks, Ordering::Relaxed);
    }

    pub fn set_owner(&self, owner_uuid: Option<Uuid>) {
        *self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = owner_uuid;
    }

    #[must_use]
    pub fn get_owner(&self) -> Option<Uuid> {
        *self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_charging(&self, charging: bool) {
        self.is_charging.store(charging, Ordering::Relaxed);
        self.mob_entity.living_entity.entity.set_synced_data(
            pumpkin_data::tracked_data::vex::DATA_FLAGS_ID,
            if charging { 1u8 } else { 0u8 },
        );
    }

    #[must_use]
    pub fn is_charging(&self) -> bool {
        self.is_charging.load(Ordering::Relaxed)
    }
}

impl Mob for VexEntity {
    fn as_vex(&self) -> Option<&VexEntity> {
        Some(self)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn on_attack(&self, _target: &dyn EntityBase) {
        let entity = self.get_entity();
        let world = entity.world.load();
        world.play_sound_fine(
            Sound::EntityVexCharge,
            pumpkin_data::sound::SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        if self.has_limited_life.load(Ordering::Relaxed) {
            let ticks = self.limited_life_ticks.fetch_sub(1, Ordering::Relaxed);
            if ticks <= 1 {
                self.limited_life_ticks.store(crate::entity::combat::VEX_DECAY_DAMAGE_INTERVAL_TICKS, Ordering::Relaxed);
                self.mob_entity
                    .living_entity
                    .damage_with_context(self, 1.0, DamageType::STARVE, None, None, None);
            }
        }

        let entity = self.get_entity();
        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..80) == 0 {
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityVexAmbient,
                pumpkin_data::sound::SoundCategory::Hostile,
                &entity.pos.load(),
            );
        }
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn burns_in_daylight(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        crate::entity::combat::get_vex_xp_reward()
    }
}

impl CustomSound for VexEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityVexHurt)
    }

    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityVexDeath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::combat::*;

    #[test]
    fn test_vex_parity_constants() {
        assert_eq!(VexEntity::XP_REWARD, 3);
        assert_eq!(crate::entity::combat::VEX_MAX_HEALTH, 14.0);
        assert_eq!(crate::entity::combat::VEX_BASE_ATTACK_DAMAGE, 4.0);
    }

    #[test]
    fn test_vex_constants_and_decay_parity() {
        assert_eq!(VEX_DEFAULT_HEALTH, 14.0);
        assert_eq!(VEX_ATTACK_DAMAGE, 4.0);
        assert_eq!(VEX_XP_REWARD, 3);
        assert_eq!(VEX_TICKS_PER_FLAP, 4);
        assert_eq!(VEX_DECAY_DAMAGE_INTERVAL_TICKS, 20);
        assert!(is_vex_flapping(12));
        assert!(!is_vex_flapping(13));
        assert_eq!(calculate_vex_summon_lifetime(30), 1200);
    }
}
