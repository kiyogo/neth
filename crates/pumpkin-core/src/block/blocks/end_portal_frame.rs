use pumpkin_data::item::Item;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::registry::BlockActionResult;
use crate::{
    block::{
        BlockBehaviour, BrokenArgs, GetComparatorOutputArgs, OnPlaceArgs, PathComputationType,
        PlacedArgs, UseWithItemArgs,
    },
    entity::EntityBase,
};

type EndPortalFrameProperties = pumpkin_data::block_properties::EndPortalFrameLikeProperties;

/// Vanilla `EndPortalFrameBlock.java:44-46`: `state.getValue(HAS_EYE) ? 15 : 0`
#[must_use]
pub const fn calculate_end_portal_frame_comparator_signal(has_eye: bool) -> u8 {
    if has_eye { 15 } else { 0 }
}

#[pumpkin_block("minecraft:end_portal_frame")]
pub struct EndPortalFrameBlock;

impl EndPortalFrameBlock {
    pub const COMPARATOR_SIGNAL_EYE: u8 = 15;
    pub const COMPARATOR_SIGNAL_EMPTY: u8 = 0;
    pub const EVENT_FRAME_FILL: u32 = 1503;
    pub const EVENT_PORTAL_SPAWN: u32 = 1038;
    pub const PORTAL_FRAME_COUNT: usize = 12;
    pub const PORTAL_INNER_SIZE: usize = 3;

    #[must_use]
    pub const fn is_valid_frame_count(count: usize) -> bool {
        count == Self::PORTAL_FRAME_COUNT
    }

    #[must_use]
    pub fn is_facing_inward(
        frame_facing: pumpkin_data::block_properties::Facing,
        direction_from_center: pumpkin_data::BlockDirection,
    ) -> bool {
        frame_facing == direction_from_center.to_facing().opposite()
    }
}

impl BlockBehaviour for EndPortalFrameBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.update_neighbors(args.position, None);
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if args.item_stack.item.id != Item::ENDER_EYE.id {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = EndPortalFrameProperties::from_state_id(state_id);
        if props.eye {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        props.eye = true;
        args.world.set_block_state(
            args.position,
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.sync_world_event(WorldEvent::EndPortalFrameFill, *args.position, 0);
        args.item_stack
            .decrement_unless_creative(args.player.gamemode.load(), 1);
        BlockActionResult::Success
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut end_portal_frame_props = EndPortalFrameProperties::default(args.block);
        end_portal_frame_props.facing = args.player.get_entity().get_horizontal_facing().opposite();

        end_portal_frame_props.to_state_id(args.block)
    }

    /// A frame with an eye reads full strength.
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = EndPortalFrameProperties::from_state_id(args.state.id);
        Some(if props.eye { 15 } else { 0 })
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
