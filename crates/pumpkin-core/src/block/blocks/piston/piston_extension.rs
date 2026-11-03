use pumpkin_data::block_properties::MovingPistonLikeProperties;
use pumpkin_data::{Block, BlockState, FacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::{BlockBehaviour, BrokenArgs, PathComputationType, PlacedArgs};

#[pumpkin_block("minecraft:moving_piston")]
pub struct PistonExtensionBlock;

impl BlockBehaviour for PistonExtensionBlock {
    fn broken(&self, args: BrokenArgs<'_>) {
        let props = MovingPistonLikeProperties::from_state_id(args.state.id);
        let base_pos = args
            .position
            .offset(props.facing.opposite().to_block_direction().to_offset());
        let (base_block, base_state) = args.world.get_block_and_state(&base_pos);

        if base_block == &Block::PISTON || base_block == &Block::STICKY_PISTON {
            let base_props = pumpkin_data::block_properties::StickyPistonLikeProperties::from_state_id(base_state.id);
            if base_props.extended {
                args.world.break_block(
                    &base_pos,
                    None,
                    BlockFlags::SKIP_DROPS | BlockFlags::NOTIFY_ALL,
                );
            }
        }
        args.world.update_neighbors(args.position, None);
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
