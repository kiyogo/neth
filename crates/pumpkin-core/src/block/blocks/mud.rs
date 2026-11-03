use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, BrokenArgs, PathComputationType, PlacedArgs};

#[pumpkin_block("minecraft:mud")]
pub struct MudBlock;

impl BlockBehaviour for MudBlock {
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
