use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId, Enchantment,
    block_properties::CampfireLikeProperties,
    damage::DamageType,
    data_component_impl::EquipmentSlot,
    effect::StatusEffect,
    fluid::Fluid,
    recipes::{CookingRecipeKind, get_cooking_recipe_with_ingredient},
    tag::{self, Taggable},
    game_event::GameEvent, world::WorldEvent,
};
use pumpkin_util::math::vector3::Vector3;
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_world::tick::TickPriority;

use pumpkin_world::world::BlockFlags;

use crate::block::entities::campfire::CampfireBlockEntity;
use crate::{
    block::{
        BlockBehaviour, BlockIsReplacing, GetStateForNeighborUpdateArgs, OnEntityCollisionArgs,
        OnPlaceArgs, OnProjectileHitArgs, PathComputationType, PlacedArgs, UseWithItemArgs,
        registry::BlockActionResult,
    },
    entity::EntityBase,
};
use std::sync::Arc;

#[pumpkin_block_from_tag("minecraft:campfires")]
pub struct CampfireBlock;

impl CampfireBlock {
    pub const DEFAULT_COOKING_TIME: i32 = 600;
    pub const BURN_COOL_SPEED: i32 = 2;
    pub const SMOKE_HEIGHT_REGULAR: i32 = 10;
    pub const SMOKE_HEIGHT_SIGNAL: i32 = 24;
    pub const FIRE_DAMAGE_REGULAR: f32 = 1.0;
    pub const FIRE_DAMAGE_SOUL: f32 = 2.0;
    pub const EVENT_EXTINGUISH_FIRE: i32 = 1009;

    #[must_use]
    pub fn douse_campfire_state(state_id: BlockStateId, block: &Block) -> BlockStateId {
        let mut props = CampfireLikeProperties::from_state_id(state_id);
        props.lit = false;
        props.to_state_id(block)
    }

    #[must_use]
    pub fn get_fire_damage(is_soul: bool) -> f32 {
        if is_soul {
            Self::FIRE_DAMAGE_SOUL
        } else {
            Self::FIRE_DAMAGE_REGULAR
        }
    }

    #[must_use]
    pub fn get_smoke_height(signal_fire: bool) -> i32 {
        if signal_fire {
            Self::SMOKE_HEIGHT_SIGNAL
        } else {
            Self::SMOKE_HEIGHT_REGULAR
        }
    }

    #[must_use]
    pub fn cool_progress(progress: i32) -> i32 {
        (progress - Self::BURN_COOL_SPEED).max(0)
    }
}

