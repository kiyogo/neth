use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use pumpkin_data::entity::EntityType;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_util::math::position::BlockPos;

use crate::entity::EntityBase;
use crate::world::World;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

const REPELLENT_DETECTION_RANGE_HORIZONTAL: i32 = 8;
const REPELLENT_DETECTION_RANGE_VERTICAL: i32 = 4;

const REQUIRES: &[MemoryModuleId] = &[
    types::NEAREST_VISIBLE_LIVING_ENTITIES.id(),
    types::NEAREST_REPELLENT.id(),
    types::NEAREST_VISIBLE_ADULT_PIGLIN.id(),
    types::NEAREST_VISIBLE_ADULT_HOGLINS.id(),
    types::VISIBLE_ADULT_PIGLIN_COUNT.id(),
    types::VISIBLE_ADULT_HOGLIN_COUNT.id(),
];

/// Mirrors vanilla `HoglinSpecificSensor.java`.
/// Scans nearest visible adult piglins and hoglins, counts them, and locates the nearest repellent.
pub struct HoglinSpecificSensor;

impl Sensor for HoglinSpecificSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let center = tick.mob.get_entity().block_pos.load();
        let repellent = find_nearest_repellent(tick.world, center);
        tick.brain.set_optional(types::NEAREST_REPELLENT, repellent);

        let mut nearest_adult_piglin: Option<Arc<dyn EntityBase>> = None;
        let mut adult_piglin_count: i32 = 0;
        let mut adult_hoglins: Vec<Arc<dyn EntityBase>> = Vec::new();

        let ctx = tick.visibility();
        if let Some(visible) = tick.brain.get(types::NEAREST_VISIBLE_LIVING_ENTITIES) {
            for entity in visible.find_all(&ctx, |e| {
                let ent = e.get_entity();
                let is_adult = ent.age.load(Relaxed) >= 0;
                let is_piglin_or_hoglin = ent.entity_type.id == EntityType::PIGLIN.id
                    || ent.entity_type.id == EntityType::PIGLIN_BRUTE.id
                    || ent.entity_type.id == EntityType::HOGLIN.id;
                is_adult && is_piglin_or_hoglin
            }) {
                let ent_type_id = entity.get_entity().entity_type.id;
                if ent_type_id == EntityType::PIGLIN.id || ent_type_id == EntityType::PIGLIN_BRUTE.id {
                    adult_piglin_count += 1;
                    if nearest_adult_piglin.is_none() {
                        nearest_adult_piglin = Some(entity.clone());
                    }
                } else if ent_type_id == EntityType::HOGLIN.id {
                    adult_hoglins.push(entity.clone());
                }
            }
        }

        let adult_hoglin_count = adult_hoglins.len() as i32;

        tick.brain
            .set_optional(types::NEAREST_VISIBLE_ADULT_PIGLIN, nearest_adult_piglin);
        tick.brain
            .set(types::NEAREST_VISIBLE_ADULT_HOGLINS, adult_hoglins);
        tick.brain
            .set(types::VISIBLE_ADULT_PIGLIN_COUNT, adult_piglin_count);
        tick.brain
            .set(types::VISIBLE_ADULT_HOGLIN_COUNT, adult_hoglin_count);
    }
}

/// Vanilla `level.findBlocksInBoxByManhattanDistance(body.blockPosition(), 8, 4)`.
fn find_nearest_repellent(world: &World, center: BlockPos) -> Option<BlockPos> {
    let max_distance =
        REPELLENT_DETECTION_RANGE_HORIZONTAL * 2 + REPELLENT_DETECTION_RANGE_VERTICAL;
    for distance in 0..=max_distance {
        for y in -REPELLENT_DETECTION_RANGE_VERTICAL..=REPELLENT_DETECTION_RANGE_VERTICAL {
            let remaining = distance - y.abs();
            if remaining < 0 {
                continue;
            }
            for x in -REPELLENT_DETECTION_RANGE_HORIZONTAL..=REPELLENT_DETECTION_RANGE_HORIZONTAL {
                let z_abs = remaining - x.abs();
                if !(0..=REPELLENT_DETECTION_RANGE_HORIZONTAL).contains(&z_abs) {
                    continue;
                }
                for z in [z_abs, -z_abs] {
                    let pos = BlockPos::new(center.0.x + x, center.0.y + y, center.0.z + z);
                    let (block, _) = world.get_block_and_state_id(&pos);
                    if block.has_tag(&tag::Block::MINECRAFT_HOGLIN_REPELLENTS) {
                        return Some(pos);
                    }
                    if z_abs == 0 {
                        break;
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hoglin_specific_sensor_parity() {
        assert_eq!(HoglinSpecificSensor.requires().len(), 6);
        assert_eq!(REPELLENT_DETECTION_RANGE_HORIZONTAL, 8);
        assert_eq!(REPELLENT_DETECTION_RANGE_VERTICAL, 4);
    }
}
