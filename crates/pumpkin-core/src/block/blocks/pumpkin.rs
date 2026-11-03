use crate::block::{BrokenArgs, PlacedArgs, UseWithItemArgs};
use crate::block::registry::BlockActionResult;
use crate::entity::Entity;
use crate::entity::item::ItemEntity;
use pumpkin_data::Block;
use pumpkin_data::block_properties::WallTorchLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

#[pumpkin_block("minecraft:pumpkin")]
pub struct PumpkinBlock;

impl PumpkinBlock {
    pub const PUMPKIN_SEEDS_DROP_AMOUNT: u8 = 4;
}

impl crate::block::BlockBehaviour for PumpkinBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if args.item_stack.item != &Item::SHEARS {
            return BlockActionResult::Pass;
        }
        let mut props = WallTorchLikeProperties::default(&Block::CARVED_PUMPKIN);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        args.world.set_block_state(
            args.position,
            props.to_state_id(&Block::CARVED_PUMPKIN),
            BlockFlags::NOTIFY_ALL,
        );
        let entity = Entity::new(
            args.world.clone(),
            args.position.to_f64(),
            &EntityType::ITEM,
        );
        let item_entity = Arc::new(ItemEntity::new(
            entity,
            ItemStack::new(4, &Item::PUMPKIN_SEEDS),
        ));
        args.world.spawn_entity(item_entity);
        args.player.damage_item_in_slot(args.equipment_slot, 1);
        BlockActionResult::Consume
    }
}
