use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, BrokenArgs, NormalUseArgs, PlacedArgs};

use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:fletching_table")]
pub struct FletchingTableBlock;

impl BlockBehaviour for FletchingTableBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn normal_use(&self, _args: NormalUseArgs<'_>) -> BlockActionResult {
        BlockActionResult::Pass
    }
}
