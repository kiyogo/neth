use pumpkin_data::tag::Taggable;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

pub const TARGET_DETECTION_DISTANCE: f32 = 8.0;
pub const TARGET_DETECTION_DISTANCE_SQ: f64 = 64.0;

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_ATTACKABLE.id(),
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
    types::HAS_HUNTING_COOLDOWN.id(),
];

/// Mirrors vanilla `AxolotlAttackablesSensor.java`.
/// Scans visible entities in water that are either always hostile or valid hunt targets (when not on cooldown).
pub struct AxolotlAttackablesSensor;

impl Sensor for AxolotlAttackablesSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let has_cooldown = tick.brain.has_memory_value(types::HAS_HUNTING_COOLDOWN.id());
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
                    if !ent.is_in_water() {
                        return false;
                    }
                    let is_hostile = ent
                        .entity_type
                        .has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_AXOLOTL_ALWAYS_HOSTILES);
                    let is_hunt = !has_cooldown
                        && ent
                            .entity_type
                            .has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_AXOLOTL_HUNT_TARGETS);
                    is_hostile || is_hunt
                })
            });

        tick.brain.set_optional(types::NEAREST_ATTACKABLE, target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axolotl_attackables_sensor_parity() {
        assert_eq!(TARGET_DETECTION_DISTANCE, 8.0);
        assert_eq!(TARGET_DETECTION_DISTANCE_SQ, 64.0);
        assert_eq!(AxolotlAttackablesSensor.requires().len(), 3);
        assert_eq!(
            AxolotlAttackablesSensor.requires()[0],
            types::NEAREST_ATTACKABLE.id()
        );
        assert_eq!(
            AxolotlAttackablesSensor.requires()[1],
            types::NEAREST_VISIBLE_LIVING_ENTITIES.id()
        );
        assert_eq!(
            AxolotlAttackablesSensor.requires()[2],
            types::HAS_HUNTING_COOLDOWN.id()
        );
    }
}
