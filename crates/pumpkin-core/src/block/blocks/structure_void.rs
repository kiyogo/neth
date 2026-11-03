use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, BrokenArgs, PlacedArgs};

#[pumpkin_block("minecraft:structure_void")]
pub struct StructureVoidBlock;

impl StructureVoidBlock {
    pub const SHAPE_SIZE: f64 = 6.0 / 16.0;
}

impl BlockBehaviour for StructureVoidBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
}
