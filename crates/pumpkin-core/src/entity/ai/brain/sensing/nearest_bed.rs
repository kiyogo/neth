use std::sync::atomic::Ordering::Relaxed;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

pub const NEAREST_BED_SCAN_RATE: i32 = 20;
pub const NEAREST_BED_SEARCH_RADIUS: i32 = 48;

const REQUIRES: &[MemoryModuleId] = &[types::NEAREST_BED.id()];

/// Mirrors vanilla `NearestBedSensor.java`.
/// Scans for a nearby bed POI for baby villagers within 48 blocks and sets `NEAREST_BED`.
pub struct NearestBedSensor;

impl Sensor for NearestBedSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let is_baby = tick.mob.get_entity().age.load(Relaxed) < 0;
        if !is_baby {
            return;
        }

        let center = tick.mob.get_entity().block_pos.load();
        let bed = crate::entity::passive::villager::goals::find_free_bed(
            tick.world,
            center,
            NEAREST_BED_SEARCH_RADIUS,
            16,
            None,
        );

        tick.brain.set_optional(types::NEAREST_BED, bed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nearest_bed_sensor_parity() {
        assert_eq!(NEAREST_BED_SCAN_RATE, 20);
        assert_eq!(NEAREST_BED_SEARCH_RADIUS, 48);
        assert_eq!(NearestBedSensor.requires(), &[types::NEAREST_BED.id()]);
    }
}
