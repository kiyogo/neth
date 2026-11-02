use std::sync::atomic::Ordering::Relaxed;

use pumpkin_data::tag::Taggable;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_VISIBLE_ADULT.id(),
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
];

/// Mirrors vanilla `AdultSensor.java`.
/// Finds the nearest visible adult of the same entity type and sets `NEAREST_VISIBLE_ADULT`.
pub struct AdultSensor;

impl Sensor for AdultSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let body_type_id = tick.mob.get_entity().entity_type.id;
        let ctx = tick.visibility();

        let nearest_adult = tick
            .brain
            .get(types::NEAREST_VISIBLE_LIVING_ENTITIES)
            .and_then(|visible| {
                visible.find_closest(&ctx, |entity| {
                    entity.get_entity().entity_type.id == body_type_id
                        && entity.get_entity().age.load(Relaxed) >= 0
                })
            });

        tick.brain
            .set_optional(types::NEAREST_VISIBLE_ADULT, nearest_adult);
    }
}

/// Mirrors vanilla `AdultSensorAnyType.java`.
/// Finds the nearest visible adult matching `#minecraft:followable_friendly_mobs` and sets `NEAREST_VISIBLE_ADULT`.
pub struct AdultSensorAnyType;

impl Sensor for AdultSensorAnyType {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let ctx = tick.visibility();

        let nearest_adult = tick
            .brain
            .get(types::NEAREST_VISIBLE_LIVING_ENTITIES)
            .and_then(|visible| {
                visible.find_closest(&ctx, |entity| {
                    entity
                        .get_entity()
                        .entity_type
                        .has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_FOLLOWABLE_FRIENDLY_MOBS)
                        && entity.get_entity().age.load(Relaxed) >= 0
                })
            });

        tick.brain
            .set_optional(types::NEAREST_VISIBLE_ADULT, nearest_adult);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adult_sensor_requires_parity() {
        assert_eq!(AdultSensor.requires().len(), 2);
        assert_eq!(AdultSensor.requires()[0], types::NEAREST_VISIBLE_ADULT.id());
        assert_eq!(
            AdultSensor.requires()[1],
            types::NEAREST_VISIBLE_LIVING_ENTITIES.id()
        );
        assert_eq!(AdultSensorAnyType.requires().len(), 2);
    }
}
