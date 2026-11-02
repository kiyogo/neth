use pumpkin_data::entity::EntityType;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

/// Mirrors vanilla `GolemSensor.java:GOLEM_SCAN_RATE` (10 seconds).
pub const GOLEM_SCAN_RATE: i32 = 200;
/// Mirrors vanilla `GolemSensor.java:MEMORY_TIME_TO_LIVE` (599 ticks, ~30 seconds).
pub const GOLEM_MEMORY_TTL: i64 = 599;

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_LIVING_ENTITIES.id(),
    types::GOLEM_DETECTED_RECENTLY.id(),
];

pub struct GolemSensor;

impl Sensor for GolemSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let golem_present = tick
            .brain
            .get(types::NEAREST_LIVING_ENTITIES)
            .is_some_and(|entities| {
                entities
                    .iter()
                    .any(|e| e.get_entity().entity_type == &EntityType::IRON_GOLEM)
            });

        if golem_present {
            tick.brain
                .set_with_expiry(types::GOLEM_DETECTED_RECENTLY, true, GOLEM_MEMORY_TTL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golem_sensor_constants_parity() {
        assert_eq!(GOLEM_SCAN_RATE, 200);
        assert_eq!(GOLEM_MEMORY_TTL, 599);
    }
}
