use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, OnLandedUponArgs, OnPlaceArgs};

type HayBlockProperties = pumpkin_data::block_properties::HayBlockProperties;

#[pumpkin_block("minecraft:hay_block")]
pub struct HayBlock;

impl BlockBehaviour for HayBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = HayBlockProperties::default(args.block);
        props.axis = args.direction.to_axis();
        props.to_state_id(args.block)
    }

    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        if let Some(living) = args.entity.get_living_entity() {
            living.handle_fall_damage(args.entity, args.fall_distance, 0.2);
        }
    }
}

