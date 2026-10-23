use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::{PotionContentsImpl, StatusEffectInstance};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;

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
        let entity = SkeletonEntityBase::new_with_attack_interval(entity, 50);

        // Vanilla: Attributes.MAX_HEALTH = 16.0
        {
            let mut attributes = entity
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 16.0;
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

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityParchedAmbient)
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        let world = self.entity.mob_entity.living_entity.entity.world.load();
        if world.get_difficulty() == pumpkin_util::Difficulty::Hard {
            Some(50)
        } else {
            Some(70)
        }
    }

    fn get_arrow_projectile(&self) -> Option<ItemStack> {
        let mut arrow = ItemStack::new(1, &Item::ARROW);

        let weakness_effect = StatusEffectInstance {
            effect_id: Cow::Borrowed("minecraft:weakness"),
            amplifier: 0,
            duration: 600,
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
