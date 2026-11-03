use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, BrokenArgs, PathComputationType, PlacedArgs};

#[pumpkin_block("minecraft:soul_sand")]
pub struct SoulSandBlock;

impl SoulSandBlock {
    pub const SLOW_SPEED_MULTIPLIER: f32 = 0.4;
}

impl BlockBehaviour for SoulSandBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn is_pathfindable(
        &self,
        _state: &pumpkin_data::BlockState,
        _computation_type: PathComputationType,
    ) -> bool {
        false
    }
}
