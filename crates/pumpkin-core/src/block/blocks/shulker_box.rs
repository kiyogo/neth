use std::sync::Arc;
use std::sync::Mutex;

use crate::block::{
    BrokenArgs, GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, OnPlaceArgs,
    OnSyncedBlockEventArgs, PlacedArgs,
};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};
use crate::world::World;
use pumpkin_data::BlockDirection;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;

use crate::block::entities::shulker_box::ShulkerBoxBlockEntity;
use pumpkin_data::FacingExt;
use pumpkin_data::BlockStateId;
use pumpkin_data::translation;
use pumpkin_inventory::Inventory;
use pumpkin_inventory::generic_container_screen_handler::create_shulker_box;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::text::TextComponent;

struct ShulkerBoxScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for ShulkerBoxScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler = create_shulker_box(sync_id, player_inventory, self.0.clone(), player);
        let screen_handler_arc = Arc::new(Mutex::new(handler));

        Some(screen_handler_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_SHULKERBOX,
            translation::bedrock::CONTAINER_SHULKERBOX
        )
    }
}

/// Computes the bounding box of the opening shulker lid for collision testing.
/// Matches vanilla `Shulker.getProgressDeltaAabb(1.0F, direction, 0.0F, 0.5F, Vec3.atBottomCenterOf(pos)).deflate(1.0E-6)`.
#[must_use]
pub fn shulker_box_lid_box(pos: &BlockPos, facing: BlockDirection) -> BoundingBox {
    let x = pos.0.x as f64;
    let y = pos.0.y as f64;
    let z = pos.0.z as f64;
    let eps = 1.0e-6;
    let (min, max) = match facing {
        BlockDirection::Up => (
            [x + eps, y + 1.0 + eps, z + eps],
            [x + 1.0 - eps, y + 1.5 - eps, z + 1.0 - eps],
        ),
        BlockDirection::Down => (
            [x + eps, y - 0.5 + eps, z + eps],
            [x + 1.0 - eps, y - eps, z + 1.0 - eps],
        ),
        BlockDirection::North => (
            [x + eps, y + eps, z - 0.5 + eps],
            [x + 1.0 - eps, y + 1.0 - eps, z - eps],
        ),
        BlockDirection::South => (
            [x + eps, y + eps, z + 1.0 + eps],
            [x + 1.0 - eps, y + 1.0 - eps, z + 1.5 - eps],
        ),
        BlockDirection::West => (
            [x - 0.5 + eps, y + eps, z + eps],
            [x - eps, y + 1.0 - eps, z + 1.0 - eps],
        ),
        BlockDirection::East => (
            [x + 1.0 + eps, y + eps, z + eps],
            [x + 1.5 - eps, y + 1.0 - eps, z + 1.0 - eps],
        ),
    };
    BoundingBox::new_array(min, max)
}

/// Matches vanilla `ShulkerBoxBlock.canOpen(state, level, pos, blockEntity)`.
#[must_use]
pub fn can_open_shulker_box(world: &World, pos: &BlockPos, facing: BlockDirection) -> bool {
    let open_pos = pos.offset(facing.to_offset());
    let open_state = world.get_block_state(&open_pos);
    if open_state.is_solid_block() {
        return false;
    }
    let lid_box = shulker_box_lid_box(pos, facing);
    let blocking = world
        .get_entities_and_players_at_box(&lid_box)
        .iter()
        .any(|e| !e.is_spectator());
    !blocking
}

#[pumpkin_block_from_tag("minecraft:shulker_boxes")]
pub struct ShulkerBoxBlock;

type EndRodLikeProperties = pumpkin_data::block_properties::EndRodLikeProperties;

impl BlockBehaviour for ShulkerBoxBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = EndRodLikeProperties::default(args.block);
        props.facing = args.direction.to_facing().opposite();
        props.to_state_id(args.block)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        // On the server, we don't need the Animation steps for now, because the client is responsible for that.
        // TODO: Do not open the shulker box when it is currently closing
        args.r#type == Self::OPEN_ANIMATION_EVENT_TYPE
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let shulker_box_entity = ShulkerBoxBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(shulker_box_entity));
        args.world.update_neighbour_for_output_signal(args.position, args.block);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.remove_block_entity(args.position);
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.update_neighbors(args.position, None);
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        let props = EndRodLikeProperties::from_state_id(state.id);
        let open_dir = props.facing.to_block_direction();
        if !can_open_shulker_box(args.world, args.position, open_dir) {
            return BlockActionResult::Pass;
        }

        if let Some(factory) = self.get_screen_handler_factory(GetScreenHandlerFactoryArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
        }) {
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::OpenShulkerBox as i32,
                1,
            );
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));
            crate::entity::mob::piglin_ai::PiglinAi::anger_nearby_piglins(
                args.world,
                args.player,
                true,
            );
        }

        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let block_entity = args.world.get_block_entity(args.position)?;
        let inventory = block_entity.get_inventory()?;
        Some(Box::new(ShulkerBoxScreenFactory(inventory)))
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }
}

impl ShulkerBoxBlock {
    pub const OPEN_ANIMATION_EVENT_TYPE: u8 = 1;
}
