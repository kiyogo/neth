use pumpkin_data::HorizontalFacingExt;
use pumpkin_data::block_properties::AcaciaShelfLikeProperties;
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_inventory::Inventory;
use pumpkin_macros::pumpkin_block_from_tag;

use crate::block::entities::shelf::ShelfBlockEntity;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetComparatorOutputArgs, OnPlaceArgs, PathComputationType, PlacedArgs,
};
use crate::entity::EntityBase;
use std::sync::Arc;

#[pumpkin_block_from_tag("minecraft:wooden_shelves")]
pub struct ShelfBlock;

impl ShelfBlock {
    pub const SLOT_COUNT: usize = 3;
    pub const MIN_COMPARATOR_SIGNAL: u8 = 0;
    pub const MAX_COMPARATOR_SIGNAL: u8 = 7;

    #[must_use]
    pub const fn calculate_comparator_output(slot0: bool, slot1: bool, slot2: bool) -> u8 {
        (if slot0 { 1 } else { 0 })
            | (if slot1 { 2 } else { 0 })
            | (if slot2 { 4 } else { 0 })
    }
}

impl BlockBehaviour for ShelfBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut properties = AcaciaShelfLikeProperties::default(args.block);

        properties.facing = args.player.get_entity().get_horizontal_facing().opposite();

        properties.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let entity = ShelfBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
            args.world.update_neighbour_for_output_signal(args.position, args.block);
            args.world.update_neighbors(args.position, None);
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(shelf) = block_entity.as_any().downcast_ref::<ShelfBlockEntity>()
        {
            for i in 0..Self::SLOT_COUNT {
                let stack = shelf.remove_stack(i);
                if !stack.is_empty() {
                    args.world.drop_stack(args.position, stack);
                }
            }
        }
        args.world.remove_block_entity(args.position);
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.update_neighbors(args.position, None);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        // Vanilla reads a shelf only through its back face.
        let props = AcaciaShelfLikeProperties::from_state_id(args.state.id);
        if args.direction != props.facing.to_block_direction().opposite() {
            return Some(0);
        }

        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(shelf) = block_entity.as_any().downcast_ref::<ShelfBlockEntity>()
        {
            let b0 = u8::from(!shelf.get_stack(0).is_empty());
            let b1 = u8::from(!shelf.get_stack(1).is_empty());
            let b2 = u8::from(!shelf.get_stack(2).is_empty());
            Some(b0 | (b1 << 1) | (b2 << 2))
        } else {
            None
        }
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        computation_type == PathComputationType::Water && state.is_waterlogged()
    }
}
