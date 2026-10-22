use pumpkin_data::block_properties::BrownMushroomBlockLikeProperties;
use pumpkin_data::{BlockDirection, BlockId, BlockStateId};

use crate::block::{
    BlockBehaviour, BlockMetadata, GetStateForNeighborUpdateArgs, OnPlaceArgs,
};

type MushroomProperties = BrownMushroomBlockLikeProperties;

pub struct HugeMushroomBlock;

impl BlockMetadata for HugeMushroomBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::BROWN_MUSHROOM_BLOCK,
            BlockId::RED_MUSHROOM_BLOCK,
            BlockId::MUSHROOM_STEM,
        ]
        .into()
    }
}

impl BlockBehaviour for HugeMushroomBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = MushroomProperties::default(args.block);
        props.down = args.world.get_block(&args.position.down()) != args.block;
        props.up = args.world.get_block(&args.position.up()) != args.block;
        props.north = args.world.get_block(&args.position.north()) != args.block;
        props.south = args.world.get_block(&args.position.south()) != args.block;
        props.west = args.world.get_block(&args.position.west()) != args.block;
        props.east = args.world.get_block(&args.position.east()) != args.block;
        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.world.get_block(args.neighbor_position) == args.block {
            let mut props = MushroomProperties::from_state_id(args.state_id);
            match args.direction {
                BlockDirection::Down => props.down = false,
                BlockDirection::Up => props.up = false,
                BlockDirection::North => props.north = false,
                BlockDirection::South => props.south = false,
                BlockDirection::West => props.west = false,
                BlockDirection::East => props.east = false,
            }
            props.to_state_id(args.block)
        } else {
            args.state_id
        }
    }
}
