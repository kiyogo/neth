use pumpkin_data::block_properties::StickyPistonLikeProperties;
use pumpkin_data::block_registry::BlockDrops;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::item::Item;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockId, BlockState};

use pumpkin_data::BlockStateId;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::random::{RandomGenerator, get_seed, xoroshiro128::Xoroshiro};

use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::player::Player;
use crate::world::World;
use crate::world::loot::LootContextParameters;
use std::borrow::Cow;
use std::sync::Arc;

pub mod blocks;
pub mod entities;
pub mod fluid;
pub mod registry;
pub mod viewer;

use crate::block::registry::BlockActionResult;
use crate::entity::EntityBase;
use crate::server::Server;
use pumpkin_data::BlockDirection;
use pumpkin_data::block_properties::WaterLikeProperties;
use pumpkin_data::block_rotation::{Mirror, Rotation};
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_inventory::screen_handler::ScreenHandlerFactory;
use pumpkin_protocol::java::server::play::SUseItemOn;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathComputationType {
    Land,
    Water,
    Air,
}

/// Vanilla's suffocation predicate, including state and block-entity overrides.
pub(crate) fn is_suffocating(
    world: &World,
    pos: &BlockPos,
    block: &Block,
    state: &BlockState,
) -> bool {
    if block.has_tag(&tag::Block::MINECRAFT_SHULKER_BOXES) {
        return world.get_block_entity(pos).is_none_or(|entity| {
            entity
                .as_any()
                .downcast_ref::<entities::shulker_box::ShulkerBoxBlockEntity>()
                .is_none_or(entities::shulker_box::ShulkerBoxBlockEntity::is_closed)
        });
    }
    if matches!(block.id, BlockId::PISTON | BlockId::STICKY_PISTON) {
        return !StickyPistonLikeProperties::from_state_id(state.id).extended;
    }
    state.is_suffocating()
}

pub trait BlockMetadata {
    fn ids() -> Box<[BlockId]>;
}

pub trait FluidMetadata {
    fn ids() -> Box<[u16]>;
}

pub(crate) fn stop_vertical_movement_after_fall(entity: &dyn EntityBase) {
    let entity = entity.get_entity();
    let mut velocity = entity.velocity.load();
    velocity.y = 0.0;
    entity.velocity.store(velocity);
}

pub(crate) fn bounce_entity_after_fall(entity: &dyn EntityBase, bounce_multiplier: f64) {
    let base_entity = entity.get_entity();
    let mut velocity = base_entity.velocity.load();

    if base_entity.is_sneaking() {
        velocity.y = 0.0;
    } else if velocity.y < 0.0 {
        let entity_factor = if entity.get_living_entity().is_some() {
            1.0
        } else {
            0.8
        };
        velocity.y = -velocity.y * bounce_multiplier * entity_factor;
    }

    base_entity.velocity.store(velocity);
}

pub(crate) fn push_entities_up(world: &Arc<World>, position: &BlockPos) {
    let block_box = BoundingBox::from_block(position);
    let old_top = f64::from(position.0.y) + 15.0 / 16.0;

    for entity in world.get_all_at_box(&block_box) {
        let base_entity = entity.get_entity();
        if base_entity.bounding_box.load().max.y <= old_top {
            continue;
        }

        base_entity.set_pos(base_entity.pos.load() + Vector3::new(0.0, 1.0 / 16.0, 0.0));
    }
}

