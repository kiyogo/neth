use pumpkin_data::block_properties::PistonType;
use pumpkin_data::{Block, BlockState, BlockStateId, FacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

use crate::block::{
    BlockBehaviour, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs,
    OnNeighborUpdateArgs, PathComputationType,
};

use super::piston::PistonProps;

pub type PistonHeadProperties = pumpkin_data::block_properties::PistonHeadLikeProperties;
pub(crate) type MovingPistonProps = pumpkin_data::block_properties::MovingPistonLikeProperties;

#[pumpkin_block("minecraft:piston_head")]
pub struct PistonHeadBlock;

impl PistonHeadBlock {
    fn is_fitting_base(
        head: PistonHeadProperties,
        base_block: &Block,
        base_state_id: BlockStateId,
    ) -> bool {
        let fitting_block = if head.r#type == PistonType::Normal {
            &Block::PISTON
        } else {
            &Block::STICKY_PISTON
        };
        if base_block != fitting_block {
            return false;
        }
        let base = PistonProps::from_state_id(base_state_id);
        base.extended && base.facing == head.facing
    }

    #[must_use]
    pub fn can_survive(world: &dyn BlockAccessor, pos: &BlockPos, state_id: BlockStateId) -> bool {
        let head = PistonHeadProperties::from_state_id(state_id);
        let base_pos = pos.offset(head.facing.opposite().to_block_direction().to_offset());
        let (base_block, base_state) = world.get_block_and_state(&base_pos);
        Self::is_fitting_base(head, base_block, base_state.id)
            || (base_block == &Block::MOVING_PISTON
                && MovingPistonProps::from_state_id(base_state.id).facing == head.facing)
    }
}

impl BlockBehaviour for PistonHeadBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        Self::can_survive(args.block_accessor, args.position, args.state.id)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let facing = PistonHeadProperties::from_state_id(args.state_id).facing;
        if args.direction.opposite() == facing.to_block_direction()
            && !Self::can_survive(args.world, args.position, args.state_id)
        {
            return Block::AIR.default_state.id;
        }
        args.state_id
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() == pumpkin_util::GameMode::Creative {
            let props = PistonHeadProperties::from_state_id(args.state.id);
            let pos = args
                .position
                .offset(props.facing.opposite().to_block_direction().to_offset());
            let (new_block, new_state) = args.world.get_block_and_state_id(&pos);
            if &Block::PISTON == new_block || &Block::STICKY_PISTON == new_block {
                let base_props = PistonProps::from_state_id(new_state);
                if base_props.extended && base_props.facing == props.facing {
                    args.world.set_block_state(
                        &pos,
                        Block::AIR.default_state.id,
                        BlockFlags::NOTIFY_ALL | BlockFlags::FORCE_STATE,
                    );
                }
            }
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let head_state = args.world.get_block_state(args.position);
        let head_props = PistonHeadProperties::from_state_id(head_state.id);
        let facing_dir = head_props.facing.to_block_direction();
        let base_pos = args
            .position
            .offset(facing_dir.opposite().to_offset());

        if !Self::can_survive(args.world.as_ref(), args.position, head_state.id) {
            args.world.set_block_state(
                args.position,
                Block::AIR.default_state.id,
                BlockFlags::FORCE_STATE | BlockFlags::NOTIFY_ALL,
            );
            return;
        }

        // Do not forward update back to the base if the update originated from the base itself!
        if args.source_block == &Block::PISTON
            || args.source_block == &Block::STICKY_PISTON
            || args.source_block == &Block::MOVING_PISTON
        {
            return;
        }

        // Forward neighbor update ONLY to piston base (matching jvmc neighborChanged)
        args.world.update_neighbor(&base_pos, &Block::PISTON_HEAD);
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
