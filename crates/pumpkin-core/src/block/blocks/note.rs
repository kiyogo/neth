use crate::block::registry::BlockActionResult;
use crate::block::{
    AttackArgs, GetStateForNeighborUpdateArgs, NormalUseArgs, OnNeighborUpdateArgs, OnPlaceArgs,
    UseWithItemArgs,
};
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{Axis, NoteblockInstrument};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockDirection, block_properties::NoteBlockLikeProperties};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::{
    block::{BlockBehaviour, OnSyncedBlockEventArgs},
    world::World,
};

use super::redstone::block_receives_redstone_power;

#[pumpkin_block("minecraft:note_block")]
pub struct NoteBlock;

impl NoteBlock {
    pub const NOTE_VOLUME: f32 = 3.0;
    pub const MIN_NOTE: u8 = 0;
    pub const MAX_NOTE: u8 = 24;
    pub const NUM_NOTES: u8 = 25;
    pub const DEFAULT_NOTE: u8 = 0;
    pub const DEFAULT_POWERED: bool = false;
    pub const DEFAULT_INSTRUMENT: NoteblockInstrument = NoteblockInstrument::Harp;

    pub fn play_note(props: &NoteBlockLikeProperties, world: &World, pos: &BlockPos) {
        if !is_base_block(props.instrument) || world.get_block_state(&pos.up()).is_air() {
            let mut event = crate::plugin::api::events::block::note_play::NotePlayEvent::new(
                *pos,
                format!("{:?}", props.instrument),
                props.note,
            );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                return;
            }
            world.add_synced_block_event(*pos, 0, 0);
        }
    }

    #[must_use]
    pub fn get_pitch_from_note(note: u8) -> f32 {
        ((f32::from(note) - 12.0) / 12.0).exp2()
    }

    #[must_use]
    pub const fn cycle_note(note: u8) -> u8 {
        (note + 1) % Self::NUM_NOTES
    }

    #[must_use]
    pub fn get_particle_color_x(note: u8) -> f32 {
        f32::from(note) / 24.0
    }

    #[must_use]
    pub const fn is_tunable_instrument(instrument: NoteblockInstrument) -> bool {
        is_base_block(instrument)
    }

    fn get_state_with_instrument(
        world: &World,
        pos: &BlockPos,
        state: BlockStateId,
        block: &Block,
    ) -> BlockStateId {
        let upper_instrument = world.get_block_state(&pos.up()).instrument;

        let mut note_props = NoteBlockLikeProperties::from_state_id(state);
        if !is_base_block(upper_instrument) {
            note_props.instrument = upper_instrument;
            return note_props.to_state_id(block);
        }
        let below_instrument = world.get_block_state(&pos.down()).instrument;
        let below_instrument = if is_base_block(below_instrument) {
            below_instrument
        } else {
            NoteblockInstrument::Harp
        };
        note_props.instrument = below_instrument;
        note_props.to_state_id(block)
    }
}

impl BlockBehaviour for NoteBlock {
    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let block_state = args.world.get_block_state(args.position);
        let mut note_props = NoteBlockLikeProperties::from_state_id(block_state.id);
        let powered = block_receives_redstone_power(args.world, args.position);
        // check if powered state changed
        if note_props.powered != powered {
            if powered {
                Self::play_note(&note_props, args.world, args.position);
            }
            note_props.powered = powered;
            args.world.set_block_state(
                args.position,
                note_props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let block_state = args.world.get_block_state(args.position);
        let mut note_props = NoteBlockLikeProperties::from_state_id(block_state.id);
        note_props.note = Self::cycle_note(note_props.note);
        args.world.set_block_state(
            args.position,
            note_props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
        Self::play_note(&note_props, args.world, args.position);

        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Custom,
            pumpkin_data::statistic::CustomStatistic::TuneNoteblock as i32,
            1,
        );

        BlockActionResult::Success
    }

    fn attack(&self, args: AttackArgs<'_>) {
        let note_props = NoteBlockLikeProperties::from_state_id(args.state.id);
        Self::play_note(&note_props, args.world, args.position);
        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Custom,
            pumpkin_data::statistic::CustomStatistic::PlayNoteblock as i32,
            1,
        );
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        // NoteBlock.useItemOn: heads clicked onto the top get placed instead of tuning the block
        if args.hit.face == &BlockDirection::Up
            && args
                .item_stack
                .item
                .has_tag(&tag::Item::MINECRAFT_NOTEBLOCK_TOP_INSTRUMENTS)
        {
            return BlockActionResult::Pass;
        }
        BlockActionResult::PassToDefaultBlockAction
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        let block_state = args.world.get_block_state(args.position);
        let note_props = NoteBlockLikeProperties::from_state_id(block_state.id);
        let instrument = note_props.instrument;
        let pitch = if is_base_block(instrument) {
            // checks if can be pitched
            Self::get_pitch_from_note(note_props.note)
        } else {
            1.0 // default pitch
        };
        // check hasCustomSound
        args.world.play_sound_raw(
            convert_instrument_to_sound(instrument) as u16,
            SoundCategory::Records,
            &args.position.to_f64(),
            3.0,
            pitch,
        );
        if is_base_block(instrument) {
            let note_pos = pumpkin_util::math::vector3::Vector3::new(
                args.position.0.x as f64 + 0.5,
                args.position.0.y as f64 + 1.2,
                args.position.0.z as f64 + 0.5,
            );
            let color_offset = pumpkin_util::math::vector3::Vector3::new(
                Self::get_particle_color_x(note_props.note),
                0.0,
                0.0,
            );
            args.world.spawn_particle(
                note_pos,
                color_offset,
                0.0,
                0,
                pumpkin_data::particle::Particle::Note,
            );
        }
        true
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        Self::get_state_with_instrument(
            args.world,
            args.position,
            Block::NOTE_BLOCK.default_state.id,
            args.block,
        )
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.direction.to_axis() == Axis::Y {
            return Self::get_state_with_instrument(
                args.world,
                args.position,
                args.state_id,
                args.block,
            );
        }
        args.state_id
    }
}

