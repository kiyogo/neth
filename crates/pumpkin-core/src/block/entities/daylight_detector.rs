use std::sync::Arc;

use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;

use crate::block::blocks::redstone::daylight_detector::DaylightDetectorBlock;
use crate::world::World;

use super::BlockEntity;

pub struct DaylightDetectorBlockEntity {
    pub position: BlockPos,
}

impl BlockEntity for DaylightDetectorBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(_nbt: &NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        Self { position }
    }

    fn write_nbt(&self, _nbt: &mut NbtCompound) {}

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn tick(&self, world: &Arc<World>) {
        if world.get_world_age() % 20 == 0 && world.dimension.has_skylight {
            DaylightDetectorBlock::update_signal_strength(world, &self.position);
        }
    }
}

impl DaylightDetectorBlockEntity {
    pub const ID: &'static str = "minecraft:daylight_detector";
    pub const TICK_INTERVAL_TICKS: i64 = 20;
    pub const MAX_POWER: u8 = 15;
    pub const SHAPE_HEIGHT_PIXELS: f64 = 6.0;

    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self { position }
    }

    pub fn update_power(world: &Arc<World>, block_pos: &BlockPos) {
        DaylightDetectorBlock::update_signal_strength(world, block_pos);
    }
}
