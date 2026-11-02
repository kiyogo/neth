use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

const REQUIRES: &[MemoryModuleId] = &[types::IS_IN_WATER.id()];

/// Mirrors vanilla `IsInWaterSensor.java`.
/// Checks if the mob is currently in water and sets `IS_IN_WATER`.
pub struct IsInWaterSensor;

impl Sensor for IsInWaterSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        if tick.mob.get_entity().is_in_water() {
            tick.brain.set(types::IS_IN_WATER, ());
        } else {
            tick.brain.erase(types::IS_IN_WATER.id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_in_water_sensor_requires_parity() {
        assert_eq!(IsInWaterSensor.requires(), &[types::IS_IN_WATER.id()]);
    }
}
