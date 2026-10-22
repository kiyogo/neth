use crate::block::{
    BlockBehaviour, BlockMetadata, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnScheduledTickArgs, PathComputationType,
};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

pub struct FrogspawnBlock;

impl BlockMetadata for FrogspawnBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::FROGSPAWN].into()
    }
}

fn can_survive(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let below = pos.down();
    let (block_below, state_below) = world.get_block_and_state(&below);
    let fluid_below_is_water = state_below.is_waterlogged() || block_below == &Block::WATER;
    let above = pos.up();
    let state_above = world.get_block_state(&above);
    fluid_below_is_water && state_above.is_air()
}

impl BlockBehaviour for FrogspawnBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if can_survive(args.world, args.position) {
            let delay = rand::rng().random_range(3600..=12000);
            args.world
                .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
            args.block.default_state.id
        } else {
            BlockStateId::AIR
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_survive(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !can_survive(args.world, args.position) {
            BlockStateId::AIR
        } else {
            args.state_id
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !can_survive(args.world.as_ref(), args.position) {
            args.world.break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        } else {
            args.world
                .set_block_state(args.position, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
            args.world.play_sound(
                Sound::BlockFrogspawnHatch,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
