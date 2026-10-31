use std::sync::{Arc, atomic::Ordering};

use pumpkin_macros::pumpkin_block;

use crate::block::entities::chiseled_bookshelf::ChiseledBookshelfBlockEntity;
use crate::{
    block::{
        BlockBehaviour, BlockHitResult, GetComparatorOutputArgs, NormalUseArgs, OnPlaceArgs,
        PlacedArgs, UseWithItemArgs, registry::BlockActionResult,
    },
    entity::{EntityBase, player::Player},
    world::World,
};
use pumpkin_data::{
    BlockStateId,
    block_properties::{ChiseledBookshelfLikeProperties, HorizontalFacing},
    game_event::GameEvent,
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
    tag,
    tag::Taggable,
};
use pumpkin_data::statistic::StatisticCategory;
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_util::math::{position::BlockPos, vector2::Vector2};

#[pumpkin_block("minecraft:chiseled_bookshelf")]
pub struct ChiseledBookshelfBlock;

impl BlockBehaviour for ChiseledBookshelfBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut properties = ChiseledBookshelfLikeProperties::default(args.block);

        // Face in the opposite direction the player is facing
        properties.facing = args.player.get_entity().get_horizontal_facing().opposite();

        properties.to_state_id(args.block)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        let properties = ChiseledBookshelfLikeProperties::from_state_id(state.id);

        if let Some(slot) = Self::get_slot_for_hit(args.hit, properties.facing) {
            if Self::is_slot_used(properties, slot) {
                if let Some(block_entity) = args.world.get_block_entity(args.position)
                    && let Some(block_entity) = block_entity
                        .as_any()
                        .downcast_ref::<ChiseledBookshelfBlockEntity>()
                {
                    Self::try_remove_book(
                        args.world,
                        args.player,
                        args.position,
                        block_entity,
                        properties,
                        slot,
                    );
                    return BlockActionResult::Success;
                }
            } else {
                return BlockActionResult::Consume;
            }
        }
        BlockActionResult::Pass
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        let properties = ChiseledBookshelfLikeProperties::from_state_id(state.id);

        if !args
            .item_stack
            .get_item()
            .has_tag(&tag::Item::MINECRAFT_BOOKSHELF_BOOKS)
        {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        if let Some(slot) = Self::get_slot_for_hit(args.hit, properties.facing) {
            if Self::is_slot_used(properties, slot) {
                return BlockActionResult::PassToDefaultBlockAction;
            } else if let Some(block_entity) = args.world.get_block_entity(args.position)
                && let Some(block_entity) = block_entity
                    .as_any()
                    .downcast_ref::<ChiseledBookshelfBlockEntity>()
            {
                Self::try_add_book(
                    args.world,
                    args.player,
                    args.position,
                    block_entity,
                    properties,
                    slot,
                    args.item_stack,
                );
                return BlockActionResult::Success;
            }
        }

        BlockActionResult::Pass
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let block_entity = ChiseledBookshelfBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(block_entity));
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(block_entity) = block_entity
                .as_any()
                .downcast_ref::<ChiseledBookshelfBlockEntity>()
        {
            let slot = block_entity.last_interacted_slot.load(Ordering::Relaxed);
            return Some(Self::calculate_comparator_output(slot));
        }
        None
    }
}

impl ChiseledBookshelfBlock {
    pub const MAX_BOOKS_IN_STORAGE: usize = 6;
    pub const BOOKS_PER_ROW: usize = 3;
    pub const ROWS: usize = 2;
    pub const COLUMNS: usize = 3;
    pub const MIN_COMPARATOR_OUTPUT: u8 = 0;
    pub const MAX_COMPARATOR_OUTPUT: u8 = 6;
    pub const DEFAULT_FACING: HorizontalFacing = HorizontalFacing::North;
    pub const DEFAULT_SLOT_OCCUPIED: bool = false;

    #[must_use]
    pub const fn get_relative_hit_coords(
        cursor_x: f32,
        cursor_y: f32,
        cursor_z: f32,
        hit_face: HorizontalFacing,
        block_facing: HorizontalFacing,
    ) -> Option<(f32, f32)> {
        if hit_face as u8 != block_facing as u8 {
            return None;
        }
        match hit_face {
            HorizontalFacing::North => Some((1.0 - cursor_x, cursor_y)),
            HorizontalFacing::South => Some((cursor_x, cursor_y)),
            HorizontalFacing::West => Some((cursor_z, cursor_y)),
            HorizontalFacing::East => Some((1.0 - cursor_z, cursor_y)),
        }
    }

    pub fn get_section(relative_coordinate: f32, max_sections: usize) -> usize {
        let targeted_pixel = relative_coordinate * 16.0;
        let section_size = 16.0 / max_sections as f32;
        let section = (targeted_pixel / section_size).floor() as isize;
        section.clamp(0, (max_sections - 1) as isize) as usize
    }

    pub fn calculate_hit_slot(hit_x: f32, hit_y: f32) -> usize {
        let row = Self::get_section(1.0 - hit_y, Self::ROWS);
        let column = Self::get_section(hit_x, Self::COLUMNS);
        column + row * Self::COLUMNS
    }

