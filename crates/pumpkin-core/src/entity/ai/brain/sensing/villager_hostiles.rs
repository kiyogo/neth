use pumpkin_data::entity::EntityType;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_HOSTILE.id(),
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
];

/// Mirrors vanilla `VillagerHostilesSensor.java:ACCEPTABLE_DISTANCE_FROM_HOSTILES`.
#[must_use]
pub fn acceptable_distance_from_hostile(entity_type: &EntityType) -> Option<f64> {
    let id = entity_type.id;
    if id == EntityType::DROWNED.id {
        Some(8.0)
    } else if id == EntityType::EVOKER.id {
        Some(12.0)
    } else if id == EntityType::HUSK.id {
        Some(8.0)
    } else if id == EntityType::ILLUSIONER.id {
        Some(12.0)
    } else if id == EntityType::PILLAGER.id {
        Some(15.0)
    } else if id == EntityType::RAVAGER.id {
        Some(12.0)
    } else if id == EntityType::VEX.id {
        Some(8.0)
    } else if id == EntityType::VINDICATOR.id {
        Some(10.0)
    } else if id == EntityType::ZOGLIN.id {
        Some(10.0)
    } else if id == EntityType::ZOMBIE.id {
        Some(8.0)
    } else if id == EntityType::ZOMBIE_VILLAGER.id {
        Some(8.0)
    } else {
        None
    }
}

pub struct VillagerHostilesSensor;

impl Sensor for VillagerHostilesSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let body_pos = tick.mob.get_entity().pos.load();
        let ctx = tick.visibility();

        let nearest_hostile = tick
            .brain
            .get(types::NEAREST_VISIBLE_LIVING_ENTITIES)
            .and_then(|visible| {
                visible.find_closest(&ctx, |entity| {
                    if let Some(threshold) =
                        acceptable_distance_from_hostile(entity.get_entity().entity_type)
                    {
                        let distance_sq =
                            entity.get_entity().pos.load().squared_distance_to_vec(&body_pos);
                        distance_sq <= threshold * threshold
                    } else {
                        false
                    }
                })
            });

        tick.brain
            .set_optional(types::NEAREST_HOSTILE, nearest_hostile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acceptable_distance_matches_vanilla_hostiles() {
        assert_eq!(acceptable_distance_from_hostile(&EntityType::DROWNED), Some(8.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::EVOKER), Some(12.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::HUSK), Some(8.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::ILLUSIONER), Some(12.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::PILLAGER), Some(15.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::RAVAGER), Some(12.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::VEX), Some(8.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::VINDICATOR), Some(10.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::ZOGLIN), Some(10.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::ZOMBIE), Some(8.0));
        assert_eq!(acceptable_distance_from_hostile(&EntityType::ZOMBIE_VILLAGER), Some(8.0));

        // Non-hostiles should not match
        assert_eq!(acceptable_distance_from_hostile(&EntityType::COW), None);
        assert_eq!(acceptable_distance_from_hostile(&EntityType::SHEEP), None);
        assert_eq!(acceptable_distance_from_hostile(&EntityType::IRON_GOLEM), None);
    }
}
