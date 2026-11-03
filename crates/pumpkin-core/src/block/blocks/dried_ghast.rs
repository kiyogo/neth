use crate::block::{
    BlockBehaviour, BlockMetadata, BrokenArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType, PlacedArgs,
    RandomTickArgs,
};
use crate::entity::EntityBase;
use pumpkin_data::block_properties::DriedGhastLikeProperties;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{BlockId, BlockState, BlockStateId};
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

pub struct DriedGhastBlock;

impl DriedGhastBlock {
    pub const MAX_HYDRATION: u8 = 3;
}

impl BlockMetadata for DriedGhastBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::DRIED_GHAST].into()
    }
}

impl BlockBehaviour for DriedGhastBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = DriedGhastLikeProperties::default(args.block);
        props.facing = args
            .player
            .get_entity()
            .get_horizontal_facing()
            .opposite();
        props.waterlogged = args.replacing.water_source();
        props.hydration = 0;
        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = DriedGhastLikeProperties::from_state_id(args.state_id);
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

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = DriedGhastLikeProperties::from_state_id(state.id);

        if props.waterlogged {
            if props.hydration < 3 {
                props.hydration += 1;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                );
                args.world.play_sound(
                    Sound::BlockDriedGhastTransition,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
            }
        } else if props.hydration > 0 {
            props.hydration -= 1;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
