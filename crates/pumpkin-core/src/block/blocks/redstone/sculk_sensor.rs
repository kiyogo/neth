use std::sync::Arc;

use crate::block::entities::calibrated_sculk_sensor::CalibratedSculkSensorBlockEntity;
use crate::block::entities::sculk_sensor::SculkSensorBlockEntity;
use crate::block::{
    BlockBehaviour, BlockMetadata, EmitsRedstonePowerArgs, GetComparatorOutputArgs,
    GetRedstonePowerArgs, OnEntityStepArgs, OnPlaceArgs, OnScheduledTickArgs, PathComputationType,
    PlacedArgs,
};
use crate::entity::EntityBase;
use crate::world::World;
use pumpkin_data::block_properties::{
    CalibratedSculkSensorLikeProperties, HorizontalFacing, SculkSensorLikeProperties,
    SculkSensorPhase,
};
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

pub struct SculkSensorBlock;

impl BlockMetadata for SculkSensorBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_SENSOR, BlockId::CALIBRATED_SCULK_SENSOR].into()
    }
}

const fn horizontal_facing_to_dir(facing: HorizontalFacing) -> BlockDirection {
    match facing {
        HorizontalFacing::North => BlockDirection::North,
        HorizontalFacing::South => BlockDirection::South,
        HorizontalFacing::West => BlockDirection::West,
        HorizontalFacing::East => BlockDirection::East,
    }
}

/// Both sensor variants carry the same phase property under different types.
fn sculk_sensor_phase(block: &Block, state_id: BlockStateId) -> SculkSensorPhase {
    if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
        CalibratedSculkSensorLikeProperties::from_state_id(state_id).sculk_sensor_phase
    } else {
        SculkSensorLikeProperties::from_state_id(state_id).sculk_sensor_phase
    }
}

impl SculkSensorBlock {
    pub const ACTIVE_TICKS: u32 = 30;
    pub const ACTIVE_TICKS_REGULAR: u32 = 30;
    pub const ACTIVE_TICKS_CALIBRATED: u32 = 10;
    pub const COOLDOWN_TICKS: u32 = 10;
    pub const LISTENER_RANGE_REGULAR: u32 = 8;
    pub const LISTENER_RANGE_CALIBRATED: u32 = 16;
    pub const DEFAULT_PHASE: SculkSensorPhase = SculkSensorPhase::Inactive;
    pub const DEFAULT_POWER: u8 = 0;
    pub const DEFAULT_WATERLOGGED: bool = false;
    pub const DEFAULT_FACING: HorizontalFacing = HorizontalFacing::North;
    pub const TONE_MAP: [u8; 16] = [0, 0, 2, 4, 6, 7, 9, 10, 12, 14, 15, 18, 19, 21, 22, 24];

    #[must_use]
    pub fn get_resonance_pitch_for_frequency(frequency: u8) -> f32 {
        let note = Self::TONE_MAP[(frequency as usize).min(15)];
        crate::block::blocks::note::NoteBlock::get_pitch_from_note(note)
    }

    #[must_use]
    pub const fn get_phase_transition_on_tick(
        current_phase: SculkSensorPhase,
    ) -> (SculkSensorPhase, u8, Option<u32>) {
        match current_phase {
            SculkSensorPhase::Active => (SculkSensorPhase::Cooldown, 0, Some(Self::COOLDOWN_TICKS)),
            SculkSensorPhase::Cooldown => (SculkSensorPhase::Inactive, 0, None),
            SculkSensorPhase::Inactive => (SculkSensorPhase::Inactive, 0, None),
        }
    }

    #[must_use]
    pub fn get_active_ticks(block_id: BlockId) -> u32 {
        if block_id == BlockId::CALIBRATED_SCULK_SENSOR {
            Self::ACTIVE_TICKS_CALIBRATED
        } else {
            Self::ACTIVE_TICKS_REGULAR
        }
    }

    #[must_use]
    pub fn get_listener_range(block_id: BlockId) -> u32 {
        if block_id == BlockId::CALIBRATED_SCULK_SENSOR {
            Self::LISTENER_RANGE_CALIBRATED
        } else {
            Self::LISTENER_RANGE_REGULAR
        }
    }

    #[must_use]
    pub fn get_redstone_strength_for_distance(distance: f32, listener_radius: u32) -> u8 {
        let power_scale = 15.0 / listener_radius as f64;
        let power = 15.0 - (power_scale * distance as f64).floor();
        (power as i32).clamp(1, 15) as u8
    }

    #[must_use]
    pub fn can_output_weak_power(
        block_id: BlockId,
        facing: Option<HorizontalFacing>,
        direction: BlockDirection,
    ) -> bool {
        if block_id == BlockId::CALIBRATED_SCULK_SENSOR {
            if let Some(f) = facing {
                direction != horizontal_facing_to_dir(f)
            } else {
                true
            }
        } else {
            true
        }
    }