impl BlockBehaviour for CampfireBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        let entity = CampfireBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(entity));
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        let props = CampfireLikeProperties::from_state_id(state.id);
        if !props.lit {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        // Vanilla CampfireBlock.useItemOn: first check campfire cooking recipes
        if let Some(recipe) = get_cooking_recipe_with_ingredient(
            args.item_stack.item,
            CookingRecipeKind::CampfireCooking,
        ) {
            if let Some(block_entity) = args.world.get_block_entity(args.position) {
                if let Some(campfire) = block_entity.as_any().downcast_ref::<CampfireBlockEntity>() {
                    for slot in 0..CampfireBlockEntity::SLOT_COUNT {
                        let mut stored = campfire.items[slot]
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if !stored.is_empty() {
                            continue;
                        }

                        *stored = args
                            .item_stack
                            .split_unless_creative(args.player.gamemode.load(), 1);
                        *campfire.cooking_times[slot]
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = 0;
                        *campfire.cooking_total_times[slot]
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = recipe.cookingtime;
                        drop(stored);

                        args.player.increment_stat(
                            pumpkin_data::statistic::StatisticCategory::Custom,
                            pumpkin_data::statistic::CustomStatistic::InteractWithCampfire as i32,
                            1,
                        );
                        args.world.update_block_entity(&block_entity);
                        return BlockActionResult::Success;
                    }

                    // All slots occupied with food, consume interaction
                    return BlockActionResult::Consume;
                }
            }
        }

        // Vanilla CampfireBlock.useItemOn: dousing campfire with shovels (ItemTags.DOUSES_CAMPFIRES)
        if args.item_stack.item.has_tag(&tag::Item::MINECRAFT_DOUSES_CAMPFIRES) {
            args.world.sync_world_event(
                WorldEvent::SoundExtinguishFire,
                *args.position,
                0,
            );
            let mut unlit_props = props;
            unlit_props.lit = false;
            args.world.set_block_state(
                args.position,
                unlit_props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            args.world.emit_game_event(
                GameEvent::BlockChange.name(),
                Vector3::new(
                    f64::from(args.position.0.x) + 0.5,
                    f64::from(args.position.0.y) + 0.5,
                    f64::from(args.position.0.z) + 0.5,
                ),
            );
            args.player.damage_item_in_slot(args.equipment_slot, 1);
            return BlockActionResult::Success;
        }

        BlockActionResult::PassToDefaultBlockAction
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if CampfireLikeProperties::from_state_id(args.state.id).lit
            && let Some(living_entity) = args.entity.get_living_entity()
        {
            let has_frost_walker_enchantment = {
                let equipment = living_entity
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equipment
                    .equipment
                    .get(&EquipmentSlot::FEET)
                    .is_some_and(|boots| {
                        boots.get_enchantment_level(&Enchantment::FROST_WALKER) != 0
                    })
            };
            let has_fire_res = living_entity
                .get_effect(&StatusEffect::FIRE_RESISTANCE)
                .is_some();
            if has_frost_walker_enchantment || has_fire_res {
                // Campfire damage is prevented by Frost Walker boots or fire resistance.
                return;
            }
            let damage_amount = Self::get_fire_damage(args.block == &Block::SOUL_CAMPFIRE);
            args.entity
                .damage(args.entity, damage_amount, DamageType::CAMPFIRE);
        }
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        let props = CampfireLikeProperties::from_state_id(args.state.id);
        if !args.projectile.get_entity().is_on_fire() || props.lit || props.waterlogged {
            return;
        }

        let mut lit_props = props;
        lit_props.lit = true;
        args.world.set_block_state(
            args.position,
            lit_props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let is_replacing_water = matches!(args.replacing, BlockIsReplacing::Water(_));
        let mut props = CampfireLikeProperties::from_state_id(args.block.default_state.id);
        props.waterlogged = is_replacing_water;
        props.signal_fire = is_signal_fire_base_block(args.world.get_block(&args.position.down()));
        props.lit = !is_replacing_water;
        props.facing = args.player.get_entity().get_horizontal_facing();
        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut props = CampfireLikeProperties::from_state_id(args.state_id);
        if props.waterlogged {
            props.lit = false;
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed,
                TickPriority::Normal,
            );
        }

        if args.direction == BlockDirection::Down {
            props.signal_fire =
                is_signal_fire_base_block(args.world.get_block(args.neighbor_position));
        }

        props.to_state_id(args.block)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn is_signal_fire_base_block(block: &Block) -> bool {
    block == &Block::HAY_BLOCK
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::block_properties::HorizontalFacing;

    fn campfire_state(block: &Block, lit: bool, waterlogged: bool) -> BlockStateId {
        let mut props = CampfireLikeProperties::default(block);
        props.lit = lit;
        props.waterlogged = waterlogged;
        props.to_state_id(block)
    }

    fn relit_state_id(
        block: &Block,
        state_id: BlockStateId,
        is_on_fire: bool,
    ) -> Option<BlockStateId> {
        let props = CampfireLikeProperties::from_state_id(state_id);
        if !is_on_fire || props.lit || props.waterlogged {
            return None;
        }
        let mut lit_props = props;
        lit_props.lit = true;
        Some(lit_props.to_state_id(block))
    }

    #[test]
    fn burning_projectile_relights_unlit_campfire() {
        for block in [&Block::CAMPFIRE, &Block::SOUL_CAMPFIRE] {
            let state_id = campfire_state(block, false, false);
            let new_state_id =
                relit_state_id(block, state_id, true).expect("campfire should relight");

            assert!(CampfireLikeProperties::from_state_id(new_state_id).lit);
            assert_eq!(Block::from_state_id(new_state_id), block);
        }
    }

    #[test]
    fn relighting_keeps_facing_and_signal_fire() {
        let mut props = CampfireLikeProperties::default(&Block::CAMPFIRE);
        props.lit = false;
        props.signal_fire = true;
        props.facing = HorizontalFacing::West;
        let state_id = props.to_state_id(&Block::CAMPFIRE);

        let new_state_id =
            relit_state_id(&Block::CAMPFIRE, state_id, true).expect("campfire should relight");
        let new_props = CampfireLikeProperties::from_state_id(new_state_id);

        assert!(new_props.lit);
        assert!(new_props.signal_fire);
        assert_eq!(new_props.facing, HorizontalFacing::West);
    }

    #[test]
    fn campfire_does_not_relight_unless_unlit_and_dry() {
        let waterlogged = campfire_state(&Block::CAMPFIRE, false, true);
        assert!(relit_state_id(&Block::CAMPFIRE, waterlogged, true).is_none());

        let lit = campfire_state(&Block::CAMPFIRE, true, false);
        assert!(relit_state_id(&Block::CAMPFIRE, lit, true).is_none());

        let unlit = campfire_state(&Block::CAMPFIRE, false, false);
        assert!(relit_state_id(&Block::CAMPFIRE, unlit, false).is_none());
    }
}
