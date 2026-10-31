use std::sync::Arc;
use std::sync::Mutex;

use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::{
    GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, OnNeighborUpdateArgs, OnPlaceArgs,
    PathComputationType, PlacedArgs,
};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};
use crate::world::World;

use crate::block::entities::hopper::HopperBlockEntity;
use pumpkin_data::block_properties::FacingHopper;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId, translation};
use pumpkin_inventory::Inventory;
use pumpkin_inventory::generic_container_screen_handler::create_hopper;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;
use pumpkin_world::world::BlockFlags;

struct HopperBlockScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for HopperBlockScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let concrete_handler = create_hopper(sync_id, player_inventory, self.0.clone(), player);
        let concrete_arc = Arc::new(Mutex::new(concrete_handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_HOPPER,
            translation::bedrock::CONTAINER_HOPPER
        )
    }
}

#[pumpkin_block("minecraft:hopper")]
pub struct HopperBlock;

impl HopperBlock {
    pub const HOPPER_CONTAINER_SIZE: usize = 5;
    pub const CONTAINER_SIZE: usize = 5;
    pub const MOVE_ITEM_SPEED: i32 = 8;
    pub const NO_COOLDOWN_TIME: i32 = -1;
    pub const SUCK_AABB_Y_MIN: f64 = 11.0 / 16.0;
    pub const SUCK_AABB_Y_MAX: f64 = 2.0;
    pub const MIN_COMPARATOR_OUTPUT: u8 = 0;
    pub const MAX_COMPARATOR_OUTPUT: u8 = 15;

    #[must_use]
    pub fn is_suction_blocked(is_full_cube: bool, has_does_not_block_tag: bool) -> bool {
        is_full_cube && !has_does_not_block_tag
    }

    pub fn calculate_facing_from_direction(direction: BlockDirection) -> FacingHopper {
        match direction {
            BlockDirection::North => FacingHopper::North,
            BlockDirection::East => FacingHopper::East,
            BlockDirection::South => FacingHopper::South,
            BlockDirection::West => FacingHopper::West,
            BlockDirection::Up | BlockDirection::Down => FacingHopper::Down,
        }
    }

    pub fn is_enabled(has_neighbor_signal: bool) -> bool {
        !has_neighbor_signal
    }

    pub fn calculate_container_comparator_output(slot_counts: &[u8], max_stack_sizes: &[u8]) -> u8 {
        let mut total_filled = 0.0f32;
        let mut has_items = false;
        let num_slots = slot_counts.len();
        if num_slots == 0 {
            return 0;
        }

        for i in 0..num_slots {
            let count = slot_counts[i] as f32;
            let max_stack = max_stack_sizes[i].max(1) as f32;
            if count > 0.0 {
                has_items = true;
                total_filled += count / max_stack;
            }
        }

        if !has_items {
            return 0;
        }

        let fullness = total_filled / num_slots as f32;
        let signal = (fullness * 14.0).floor() as u8 + 1;
        signal.min(15)
    }
}

type HopperLikeProperties = pumpkin_data::block_properties::HopperLikeProperties;

impl BlockBehaviour for HopperBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if let Some(factory) = self.get_screen_handler_factory(GetScreenHandlerFactoryArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
        }) {
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::InspectHopper as i32,
                1,
            );
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));
        }

        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let block_entity = args.world.get_block_entity(args.position)?;
        let inventory = block_entity.get_inventory()?;
        Some(Box::new(HopperBlockScreenFactory(inventory)))
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = HopperLikeProperties::default(args.block);
        props.facing = Self::calculate_facing_from_direction(args.direction);
        props.enabled = true;
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let props = HopperLikeProperties::from_state_id(args.state_id);
        let hopper_block_entity = HopperBlockEntity::new(*args.position, props.facing);
        args.world.add_block_entity(Arc::new(hopper_block_entity));
        if Block::from_state_id(args.old_state_id) != Block::from_state_id(args.state_id) {
            check_powered_state(args.world, args.position, args.state_id, args.block);
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        // Same guard as `RedstoneGateBlock::on_neighbor_update` (abstract_redstone_gate.rs):
        // the block at this position may have already changed since dispatch.
        if args.world.get_block(args.position) != args.block {
            return;
        }
        check_powered_state(
            args.world,
            args.position,
            args.world.get_block_state_id(args.position),
            args.block,
        );
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn check_powered_state(world: &Arc<World>, pos: &BlockPos, state_id: BlockStateId, block: &Block) {
    let signal = !block_receives_redstone_power(world, pos);
    let mut state = HopperLikeProperties::from_state_id(state_id);
    if signal != state.enabled {
        state.enabled = signal;
        world.set_block_state(pos, state.to_state_id(block), BlockFlags::NOTIFY_LISTENERS);
    }
}
