use std::sync::Arc;
use std::sync::Mutex;

use crate::block::entities::enchanting_table::EnchantingTableBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetScreenHandlerFactoryArgs, NormalUseArgs, PathComputationType,
    PlacedArgs,
};
use pumpkin_data::{Block, BlockState, translation};
use pumpkin_inventory::enchanting::enchanting_screen_handler::EnchantingTableScreenHandler;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_inventory::{Inventory, SimpleInventory};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::text::TextComponent;

#[pumpkin_block("minecraft:enchanting_table")]
pub struct EnchantingTableBlock;

impl EnchantingTableBlock {
    pub const MAX_BOOKSHELVES: i32 = 15;
    pub const TOTAL_BOOKSHELF_POSITIONS: usize = 32;

    /// Vanilla EnchantingTableBlock.java:31-34:
    /// All 32 candidate bookshelf offsets surrounding the enchanting table:
    /// x in -2..=2, z in -2..=2, y in 0..=1, filtered by (|x| == 2 || |z| == 2).
    pub const BOOKSHELF_OFFSETS: [(i32, i32, i32); 32] = [
        // y = 0 (16 offsets)
        (-2, 0, -2), (-1, 0, -2), (0, 0, -2), (1, 0, -2), (2, 0, -2),
        (-2, 0, -1),                                       (2, 0, -1),
        (-2, 0,  0),                                       (2, 0,  0),
        (-2, 0,  1),                                       (2, 0,  1),
        (-2, 0,  2), (-1, 0,  2), (0, 0,  2), (1, 0,  2), (2, 0,  2),
        // y = 1 (16 offsets)
        (-2, 1, -2), (-1, 1, -2), (0, 1, -2), (1, 1, -2), (2, 1, -2),
        (-2, 1, -1),                                       (2, 1, -1),
        (-2, 1,  0),                                       (2, 1,  0),
        (-2, 1,  1),                                       (2, 1,  1),
        (-2, 1,  2), (-1, 1,  2), (0, 1,  2), (1, 1,  2), (2, 1,  2),
    ];

    /// Vanilla EnchantingTableBlock.java:43:
    /// The intermediate block between table and bookshelf must be at (offset.x / 2, offset.y, offset.z / 2).
    #[must_use]
    pub const fn get_transmitter_offset(offset: (i32, i32, i32)) -> (i32, i32, i32) {
        (offset.0 / 2, offset.1, offset.2 / 2)
    }

    #[must_use]
    pub fn is_valid_bookshelf_at_offset(
        _offset: (i32, i32, i32),
        is_bookshelf_at_offset: bool,
        is_transmitter_clear: bool,
    ) -> bool {
        is_bookshelf_at_offset && is_transmitter_clear
    }
}

/// Vanilla `EnchantmentHelper.getEnchantmentCost`:
/// `selected = random.nextInt(8) + 1 + (bookcases >> 1) + random.nextInt(bookcases + 1)`
#[must_use]
pub const fn calculate_enchantment_cost(slot: usize, mut bookcases: i32, selected: i32) -> i32 {
    if bookcases > 15 {
        bookcases = 15;
    }
    let cost = match slot {
        0 => {
            let c = selected / 3;
            if c < 1 { 1 } else { c }
        }
        1 => {
            let c = selected * 2 / 3 + 1;
            if c < 1 { 1 } else { c }
        }
        2 => {
            let b2 = bookcases * 2;
            let c = if selected > b2 { selected } else { b2 };
            if c < 1 { 1 } else { c }
        }
        _ => 0,
    };
    if cost < (slot as i32 + 1) {
        0
    } else {
        cost
    }
}

impl BlockBehaviour for EnchantingTableBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        let entity = EnchantingTableBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(entity));
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.remove_block_entity(args.position);
        args.world.update_neighbors(args.position, None);
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if let Some(factory) = self.get_screen_handler_factory(GetScreenHandlerFactoryArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
        }) {
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));
        }
        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let mut bookshelf_count = 0;
        for &offset in &Self::BOOKSHELF_OFFSETS {
            let shelf_pos = args.position.add(offset.0, offset.1, offset.2);
            let trans_offset = Self::get_transmitter_offset(offset);
            let trans_pos = args.position.add(trans_offset.0, trans_offset.1, trans_offset.2);

            if Self::is_bookshelf(args.world, &shelf_pos)
                && Self::is_transmitter(args.world, &trans_pos)
            {
                bookshelf_count += 1;
            }
        }
        let bookshelf_count = bookshelf_count.min(Self::MAX_BOOKSHELVES);

        let seed = args.player.enchantment_seed();
        Some(Box::new(EnchantingTableScreenFactory {
            bookshelf_count,
            seed,
        }))
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

impl EnchantingTableBlock {
    pub fn is_bookshelf(world: &Arc<crate::world::World>, pos: &BlockPos) -> bool {
        let state = world.get_block_state(pos);
        let block = pumpkin_data::Block::from_state_id(state.id);
        block == &Block::BOOKSHELF
    }

    pub fn is_transmitter(world: &Arc<crate::world::World>, pos: &BlockPos) -> bool {
        let state = world.get_block_state(pos);
        let block = pumpkin_data::Block::from_state_id(state.id);
        block == &Block::AIR || block == &Block::CAVE_AIR || block == &Block::VOID_AIR
    }
}

struct EnchantingTableScreenFactory {
    bookshelf_count: i32,
    seed: i32,
}

impl ScreenHandlerFactory for EnchantingTableScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let inventory: Arc<dyn Inventory> = Arc::new(SimpleInventory::new(2));
        let handler = EnchantingTableScreenHandler::new(
            sync_id,
            player_inventory,
            &inventory,
            self.seed,
            self.bookshelf_count,
        );
        let screen_handler_arc = Arc::new(Mutex::new(handler));
        Some(screen_handler_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_ENCHANT,
            translation::bedrock::CONTAINER_ENCHANT
        )
    }
}
