use crate::block::{
    BlockBehaviour, BlockMetadata, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType,
};
use pumpkin_data::block_properties::MangroveRootsLikeProperties;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::{BlockId, BlockState, BlockStateId};
use pumpkin_world::tick::TickPriority;

pub struct HeavyCoreBlock;

impl BlockMetadata for HeavyCoreBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::HEAVY_CORE].into()
    }
}

impl BlockBehaviour for HeavyCoreBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = MangroveRootsLikeProperties::default(args.block);
        props.waterlogged = args.replacing.water_source();
        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = MangroveRootsLikeProperties::from_state_id(args.state_id);
        if props.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed,
                TickPriority::Normal,
            );
        }
        args.state_id
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
