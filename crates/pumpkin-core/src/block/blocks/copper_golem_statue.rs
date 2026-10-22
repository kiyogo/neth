use pumpkin_data::block_properties::{
    CopperGolemPose, CopperGolemStatueLikeProperties, EnumVariants,
};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockId, BlockState, BlockStateId};
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

use crate::block::blocks::weathering_copper::change_over_time;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BlockMetadata, GetComparatorOutputArgs, GetStateForNeighborUpdateArgs,
    NormalUseArgs, OnPlaceArgs, PathComputationType, RandomTickArgs, UseWithItemArgs,
};
use crate::entity::EntityBase;

pub struct CopperGolemStatueBlock;

impl BlockMetadata for CopperGolemStatueBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::COPPER_GOLEM_STATUE,
            BlockId::EXPOSED_COPPER_GOLEM_STATUE,
            BlockId::WEATHERED_COPPER_GOLEM_STATUE,
            BlockId::OXIDIZED_COPPER_GOLEM_STATUE,
            BlockId::WAXED_COPPER_GOLEM_STATUE,
            BlockId::WAXED_EXPOSED_COPPER_GOLEM_STATUE,
            BlockId::WAXED_WEATHERED_COPPER_GOLEM_STATUE,
            BlockId::WAXED_OXIDIZED_COPPER_GOLEM_STATUE,
        ]
        .into()
    }
}

const fn is_waxed(id: BlockId) -> bool {
    matches!(
        id,
        BlockId::WAXED_COPPER_GOLEM_STATUE
            | BlockId::WAXED_EXPOSED_COPPER_GOLEM_STATUE
            | BlockId::WAXED_WEATHERED_COPPER_GOLEM_STATUE
            | BlockId::WAXED_OXIDIZED_COPPER_GOLEM_STATUE
    )
}

impl BlockBehaviour for CopperGolemStatueBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = CopperGolemStatueLikeProperties::default(args.block);
        props.facing = args
            .player
            .get_entity()
            .get_horizontal_facing()
            .opposite();
        props.waterlogged = args.replacing.water_source();
        props.copper_golem_pose = CopperGolemPose::Standing;
        props.to_state_id(args.block)
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        // If holding an axe, pass so scraping / deoxidizing can happen.
        if args.item_stack.item.has_tag(&pumpkin_data::tag::Item::MINECRAFT_AXES) {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        self.normal_use(NormalUseArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
            hit: args.hit,
        })
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let current_state = args.world.get_block_state(args.position);
        let mut props = CopperGolemStatueLikeProperties::from_state_id(current_state.id);
        let next_index = (props.copper_golem_pose.to_index() + 1) % 4;
        props.copper_golem_pose = CopperGolemPose::from_index(next_index);

        let new_state_id = props.to_state_id(args.block);
        args.world
            .set_block_state(args.position, new_state_id, BlockFlags::NOTIFY_ALL);

        args.world.play_sound(
            Sound::EntityCopperGolemBecomeStatue,
            SoundCategory::Blocks,
            &args.position.to_f64(),
        );

        BlockActionResult::Consume
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = CopperGolemStatueLikeProperties::from_state_id(args.state.id);
        Some(props.copper_golem_pose.to_index() as u8 + 1)
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        if !is_waxed(args.block.id) {
            change_over_time(args.world, args.position, args.block);
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = CopperGolemStatueLikeProperties::from_state_id(args.state_id);
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
