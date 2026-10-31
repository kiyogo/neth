use std::sync::Arc;

use pumpkin_macros::pumpkin_block;

use pumpkin_data::block_properties::TrialSpawnerState;

use crate::block::entities::trial_spawner::TrialSpawnerBlockEntity;
use crate::block::{BlockBehaviour, BrokenArgs, PlacedArgs};

#[pumpkin_block("minecraft:trial_spawner")]
pub struct TrialSpawnerBlock;

impl TrialSpawnerBlock {
    pub const DEFAULT_PLAYER_SCAN_RANGE: i32 = 14;
    pub const DEFAULT_COOLDOWN_TICKS: i64 = 36000;
    pub const DETECT_PLAYER_SPAWN_BUFFER_TICKS: i64 = 40;
    pub const MAX_MOB_TRACKING_DISTANCE: f64 = 47.0;

    /// Vanilla TrialSpawnerConfig.java:59: (int)Math.floor(this.totalMobs + this.totalMobsAddedPerPlayer * additionalPlayers)
    #[must_use]
    pub fn calculate_target_total_mobs(base: f32, added_per_player: f32, player_count: usize) -> i32 {
        let additional = player_count.saturating_sub(1) as f32 * added_per_player;
        (base + additional).floor() as i32
    }

    /// Vanilla TrialSpawnerConfig.java:63: (int)Math.floor(this.simultaneousMobs + this.simultaneousMobsAddedPerPlayer * additionalPlayers)
    #[must_use]
    pub fn calculate_target_simultaneous_mobs(base: f32, added_per_player: f32, player_count: usize) -> i32 {
        let additional = player_count.saturating_sub(1) as f32 * added_per_player;
        (base + additional).floor() as i32
    }

    #[must_use]
    pub fn is_within_player_scan_range(distance_sq: f64, range: i32) -> bool {
        distance_sq <= f64::from(range * range)
    }

    #[must_use]
    pub fn is_within_mob_tracking_distance(distance_sq: f64) -> bool {
        distance_sq <= Self::MAX_MOB_TRACKING_DISTANCE * Self::MAX_MOB_TRACKING_DISTANCE
    }

    pub const DELAY_BEFORE_EJECT_AFTER_KILLING_LAST_MOB: i64 = 40;
    pub const TIME_BETWEEN_EACH_EJECTION: i64 = 30;
    pub const EVENT_TRIAL_SPAWNER_EJECT_ITEM: i32 = 3014;
    pub const LIGHT_LEVEL_INACTIVE: u8 = 0;
    pub const LIGHT_LEVEL_WAITING_FOR_PLAYERS: u8 = 4;
    pub const LIGHT_LEVEL_ACTIVE: u8 = 8;
    pub const LIGHT_LEVEL_COOLDOWN: u8 = 0;
    pub const SPINNING_MOB_SPEED_INACTIVE: f64 = -1.0;
    pub const SPINNING_MOB_SPEED_WAITING_FOR_PLAYERS: f64 = 200.0;
    pub const SPINNING_MOB_SPEED_ACTIVE: f64 = 1000.0;

    #[must_use]
    pub fn get_light_level(state: TrialSpawnerState) -> u8 {
        match state {
            TrialSpawnerState::Inactive | TrialSpawnerState::Cooldown => Self::LIGHT_LEVEL_INACTIVE,
            TrialSpawnerState::WaitingForPlayers => Self::LIGHT_LEVEL_WAITING_FOR_PLAYERS,
            TrialSpawnerState::Active
            | TrialSpawnerState::WaitingForRewardEjection
            | TrialSpawnerState::EjectingReward => Self::LIGHT_LEVEL_ACTIVE,
        }
    }

    #[must_use]
    pub fn get_spinning_mob_speed(state: TrialSpawnerState) -> f64 {
        match state {
            TrialSpawnerState::Inactive
            | TrialSpawnerState::WaitingForRewardEjection
            | TrialSpawnerState::EjectingReward
            | TrialSpawnerState::Cooldown => Self::SPINNING_MOB_SPEED_INACTIVE,
            TrialSpawnerState::WaitingForPlayers => Self::SPINNING_MOB_SPEED_WAITING_FOR_PLAYERS,
            TrialSpawnerState::Active => Self::SPINNING_MOB_SPEED_ACTIVE,
        }
    }

    #[must_use]
    pub fn is_capable_of_spawning(state: TrialSpawnerState) -> bool {
        matches!(state, TrialSpawnerState::WaitingForPlayers | TrialSpawnerState::Active)
    }
}

impl BlockBehaviour for TrialSpawnerBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        let entity = TrialSpawnerBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(entity));
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.remove_block_entity(args.position);
    }
}
