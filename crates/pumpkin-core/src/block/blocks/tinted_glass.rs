use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, BrokenArgs, PlacedArgs};

#[pumpkin_block("minecraft:tinted_glass")]
pub struct TintedGlassBlock;

impl BlockBehaviour for TintedGlassBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
}