pub trait BlockBehaviour: Send + Sync {
    fn is_valid_bonemeal_target(&self, _args: BonemealArgs<'_>) -> bool {
        false
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, _args: BonemealArgs<'_>) {}

    fn normal_use(&self, _args: NormalUseArgs<'_>) -> BlockActionResult {
        BlockActionResult::Pass
    }

    fn get_screen_handler_factory(
        &self,
        _args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        None
    }

    fn use_with_item(&self, _args: UseWithItemArgs<'_>) -> BlockActionResult {
        BlockActionResult::PassToDefaultBlockAction
    }

    fn on_entity_collision(&self, _args: OnEntityCollisionArgs<'_>) {}

    fn on_projectile_hit(&self, _args: OnProjectileHitArgs<'_>) {}

    /// Called when an entity is standing on / walking over the top face of this block.
    fn on_entity_step(&self, _args: OnEntityStepArgs<'_>) {}

    fn should_drop_items_on_explosion(&self) -> bool {
        true
    }

    fn explode(&self, _args: ExplodeArgs<'_>) {}

    /// Handles the block event, which is an event specific to a block with an integer ID and data.
    ///
    /// returns whether the event was handled successfully
    fn on_synced_block_event(&self, _args: OnSyncedBlockEventArgs<'_>) -> bool {
        false
    }

    /// getPlacementState in source code
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        args.block.default_state.id
    }

    fn random_tick(&self, _args: RandomTickArgs<'_>) {}

    fn can_place_at(&self, _args: CanPlaceAtArgs<'_>) -> bool {
        true
    }

    fn can_update_at(&self, _args: CanUpdateAtArgs<'_>) -> bool {
        false
    }

    /// onBlockAdded in source code
    fn placed(&self, _args: PlacedArgs<'_>) {}

    fn player_placed(&self, _args: PlayerPlacedArgs<'_>) {}

    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        if let Some(living) = args.entity.get_living_entity() {
            living.handle_fall_damage(args.entity, args.fall_distance, 1.0);
        }
    }

    fn update_entity_movement_after_fall_on(&self, args: UpdateEntityMovementAfterFallOnArgs<'_>) {
        stop_vertical_movement_after_fall(args.entity);
    }

    /// Called when a player starts digging this block, except in creative.
    fn attack(&self, _args: AttackArgs<'_>) {}

    fn broken(&self, _args: BrokenArgs<'_>) {}

    fn on_neighbor_update(&self, _args: OnNeighborUpdateArgs<'_>) {}

    /// Called if a block state is replaced or it replaces another state
    fn prepare(&self, _args: PrepareArgs<'_>) {}

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        args.state_id
    }

    fn on_scheduled_tick(&self, _args: OnScheduledTickArgs<'_>) {}

    fn on_state_replaced(&self, _args: OnStateReplacedArgs<'_>) {}

    // --- Redstone/Comparator Methods ---

    /// Sides where redstone connects to
    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        false
    }

    /// Weak redstone power, aka. block that should be powered needs to be directly next to the source block
    fn get_weak_redstone_power(&self, _args: GetRedstonePowerArgs<'_>) -> u8 {
        0
    }

    /// Strong redstone power. this can power a block that then gives power
    fn get_strong_redstone_power(&self, _args: GetRedstonePowerArgs<'_>) -> u8 {
        0
    }

    fn get_comparator_output(&self, _args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        None
    }

    fn get_inside_collision_shape(&self, _args: GetInsideCollisionShapeArgs<'_>) -> BoundingBox {
        BoundingBox::full_block()
    }

    /// Whether entities in the way stop this block from being placed.
    ///
    /// Vanilla checks placement against `getCollisionShape`
    /// I had to add this just for scaffolding idk where else its used
    fn has_placement_collision(&self) -> bool {
        true
    }

    fn mirror(&self, block: &Block, state_id: BlockStateId, mirror: Mirror) -> &'static BlockState {
        block.mirror(state_id, mirror)
    }

    fn rotate(
        &self,
        block: &Block,
        state_id: BlockStateId,
        rotation: Rotation,
    ) -> &'static BlockState {
        block.rotate(state_id, rotation)
    }

    fn is_pathfindable(&self, state: &BlockState, computation_type: PathComputationType) -> bool {
        match computation_type {
            PathComputationType::Water => {
                state.is_waterlogged()
                    || Fluid::from_state_id(state.id)
                        .is_some_and(|f| f.has_tag(&tag::Fluid::MINECRAFT_WATER))
            }
            PathComputationType::Land | PathComputationType::Air => !state.is_full_cube(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct BonemealArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
    pub state_id: BlockStateId,
}

pub struct NormalUseArgs<'a> {
    pub server: &'a Server,
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
    pub player: &'a Arc<Player>,
    pub hit: &'a BlockHitResult<'a>,
}

#[derive(Clone, Copy)]
pub struct GetScreenHandlerFactoryArgs<'a> {
    pub server: &'a Server,
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
    pub player: &'a Arc<Player>,
}

pub struct UseWithItemArgs<'a> {
    pub server: &'a Server,
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
    pub player: &'a Arc<Player>,
    pub hit: &'a BlockHitResult<'a>,
    pub item_stack: &'a mut ItemStack,
    pub equipment_slot: &'a EquipmentSlot,
}

pub struct BlockHitResult<'a> {
    pub face: &'a BlockDirection,
    pub cursor_pos: &'a Vector3<f32>,
}

pub struct OnEntityCollisionArgs<'a> {
    pub server: &'a Server,
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
    pub entity: &'a dyn EntityBase,
}

pub struct OnProjectileHitArgs<'a> {
    pub server: &'a Server,
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
    pub projectile: &'a dyn EntityBase,
    pub hit_pos: &'a Vector3<f64>,
}

pub struct OnEntityStepArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
    pub entity: &'a dyn EntityBase,
    pub below_supporting_block: bool,
}

pub struct ExplodeArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
}

pub struct OnSyncedBlockEventArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
    pub r#type: u8,
    pub data: u8,
}

pub struct OnPlaceArgs<'a> {
    pub server: &'a Server,
    pub world: &'a World,
    pub block: &'a Block,
    pub position: &'a BlockPos,
    pub direction: BlockDirection,
    pub player: &'a Player,
    pub replacing: BlockIsReplacing,
    pub use_item_on: &'a SUseItemOn,
}

pub struct RandomTickArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
}

