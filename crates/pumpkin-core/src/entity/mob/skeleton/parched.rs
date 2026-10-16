use std::sync::Arc;

use pumpkin_data::sound::Sound;

use crate::entity::{
    Entity,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, skeleton::SkeletonEntityBase},
};

pub struct ParchedSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl ParchedSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
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
}

impl CustomSound for ParchedSkeletonEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonHurt)
    }
}
