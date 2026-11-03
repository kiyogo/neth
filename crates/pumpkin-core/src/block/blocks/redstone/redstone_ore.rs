use crate::block::{
    BlockBehaviour, BlockMetadata, BrokenArgs, OnEntityCollisionArgs, OnEntityStepArgs, PlacedArgs,
    RandomTickArgs, UseWithItemArgs, registry::BlockActionResult,
};
use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::world::World;
use pumpkin_data::block_properties::RedstoneOreLikeProperties;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::{Block, BlockId, BlockState};
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::server::play::SUseItemOn;
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

pub struct RedstoneOreBlock;

impl BlockMetadata for RedstoneOreBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::REDSTONE_ORE, BlockId::DEEPSLATE_REDSTONE_ORE].into()
    }
}

impl RedstoneOreBlock {
    pub const MIN_XP: u32 = 1;
    pub const MAX_XP: u32 = 5;
    pub const LIGHT_LEVEL_LIT: u8 = 9;
    pub const LIGHT_LEVEL_UNLIT: u8 = 0;
    pub const DEFAULT_LIT: bool = false;
    pub const PARTICLE_OFFSET: f64 = 0.5625;

    #[must_use]
    pub const fn get_light_emission(lit: bool) -> u8 {
        if lit {
            Self::LIGHT_LEVEL_LIT
        } else {
            Self::LIGHT_LEVEL_UNLIT
        }
    }

    fn light_up(world: &Arc<World>, pos: &BlockPos, block: &Block, state: &BlockState) {
        let mut props = RedstoneOreLikeProperties::from_state_id(state.id);
        if !props.lit {
            props.lit = true;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
        }
    }
}

impl BlockBehaviour for RedstoneOreBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn broken(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative {
            let xp_count = rand::random_range(Self::MIN_XP..=Self::MAX_XP);
            ExperienceOrbEntity::spawn(args.world, args.position.to_centered_f64(), xp_count);
        }
        args.world.update_neighbors(args.position, None);
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        Self::light_up(args.world, args.position, args.block, state);
        if args.item_stack.is_empty() {
            return BlockActionResult::Success;
        }
        let Some(placed_block) = Block::from_item_id(args.item_stack.item.id) else {
            return BlockActionResult::Success;
        };
        // Vanilla only passes a block item on when BlockPlaceContext.canPlace() holds; otherwise the use is consumed.
        let use_item_on = SUseItemOn {
            hand: VarInt(i32::from(*args.equipment_slot == EquipmentSlot::OFF_HAND)),
            position: *args.position,
            face: VarInt(i32::from(args.hit.face.to_index())),
            cursor_pos: *args.hit.cursor_pos,
            inside_block: false,
            is_against_world_border: false,
            sequence: VarInt(0),
        };
        if args
            .server
            .block_registry
            .placement_target(
                args.world,
                args.player,
                placed_block,
                &use_item_on,
                *args.position,
                *args.hit.face,
            )
            .is_some()
        {
            BlockActionResult::Pass
        } else {
            BlockActionResult::Success
        }
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        Self::light_up(args.world, args.position, args.block, state);
    }

    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        Self::light_up(args.world, args.position, args.block, state);
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = RedstoneOreLikeProperties::from_state_id(state.id);

        if props.lit {
            props.lit = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}
