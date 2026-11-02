use pumpkin_data::attributes::Attributes;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, NearestVisibleLivingEntities, types};
use super::{Sensor, entities_in_inflated_box};

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_LIVING_ENTITIES.id(),
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
    types::NEAREST_ATTACKABLE.id(),
];

/// Mirrors vanilla `BreezeAttackEntitySensor.java`.
/// Scans nearest living entities and targets the closest attackable entity (non-creative, non-spectator).
pub struct BreezeAttackEntitySensor;

impl Sensor for BreezeAttackEntitySensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let follow_range = tick
            .mob
            .get_mob_entity()
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        let nearby = entities_in_inflated_box(
            tick.world,
            tick.mob.get_entity(),
            follow_range,
            follow_range,
            follow_range,
            |entity| {
                entity.get_living_entity().is_some()
                    && super::super::behavior::utils::is_alive(entity.as_ref())
            },
        );

        tick.brain
            .set(types::NEAREST_LIVING_ENTITIES, nearby.clone());
        tick.brain.set(
            types::NEAREST_VISIBLE_LIVING_ENTITIES,
            NearestVisibleLivingEntities::new(nearby.clone()),
        );

        let body_id = tick.mob.get_entity().entity_id;

        let attackable = nearby
            .into_iter()
            .find(|entity| {
                let ent = entity.get_entity();
                if ent.entity_id == body_id || !ent.is_alive() {
                    return false;
                }
                if let Some(player) = entity.get_player() {
                    if !player.living_entity.is_part_of_game() {
                        return false;
                    }
                }
                true
            });

        tick.brain.set_optional(types::NEAREST_ATTACKABLE, attackable);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breeze_attack_entity_sensor_parity() {
        assert_eq!(BreezeAttackEntitySensor.requires().len(), 3);
        assert_eq!(
            BreezeAttackEntitySensor.requires()[0],
            types::NEAREST_LIVING_ENTITIES.id()
        );
        assert_eq!(
            BreezeAttackEntitySensor.requires()[1],
            types::NEAREST_VISIBLE_LIVING_ENTITIES.id()
        );
        assert_eq!(
            BreezeAttackEntitySensor.requires()[2],
            types::NEAREST_ATTACKABLE.id()
        );
    }
}
