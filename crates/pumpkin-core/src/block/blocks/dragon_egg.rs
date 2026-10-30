use crate::block::blocks::falling::FallingBlock;
use crate::block::registry::BlockActionResult;
use crate::block::{
    AttackArgs, BlockBehaviour, GetStateForNeighborUpdateArgs, NormalUseArgs, OnScheduledTickArgs,
    PathComputationType, PlacedArgs,
};
use crate::world::World;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use rand::{RngExt, rng};
use std::sync::Arc;

#[pumpkin_block("minecraft:dragon_egg")]
pub struct DragonEggBlock;

impl DragonEggBlock {
    pub const DELAY_AFTER_PLACE: u32 = 5;
    pub const HORIZONTAL_TELEPORT_RADIUS: i32 = 16;
    pub const VERTICAL_TELEPORT_RADIUS: i32 = 8;
    pub const TELEPORT_ATTEMPTS: usize = 1000;
    pub const EVENT_DRAGON_EGG_TELEPORT: u32 = 2015;
    pub const DUST_COLOR: i32 = -16777216;

    #[must_use]
    pub const fn pack_teleport_difference(pos: &BlockPos, target: &BlockPos) -> i32 {
        pos.pack_difference_in_position(
            target,
            Self::HORIZONTAL_TELEPORT_RADIUS,
            Self::VERTICAL_TELEPORT_RADIUS,
            Self::HORIZONTAL_TELEPORT_RADIUS,
        )
    }

    pub fn teleport(world: &Arc<World>, pos: &BlockPos) {
        for _ in 0..Self::TELEPORT_ATTEMPTS {
            let offset_x = rng().random_range(0..Self::HORIZONTAL_TELEPORT_RADIUS)
                - rng().random_range(0..Self::HORIZONTAL_TELEPORT_RADIUS);
            let offset_y = rng().random_range(0..Self::VERTICAL_TELEPORT_RADIUS)
                - rng().random_range(0..Self::VERTICAL_TELEPORT_RADIUS);
            let offset_z = rng().random_range(0..Self::HORIZONTAL_TELEPORT_RADIUS)
                - rng().random_range(0..Self::HORIZONTAL_TELEPORT_RADIUS);

            let test_pos = pos.add(offset_x, offset_y, offset_z);

            // Bounds check inside build height (-64 to 319 for overworld)
            if test_pos.0.y < -64 || test_pos.0.y >= 320 {
                continue;
            }

            let state = world.get_block_state(&test_pos);
            let below_state = world.get_block_state(&test_pos.down());

            if state.is_air() && !below_state.is_air() {
                let packed_diff = Self::pack_teleport_difference(pos, &test_pos);
                world.sync_world_event(WorldEvent::ParticlesDragonEggTeleport, *pos, packed_diff);
                let current_state = world.get_block_state(pos);
                world.set_block_state(
                    &test_pos,
                    current_state.id,
                    pumpkin_world::world::BlockFlags::NOTIFY_ALL,
                );
                world.set_block_state(
                    pos,
                    pumpkin_data::Block::AIR.default_state.id,
                    pumpkin_world::world::BlockFlags::NOTIFY_ALL,
                );
                return;
            }
        }
    }
}

impl BlockBehaviour for DragonEggBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        FallingBlock::placed_with_delay(&args, Self::DELAY_AFTER_PLACE);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        FallingBlock::get_state_for_neighbor_update_with_delay(&args, Self::DELAY_AFTER_PLACE)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        Self::teleport(args.world, args.position);
        BlockActionResult::Success
    }

    fn attack(&self, args: AttackArgs<'_>) {
        Self::teleport(args.world, args.position);
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        FallingBlock::on_scheduled_tick(&FallingBlock, args);
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
