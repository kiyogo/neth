use std::sync::Arc;

use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, OnPlaceArgs, OnScheduledTickArgs, PlacedArgs};
use pumpkin_data::{
    BlockId, BlockStateId, block_properties::SculkCatalystLikeProperties,
    sound::{Sound, SoundCategory},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

pub struct SculkCatalystBlock;

impl BlockMetadata for SculkCatalystBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_CATALYST].into()
    }
}

impl SculkCatalystBlock {
    pub const PULSE_TICKS: u32 = 8;
    pub const LISTENER_RADIUS: i32 = 8;
    pub const XP_REWARD: i32 = 5;
    pub const MAX_GROWTH_RATE_RADIUS: i32 = 24;
    pub const MAX_CHARGE: i32 = 1000;
    pub const MAX_DECAY_FACTOR: f32 = 0.5;
    pub const MAX_CURSORS: usize = 32;
    pub const SHRIEKER_PLACEMENT_RATE: i32 = 11;
    pub const MAX_CURSOR_DISTANCE: i32 = 1024;
    pub const DEFAULT_BLOOM: bool = false;

    // Level spreader defaults
    pub const LEVEL_GROWTH_SPAWN_COST: i32 = 10;
    pub const LEVEL_NO_GROWTH_RADIUS: i32 = 4;
    pub const LEVEL_CHARGE_DECAY_RATE: i32 = 10;
    pub const LEVEL_ADDITIONAL_DECAY_RATE: i32 = 5;

    // Worldgen spreader defaults
    pub const WORLDGEN_GROWTH_SPAWN_COST: i32 = 50;
    pub const WORLDGEN_NO_GROWTH_RADIUS: i32 = 1;
    pub const WORLDGEN_CHARGE_DECAY_RATE: i32 = 5;
    pub const WORLDGEN_ADDITIONAL_DECAY_RATE: i32 = 10;

    #[must_use]
    pub const fn clamp_charge(charge: i32) -> i32 {
        if charge < 0 {
            0
        } else if charge > Self::MAX_CHARGE {
            Self::MAX_CHARGE
        } else {
            charge
        }
    }
    pub fn bloom(world: &Arc<crate::world::World>, pos: &BlockPos) {
        let block = world.get_block(pos);
        if block.id != BlockId::SCULK_CATALYST {
            return;
        }
        let state = world.get_block_state(pos);
        let mut props = SculkCatalystLikeProperties::from_state_id(state.id);
        if !props.bloom {
            props.bloom = true;
            world.set_block_state(
                pos,
                props.to_state_id(block),
                BlockFlags::NOTIFY_ALL,
            );
            world.schedule_block_tick(block, *pos, 8, TickPriority::Normal);
        }
        world.play_sound(
            Sound::BlockSculkCatalystBloom,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
        );
    }
}

impl BlockBehaviour for SculkCatalystBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        let be = crate::block::entities::sculk_catalyst::SculkCatalystBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(be));
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.remove_block_entity(args.position);
        args.world.update_neighbors(args.position, None);
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = SculkCatalystLikeProperties::default(args.block);
        props.bloom = false;
        props.to_state_id(args.block)
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = SculkCatalystLikeProperties::from_state_id(state.id);
        if props.bloom {
            props.bloom = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}
