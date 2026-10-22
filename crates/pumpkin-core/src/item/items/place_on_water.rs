use std::any::Any;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::world::BlockFlags;
use pumpkin_data::Block;
use pumpkin_data::item::Item;
use pumpkin_data::sound::Sound;

pub struct PlaceOnWaterBlockItem;

impl ItemMetadata for PlaceOnWaterBlockItem {
    fn ids() -> Box<[u16]> {
        [Item::LILY_PAD.id, Item::FROGSPAWN.id].into()
    }
}

impl ItemBehaviour for PlaceOnWaterBlockItem {
    fn normal_use(&self, item: &Item, player: &Player) {
        let (yaw, pitch) = player.rotation();
        self.normal_use_with_hand(item, player, yaw, pitch, pumpkin_util::Hand::Right);
    }

    fn normal_use_with_rotation(&self, item: &Item, player: &Player, yaw: f32, pitch: f32) {
        self.normal_use_with_hand(item, player, yaw, pitch, pumpkin_util::Hand::Right);
    }

    fn normal_use_with_hand(
        &self,
        item: &Item,
        player: &Player,
        _yaw: f32,
        _pitch: f32,
        hand: pumpkin_util::Hand,
    ) {
        let world = player.world();
        let (start_pos, end_pos) = self.get_start_and_end_pos(player);
        let hit = world.clip(&pumpkin_world::clip::ClipContext::new(
            start_pos,
            end_pos,
            pumpkin_world::clip::ClipBlock::Outline,
            pumpkin_world::clip::ClipFluid::SourceOnly,
        ));
        if hit.is_miss {
            return;
        }

        let hit_pos = hit.block_pos;

        let above_pos = hit_pos.up();
        let above_state = world.get_block_state(&above_pos);
        if above_state.is_air() {
            let (placed_block, sound) = if item.id == Item::LILY_PAD.id {
                (&Block::LILY_PAD, Sound::BlockLilyPadPlace)
            } else {
                (&Block::FROGSPAWN, Sound::BlockFrogspawnPlace)
            };

            world.set_block_state(
                &above_pos,
                placed_block.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
            world.play_sound(
                sound,
                pumpkin_data::sound::SoundCategory::Blocks,
                &above_pos.to_f64(),
            );

            let mut stack = player.inventory.get_stack_in_hand(hand);
            if !stack.is_empty() && stack.item.id == item.id {
                stack.decrement_unless_creative(player.gamemode.load(), 1);
                player.inventory.set_stack_in_hand(hand, stack);
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
