use std::sync::Arc;
use std::sync::Mutex;

use crate::block::{
    GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, PathComputationType, PlacedArgs,
};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};

use crate::block::entities::brewing_stand::BrewingStandBlockEntity;
use pumpkin_data::BlockState;
use pumpkin_data::translation;
use pumpkin_inventory::Inventory;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::text::TextComponent;

struct BrewingScreenFactory(
    Arc<dyn Inventory>,
    Arc<dyn crate::block::entities::PropertyDelegate>,
);

impl ScreenHandlerFactory for BrewingScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let inventory = self.0.clone();
        pumpkin_inventory::brewing::create_brewing(sync_id, player_inventory, inventory, &self.1)
            .map(|handler| Arc::new(Mutex::new(handler)) as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_BREWING,
            translation::bedrock::CONTAINER_BREWING
        )
    }
}

#[pumpkin_block("minecraft:brewing_stand")]
pub struct BrewingStandBlock;

impl BrewingStandBlock {
    pub const BREWING_TIME_TICKS: i32 = 400;
    pub const DEFAULT_FUEL_USES: i32 = 20;
    pub const INVENTORY_SIZE: usize = 5;
    pub const EVENT_BREW_COMPLETE: i32 = 1035;

    pub const SLOT_POTION_0: usize = 0;
    pub const SLOT_POTION_1: usize = 1;
    pub const SLOT_POTION_2: usize = 2;
    pub const SLOT_INGREDIENT: usize = 3;
    pub const SLOT_FUEL: usize = 4;

    #[must_use]
    pub fn calculate_bottle_bitmask(b0: bool, b1: bool, b2: bool) -> u8 {
        (if b0 { 1 } else { 0 }) | (if b1 { 2 } else { 0 }) | (if b2 { 4 } else { 0 })
    }

    #[must_use]
    pub fn calculate_comparator_signal(fill_fractions: &[f32; 5]) -> u8 {
        let mut total = 0.0;
        let mut has_items = false;
        for &f in fill_fractions {
            if f > 0.0 {
                has_items = true;
                total += f.clamp(0.0, 1.0);
            }
        }
        if !has_items {
            return 0;
        }
        let percent = total / 5.0;
        (1.0 + percent * 14.0).floor() as u8
    }
}

impl BlockBehaviour for BrewingStandBlock {
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
                pumpkin_data::statistic::CustomStatistic::InteractWithBrewingstand as i32,
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
        let inventory = block_entity.clone().get_inventory()?;
        let pd = block_entity.to_property_delegate()?;
        Some(Box::new(BrewingScreenFactory(inventory, pd)))
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let be = BrewingStandBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(be));
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