pub struct CanPlaceAtArgs<'a> {
    pub server: Option<&'a Server>,
    pub world: Option<&'a World>,
    pub block_accessor: &'a dyn BlockAccessor,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
    pub direction: Option<BlockDirection>,
    pub player: Option<&'a Player>,
    pub use_item_on: Option<&'a SUseItemOn>,
}

pub struct CanUpdateAtArgs<'a> {
    pub world: &'a World,
    pub block: &'a Block,
    pub state_id: BlockStateId,
    pub position: &'a BlockPos,
    pub direction: BlockDirection,
    pub player: &'a Player,
    pub use_item_on: &'a SUseItemOn,
}

pub struct PlacedArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub state_id: BlockStateId,
    pub old_state_id: BlockStateId,
    pub position: &'a BlockPos,
    pub notify: bool,
}

pub struct PlayerPlacedArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub state_id: BlockStateId,
    pub position: &'a BlockPos,
    pub direction: BlockDirection,
    pub player: &'a Player,
    pub stack: &'a ItemStack,
}

pub struct OnLandedUponArgs<'a> {
    pub world: &'a Arc<World>,
    pub position: &'a BlockPos,
    pub fall_distance: f32,
    pub entity: &'a dyn EntityBase,
}

pub struct UpdateEntityMovementAfterFallOnArgs<'a> {
    pub entity: &'a dyn EntityBase,
}

pub struct AttackArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
    pub player: &'a Arc<Player>,
}

pub struct BrokenArgs<'a> {
    pub block: &'a Block,
    pub player: &'a Arc<Player>,
    pub position: &'a BlockPos,
    pub server: &'a Server,
    pub world: &'a Arc<World>,
    pub state: &'a BlockState,
}

pub struct OnNeighborUpdateArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
    pub source_block: &'a Block,
    pub notify: bool,
}

pub struct PrepareArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub state_id: BlockStateId,
    pub position: &'a BlockPos,
    pub flags: BlockFlags,
}

pub struct GetStateForNeighborUpdateArgs<'a> {
    pub world: &'a World,
    pub block: &'a Block,
    pub state_id: BlockStateId,
    pub position: &'a BlockPos,
    pub direction: BlockDirection,
    pub neighbor_position: &'a BlockPos,
    pub neighbor_state_id: BlockStateId,
}

