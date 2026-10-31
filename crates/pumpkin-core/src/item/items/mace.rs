use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::item::Item;
use pumpkin_util::GameMode;

pub struct MaceItem;

impl MaceItem {
    pub const DEFAULT_ATTACK_DAMAGE: f32 = 5.0;
    pub const DEFAULT_ATTACK_SPEED: f32 = -3.4;
    pub const SMASH_ATTACK_FALL_THRESHOLD: f32 = 1.5;
    pub const SMASH_ATTACK_HEAVY_THRESHOLD: f32 = 5.0;
    pub const SMASH_ATTACK_KNOCKBACK_RADIUS: f32 = 3.5;
    pub const SMASH_ATTACK_KNOCKBACK_POWER: f32 = 0.7;
    pub const SMASH_PARTICLES_EVENT_ID: i32 = 2013;
    pub const SMASH_PARTICLES_DATA: i32 = 750;

    #[must_use]
    pub const fn can_smash_attack(fall_distance: f32, is_fall_flying: bool) -> bool {
        fall_distance > Self::SMASH_ATTACK_FALL_THRESHOLD && !is_fall_flying
    }

    /// Vanilla MaceItem.java:95-105:
    /// Piecewise fall distance bonus:
    /// - <= 3.0: 4.0 * fallDistance
    /// - <= 8.0: 12.0 + 2.0 * (fallDistance - 3.0)
    /// - > 8.0: 22.0 + 1.0 * (fallDistance - 8.0)
    #[must_use]
    pub fn calculate_smash_damage_bonus(fall_distance: f64) -> f64 {
        if fall_distance <= 3.0 {
            4.0 * fall_distance
        } else if fall_distance <= 8.0 {
            12.0 + 2.0 * (fall_distance - 3.0)
        } else {
            22.0 + (fall_distance - 8.0)
        }
    }

    #[must_use]
    pub const fn is_heavy_smash(fall_distance: f32) -> bool {
        fall_distance > Self::SMASH_ATTACK_HEAVY_THRESHOLD
    }
}

impl ItemMetadata for MaceItem {
    fn ids() -> Box<[u16]> {
        [Item::MACE.id].into()
    }
}
impl ItemBehaviour for MaceItem {
    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
