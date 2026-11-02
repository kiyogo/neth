use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::entity::Entity;
use crate::entity::combat::{
    calculate_end_crystal_placement_box, calculate_end_crystal_spawn_position,
    can_place_end_crystal_on,
};
use crate::entity::decoration::end_crystal::EndCrystalEntity;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::{Block, BlockDirection};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

pub struct EndCrystalItem;

impl ItemMetadata for EndCrystalItem {
    fn ids() -> Box<[u16]> {
        [Item::END_CRYSTAL.id].into()
    }
}

impl ItemBehaviour for EndCrystalItem {
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        _block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        let world = player.world();
        let block = world.get_block(&location);
        if !can_place_end_crystal_on(block.id) {
            return BlockActionResult::Fail;
        }

        let above = location.up();
        if !world.get_block_state(&above).is_air()
            || world
                .get_all_at_box(&calculate_end_crystal_placement_box(location))
                .iter()
                .any(|entity| !entity.is_spectator())
        {
            return BlockActionResult::Fail;
        }

        let spawn_pos = calculate_end_crystal_spawn_position(location);
        let entity = Entity::new(world.clone(), spawn_pos, &EntityType::END_CRYSTAL);
        let end_crystal = Arc::new(EndCrystalEntity::new(entity));
        world.spawn_entity(end_crystal.clone());
        end_crystal.set_show_bottom(false);
        world.emit_game_event(GameEvent::EntityPlace.name(), spawn_pos);
        item.decrement_unless_creative(player.gamemode.load(), 1);

        if let Some(ref fight_mutex) = world.dragon_fight
            && let Ok(mut fight) = fight_mutex.lock()
        {
            fight.try_respawn(&world);
        }

        BlockActionResult::Success
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
