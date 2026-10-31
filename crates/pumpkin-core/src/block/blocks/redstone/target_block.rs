use pumpkin_data::block_properties::TargetProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, EmitsRedstonePowerArgs, GetRedstonePowerArgs, OnPlaceArgs, OnProjectileHitArgs,
    OnScheduledTickArgs, OnStateReplacedArgs,
};

use pumpkin_data::block_properties::Axis;

fn get_redstone_strength(
    hit_pos: &Vector3<f64>,
    block_pos: &pumpkin_util::math::position::BlockPos,
) -> u8 {
    let dx = (hit_pos.x - (f64::from(block_pos.0.x) + 0.5)).abs();
    let dy = (hit_pos.y - (f64::from(block_pos.0.y) + 0.5)).abs();
    let dz = (hit_pos.z - (f64::from(block_pos.0.z) + 0.5)).abs();

    let axis = if dy >= dx && dy >= dz {
        Axis::Y
    } else if dz >= dx && dz >= dy {
        Axis::Z
    } else {
        Axis::X
    };

    TargetBlock::calculate_hit_power_on_axis(dx, dy, dz, axis)
}

#[pumpkin_block("minecraft:target")]
pub struct TargetBlock;

impl TargetBlock {
    pub const ACTIVATION_TICKS_ARROWS: u32 = 20;
    pub const ACTIVATION_TICKS_OTHER: u32 = 8;
    pub const MIN_POWER: u8 = 1;
    pub const MAX_POWER: u8 = 15;
    pub const DEFAULT_POWER: u8 = 0;
    pub const BULLSEYE_MIN_DISTANCE: f64 = 30.0;

    #[must_use]
    pub fn is_arrow_projectile(entity_type: &EntityType) -> bool {
        entity_type == &EntityType::ARROW
            || entity_type == &EntityType::SPECTRAL_ARROW
            || entity_type == &EntityType::TRIDENT
    }

    #[must_use]
    pub fn get_activation_ticks(entity_type: &EntityType) -> u32 {
        if Self::is_arrow_projectile(entity_type) {
            Self::ACTIVATION_TICKS_ARROWS
        } else {
            Self::ACTIVATION_TICKS_OTHER
        }
    }

    #[must_use]
    pub fn calculate_power_from_distance(distance: f64) -> u8 {
        let norm = ((0.5 - distance) / 0.5).clamp(0.0, 1.0);
        let power = (15.0 * norm).ceil() as u8;
        power.clamp(1, 15)
    }

    #[must_use]
    pub fn calculate_hit_power_on_axis(
        dist_x: f64,
        dist_y: f64,
        dist_z: f64,
        hit_axis: Axis,
    ) -> u8 {
        let distance = match hit_axis {
            Axis::Y => dist_x.max(dist_z),
            Axis::Z => dist_x.max(dist_y),
            Axis::X => dist_y.max(dist_z),
        };
        Self::calculate_power_from_distance(distance)
    }

    #[must_use]
    pub fn calculate_hit_power(
        hit_pos: &Vector3<f64>,
        block_pos: &pumpkin_util::math::position::BlockPos,
    ) -> u8 {
        get_redstone_strength(hit_pos, block_pos)
    }

    #[must_use]
    pub const fn emits_strong_power() -> bool {
        false
    }
}

impl BlockBehaviour for TargetBlock {
    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = TargetProperties::from_state_id(args.state.id);
        props.r#power
    }

    fn get_strong_redstone_power(&self, _args: GetRedstonePowerArgs<'_>) -> u8 {
        0
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        let power = Self::calculate_hit_power(args.hit_pos, args.position);
        if let Some(owner_id) = args.projectile.get_owner_id()
            && let Some(player) = args.world.get_player_by_id(owner_id)
        {
            player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::TargetHit as i32,
                1,
            );
            if power == 15 {
                let player_pos = player.position();
                let dx = player_pos.x - args.hit_pos.x;
                let dz = player_pos.z - args.hit_pos.z;
                if dx.hypot(dz) >= Self::BULLSEYE_MIN_DISTANCE {
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::Bullseye,
                    );
                }
            }
        }
        if !args
            .world
            .is_block_tick_scheduled(args.position, args.block)
        {
            let mut props = TargetProperties::from_state_id(args.state.id);
            props.r#power = power;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            let delay = Self::get_activation_ticks(args.projectile.get_entity().entity_type);
            args.world
                .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = TargetProperties::from_state_id(state_id);
        if props.r#power != 0 {
            props.r#power = 0;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            let props = TargetProperties::from_state_id(args.old_state_id);
            if props.r#power > 0 {
                args.world.update_neighbors(args.position, None);
            }
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> pumpkin_data::BlockStateId {
        let mut props = TargetProperties::default(args.block);
        props.r#power = 0;
        props.to_state_id(args.block)
    }
}