pub struct OnScheduledTickArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub position: &'a BlockPos,
}

pub struct OnStateReplacedArgs<'a> {
    pub world: &'a Arc<World>,
    pub block: &'a Block,
    pub old_state_id: BlockStateId,
    pub position: &'a BlockPos,
    pub moved: bool,
}

pub struct EmitsRedstonePowerArgs<'a> {
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub direction: BlockDirection,
}

pub struct GetRedstonePowerArgs<'a> {
    pub world: &'a World,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
    pub direction: BlockDirection,
}

pub struct GetComparatorOutputArgs<'a> {
    pub world: &'a World,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
    /// Face of this block that the reading comparator sits against.
    pub direction: BlockDirection,
}

pub struct GetInsideCollisionShapeArgs<'a> {
    pub world: &'a World,
    pub block: &'a Block,
    pub state: &'a BlockState,
    pub position: &'a BlockPos,
}

#[derive(Clone, PartialEq, Eq)]
pub struct BlockEvent {
    pub pos: BlockPos,
    pub r#type: u8,
    pub data: u8,
}

/// Returns the block name whose loot table should be used.
/// Wall variants (e.g. wall torches, wall signs, wall banners, wall heads, wall fans)
/// share the loot table of their non-wall counterpart but do not have their own loot table file.
#[must_use]
pub fn loot_table_name(block: &Block) -> Cow<'_, str> {
    if let Some(rest) = block.name.strip_prefix("wall_") {
        Cow::Borrowed(rest)
    } else if block.name.contains("_wall_") {
        Cow::Owned(block.name.replacen("_wall_", "_", 1))
    } else {
        Cow::Borrowed(block.name)
    }
}

#[must_use]
pub fn loot_table_block(block: &Block) -> &Block {
    let name = loot_table_name(block);
    Block::from_name(&name).unwrap_or(block)
}

/// The items a block drops: its loot table, or what the registration of a custom block says.
fn block_loot(world: &Arc<World>, block: &Block, params: &LootContextParameters) -> Vec<ItemStack> {
    let Some(info) = block.dynamic_info() else {
        let Some(table) = block.loot_table else {
            return Vec::new();
        };
        return loot_table_items(world, table, params);
    };
    match &info.registration.drops {
        BlockDrops::Nothing => Vec::new(),
        BlockDrops::LootTable(key) => loot_table_items(world, key, params),
        BlockDrops::SelfItem => {
            // The `survives_explosion` condition of the vanilla block loot tables.
            if params
                .explosion_radius
                .is_some_and(|radius| rand::random::<f32>() > 1.0 / radius)
            {
                return Vec::new();
            }
            Item::from_id(block.item_id)
                .filter(|_| block.item_id != 0)
                .map(|item| vec![ItemStack::new(1, item)])
                .unwrap_or_default()
        }
    }
}

fn loot_table_items(
    world: &Arc<World>,
    key: &str,
    params: &LootContextParameters,
) -> Vec<ItemStack> {
    let Some(loot_table) = world.get_loot_table(key) else {
        return Vec::new();
    };
    let seed: i64 = rand::random();
    crate::world::loot::generate_loot_from_handle(&loot_table, seed, params)
}

/// Rolls the block's loot table and fires `BlockDropItemEvent`, without spawning anything.
pub fn collect_loot(
    world: &Arc<World>,
    block: &Block,
    pos: &BlockPos,
    params: &LootContextParameters,
) -> Vec<ItemStack> {
    let loot_block = loot_table_block(block);
    let items = block_loot(world, loot_block, params);
    if items.is_empty() {
        return items;
    }

    let mut event = crate::plugin::block::block_drop_item::BlockDropItemEvent {
        block_pos: *pos,
        world: world.clone(),
        player: None,
        items,
        cancelled: false,
    };
    if let Some(server) = world.server.upgrade() {
        server.plugin_manager.fire_blocking(&server, &mut event);
    }
    if event.cancelled {
        return Vec::new();
    }

    let block_entity = world.get_block_entity(pos);
    let mut items = event.items;
    if let Some(block_entity) = &block_entity {
        for stack in &mut items {
            if Block::from_item_id(stack.item.id) == Some(loot_block) {
                block_entity.collect_item_components(stack);
            }
        }
    }
    items
}

