use std::sync::Arc;

use pumpkin_macros::pumpkin_block;

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
