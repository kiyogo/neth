use std::sync::Arc;

use pumpkin_data::block_properties::DecoratedPotLikeProperties;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;

use crate::block::entities::decorated_pot::DecoratedPotBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetComparatorOutputArgs, NormalUseArgs, OnPlaceArgs,
    PathComputationType, PlacedArgs, UseWithItemArgs,
};

#[pumpkin_block("minecraft:decorated_pot")]
pub struct DecoratedPotBlock;

impl DecoratedPotBlock {
    pub const TOTAL_SHERD_FACES: usize = 4;
    pub const INSERT_DUST_PLUME_PARTICLES: i32 = 7;

    /// Vanilla AbstractContainerMenu.getRedstoneSignalFromBlockEntity
    /// Returns 0 if empty; otherwise 1 + floor((count / maxStack) * 14.0)
    #[must_use]
    pub fn calculate_comparator_output(item_count: u8, max_stack_size: u8) -> u8 {
        if item_count == 0 {
            0
        } else {
            let max_count = f32::from(max_stack_size.max(1));
            let filled = (f32::from(item_count) / max_count).min(1.0);
            1 + (filled * 14.0).floor() as u8
        }
    }

    /// Vanilla DecoratedPotBlock.java:117: 0.7F + 0.5F * (count / maxStackSize)
    #[must_use]
    pub fn calculate_insert_pitch(count: u8, max_stack_size: u8) -> f32 {
        let max_count = f32::from(max_stack_size.max(1));
        let pitch_bend = f32::from(count) / max_count;
        0.7 + 0.5 * pitch_bend
    }

    /// Vanilla DecoratedPotBlock.java:191:
    /// Pots shatter into 4 sherds/bricks when broken with a tool without silk touch
    #[must_use]
    pub const fn can_break_pot_shatter(is_tool: bool, has_silk_touch: bool) -> bool {
        is_tool && !has_silk_touch
    }
}

impl BlockBehaviour for DecoratedPotBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = DecoratedPotLikeProperties::from_state_id(args.block.default_state.id);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let entity = DecoratedPotBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(entity));
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if args.item_stack.item_count == 0 {
            return self.normal_use(NormalUseArgs {
                server: args.server,
                world: args.world,
                block: args.block,
                position: args.position,
                player: args.player,
                hit: args.hit,
            });
        }

        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(pot_entity) = block_entity
                .as_any()
                .downcast_ref::<DecoratedPotBlockEntity>()
        {
            if pot_entity.try_insert_item(args.item_stack, 1) {
                let (count, max_stack) = pot_entity
                    .get_item()
                    .map_or((1, 64), |it| (it.item_count, it.get_max_stack_size()));
                let pitch_bend = f32::from(count) / f32::from(max_stack.max(1));
                let pitch = 0.7 + 0.5 * pitch_bend;
                args.world.play_sound_fine(
                    Sound::BlockDecoratedPotInsert,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                    1.0,
                    pitch,
                );
                // Vanilla `BlockEntity.setChanged` notifies comparators.
                args.world
                    .update_neighbour_for_output_signal(args.position, args.block);
            } else {
                args.world.play_sound(
                    Sound::BlockDecoratedPotInsertFail,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
            }
            return BlockActionResult::Success;
        }

        BlockActionResult::Pass
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        args.world.play_sound(
            Sound::BlockDecoratedPotInsertFail,
            SoundCategory::Blocks,
            &args.position.to_f64(),
        );
        BlockActionResult::Success
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.play_sound(
            Sound::BlockDecoratedPotShatter,
            SoundCategory::Blocks,
            &args.position.to_f64(),
        );
        args.world
            .drop_stack(args.position, ItemStack::new(4, &Item::BRICK));
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(pot_entity) = block_entity
                .as_any()
                .downcast_ref::<DecoratedPotBlockEntity>()
        {
            Some(pot_entity.get_comparator_output())
        } else {
            Some(0)
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
