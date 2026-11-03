use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BrokenArgs, NormalUseArgs, OnPlaceArgs, PlacedArgs};

use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::StructureBlockLikeProperties;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::PermissionLvl;

#[pumpkin_block("minecraft:structure_block")]
pub struct StructureBlock;

impl BlockBehaviour for StructureBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let props = StructureBlockLikeProperties::default(args.block);
        props.to_state_id(args.block)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        {
            if args.player.permission_lvl.load() < PermissionLvl::Two {
                return BlockActionResult::Pass;
            }
            let Some(block_entity) = args.world.get_block_entity(args.position) else {
                return BlockActionResult::Pass;
            };
            args.world.update_block_entity(&block_entity);

            BlockActionResult::Success
        }
    }
}