pub fn drop_loot(
    world: &Arc<World>,
    block: &Block,
    pos: &BlockPos,
    experience: bool,
    params: &LootContextParameters,
) {
    for stack in collect_loot(world, block, pos, params) {
        world.drop_stack(pos, stack);
    }

    let has_silk_touch = params.tool.as_ref().is_some_and(|tool| {
        pumpkin_data::Enchantment::from_name("silk_touch")
            .is_some_and(|e| tool.get_enchantment_level(e) > 0)
    });

    if experience
        && !has_silk_touch
        && let Some(experience) = &block.experience
    {
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));
        let amount = experience.experience.get(&mut random);
        if amount > 0 {
            let mut event = crate::plugin::block::block_exp::BlockExpEvent {
                block_pos: *pos,
                world: world.clone(),
                exp: amount,
            };
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.exp > 0 {
                ExperienceOrbEntity::spawn(world, pos.to_f64(), event.exp as u32);
            }
        }
    }
}

/// Drops the block's item with all collected components for a filled creative container.
/// Logs and skips an invalid block item; plugin cancellation and item edits remain authoritative.
pub fn drop_block_entity_item(
    world: &Arc<World>,
    block: &Block,
    pos: &BlockPos,
    block_entity: &dyn entities::BlockEntity,
) {
    let Some(item) = pumpkin_data::item::Item::from_id(block.item_id) else {
        tracing::warn!(
            "Cannot drop block {} with unknown item ID {}",
            block.name,
            block.item_id
        );
        return;
    };
    let mut stack = ItemStack::new(1, item);
    stack.patch = block_entity
        .collect_components()
        .into_iter()
        .map(|(kind, value)| (kind, Some(value)))
        .collect();

    let mut event = crate::plugin::block::block_drop_item::BlockDropItemEvent {
        block_pos: *pos,
        world: world.clone(),
        player: None,
        items: vec![stack],
        cancelled: false,
    };
    if let Some(server) = world.server.upgrade() {
        server.plugin_manager.fire_blocking(&server, &mut event);
    }
    if !event.cancelled {
        for stack in event.items {
            world.drop_stack(pos, stack);
        }
    }
}

pub fn calc_block_breaking(player: &Player, state: &BlockState, block: &'static Block) -> f32 {
    let hardness = state.hardness;
    #[expect(clippy::float_cmp)]
    if hardness == -1.0 {
        // unbreakable
        return 0.0;
    }
    let i = if player.can_harvest(state, block) {
        30.0
    } else {
        100.0
    };

    player.get_mining_speed(block) / hardness / i
}

#[derive(PartialEq, Eq, Debug)]
pub enum BlockIsReplacing {
    Itself(BlockStateId),
    Water(u8),
    Other,
    None,
}

impl BlockIsReplacing {
    #[must_use]
    /// Returns true if the block was a water source block.
    pub const fn water_source(&self) -> bool {
        match self {
            // Level 0 means the water is a source block
            Self::Water(level) => *level == 0,
            _ => false,
        }
    }
}

/// Where a block item places its block, like vanilla's `BlockPlaceContext`.
pub struct BlockPlaceContext {
    /// `getClickedPos`: the position the block is placed at.
    pub position: BlockPos,
    pub clicked_face: BlockDirection,
    /// `isInside`: the click came from inside the clicked block.
    pub inside: bool,
    pub replacing: BlockIsReplacing,
}

impl BlockPlaceContext {
    /// `BlockPlaceContext.at`: places into the replaceable block at `position` as if its
    /// `direction` face was clicked.
    #[must_use]
    pub fn at(world: &World, position: BlockPos, direction: BlockDirection) -> Self {
        let state_id = world.get_block_state_id(&position);
        let replacing = if Block::from_state_id(state_id) == &Block::WATER {
            BlockIsReplacing::Water(WaterLikeProperties::from_state_id(state_id).level)
        } else {
            BlockIsReplacing::Other
        };
        Self {
            position,
            clicked_face: direction,
            inside: false,
            replacing,
        }
    }
}

