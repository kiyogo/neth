use crate::block::{
    BlockBehaviour, BlockMetadata, BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs,
};
use pumpkin_data::block_properties::PaleHangingMossLikeProperties;
use pumpkin_data::{Block, BlockDirection, BlockId, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

pub struct HangingMossBlock;

impl BlockMetadata for HangingMossBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::PALE_HANGING_MOSS].into()
    }
}

fn can_survive(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
    let above = pos.up();
    let (block_above, state_above) = world.get_block_and_state(&above);
    block_above == &Block::PALE_HANGING_MOSS || state_above.is_side_solid(BlockDirection::Down)
}

impl BlockBehaviour for HangingMossBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if can_survive(args.world, args.position) {
            let below = args.position.down();
            let block_below = args.world.get_block(&below);
            let mut props = PaleHangingMossLikeProperties::default(args.block);
            props.tip = block_below != &Block::PALE_HANGING_MOSS;
            props.to_state_id(args.block)
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
            let below = args.position.down();
            let block_below = args.world.get_block(&below);
            let mut props = PaleHangingMossLikeProperties::from_state_id(args.state_id);
            props.tip = block_below != &Block::PALE_HANGING_MOSS;
            props.to_state_id(args.block)
        }
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let mut cur = *args.position;
        while args.world.get_block(&cur) == &Block::PALE_HANGING_MOSS {
            cur = cur.down();
        }
        args.world.get_block_state(&cur).is_air()
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let mut cur = *args.position;
        while args.world.get_block(&cur) == &Block::PALE_HANGING_MOSS {
            cur = cur.down();
        }
        if args.world.get_block_state(&cur).is_air() {
            let mut props = PaleHangingMossLikeProperties::default(args.block);
            props.tip = true;
            args.world.set_block_state(
                &cur,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}
