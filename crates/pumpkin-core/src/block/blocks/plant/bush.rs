use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;

use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, PlacedArgs, blocks::plant::PlantBlockBase};

pub struct BushBlock;

impl BlockMetadata for BushBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BUSH, BlockId::RED_SHRUB].into()
    }
}

impl BlockBehaviour for BushBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }
}

impl PlantBlockBase for BushBlock {}
