use std::borrow::Cow;
use std::sync::Arc;

use pumpkin_data::data_component_impl::{PotionContentsImpl, StatusEffectInstance};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;

use crate::entity::combat::{
    STRAY_DEFAULT_ATTACK_INTERVAL, STRAY_HARD_ATTACK_INTERVAL, STRAY_MAX_HEALTH,
    STRAY_SLOWNESS_DURATION_TICKS, STRAY_XP_REWARD, calculate_stray_attack_interval,
};
use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, equipment::RegionalDifficulty, skeleton::SkeletonEntityBase},
};
use crate::world::World;

pub struct StraySkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl StraySkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let stray = Self { entity };
        Arc::new(stray)
    }
}

impl CustomSound for StraySkeletonEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityStrayDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityStrayHurt)
    }
}

impl Mob for StraySkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn as_stray(&self) -> Option<&StraySkeletonEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityStrayAmbient)
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        let world = self.get_entity().world.load();
        let is_hard = world.get_difficulty() == pumpkin_util::Difficulty::Hard;
        Some(calculate_stray_attack_interval(is_hard))
    }

    fn get_base_experience_reward(&self) -> u32 {
        STRAY_XP_REWARD
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

    fn get_arrow_projectile(&self) -> Option<ItemStack> {
        let mut arrow = ItemStack::new(1, &Item::ARROW);

        let slowness_effect = StatusEffectInstance {
            effect_id: Cow::Borrowed("minecraft:slowness"),
            amplifier: 0,
            duration: STRAY_SLOWNESS_DURATION_TICKS,
            ambient: false,
            show_particles: true,
            show_icon: true,
        };

        let potion_contents = PotionContentsImpl {
            potion_id: None,
            custom_color: None,
            custom_effects: vec![slowness_effect],
            custom_name: None,
        };

        arrow.set_data_component(potion_contents);
        Some(arrow)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stray_slowness_and_interval_parity() {
        assert_eq!(STRAY_MAX_HEALTH, 20.0);
        assert_eq!(STRAY_SLOWNESS_DURATION_TICKS, 600);
        assert_eq!(STRAY_HARD_ATTACK_INTERVAL, 20);
        assert_eq!(STRAY_DEFAULT_ATTACK_INTERVAL, 40);
        assert_eq!(calculate_stray_attack_interval(true), 20);
        assert_eq!(calculate_stray_attack_interval(false), 40);
        assert_eq!(STRAY_XP_REWARD, 5);
    }
}
