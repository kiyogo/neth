use std::sync::Arc;

use pumpkin_data::block_properties::SuspiciousSandLikeProperties;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockId, BlockStateId};
use pumpkin_world::world::BlockFlags;

use crate::block::entities::brushable_block::BrushableBlockBlockEntity;
use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, OnPlaceArgs, PlacedArgs};

pub struct BrushableBlock;

impl BlockMetadata for BrushableBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SUSPICIOUS_SAND, BlockId::SUSPICIOUS_GRAVEL].into()
    }
}

impl BrushableBlock {
    /// Vanilla: BrushableBlockEntity.REQUIRED_BRUSHES_TO_BREAK = 10;
    pub const REQUIRED_BRUSHES_TO_BREAK: i32 = 10;

    /// Vanilla: BrushableBlockEntity.BRUSH_COOLDOWN_TICKS = 10; (0.5s)
    pub const BRUSH_COOLDOWN_TICKS: i64 = 10;

    /// Vanilla: BrushableBlockEntity.BRUSH_RESET_TICKS = 40; (2.0s)
    pub const BRUSH_RESET_TICKS: i64 = 40;

    /// Vanilla: BrushableBlockEntity.checkReset: retractionSpeed = 4 ticks
    pub const RETRACTION_SPEED_TICKS: i64 = 4;

    /// Vanilla: 	his.brushCount = Math.max(0, this.brushCount - 2);
    pub const RETRACTION_BRUSH_STEP: i32 = 2;

    /// Vanilla: BrushableBlock.TICK_DELAY = 2;
    pub const TICK_DELAY: u32 = 2;

    /// Max visual dusted property value in blockstate (0..3)
    pub const MAX_DUSTED_STAGE: u8 = 3;

    /// Computes visual dusted stage from brush count (BrushableBlockEntity.java:230-238):
    /// - 0 hits -> 0
    /// - 1..2 hits (< 3) -> 1
    /// - 3..5 hits (< 6) -> 2
    /// - 6..9 hits (>= 6) -> 3
    #[must_use]
    pub const fn calculate_dusted_stage(brush_count: i32) -> u8 {
        if brush_count <= 0 {
            0
        } else if brush_count < 3 {
            1
        } else if brush_count < 6 {
            2
        } else {
            3
        }
    }

    /// Determines if brush count reached threshold to break block and eject item.
    #[must_use]
    pub const fn is_brushing_complete(brush_count: i32) -> bool {
        brush_count >= Self::REQUIRED_BRUSHES_TO_BREAK
    }

    /// Retraction decay calculation: Math.max(0, brush_count - 2)
    #[must_use]
    pub const fn decay_brush_count(current_brush_count: i32) -> i32 {
        if current_brush_count <= Self::RETRACTION_BRUSH_STEP {
            0
        } else {
            current_brush_count - Self::RETRACTION_BRUSH_STEP
        }
    }

    pub fn brush(
        world: &Arc<crate::world::World>,
        pos: &pumpkin_util::math::position::BlockPos,
        block: &Block,
    ) {
        let state = world.get_block_state(pos);
        let mut props = SuspiciousSandLikeProperties::from_state_id(state.id);

        let is_gravel = block.id == BlockId::SUSPICIOUS_GRAVEL;

        if let Some(be) = world.get_block_entity(pos)
            && let Some(brush_be) = be.as_any().downcast_ref::<BrushableBlockBlockEntity>()
        {
            let mut hits = brush_be
                .hits
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *hits += 1;

            if Self::is_brushing_complete(*hits) {
                let item = brush_be
                    .item
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                if let Some(item_stack) = item {
                    world.drop_stack(pos, item_stack);
                }

                let target_block = if is_gravel {
                    &Block::GRAVEL
                } else {
                    &Block::SAND
                };

                world.set_block_state(pos, target_block.default_state.id, BlockFlags::NOTIFY_ALL);

                let sound = if is_gravel {
                    Sound::ItemBrushBrushingGravelComplete
                } else {
                    Sound::ItemBrushBrushingSandComplete
                };

                world.play_sound(sound, SoundCategory::Blocks, &pos.to_f64());
            } else {
                props.dusted = Self::calculate_dusted_stage(*hits);
                world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);

                let sound = if is_gravel {
                    Sound::ItemBrushBrushingGravel
                } else {
                    Sound::ItemBrushBrushingSand
                };

                world.play_sound(sound, SoundCategory::Blocks, &pos.to_f64());
            }
        }
    }
}

impl BlockBehaviour for BrushableBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let props = SuspiciousSandLikeProperties::default(args.block);
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let entity = BrushableBlockBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(entity));
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        if let Some(be) = args.world.get_block_entity(args.position)
            && let Some(brush_be) = be.as_any().downcast_ref::<BrushableBlockBlockEntity>()
            && let Some(contained) = brush_be
                .item
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
        {
            args.world.drop_stack(args.position, contained);
        }
    }
}
