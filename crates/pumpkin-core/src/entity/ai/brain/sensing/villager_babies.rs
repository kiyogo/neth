use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use pumpkin_data::entity::EntityType;

use crate::entity::EntityBase;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

const REQUIRES: &[MemoryModuleId] = &[
    types::VISIBLE_VILLAGER_BABIES.id(),
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
];

pub struct VillagerBabiesSensor;

impl Sensor for VillagerBabiesSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let ctx = tick.visibility();

        let babies: Vec<Arc<dyn EntityBase>> = tick
            .brain
            .get(types::NEAREST_VISIBLE_LIVING_ENTITIES)
            .map(|visible| {
                visible
                    .find_all(&ctx, |entity| {
                        entity.get_entity().entity_type == &EntityType::VILLAGER
                            && entity.get_entity().age.load(Relaxed) < 0
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        tick.brain.set(types::VISIBLE_VILLAGER_BABIES, babies);
    }
}