    pub fn calculate_comparator_output(last_interacted_slot: i8) -> u8 {
        if (0..Self::MAX_BOOKS_IN_STORAGE as i8).contains(&last_interacted_slot) {
            (last_interacted_slot + 1) as u8
        } else {
            0
        }
    }

    pub fn is_slot_occupied(properties: &ChiseledBookshelfLikeProperties, slot: usize) -> bool {
        match slot {
            0 => properties.slot_0_occupied,
            1 => properties.slot_1_occupied,
            2 => properties.slot_2_occupied,
            3 => properties.slot_3_occupied,
            4 => properties.slot_4_occupied,
            5 => properties.slot_5_occupied,
            _ => false,
        }
    }

    pub fn slot_bitmask_from_properties(properties: &ChiseledBookshelfLikeProperties) -> u8 {
        let mut mask = 0u8;
        if properties.slot_0_occupied {
            mask |= 1 << 0;
        }
        if properties.slot_1_occupied {
            mask |= 1 << 1;
        }
        if properties.slot_2_occupied {
            mask |= 1 << 2;
        }
        if properties.slot_3_occupied {
            mask |= 1 << 3;
        }
        if properties.slot_4_occupied {
            mask |= 1 << 4;
        }
        if properties.slot_5_occupied {
            mask |= 1 << 5;
        }
        mask
    }

    pub fn get_occupied_slot_count(properties: &ChiseledBookshelfLikeProperties) -> usize {
        Self::slot_bitmask_from_properties(properties).count_ones() as usize
    }
    fn try_add_book(
        world: &Arc<World>,
        player: &Player,
        position: &BlockPos,
        entity: &ChiseledBookshelfBlockEntity,
        properties: ChiseledBookshelfLikeProperties,
        slot: i8,
        item: &mut ItemStack,
    ) {
        player.increment_stat(StatisticCategory::Used, item.item.id as i32, 1);

        let sound = if item.get_item() == &Item::ENCHANTED_BOOK {
            Sound::BlockChiseledBookshelfInsertEnchanted
        } else {
            Sound::BlockChiseledBookshelfInsert
        };

        entity.set_book(
            slot as usize,
            item.split_unless_creative(player.gamemode.load(), 1),
        );
        entity.update_state(properties, world, slot as usize);

        world.play_sound(sound, SoundCategory::Blocks, &position.to_centered_f64());
        world.emit_game_event(GameEvent::BlockChange.name(), position.to_centered_f64());
    }

    fn try_remove_book(
        world: &Arc<World>,
        player: &Arc<Player>,
        position: &BlockPos,
        entity: &ChiseledBookshelfBlockEntity,
        properties: ChiseledBookshelfLikeProperties,
        slot: i8,
    ) {
        let mut stack = entity.remove_book(slot as usize, 1);

        let sound = if stack.get_item() == &Item::ENCHANTED_BOOK {
            Sound::BlockChiseledBookshelfPickupEnchanted
        } else {
            Sound::BlockChiseledBookshelfPickup
        };

        if !player.get_inventory().insert_stack_anywhere(&mut stack) {
            // Drop the item on the ground if the player cannot hold it because of a full inventory
            player.drop_item(stack);
        }
        entity.update_state(properties, world, slot as usize);

        world.play_sound(sound, SoundCategory::Blocks, &position.to_centered_f64());
        world.emit_game_event(GameEvent::BlockChange.name(), position.to_centered_f64());
    }

    fn get_slot_for_hit(hit: &BlockHitResult<'_>, facing: HorizontalFacing) -> Option<i8> {
        Self::get_hit_pos(hit, facing).map(|pos| Self::calculate_hit_slot(pos.x, pos.y) as i8)
    }

    fn get_hit_pos(hit: &BlockHitResult<'_>, facing: HorizontalFacing) -> Option<Vector2<f32>> {
        // If the direction is not horizontal, we cannot hit a slot
        let direction = hit.face.to_horizontal_facing()?;

        // If the facing direction does not match the block's facing, we cannot hit a slot
        if facing != direction {
            return None;
        }

        match direction {
            HorizontalFacing::North => Some(Vector2::new(1.0 - hit.cursor_pos.x, hit.cursor_pos.y)),
            HorizontalFacing::South => Some(Vector2::new(hit.cursor_pos.x, hit.cursor_pos.y)),
            HorizontalFacing::West => Some(Vector2::new(hit.cursor_pos.z, hit.cursor_pos.y)),
            HorizontalFacing::East => Some(Vector2::new(1.0 - hit.cursor_pos.z, hit.cursor_pos.y)),
        }
    }

    const fn is_slot_used(properties: ChiseledBookshelfLikeProperties, slot: i8) -> bool {
        match slot {
            0 => properties.slot_0_occupied,
            1 => properties.slot_1_occupied,
            2 => properties.slot_2_occupied,
            3 => properties.slot_3_occupied,
            4 => properties.slot_4_occupied,
            5 => properties.slot_5_occupied,
            _ => false,
        }
    }
}
