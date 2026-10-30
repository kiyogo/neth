use std::sync::Arc;

use crate::block::{
    CanPlaceAtArgs, EmitsRedstonePowerArgs, GetRedstonePowerArgs, GetStateForNeighborUpdateArgs,
    OnPlaceArgs, OnStateReplacedArgs, blocks::abstract_wall_mounting::WallMountedBlock,
};
use pumpkin_data::{
    Block, BlockDirection, BlockStateId, HorizontalFacingExt,
    block_properties::{AttachFace, LeverLikeProperties},
    game_event::GameEvent,
    sound::{Sound, SoundCategory},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::{
    block::{
        registry::BlockActionResult,
        {BlockBehaviour, NormalUseArgs},
    },
    world::World,
};

fn toggle_lever(world: &Arc<World>, block_pos: &BlockPos) {
    let (block, state) = world.get_block_and_state_id(block_pos);

    let mut lever_props = LeverLikeProperties::from_state_id(state);
    lever_props.powered = !lever_props.powered;
    world.set_block_state(
        block_pos,
        lever_props.to_state_id(block),
        BlockFlags::NOTIFY_ALL,
    );

    LeverBlock::update_neighbors(world, block_pos, lever_props);

    play_lever_sound(world, block_pos, lever_props.powered);

    let game_event = if lever_props.powered {
        GameEvent::BlockActivate
    } else {
        GameEvent::BlockDeactivate
    };
    world.emit_game_event(game_event.name(), block_pos.to_centered_f64());
}

fn play_lever_sound(world: &Arc<World>, block_pos: &BlockPos, powered: bool) {
    world.play_sound_fine(
        Sound::BlockLeverClick,
        SoundCategory::Blocks,
        &block_pos.to_centered_f64(),
        LeverBlock::CLICK_VOLUME,
        LeverBlock::get_click_pitch(powered),
    );
}

#[pumpkin_block("minecraft:lever")]
pub struct LeverBlock;

impl BlockBehaviour for LeverBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        toggle_lever(args.world, args.position);
        BlockActionResult::Success
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LeverLikeProperties::from_state_id(args.state.id);
        if props.powered { Self::POWER_ON } else { Self::POWER_OFF }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = LeverLikeProperties::from_state_id(args.state.id);
        if props.powered && props.get_direction() == args.direction {
            Self::POWER_ON
        } else {
            Self::POWER_OFF
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            let lever_props = LeverLikeProperties::from_state_id(args.old_state_id);
            if lever_props.powered {
                Self::update_neighbors(args.world, args.position, lever_props);
            }
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = LeverLikeProperties::default(args.block);
        (props.face, props.facing) =
            WallMountedBlock::get_placement_face(self, args.player, args.direction);

        props.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        // Use the provided direction, or fallback to the current state's direction if missing
        let direction = args
            .direction
            .unwrap_or_else(|| self.get_direction(args.state.id, args.block));

        WallMountedBlock::can_place_at(self, args.block_accessor, args.position, direction)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        WallMountedBlock::get_state_for_neighbor_update(self, args)
    }
}

impl WallMountedBlock for LeverBlock {
    fn get_direction(&self, state_id: BlockStateId, _block: &Block) -> BlockDirection {
        let props = LeverLikeProperties::from_state_id(state_id);
        match props.face {
            AttachFace::Floor => BlockDirection::Up,
            AttachFace::Ceiling => BlockDirection::Down,
            AttachFace::Wall => props.facing.to_block_direction(),
        }
    }
}

impl LeverBlock {
    pub const POWER_ON: u8 = 15;
    pub const POWER_OFF: u8 = 0;
    pub const CLICK_VOLUME: f32 = 0.3;
    pub const CLICK_PITCH_ON: f32 = 0.6;
    pub const CLICK_PITCH_OFF: f32 = 0.5;
    pub const DUST_COLOR_RED: i32 = 16711680;

    #[must_use]
    pub const fn get_click_pitch(powered: bool) -> f32 {
        if powered {
            Self::CLICK_PITCH_ON
        } else {
            Self::CLICK_PITCH_OFF
        }
    }

    fn update_neighbors(
        world: &Arc<World>,
        block_pos: &BlockPos,
        lever_props: LeverLikeProperties,
    ) {
        let direction = lever_props.get_direction().opposite();
        world.update_neighbors(block_pos, None);
        world.update_neighbors(&block_pos.offset(direction.to_offset()), None);
    }
}

pub trait LeverLikePropertiesExt {
    fn get_direction(&self) -> BlockDirection;
}

impl LeverLikePropertiesExt for LeverLikeProperties {
    fn get_direction(&self) -> BlockDirection {
        match self.face {
            AttachFace::Ceiling => BlockDirection::Down,
            AttachFace::Floor => BlockDirection::Up,
            AttachFace::Wall => self.facing.to_block_direction(),
        }
    }
}
