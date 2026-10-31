use std::sync::Arc;

use crate::{
    block::{
        BlockBehaviour, GetComparatorOutputArgs, NormalUseArgs, OnScheduledTickArgs,
        PathComputationType, PlacedArgs, UseWithItemArgs, registry::BlockActionResult,
    },
    entity::{Entity, item::ItemEntity},
    world::World,
};
use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId, block_properties::ComposterLikeProperties,
    data_component_impl::CompostableImpl, entity::EntityType, item::Item, item_stack::ItemStack,
    sound::{Sound, SoundCategory}, world::WorldEvent,
};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use rand::RngExt;

#[pumpkin_block("minecraft:composter")]
pub struct ComposterBlock;

impl BlockBehaviour for ComposterBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state_id(args.position);
        let props = ComposterLikeProperties::from_state_id(state_id);
        if props.level == Self::READY {
            self.clear_composter(args.world, args.position, state_id, args.block);
            return BlockActionResult::Success;
        }

        BlockActionResult::Pass
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state_id(args.position);
        let props = ComposterLikeProperties::from_state_id(state_id);
        let level = props.level;

        // Check if the composter is full
        if level == Self::READY {
            self.clear_composter(args.world, args.position, state_id, args.block);
            return BlockActionResult::Success;
        }

        let item_stack = &mut *args.item_stack;
        let Some(compostable) = item_stack.get_data_component::<CompostableImpl>() else {
            return BlockActionResult::Pass;
        };
        let chance = compostable.chance;

        // Consume one item from the stack (if in survival mode)
        if !args.player.has_infinite_materials() {
            item_stack.decrement(1);
        }

        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Used,
            item_stack.item.id as i32,
            1,
        );

        // Determine if the composter level should increase (pure chance roll, matching vanilla)
        let increased = level < Self::MAX_LEVEL && rand::rng().random_bool(f64::from(chance));
        if increased {
            self.update_level_composter(
                args.world,
                args.position,
                state_id,
                args.block,
                level + 1,
            );
        }

        // Vanilla levelEvent 1500: data 1 for success (particles + sound), 0 for failure (sound only)
        args.world.sync_world_event(
            WorldEvent::ComposterFill,
            *args.position,
            if increased { 1 } else { 0 },
        );

        BlockActionResult::Success
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let props = ComposterLikeProperties::from_state_id(args.state_id);
        if props.level == Self::MAX_LEVEL {
            args.world.schedule_block_tick(
                args.block,
                *args.position,
                Self::FULL_COMPOSTING_TICKS,
                TickPriority::Normal,
            );
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let props = ComposterLikeProperties::from_state_id(state_id);
        let level = props.level;
        if level == Self::MAX_LEVEL {
            self.update_level_composter(
                args.world,
                args.position,
                state_id,
                args.block,
                Self::READY,
            );
            args.world.play_sound(
                Sound::BlockComposterReady,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
            );
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = ComposterLikeProperties::from_state_id(args.state.id);
        Some(Self::calculate_comparator_output(props.level))
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

impl ComposterBlock {
    pub const READY: u8 = 8;
    pub const MIN_LEVEL: u8 = 0;
    pub const MAX_LEVEL: u8 = 7;
    pub const NUM_LEVELS: u8 = 9;
    pub const FULL_COMPOSTING_TICKS: u32 = 20;
    pub const COMPOSTER_FILL_EVENT: i32 = 1500;
    pub const MIN_COMPARATOR_OUTPUT: u8 = 0;
    pub const MAX_COMPARATOR_OUTPUT: u8 = 8;

    pub fn calculate_comparator_output(level: u8) -> u8 {
        level.min(Self::READY)
    }

    pub fn is_ready(level: u8) -> bool {
        level == Self::READY
    }

    pub fn can_compost_more(level: u8) -> bool {
        level < Self::MAX_LEVEL
    }

    pub fn can_hopper_insert(level: u8, face: BlockDirection) -> bool {
        face == BlockDirection::Up && level < Self::MAX_LEVEL
    }

    pub fn can_hopper_extract(level: u8, face: BlockDirection) -> bool {
        face == BlockDirection::Down && level == Self::READY
    }

    pub fn should_level_increase(level: u8, chance: f32, roll: f32) -> bool {
        level < Self::MAX_LEVEL && roll < chance
    }

    pub fn update_level_composter(
        &self,
        world: &Arc<World>,
        location: &BlockPos,
        state_id: BlockStateId,
        block: &Block,
        level: u8,
    ) {
        let mut props = ComposterLikeProperties::from_state_id(state_id);
        props.level = level;
        world.set_block_state(location, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
        if level == Self::MAX_LEVEL {
            world.schedule_block_tick(
                block,
                *location,
                Self::FULL_COMPOSTING_TICKS,
                TickPriority::Normal,
            );
        }
    }

    pub fn clear_composter(
        &self,
        world: &Arc<World>,
        location: &BlockPos,
        state_id: BlockStateId,
        block: &Block,
    ) {
        self.update_level_composter(world, location, state_id, block, Self::MIN_LEVEL);
        world.play_sound(
            Sound::BlockComposterEmpty,
            SoundCategory::Blocks,
            &location.to_centered_f64(),
        );

        let item_position = {
            let mut rng = rand::rng();
            location.to_centered_f64().add_raw(
                rng.random_range(-0.35..=0.35),
                rng.random_range(-0.35..=0.35) + 0.51,
                rng.random_range(-0.35..=0.35),
            )
        };

        let item_entity = ItemEntity::new(
            Entity::new(world.clone(), item_position, &EntityType::ITEM),
            ItemStack::new(1, &Item::BONE_MEAL),
        );

        world.spawn_entity(Arc::new(item_entity));
    }
}
