use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;

use crate::{
    block::{
        BlockBehaviour, BlockMetadata, CanPlaceAtArgs, EmitsRedstonePowerArgs,
        GetRedstonePowerArgs, GetStateForNeighborUpdateArgs, OnEntityCollisionArgs,
        OnScheduledTickArgs, OnStateReplacedArgs,
    },
    world::World,
};

use super::{PressurePlate, detection_box_at};

/// This is for Gold and Iron Pressure Plate
pub struct WeightedPressurePlateBlock;

impl WeightedPressurePlateBlock {
    pub const LIGHT_MAX_WEIGHT: usize = 15;
    pub const HEAVY_MAX_WEIGHT: usize = 150;
    pub const MAX_POWER: u8 = 15;
    pub const PRESSED_TICKS: u32 = 10;

    #[must_use]
    pub fn signal_strength(entity_count: usize, max_weight: usize) -> u8 {
        let count = entity_count.min(max_weight);
        if count == 0 {
            return 0;
        }
        (count as f32 / max_weight as f32 * 15.0).ceil() as u8
    }

    #[must_use]
    pub fn get_max_weight(block: &Block) -> usize {
        if block == &Block::LIGHT_WEIGHTED_PRESSURE_PLATE {
            Self::LIGHT_MAX_WEIGHT
        } else {
            Self::HEAVY_MAX_WEIGHT
        }
    }
}

pub(super) fn signal_strength(entity_count: usize, max_weight: usize) -> u8 {
    WeightedPressurePlateBlock::signal_strength(entity_count, max_weight)
}

type PressurePlateProps = pumpkin_data::block_properties::LightWeightedPressurePlateLikeProperties;

impl BlockMetadata for WeightedPressurePlateBlock {
    fn ids() -> Box<[BlockId]> {
        // light = Gold
        // heavy = Iron
        [
            BlockId::LIGHT_WEIGHTED_PRESSURE_PLATE,
            BlockId::HEAVY_WEIGHTED_PRESSURE_PLATE,
        ]
        .into()
    }
}

impl BlockBehaviour for WeightedPressurePlateBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        self.on_entity_collision_pp(args);
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let output = self.get_redstone_output(args.block, state.id);
        if output > 0 {
            let (block, state) = args.world.get_block_and_state(args.position);
            Self.update_plate_state(args.world, args.position, block, state, output);
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        self.on_state_replaced_pp(args);
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        self.get_redstone_output(args.block, args.state.id)
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.direction == BlockDirection::Up {
            return self.get_redstone_output(args.block, args.state.id);
        }
        0
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        self.get_state_for_neighbor_update_pp(args)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.world
            .is_some_and(|world| Self::can_pressure_plate_place_at(world, args.position))
    }
}

impl PressurePlate for WeightedPressurePlateBlock {
    fn get_redstone_output(&self, _block: &Block, state: BlockStateId) -> u8 {
        let props = PressurePlateProps::from_state_id(state);
        props.power
    }

    fn calculate_redstone_output(&self, world: &World, block: &Block, pos: &BlockPos) -> u8 {
        // light = Gold
        // heavy = Iron
        let weight = Self::get_max_weight(block);
        let aabb = detection_box_at(pos);
        let len = world.get_entities_at_box(&aabb).len() + world.get_players_at_box(&aabb).len();
        signal_strength(len, weight)
    }

    fn set_redstone_output(&self, block: &Block, state: &BlockState, output: u8) -> BlockStateId {
        let mut props = PressurePlateProps::from_state_id(state.id);
        props.power = output;
        props.to_state_id(block)
    }

    fn tick_rate(&self) -> u32 {
        10
    }
}
