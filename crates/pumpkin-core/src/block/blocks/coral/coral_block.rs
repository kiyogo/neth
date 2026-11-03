use crate::block::{
    BlockBehaviour, BlockMetadata, BrokenArgs, GetStateForNeighborUpdateArgs, OnScheduledTickArgs,
    PlacedArgs,
    blocks::coral::{is_dead_coral, scan_for_water, try_schedule_die_tick},
};
use pumpkin_data::{Block, BlockId, BlockStateId};
use pumpkin_world::world::BlockFlags;

pub struct CoralBlock;

impl CoralBlock {
    pub const CORAL_TYPES_COUNT: usize = 5;
    pub const MIN_DECAY_TICKS: u32 = 60;
    pub const MAX_DECAY_TICKS: u32 = 100;
}

impl BlockMetadata for CoralBlock {
    fn ids() -> Box<[BlockId]> {
        Box::new([
            BlockId::BRAIN_CORAL_BLOCK,
            BlockId::BUBBLE_CORAL_BLOCK,
            BlockId::FIRE_CORAL_BLOCK,
            BlockId::HORN_CORAL_BLOCK,
            BlockId::TUBE_CORAL_BLOCK,
            BlockId::DEAD_BRAIN_CORAL_BLOCK,
            BlockId::DEAD_BUBBLE_CORAL_BLOCK,
            BlockId::DEAD_FIRE_CORAL_BLOCK,
            BlockId::DEAD_HORN_CORAL_BLOCK,
            BlockId::DEAD_TUBE_CORAL_BLOCK,
        ])
    }
}

impl BlockBehaviour for CoralBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        if !scan_for_water(args.world, args.position) && !is_dead_coral(args.block) {
            try_schedule_die_tick(args.block, args.world, args.position);
        }
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !scan_for_water(args.world, args.position) && !is_dead_coral(args.block) {
            try_schedule_die_tick(args.block, args.world, args.position);
        }
        args.state_id
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !scan_for_water(args.world, args.position) && !is_dead_coral(args.block) {
            let Some(dead_block) = get_dead_coral_block_type(args.block.id) else {
                return;
            };
            let dead_block_state_id = dead_block.default_state.id;
            args.world
                .set_block_state(args.position, dead_block_state_id, BlockFlags::NOTIFY_ALL);
        }
    }
}

const fn get_dead_coral_block_type(id: BlockId) -> Option<&'static Block> {
    match id {
        BlockId::BRAIN_CORAL_BLOCK => Some(&Block::DEAD_BRAIN_CORAL_BLOCK),
        BlockId::BUBBLE_CORAL_BLOCK => Some(&Block::DEAD_BUBBLE_CORAL_BLOCK),
        BlockId::FIRE_CORAL_BLOCK => Some(&Block::DEAD_FIRE_CORAL_BLOCK),
        BlockId::HORN_CORAL_BLOCK => Some(&Block::DEAD_HORN_CORAL_BLOCK),
        BlockId::TUBE_CORAL_BLOCK => Some(&Block::DEAD_TUBE_CORAL_BLOCK),
        _ => None,
    }
}
