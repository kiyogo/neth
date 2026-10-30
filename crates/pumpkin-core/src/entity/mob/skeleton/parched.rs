use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::{PotionContentsImpl, StatusEffectInstance};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;

use crate::entity::combat::{
    PARCHED_DEFAULT_ATTACK_INTERVAL, PARCHED_HARD_ATTACK_INTERVAL, PARCHED_MAX_HEALTH,
    PARCHED_WEAKNESS_DURATION_TICKS, PARCHED_XP_REWARD, calculate_parched_attack_interval,
};
use crate::entity::{
    Entity,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, equipment::RegionalDifficulty, skeleton::SkeletonEntityBase},
};
use crate::world::World;

pub struct ParchedSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl ParchedSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new_with_attack_interval(
            entity,
            PARCHED_HARD_ATTACK_INTERVAL,
        );

        // Vanilla: Attributes.MAX_HEALTH = 16.0
        {
            let mut attributes = entity
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = f64::from(PARCHED_MAX_HEALTH);
                health.dirty.store(true, Ordering::Relaxed);
            }
        }

        let parched = Self { entity };
        Arc::new(parched)
    }
}

impl Mob for ParchedSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn as_parched(&self) -> Option<&ParchedSkeletonEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityParchedAmbient)
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        let world = self.entity.mob_entity.living_entity.entity.world.load();
        let is_hard = world.get_difficulty() == pumpkin_util::Difficulty::Hard;
        Some(calculate_parched_attack_interval(is_hard))
    }

    fn get_base_experience_reward(&self) -> u32 {
        PARCHED_XP_REWARD
    }

    fn get_arrow_projectile(&self) -> Option<ItemStack> {
        let mut arrow = ItemStack::new(1, &Item::ARROW);

        let weakness_effect = StatusEffectInstance {
            effect_id: Cow::Borrowed("minecraft:weakness"),
            amplifier: 0,
            duration: PARCHED_WEAKNESS_DURATION_TICKS,
            ambient: false,
            show_particles: true,
            show_icon: true,
        };

        let potion_contents = PotionContentsImpl {
            potion_id: None,
            custom_color: None,
            custom_effects: vec![weakness_effect],
            custom_name: None,
        };

        arrow.set_data_component(potion_contents);
        Some(arrow)
    }

    fn populate_default_equipment_slots(
        &self,
        world: &Arc<World>,
        difficulty: &RegionalDifficulty,
    ) {
        self.entity
            .populate_default_equipment_slots(world, difficulty);
    }

    fn populate_default_equipment_enchantments(&self, difficulty: &RegionalDifficulty) {
        self.entity
            .populate_default_equipment_enchantments(difficulty);
    }
}

impl CustomSound for ParchedSkeletonEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityParchedDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityParchedHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parched_parity_constants() {
        assert_eq!(PARCHED_MAX_HEALTH, 16.0);
        assert_eq!(PARCHED_XP_REWARD, 5);
        assert_eq!(PARCHED_WEAKNESS_DURATION_TICKS, 600);
        assert_eq!(PARCHED_HARD_ATTACK_INTERVAL, 50);
        assert_eq!(PARCHED_DEFAULT_ATTACK_INTERVAL, 70);
        assert_eq!(calculate_parched_attack_interval(true), 50);
        assert_eq!(calculate_parched_attack_interval(false), 70);
    }
}
