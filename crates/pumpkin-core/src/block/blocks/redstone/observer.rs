use std::sync::Arc;

use crate::block::{
    EmitsRedstonePowerArgs, GetRedstonePowerArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnScheduledTickArgs, OnStateReplacedArgs, PlacedArgs,
};
use crate::entity::EntityBase;
use pumpkin_data::{Block, BlockDirection, BlockStateId, FacingExt, block_properties::Facing, block_properties::ObserverLikeProperties};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};

use crate::{block::BlockBehaviour, world::World};

#[pumpkin_block("minecraft:observer")]
pub struct ObserverBlock;

impl BlockBehaviour for ObserverBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = ObserverLikeProperties::default(args.block);
        props.facing = Self::calculate_facing_from_player(
            &args.player.get_entity().get_entity_facing_order(),
        );
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        // A powered observer arriving without its unpower tick (clone, structure) would stay
        // powered forever, so vanilla resets it here.
        let mut props = ObserverLikeProperties::from_state_id(args.state_id);
        if props.powered
            && !args
                .world
                .is_block_tick_scheduled(args.position, &Block::OBSERVER)
        {
            props.powered = false;
            // Vanilla flag 18 (clients, no shape updates); MOVED is what skips shape updates here.
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS | BlockFlags::MOVED,
            );
            Self::update_neighbors(args.world, args.block, args.position, props);
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = ObserverLikeProperties::from_state_id(state.id);

        if props.powered {
            props.powered = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        } else {
            props.powered = true;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
            args.world
                .schedule_block_tick(args.block, *args.position, Self::PULSE_DURATION, TickPriority::Normal);
        }
        Self::update_neighbors(args.world, args.block, args.position, props);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = ObserverLikeProperties::from_state_id(args.state_id);

        if props.facing.to_block_direction() == args.direction
            && !props.powered
            && !args
                .world
                .is_block_tick_scheduled(args.position, &Block::OBSERVER)
        {
            Self::schedule_tick(args.world, args.position);
        }

        args.state_id
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = ObserverLikeProperties::from_state_id(args.state.id);
        if props.facing.to_block_direction() == args.direction && props.powered {
            Self::POWER_ON
        } else {
            Self::POWER_OFF
        }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        self.get_weak_redstone_power(args)
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        let props = ObserverLikeProperties::from_state_id(args.old_state_id);
        if props.powered
            && args
                .world
                .is_block_tick_scheduled(args.position, &Block::OBSERVER)
        {
            Self::update_neighbors(args.world, args.block, args.position, props);
        }
    }
}

impl ObserverBlock {
    pub const PULSE_DURATION: u32 = 2;
    pub const DETECTION_DELAY: u32 = 2;
    pub const POWER_ON: u8 = 15;
    pub const POWER_OFF: u8 = 0;
    pub const DEFAULT_POWERED: bool = false;
    pub const DEFAULT_FACING: Facing = Facing::South;

    #[must_use]
    pub const fn get_power_signal(powered: bool) -> u8 {
        if powered {
            Self::POWER_ON
        } else {
            Self::POWER_OFF
        }
    }

    #[must_use]
    pub fn emits_power_in_direction(
        observer_facing: BlockDirection,
        query_direction: BlockDirection,
    ) -> bool {
        observer_facing == query_direction
    }

    #[must_use]
    pub const fn get_observed_direction(facing: BlockDirection) -> BlockDirection {
        facing
    }

    #[must_use]
    pub fn get_output_direction(facing: BlockDirection) -> BlockDirection {
        facing.opposite()
    }

    #[must_use]
    pub const fn calculate_facing_from_player(facing_order: &[Facing; 6]) -> Facing {
        facing_order[0]
    }

    #[must_use]
    pub fn is_observed_direction(
        observer_facing: BlockDirection,
        neighbor_dir: BlockDirection,
    ) -> bool {
        observer_facing == neighbor_dir
    }

    fn update_neighbors(
        world: &Arc<World>,
        block: &Block,
        block_pos: &BlockPos,
        props: ObserverLikeProperties,
    ) {
        let facing = props.facing.to_block_direction();
        let opposite_facing_pos = block_pos.offset(facing.opposite().to_offset());

        world.update_neighbor(&opposite_facing_pos, block);
        world.update_neighbors_at(&opposite_facing_pos, block, Some(facing));
    }

    fn schedule_tick(world: &World, block_pos: &BlockPos) {
        world.schedule_block_tick(&Block::OBSERVER, *block_pos, Self::DETECTION_DELAY, TickPriority::Normal);
    }
}
