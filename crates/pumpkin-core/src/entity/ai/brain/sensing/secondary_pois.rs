use pumpkin_data::Block;
use pumpkin_data::villager::VillagerProfession;
use pumpkin_util::math::position::BlockPos;

use super::super::BrainTick;
use super::super::memory::{MemoryModuleId, types};
use super::Sensor;

pub const SECONDARY_POI_SCAN_RATE: i32 = 40;
pub const HORIZONTAL_SEARCH: i32 = 4;
pub const VERTICAL_SEARCH: i32 = 2;

const REQUIRES: &[MemoryModuleId] = &[types::SECONDARY_JOB_SITE.id()];

/// Mirrors vanilla `SecondaryPoiSensor.java`.
/// Scans blocks in a 9x5x9 box around the villager for secondary POI blocks
/// corresponding to its profession (e.g. Farmland for Farmers).
pub struct SecondaryPoiSensor;

impl Sensor for SecondaryPoiSensor {
    fn requires(&self) -> &'static [MemoryModuleId] {
        REQUIRES
    }

    fn do_tick(&mut self, tick: &mut BrainTick<'_>) {
        let is_farmer = tick
            .mob
            .as_villager()
            .is_some_and(|v| v.villager_data.lock().unwrap().profession_enum() == VillagerProfession::Farmer);

        if !is_farmer {
            tick.brain.erase(types::SECONDARY_JOB_SITE.id());
            return;
        }

        let center = tick.mob.get_entity().block_pos.load();
        let mut job_sites = Vec::new();

        for x in -HORIZONTAL_SEARCH..=HORIZONTAL_SEARCH {
            for y in -VERTICAL_SEARCH..=VERTICAL_SEARCH {
                for z in -HORIZONTAL_SEARCH..=HORIZONTAL_SEARCH {
                    let test_pos = BlockPos::new(center.0.x + x, center.0.y + y, center.0.z + z);
                    let (block, _) = tick.world.get_block_and_state_id(&test_pos);
                    if block.id == Block::FARMLAND.id {
                        if let Some(global_pos) =
                            crate::entity::ai::brain::behavior::utils::global_pos_in(
                                tick.world, test_pos,
                            )
                        {
                            job_sites.push(global_pos);
                        }
                    }
                }
            }
        }

        if !job_sites.is_empty() {
            tick.brain.set(types::SECONDARY_JOB_SITE, job_sites);
        } else {
            tick.brain.erase(types::SECONDARY_JOB_SITE.id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secondary_poi_sensor_parity() {
        assert_eq!(SECONDARY_POI_SCAN_RATE, 40);
        assert_eq!(HORIZONTAL_SEARCH, 4);
        assert_eq!(VERTICAL_SEARCH, 2);
        assert_eq!(SecondaryPoiSensor.requires(), &[types::SECONDARY_JOB_SITE.id()]);
    }
}
