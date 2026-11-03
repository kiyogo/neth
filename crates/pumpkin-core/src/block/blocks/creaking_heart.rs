use std::sync::Arc;

use pumpkin_data::block_properties::{Axis, CreakingHeartLikeProperties, CreakingHeartState};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{BlockId, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::creaking_heart::CreakingHeartBlockEntity;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetComparatorOutputArgs, OnNeighborUpdateArgs, OnPlaceArgs,
    PathComputationType, PlacedArgs,
};

#[pumpkin_block("minecraft:creaking_heart")]
pub struct CreakingHeartBlock;

impl CreakingHeartBlock {
    // Vanilla CreakingHeartBlock.java:44-58, 179 & CreakingHeartBlockEntity.java:44-59 parity constants
    pub const NATURAL_BREAK_MIN_XP: u32 = crate::entity::combat::CREAKING_HEART_NATURAL_BREAK_MIN_XP;
    pub const NATURAL_BREAK_MAX_XP: u32 = crate::entity::combat::CREAKING_HEART_NATURAL_BREAK_MAX_XP;
    pub const COMPARATOR_MAX_DISTANCE: f64 = crate::entity::combat::CREAKING_HEART_COMPARATOR_MAX_DISTANCE;
    pub const COMPARATOR_MAX_SIGNAL: i32 = crate::entity::combat::CREAKING_HEART_COMPARATOR_MAX_SIGNAL;
    pub const RESIN_CLUMPS_MIN: i32 = crate::entity::combat::CREAKING_HEART_RESIN_CLUMPS_MIN;
    pub const RESIN_CLUMPS_MAX: i32 = crate::entity::combat::CREAKING_HEART_RESIN_CLUMPS_MAX;
    pub const HURT_CALL_TOTAL_TICKS: i32 = crate::entity::combat::CREAKING_HEART_HURT_CALL_TOTAL_TICKS;
    pub const HURT_CALL_INTERVAL: i32 = crate::entity::combat::CREAKING_HEART_HURT_CALL_INTERVAL;
    pub const HURT_CALL_PARTICLE_TICKS: i32 = crate::entity::combat::CREAKING_HEART_HURT_CALL_PARTICLE_TICKS;
    pub const DISTANCE_CREAKING_TOO_FAR: f64 = crate::entity::combat::CREAKING_HEART_DISTANCE_CREAKING_TOO_FAR;
    pub const PLAYER_DETECTION_RANGE: f64 = crate::entity::combat::CREAKING_HEART_PLAYER_DETECTION_RANGE;
    pub const TICKS_GRACE_PERIOD: i32 = crate::entity::combat::CREAKING_HEART_TICKS_GRACE_PERIOD;
    pub const TRAIL_COLOR_TOWARDS_CREAKING: i32 = crate::entity::combat::CREAKING_HEART_TRAIL_COLOR_TOWARDS_CREAKING;
    pub const TRAIL_COLOR_TOWARDS_HEART: i32 = crate::entity::combat::CREAKING_HEART_TRAIL_COLOR_TOWARDS_HEART;

    pub const fn is_pale_oak_log(id: BlockId) -> bool {
        matches!(
            id,
            BlockId::PALE_OAK_LOG
                | BlockId::PALE_OAK_WOOD
                | BlockId::STRIPPED_PALE_OAK_LOG
                | BlockId::STRIPPED_PALE_OAK_WOOD
        )
    }

    pub fn check_active_logs(
        world: &dyn pumpkin_world::world::BlockAccessor,
        pos: &pumpkin_util::math::position::BlockPos,
        axis: Axis,
    ) -> bool {
        let (pos_a, pos_b) = match axis {
            Axis::X => (pos.west(), pos.east()),
            Axis::Y => (pos.down(), pos.up()),
            Axis::Z => (pos.north(), pos.south()),
        };

        let block_a = world.get_block_state_id(&pos_a).to_block_id();
        let block_b = world.get_block_state_id(&pos_b).to_block_id();

        Self::is_pale_oak_log(block_a) && Self::is_pale_oak_log(block_b)
    }

    #[must_use]
    pub fn calculate_analog_output_signal(distance: Option<f64>) -> i32 {
        crate::entity::combat::calculate_creaking_heart_analog_signal(distance)
    }

