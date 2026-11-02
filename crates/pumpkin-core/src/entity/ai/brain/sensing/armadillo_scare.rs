use std::sync::atomic::Ordering::Relaxed;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

pub const ARMADILLO_SCARE_SCAN_RATE: i32 = 5;
pub const ARMADILLO_SCARE_TTL: i64 = 80;

const REQUIRES: &[MemoryModuleId] = &[types::NEAREST_LIVING_ENTITIES.id()];

/// Mirrors vanilla `MobSensor<Armadillo>` with `ARMADILLO_SCARE_DETECTED`.
/// Scans nearby living entities every 5 ticks; if any threat is present and the armadillo can stay rolled up,
/// sets `DANGER_DETECTED_RECENTLY` with an 80-tick expiry.
pub struct ArmadilloScareSensor;

impl Sensor for ArmadilloScareSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let mob_entity = tick.mob.get_mob_entity();
        let entity = &mob_entity.living_entity.entity;

        let in_liquid = entity.is_in_water() || entity.touching_lava.load(Relaxed);
        let is_panicking = tick.mob.is_panicking();
        let is_leashed = entity.is_leashed();
        let is_passenger = entity.has_vehicle();
        let is_vehicle = entity.has_passengers();

        let can_stay_rolled_up = crate::entity::passive::armadillo::ArmadilloEntity::can_stay_rolled_up_predicate(
            is_panicking,
            in_liquid,
            is_leashed,
            is_passenger,
            is_vehicle,
        );

        if !can_stay_rolled_up {
            tick.brain.erase(types::DANGER_DETECTED_RECENTLY.id());
            return;
        }

        let is_scared_by_threat = if let Some(armadillo) = tick.mob.as_armadillo() {
            if let Some(nearby) = tick.brain.get(types::NEAREST_LIVING_ENTITIES) {
                nearby.iter().any(|other| armadillo.is_scared_by(other.as_ref()))
            } else {
                false
            }
        } else {
            false
        };

        if is_scared_by_threat {
            tick.brain.set_with_expiry(
                types::DANGER_DETECTED_RECENTLY,
                true,
                ARMADILLO_SCARE_TTL,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_armadillo_scare_sensor_parity() {
        assert_eq!(ARMADILLO_SCARE_SCAN_RATE, 5);
        assert_eq!(ARMADILLO_SCARE_TTL, 80);
        assert_eq!(ArmadilloScareSensor.requires(), &[types::NEAREST_LIVING_ENTITIES.id()]);
    }
}
