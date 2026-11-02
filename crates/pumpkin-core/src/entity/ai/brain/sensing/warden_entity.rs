use std::sync::Arc;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;

use crate::entity::EntityBase;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, NearestVisibleLivingEntities, types};
use super::{Sensor, entities_in_inflated_box};

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_LIVING_ENTITIES.id(),
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
    types::NEAREST_ATTACKABLE.id(),
];

/// Mirrors vanilla `WardenEntitySensor.java`.
/// Scans nearest living entities, prioritizing players, then non-players (excluding other wardens and armor stands).
pub struct WardenEntitySensor;

impl WardenEntitySensor {
    fn can_target_entity(body_id: i32, entity: &dyn EntityBase) -> bool {
        let ent = entity.get_entity();
        if ent.entity_id == body_id {
            return false;
        }
        if !ent.is_alive() {
            return false;
        }
        if ent.entity_type.id == EntityType::WARDEN.id
            || ent.entity_type.id == EntityType::ARMOR_STAND.id
        {
            return false;
        }
        if let Some(player) = entity.get_player() {
            if !player.living_entity.is_part_of_game() {
                return false;
            }
        }
        true
    }
}

impl Sensor for WardenEntitySensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        // NearestLivingEntitySensor::doTick logic
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

        // Prioritize Player, then any valid living entity
        let closest_player = nearby
            .iter()
            .filter(|e| e.get_entity().entity_type.id == EntityType::PLAYER.id)
            .find(|e| Self::can_target_entity(body_id, e.as_ref()))
            .cloned();

        let attackable: Option<Arc<dyn EntityBase>> = closest_player.or_else(|| {
            nearby
                .iter()
                .filter(|e| e.get_entity().entity_type.id != EntityType::PLAYER.id)
                .find(|e| Self::can_target_entity(body_id, e.as_ref()))
                .cloned()
        });

        tick.brain.set_optional(types::NEAREST_ATTACKABLE, attackable);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warden_entity_sensor_requires_parity() {
        assert_eq!(WardenEntitySensor.requires().len(), 3);
        assert_eq!(
            WardenEntitySensor.requires()[0],
            types::NEAREST_LIVING_ENTITIES.id()
        );
        assert_eq!(
            WardenEntitySensor.requires()[1],
            types::NEAREST_VISIBLE_LIVING_ENTITIES.id()
        );
        assert_eq!(
            WardenEntitySensor.requires()[2],
            types::NEAREST_ATTACKABLE.id()
        );
    }
}
