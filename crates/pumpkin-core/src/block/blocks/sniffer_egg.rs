use pumpkin_data::block_properties::SnifferEggLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use uuid::Uuid;

use crate::block::{
    BlockBehaviour, BrokenArgs, OnPlaceArgs, OnScheduledTickArgs, PathComputationType, PlacedArgs,
};
use crate::entity::r#type::from_type;

#[pumpkin_block("minecraft:sniffer_egg")]
pub struct SnifferEggBlock;

impl SnifferEggBlock {
    pub const MAX_HATCH_LEVEL: i32 = 2;
    pub const REGULAR_HATCH_TIME_TICKS: u32 = 24000;
    pub const BOOSTED_HATCH_TIME_TICKS: u32 = 12000;
    pub const RANDOM_HATCH_OFFSET_TICKS: u32 = 300;
    pub const EVENT_SNIFFER_EGG_BOOST: u32 = 3009;
    pub const BABY_SNIFFER_AGE: i32 = -24000;

    #[must_use]
    pub fn hatch_boost(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        world
            .get_block(&pos.down())
            .has_tag(&tag::Block::MINECRAFT_SNIFFER_EGG_HATCH_BOOST)
    }

    #[must_use]
    pub const fn get_hatch_delay(boosted: bool, random_offset: u32) -> u32 {
        let hatch_time = if boosted {
            Self::BOOSTED_HATCH_TIME_TICKS
        } else {
            Self::REGULAR_HATCH_TIME_TICKS
        };
        (hatch_time / 3) + random_offset
    }
}

impl BlockBehaviour for SnifferEggBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let props = SnifferEggLikeProperties::default(args.block);
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let boosted = Self::hatch_boost(args.world.as_ref(), args.position);
        if boosted {
            args.world.sync_world_event(
                WorldEvent::ParticlesEggCrack,
                *args.position,
                0,
            );
        }

        let random_offset = rand::random::<u32>() % Self::RANDOM_HATCH_OFFSET_TICKS;
        let delay = Self::get_hatch_delay(boosted, random_offset);
        args.world
            .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = SnifferEggLikeProperties::from_state_id(state_id);

        if i32::from(props.hatch) < Self::MAX_HATCH_LEVEL {
            props.hatch += 1;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );

            let pitch = 0.9 + rand::random::<f32>() * 0.2;
            args.world.play_sound_fine(
                Sound::BlockSnifferEggCrack,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
                0.7,
                pitch,
            );

            let boosted = Self::hatch_boost(args.world.as_ref(), args.position);
            let random_offset = rand::random::<u32>() % Self::RANDOM_HATCH_OFFSET_TICKS;
            let delay = Self::get_hatch_delay(boosted, random_offset);
            args.world
                .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
        } else {
            args.world
                .break_block(args.position, None, BlockFlags::SKIP_DROPS);

            let pitch = 0.9 + rand::random::<f32>() * 0.2;
            args.world.play_sound_fine(
                Sound::BlockSnifferEggHatch,
                SoundCategory::Blocks,
                &args.position.to_centered_f64(),
                0.7,
                pitch,
            );

            if args.world.level_info.load().game_rules.spawn_mobs {
                let spawn_pos = args.position.to_centered_f64();
                let sniffer =
                    from_type(&EntityType::SNIFFER, spawn_pos, args.world, Uuid::new_v4());
                sniffer.get_entity().set_age(Self::BABY_SNIFFER_AGE);
                args.world.spawn_entity_non_save(sniffer);
            }
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        let pitch = 0.9 + rand::random::<f32>() * 0.2;
        args.world.play_sound_fine(
            Sound::BlockSnifferEggCrack,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
            0.7,
            pitch,
        );
        args.world
            .drop_stack(args.position, ItemStack::new(1, &Item::SNIFFER_EGG));
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
