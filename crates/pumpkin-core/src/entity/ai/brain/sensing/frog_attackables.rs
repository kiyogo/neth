use std::sync::atomic::Ordering;

use crate::entity::combat;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

pub const TARGET_DETECTION_DISTANCE: f32 = 10.0;
pub const TARGET_DETECTION_DISTANCE_SQ: f64 = 100.0;

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_ATTACKABLE.id(),
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
    types::UNREACHABLE_TONGUE_TARGETS.id(),
];

/// Mirrors vanilla `FrogAttackablesSensor.java`.
/// Scans visible entities that the frog can eat (small slimes and magma cubes)
/// within 10 blocks that are not currently unreachable.
pub struct FrogAttackablesSensor;

impl Sensor for FrogAttackablesSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let unreachable = tick.brain.get(types::UNREACHABLE_TONGUE_TARGETS);
        let body_pos = tick.mob.get_entity().pos.load();
        let ctx = tick.visibility();

        let target = tick
            .brain
            .get(types::NEAREST_VISIBLE_LIVING_ENTITIES)
            .and_then(|visible| {
                visible.find_closest(&ctx, |entity| {
                    let ent = entity.get_entity();
                    if !ent.is_alive() {
                        return false;
                    }
                    if ent.pos.load().squared_distance_to_vec(&body_pos) > TARGET_DETECTION_DISTANCE_SQ {
                        return false;
                    }
                    let size = ent.data.load(Ordering::Relaxed);
                    if !combat::can_frog_eat_entity(ent.entity_type, size) {
                        return false;
                    }
                    if let Some(list) = &unreachable {
                        if list.contains(&ent.entity_uuid) {
                            return false;
                        }
                    }
                    true
                })
            });

        tick.brain.set_optional(types::NEAREST_ATTACKABLE, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frog_attackables_sensor_parity() {
        assert_eq!(TARGET_DETECTION_DISTANCE, 10.0);
        assert_eq!(TARGET_DETECTION_DISTANCE_SQ, 100.0);
        assert_eq!(FrogAttackablesSensor.requires().len(), 3);
        assert_eq!(
            FrogAttackablesSensor.requires()[0],
            types::NEAREST_ATTACKABLE.id()
        );
        assert_eq!(
            FrogAttackablesSensor.requires()[1],
            types::NEAREST_VISIBLE_LIVING_ENTITIES.id()
        );
        assert_eq!(
            FrogAttackablesSensor.requires()[2],
            types::UNREACHABLE_TONGUE_TARGETS.id()
        );
    }
}