    #[must_use]
    pub fn calculate_hurt_audio_progress(emitter: i32) -> f32 {
        crate::entity::combat::calculate_creaking_heart_hurt_progress(emitter)
    }

    #[must_use]
    pub fn calculate_hurt_audio_volume(emitter: i32) -> f32 {
        crate::entity::combat::calculate_creaking_heart_hurt_volume(emitter)
    }

    #[must_use]
    pub const fn is_creaking_too_far(distance: f64) -> bool {
        crate::entity::combat::is_creaking_too_far_from_heart(distance)
    }

    #[must_use]
    pub const fn should_play_hurt_call(emitter: i32) -> bool {
        emitter > 0 && emitter % Self::HURT_CALL_INTERVAL == 0
    }

    #[must_use]
    pub const fn should_emit_dual_particles(emitter: i32) -> bool {
        emitter > Self::HURT_CALL_PARTICLE_TICKS
    }
}

impl BlockBehaviour for CreakingHeartBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = CreakingHeartLikeProperties::from_state_id(args.block.default_state.id);
        props.axis = match args.direction {
            pumpkin_data::BlockDirection::North | pumpkin_data::BlockDirection::South => Axis::Z,
            pumpkin_data::BlockDirection::East | pumpkin_data::BlockDirection::West => Axis::X,
            pumpkin_data::BlockDirection::Up | pumpkin_data::BlockDirection::Down => Axis::Y,
        };
        props.creaking_heart_state = CreakingHeartState::Uprooted;
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let entity = CreakingHeartBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
            args.world.update_neighbour_for_output_signal(args.position, args.block);

            let state_id = args.world.get_block_state_id(args.position);
            let mut props = CreakingHeartLikeProperties::from_state_id(state_id);

            if Self::check_active_logs(args.world.as_ref(), args.position, props.axis) {
                props.creaking_heart_state = CreakingHeartState::Dormant;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                );

                args.world.play_sound(
                    Sound::BlockCreakingHeartSpawn,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
            }
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        {
            let state_id = args.world.get_block_state_id(args.position);
            let mut props = CreakingHeartLikeProperties::from_state_id(state_id);

            let active_logs =
                Self::check_active_logs(args.world.as_ref(), args.position, props.axis);
            let new_state = if active_logs {
                if props.creaking_heart_state == CreakingHeartState::Uprooted {
                    args.world.play_sound(
                        Sound::BlockCreakingHeartSpawn,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                }
                CreakingHeartState::Dormant
            } else {
                CreakingHeartState::Uprooted
            };

            if props.creaking_heart_state != new_state {
                props.creaking_heart_state = new_state;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                );
                args.world.update_neighbour_for_output_signal(args.position, args.block);
            }
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.play_sound(
            Sound::BlockCreakingHeartBreak,
            SoundCategory::Blocks,
            &args.position.to_f64(),
        );

        // Vanilla CreakingHeartBlock.java:178-180: natural heart drops 20..=24 XP
        let props = CreakingHeartLikeProperties::from_state_id(args.state.id);
        if props.natural {
            let xp = rand::random_range(Self::NATURAL_BREAK_MIN_XP..=Self::NATURAL_BREAK_MAX_XP);
            crate::entity::experience_orb::ExperienceOrbEntity::spawn(
                args.world,
                args.position.to_centered_f64(),
                xp,
            );
        }
        args.world.remove_block_entity(args.position);
        args.world.update_neighbour_for_output_signal(args.position, args.block);
        args.world.update_neighbors(args.position, None);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        let props = CreakingHeartLikeProperties::from_state_id(args.state.id);
        if props.creaking_heart_state == CreakingHeartState::Uprooted {
            return Some(0);
        }

        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(heart_entity) = block_entity
                .as_any()
                .downcast_ref::<CreakingHeartBlockEntity>()
        {
            let signal = heart_entity
                .output_signal
                .load(std::sync::atomic::Ordering::Relaxed);
            Some(signal.clamp(0, 15) as u8)
        } else {
            Some(0)
        }
    }

    fn is_pathfindable(&self, _state: &pumpkin_data::BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