/// Vanilla `AbstractContainerMenu.getRedstoneSignalFromBlockEntity`: read a block
/// entity's own inventory as a comparator would.
#[must_use]
pub fn container_comparator_output(args: &GetComparatorOutputArgs<'_>) -> Option<u8> {
    let inventory = args
        .world
        .get_block_entity(args.position)?
        .get_inventory()?;
    Some(calculate_comparator_output(inventory.as_ref()))
}

pub fn calculate_comparator_output(inventory: &dyn pumpkin_inventory::Inventory) -> u8 {
    let size = inventory.size();
    if size == 0 {
        return 0;
    }
    let mut fill_sum = 0.0;
    let mut non_empty_count = 0;
    for i in 0..size {
        let stack = inventory.get_stack(i);
        if !stack.is_empty() {
            let max_stack = stack.get_max_stack_size() as f32;
            let count = stack.item_count as f32;
            fill_sum += count / max_stack;
            non_empty_count += 1;
        }
    }
    if non_empty_count == 0 {
        return 0;
    }
    let percentage = fill_sum / (size as f32);
    let output = 1.0 + percentage * 14.0;
    output.floor() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loot_table_name_wall_variants() {
        assert_eq!(loot_table_name(&Block::WALL_TORCH), "torch");
        assert_eq!(loot_table_name(&Block::SOUL_WALL_TORCH), "soul_torch");
        assert_eq!(loot_table_name(&Block::REDSTONE_WALL_TORCH), "redstone_torch");
        assert_eq!(loot_table_name(&Block::COPPER_WALL_TORCH), "copper_torch");

        assert_eq!(loot_table_name(&Block::OAK_WALL_SIGN), "oak_sign");
        assert_eq!(loot_table_name(&Block::OAK_WALL_HANGING_SIGN), "oak_hanging_sign");
        assert_eq!(loot_table_name(&Block::WHITE_WALL_BANNER), "white_banner");
        assert_eq!(loot_table_name(&Block::ZOMBIE_WALL_HEAD), "zombie_head");
        assert_eq!(loot_table_name(&Block::SKELETON_WALL_SKULL), "skeleton_skull");
        assert_eq!(loot_table_name(&Block::TUBE_CORAL_WALL_FAN), "tube_coral_fan");

        // Non-wall-variants with "wall" in their name must remain unchanged
        assert_eq!(loot_table_name(&Block::COBBLESTONE_WALL), "cobblestone_wall");
        assert_eq!(loot_table_name(&Block::STONE), "stone");
    }

    #[test]
    fn test_rotated_pillar_axis_placement() {
        use pumpkin_data::block_properties::{Axis, PaleOakWoodLikeProperties};
        for block in [
            &Block::DEEPSLATE,
            &Block::BASALT,
            &Block::POLISHED_BASALT,
            &Block::QUARTZ_PILLAR,
            &Block::PURPUR_PILLAR,
            &Block::BONE_BLOCK,
            &Block::OCHRE_FROGLIGHT,
            &Block::BAMBOO_BLOCK,
        ] {
            let mut props = PaleOakWoodLikeProperties::default(block);
            props.axis = BlockDirection::East.to_axis();
            let state_id = props.to_state_id(block);
            let read_back = PaleOakWoodLikeProperties::from_state_id(state_id);
            assert_eq!(read_back.axis, Axis::X);

            let mut props_y = PaleOakWoodLikeProperties::default(block);
            props_y.axis = BlockDirection::Up.to_axis();
            let state_id_y = props_y.to_state_id(block);
            let read_back_y = PaleOakWoodLikeProperties::from_state_id(state_id_y);
            assert_eq!(read_back_y.axis, Axis::Y);

            let mut props_z = PaleOakWoodLikeProperties::default(block);
            props_z.axis = BlockDirection::North.to_axis();
            let state_id_z = props_z.to_state_id(block);
            let read_back_z = PaleOakWoodLikeProperties::from_state_id(state_id_z);
            assert_eq!(read_back_z.axis, Axis::Z);
        }
    }

    #[test]
    fn test_skull_wall_variants_metadata() {
        use crate::block::blocks::skull_block::SkullBlock;
        use crate::block::blocks::wither_skull::WitherSkeletonSkullBlock;
        let skull_ids = SkullBlock::ids();
        assert!(skull_ids.contains(&BlockId::SKELETON_WALL_SKULL));
        assert!(skull_ids.contains(&BlockId::PLAYER_WALL_HEAD));
        assert!(skull_ids.contains(&BlockId::ZOMBIE_WALL_HEAD));
        assert!(skull_ids.contains(&BlockId::CREEPER_WALL_HEAD));
        assert!(skull_ids.contains(&BlockId::PIGLIN_WALL_HEAD));
        assert!(skull_ids.contains(&BlockId::DRAGON_WALL_HEAD));

        let wither_ids = WitherSkeletonSkullBlock::ids();
        assert!(wither_ids.contains(&BlockId::WITHER_SKELETON_SKULL));
        assert!(wither_ids.contains(&BlockId::WITHER_SKELETON_WALL_SKULL));
    }

    #[test]
    fn test_huge_mushroom_properties() {
        use pumpkin_data::block_properties::BrownMushroomBlockLikeProperties;
        for block in [
            &Block::BROWN_MUSHROOM_BLOCK,
            &Block::RED_MUSHROOM_BLOCK,
            &Block::MUSHROOM_STEM,
        ] {
            let mut props = BrownMushroomBlockLikeProperties::default(block);
            assert!(props.up && props.down && props.north && props.south && props.east && props.west);

            // Connect on down
            props.down = false;
            let state_id = props.to_state_id(block);
            let read_back = BrownMushroomBlockLikeProperties::from_state_id(state_id);
            assert!(!read_back.down);
            assert!(read_back.up);
        }
    }

    #[test]
    fn test_copper_golem_statue_cycle() {
        use pumpkin_data::block_properties::{
            CopperGolemPose, CopperGolemStatueLikeProperties, EnumVariants,
        };
        for block in [
            &Block::COPPER_GOLEM_STATUE,
            &Block::WAXED_COPPER_GOLEM_STATUE,
            &Block::OXIDIZED_COPPER_GOLEM_STATUE,
            &Block::WAXED_OXIDIZED_COPPER_GOLEM_STATUE,
        ] {
            let mut props = CopperGolemStatueLikeProperties::default(block);
            assert_eq!(props.copper_golem_pose, CopperGolemPose::Standing);

            // Cycle through all 4 poses
            let next_idx = (props.copper_golem_pose.to_index() + 1) % 4;
            props.copper_golem_pose = CopperGolemPose::from_index(next_idx);
            assert_eq!(props.copper_golem_pose, CopperGolemPose::Sitting);

            let next_idx = (props.copper_golem_pose.to_index() + 1) % 4;
            props.copper_golem_pose = CopperGolemPose::from_index(next_idx);
            assert_eq!(props.copper_golem_pose, CopperGolemPose::Running);

            let next_idx = (props.copper_golem_pose.to_index() + 1) % 4;
            props.copper_golem_pose = CopperGolemPose::from_index(next_idx);
            assert_eq!(props.copper_golem_pose, CopperGolemPose::Star);

            let next_idx = (props.copper_golem_pose.to_index() + 1) % 4;
            props.copper_golem_pose = CopperGolemPose::from_index(next_idx);
            assert_eq!(props.copper_golem_pose, CopperGolemPose::Standing);
        }
    }

    #[test]
    fn test_heavy_core_properties() {
        use pumpkin_data::block_properties::MangroveRootsLikeProperties;
        let block = &Block::HEAVY_CORE;
        let mut props = MangroveRootsLikeProperties::default(block);
        assert!(!props.waterlogged);
        props.waterlogged = true;
        let state_id = props.to_state_id(block);
        let read_back = MangroveRootsLikeProperties::from_state_id(state_id);
        assert!(read_back.waterlogged);
    }
}