    #[must_use]
    pub const fn emits_strong_power(direction: BlockDirection) -> bool {
        matches!(direction, BlockDirection::Up)
    }
    pub fn trigger(world: &Arc<World>, pos: &BlockPos, block: &Block, power: u8) {
        if block.id == BlockId::SCULK_SENSOR {
            let state = world.get_block_state(pos);
            let mut props = SculkSensorLikeProperties::from_state_id(state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Inactive {
                if let Some(be) = world.get_block_entity(pos)
                    && let Some(sensor_be) = be.as_any().downcast_ref::<SculkSensorBlockEntity>()
                {
                    *sensor_be
                        .last_vibration_frequency
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = power as i32;
                }

                props.sculk_sensor_phase = SculkSensorPhase::Active;
                props.power = power;
                world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
                world.update_neighbors(pos, None);
                world.schedule_block_tick(block, *pos, Self::ACTIVE_TICKS_REGULAR, TickPriority::Normal);
                Self::notify_nearby_shriekers(world, pos);
            }
        } else if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let state = world.get_block_state(pos);
            let mut props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Inactive {
                let back_dir = horizontal_facing_to_dir(props.facing).opposite();
                let back_pos = pos.offset(back_dir.to_offset());
                let back_state = world.get_block_state(&back_pos);
                let back_block = Block::from_state_id(back_state.id);

                let calibrated_freq = world
                    .block_registry
                    .get_weak_redstone_power(back_block, world, &back_pos, back_state, back_dir);

                if calibrated_freq > 0 && calibrated_freq != power {
                    return;
                }

                if let Some(be) = world.get_block_entity(pos)
                    && let Some(cal_be) = be
                        .as_any()
                        .downcast_ref::<CalibratedSculkSensorBlockEntity>()
                {
                    *cal_be
                        .last_vibration_frequency
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = power as i32;
                }

                props.sculk_sensor_phase = SculkSensorPhase::Active;
                props.power = power;
                world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
                world.update_neighbors(pos, None);
                world.schedule_block_tick(block, *pos, Self::ACTIVE_TICKS_CALIBRATED, TickPriority::Normal);
                Self::notify_nearby_shriekers(world, pos);
            }
        }
    }

    pub fn notify_nearby_shriekers(world: &Arc<World>, sensor_pos: &BlockPos) {
        let players = world.get_nearby_players(sensor_pos.to_centered_f64(), 16.0);
        let player = players
            .into_iter()
            .find(|p| !p.is_spectator() && p.get_entity().is_alive());
        let Some(player) = player else {
            return;
        };

        const SHRIEKER_LISTENER_RADIUS: i32 = 8;
        for dx in -SHRIEKER_LISTENER_RADIUS..=SHRIEKER_LISTENER_RADIUS {
            for dy in -SHRIEKER_LISTENER_RADIUS..=SHRIEKER_LISTENER_RADIUS {
                for dz in -SHRIEKER_LISTENER_RADIUS..=SHRIEKER_LISTENER_RADIUS {
                    if dx * dx + dy * dy + dz * dz > SHRIEKER_LISTENER_RADIUS * SHRIEKER_LISTENER_RADIUS {
                        continue;
                    }
                    let target_pos = BlockPos::new(sensor_pos.0.x + dx, sensor_pos.0.y + dy, sensor_pos.0.z + dz);
                    if world.get_block(&target_pos).id == BlockId::SCULK_SHRIEKER {
                        crate::block::blocks::sculk::sculk_shrieker::SculkShriekerBlock::try_shriek(world, &target_pos, &player);
                    }
                }
            }
        }
    }
}

impl BlockBehaviour for SculkSensorBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let mut props = CalibratedSculkSensorLikeProperties::default(args.block);
            props.facing = args.player.living_entity.entity.get_horizontal_facing();
            props.waterlogged = args.replacing.water_source();
            props.to_state_id(args.block)
        } else {
            let mut props = SculkSensorLikeProperties::default(args.block);
            props.waterlogged = args.replacing.water_source();
            props.to_state_id(args.block)
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let entity = CalibratedSculkSensorBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        } else if args.block.id == BlockId::SCULK_SENSOR {
            let entity = SculkSensorBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        }
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if Self::emits_strong_power(args.direction) {
            self.get_weak_redstone_power(args)
        } else {
            0
        }
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.block.id == BlockId::SCULK_SENSOR {
            let props = SculkSensorLikeProperties::from_state_id(args.state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Active {
                props.power
            } else {
                0
            }
        } else if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let props = CalibratedSculkSensorLikeProperties::from_state_id(args.state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Active
                && Self::can_output_weak_power(args.block.id, Some(props.facing), args.direction)
            {
                props.power
            } else {
                0
            }
        } else {
            0
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        // Vanilla reads the frequency only while the sensor is active.
        if sculk_sensor_phase(args.block, args.state.id) != SculkSensorPhase::Active {
            return Some(0);
        }

        let be = args.world.get_block_entity(args.position)?;
        if let Some(sensor_be) = be.as_any().downcast_ref::<SculkSensorBlockEntity>() {
            return Some(
                *sensor_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) as u8,
            );
        }
        if let Some(cal_be) = be
            .as_any()
            .downcast_ref::<CalibratedSculkSensorBlockEntity>()
        {
            return Some(
                *cal_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) as u8,
            );
        }
        None
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        if args.block.id == BlockId::SCULK_SENSOR {
            let mut props = SculkSensorLikeProperties::from_state_id(state.id);
            match props.sculk_sensor_phase {
                SculkSensorPhase::Active => {
                    props.sculk_sensor_phase = SculkSensorPhase::Cooldown;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        Self::COOLDOWN_TICKS,
                        TickPriority::Normal,
                    );
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                }
                SculkSensorPhase::Inactive => {}
            }
        } else if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let mut props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            match props.sculk_sensor_phase {
                SculkSensorPhase::Active => {
                    props.sculk_sensor_phase = SculkSensorPhase::Cooldown;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        Self::COOLDOWN_TICKS,
                        TickPriority::Normal,
                    );
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                }
                SculkSensorPhase::Inactive => {}
            }
        }
    }

    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        if !args.entity.get_entity().is_sneaking() {
            Self::trigger(args.world, args.position, args.block, 1);
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