const fn convert_instrument_to_sound(instrument: NoteblockInstrument) -> Sound {
    match instrument {
        NoteblockInstrument::Harp => Sound::BlockNoteBlockHarp,
        NoteblockInstrument::Basedrum => Sound::BlockNoteBlockBasedrum,
        NoteblockInstrument::Snare => Sound::BlockNoteBlockSnare,
        NoteblockInstrument::Hat => Sound::BlockNoteBlockHat,
        NoteblockInstrument::Bass => Sound::BlockNoteBlockBass,
        NoteblockInstrument::Flute => Sound::BlockNoteBlockFlute,
        NoteblockInstrument::Bell => Sound::BlockNoteBlockBell,
        NoteblockInstrument::Guitar => Sound::BlockNoteBlockGuitar,
        NoteblockInstrument::Chime => Sound::BlockNoteBlockChime,
        NoteblockInstrument::Xylophone => Sound::BlockNoteBlockXylophone,
        NoteblockInstrument::IronXylophone => Sound::BlockNoteBlockIronXylophone,
        NoteblockInstrument::CowBell => Sound::BlockNoteBlockCowBell,
        NoteblockInstrument::Didgeridoo => Sound::BlockNoteBlockDidgeridoo,
        NoteblockInstrument::Bit => Sound::BlockNoteBlockBit,
        NoteblockInstrument::Banjo => Sound::BlockNoteBlockBanjo,
        NoteblockInstrument::Pling => Sound::BlockNoteBlockPling,
        NoteblockInstrument::Zombie => Sound::BlockNoteBlockImitateZombie,
        NoteblockInstrument::Skeleton => Sound::BlockNoteBlockImitateSkeleton,
        NoteblockInstrument::Creeper => Sound::BlockNoteBlockImitateCreeper,
        NoteblockInstrument::Dragon => Sound::BlockNoteBlockImitateEnderDragon,
        NoteblockInstrument::WitherSkeleton => Sound::BlockNoteBlockImitateWitherSkeleton,
        NoteblockInstrument::Piglin => Sound::BlockNoteBlockImitatePiglin,
        NoteblockInstrument::CustomHead => Sound::UiButtonClick,
        NoteblockInstrument::Trumpet => Sound::BlockNoteBlockTrumpet,
        NoteblockInstrument::TrumpetExposed => Sound::BlockNoteBlockTrumpetExposed,
        NoteblockInstrument::TrumpetOxidized => Sound::BlockNoteBlockTrumpetOxidized,
        NoteblockInstrument::TrumpetWeathered => Sound::BlockNoteBlockTrumpetWeathered,
    }
}

pub const fn is_base_block(instrument: NoteblockInstrument) -> bool {
    matches!(
        instrument,
        NoteblockInstrument::Harp
            | NoteblockInstrument::Basedrum
            | NoteblockInstrument::Snare
            | NoteblockInstrument::Hat
            | NoteblockInstrument::Bass
            | NoteblockInstrument::Flute
            | NoteblockInstrument::Bell
            | NoteblockInstrument::Guitar
            | NoteblockInstrument::Chime
            | NoteblockInstrument::Xylophone
            | NoteblockInstrument::IronXylophone
            | NoteblockInstrument::CowBell
            | NoteblockInstrument::Didgeridoo
            | NoteblockInstrument::Bit
            | NoteblockInstrument::Banjo
            | NoteblockInstrument::Pling
            | NoteblockInstrument::Trumpet
            | NoteblockInstrument::TrumpetExposed
            | NoteblockInstrument::TrumpetOxidized
            | NoteblockInstrument::TrumpetWeathered
    )
}
