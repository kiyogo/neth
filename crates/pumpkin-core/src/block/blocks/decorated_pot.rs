use std::sync::Arc;

use pumpkin_data::block_properties::DecoratedPotLikeProperties;
use pumpkin_data::enchantment::Enchantment;
use pumpkin_data::entity::EntityType;
use pumpkin_data::tag;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::vector3::Vector3;
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::decorated_pot::{DecoratedPotBlockEntity, PotWobbleStyle};
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetComparatorOutputArgs, NormalUseArgs, OnPlaceArgs,
    OnProjectileHitArgs, OnSyncedBlockEventArgs, PathComputationType, PlacedArgs, UseWithItemArgs,
};
use crate::world::World;

#[pumpkin_block("minecraft:decorated_pot")]
pub struct DecoratedPotBlock;

impl DecoratedPotBlock {
    pub const TOTAL_SHERD_FACES: usize = 4;
    pub const INSERT_DUST_PLUME_PARTICLES: i32 = 7;
    pub const SHERD_DYNAMIC_DROP_ID: &'static str = "sherds";
    pub const WOBBLE_POSITIVE_TICKS: u32 = 7;
    pub const WOBBLE_NEGATIVE_TICKS: u32 = 10;
    pub const EVENT_POT_WOBBLES: u32 = 1;
    pub const BASE_INSERT_PITCH: f32 = 0.7;
    pub const PITCH_BEND_SCALE: f32 = 0.5;

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
        Self::BASE_INSERT_PITCH + Self::PITCH_BEND_SCALE * pitch_bend
    }

    /// Vanilla DecoratedPotBlock.java:191:
    /// Pots shatter into 4 sherds/bricks when broken with a tool without silk touch
    #[must_use]
    pub const fn can_break_pot_shatter(is_tool: bool, has_silk_touch: bool) -> bool {
        is_tool && !has_silk_touch
    }

    /// Checks if the projectile is in Vanilla `EntityTypeTags.IMPACT_PROJECTILES`
    #[must_use]
    pub fn is_impact_projectile(entity_type: &EntityType) -> bool {
        tag::EntityType::MINECRAFT_IMPACT_PROJECTILES
            .1
            .contains(&entity_type.id)
    }

    /// Checks if the item is in Vanilla `ItemTags.BREAKS_DECORATED_POTS`
    #[must_use]
    pub fn is_breaks_decorated_pot_tool(item: &Item) -> bool {
        tag::Item::MINECRAFT_BREAKS_DECORATED_POTS.1.contains(&item.id)
    }

    /// Drops the 4 constituent sherds or bricks when the pot shatters.
    pub fn drop_pot_sherds(world: &Arc<World>, position: &BlockPos) {
        let sherds = if let Some(block_entity) = world.get_block_entity(position)
            && let Some(pot) = block_entity.as_any().downcast_ref::<DecoratedPotBlockEntity>()
            && let Some(decorations) = pot.get_decorations()
        {
            let mut items = Vec::new();
            for opt_dec in decorations.decorations {
                if let Some(dec) = opt_dec {
                    items.push(ItemStack::new(1, dec.item));
                } else {
                    items.push(ItemStack::new(1, &Item::BRICK));
                }
            }
            items
        } else {
            vec![ItemStack::new(4, &Item::BRICK)]
        };

        for item in sherds {
            world.drop_stack(position, item);
        }
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
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.update_neighbors(args.position, None);
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
                let pitch = Self::calculate_insert_pitch(count, max_stack);
                args.world.play_sound_fine(
                    Sound::BlockDecoratedPotInsert,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                    1.0,
                    pitch,
                );
                // Positive wobble
                args.world.add_synced_block_event(
                    *args.position,
                    Self::EVENT_POT_WOBBLES as u8,
                    PotWobbleStyle::Positive as u8,
                );
                // Dust plume particles
                let particle_pos = Vector3::new(
                    args.position.0.x as f64 + 0.5,
                    args.position.0.y as f64 + 1.2,
                    args.position.0.z as f64 + 0.5,
                );
                args.world.spawn_particle(
                    particle_pos,
                    Vector3::new(0.0, 0.0, 0.0),
                    0.0,
                    Self::INSERT_DUST_PLUME_PARTICLES,
                    Particle::DustPlume,
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
                // Negative wobble on insert fail
                args.world.add_synced_block_event(
                    *args.position,
                    Self::EVENT_POT_WOBBLES as u8,
                    PotWobbleStyle::Negative as u8,
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
        // Negative wobble on empty-hand interact
        args.world.add_synced_block_event(
            *args.position,
            Self::EVENT_POT_WOBBLES as u8,
            PotWobbleStyle::Negative as u8,
        );
        BlockActionResult::Success
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == Self::EVENT_POT_WOBBLES as u8
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        if !Self::is_impact_projectile(args.projectile.get_entity().entity_type) {
            return;
        }

        // Vanilla DecoratedPotBlock.java:210-216: projectile shatters pot
        args.world.play_sound(
            Sound::BlockDecoratedPotShatter,
            SoundCategory::Blocks,
            &args.position.to_f64(),
        );
        Self::drop_pot_sherds(args.world, args.position);
        args.world.set_block_state(
            args.position,
            BlockStateId::AIR,
            BlockFlags::NOTIFY_ALL,
        );
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        let held_item = args.player.inventory().held_item();
        let is_tool = Self::is_breaks_decorated_pot_tool(held_item.item);
        let has_silk_touch = held_item
            .get_enchantment_level(&Enchantment::SILK_TOUCH)
            > 0;

        if Self::can_break_pot_shatter(is_tool, has_silk_touch) {
            args.world.play_sound(
                Sound::BlockDecoratedPotShatter,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
            Self::drop_pot_sherds(args.world, args.position);
        }
        args.world.remove_block_entity(args.position);
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.update_neighbors(args.position, None);
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
