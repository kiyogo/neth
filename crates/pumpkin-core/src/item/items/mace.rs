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
    pub const DURABILITY: u32 = 500;
    pub const ENCHANTABILITY: u32 = 15;
    pub const MINING_SPEED: f32 = 1.0;
    pub const MINING_DURABILITY_COST: u32 = 2;
    pub const WEAPON_DURABILITY_COST: u32 = 1;
    pub const ATTACKER_RESET_DELTA_Y: f64 = 0.01;
    pub const KNOCKBACK_UPWARD_BURST: f64 = 0.7;

    // Density enchantment constants (Vanilla Enchantments.java:1165-1181)
    pub const DENSITY_DAMAGE_PER_LEVEL_PER_BLOCK: f64 = 0.5;
    pub const DENSITY_MAX_LEVEL: u32 = 5;

    // Breach enchantment constants (Vanilla Enchantments.java:1182-1198)
    pub const BREACH_ARMOR_REDUCTION_PER_LEVEL: f32 = 0.15;
    pub const BREACH_MAX_LEVEL: u32 = 4;

    // Wind Burst enchantment constants (Vanilla Enchantments.java:1199-1235)
    pub const WIND_BURST_MAX_LEVEL: u32 = 3;
    pub const WIND_BURST_FALL_THRESHOLD: f32 = 1.5;
    pub const WIND_BURST_POWERS: [f32; 3] = [1.2, 1.75, 2.2];

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

    /// Density bonus: 0.5 damage per level per fallen block (Vanilla Enchantments.java:1180)
    #[must_use]
    pub fn calculate_density_damage_bonus(density_level: u32, fall_distance: f64) -> f64 {
        (density_level as f64) * Self::DENSITY_DAMAGE_PER_LEVEL_PER_BLOCK * fall_distance
    }

    /// Calculates total bonus smash damage including Density
    #[must_use]
    pub fn calculate_total_smash_damage(fall_distance: f64, density_level: u32) -> f64 {
        Self::calculate_smash_damage_bonus(fall_distance)
            + Self::calculate_density_damage_bonus(density_level, fall_distance)
    }

    /// Breach effectiveness: reduces effective defense by 15% per level (Vanilla Enchantments.java:1197)
    #[must_use]
    pub fn calculate_breach_effective_armor(defense: f32, breach_level: u32) -> f32 {
        let reduction = (breach_level as f32 * Self::BREACH_ARMOR_REDUCTION_PER_LEVEL).min(1.0);
        defense * (1.0 - reduction)
    }

    /// Wind Burst explosion power: Level 1 = 1.2, Level 2 = 1.75, Level 3 = 2.2
    #[must_use]
    pub fn get_wind_burst_power(level: u32) -> f32 {
        match level {
            1 => Self::WIND_BURST_POWERS[0],
            2 => Self::WIND_BURST_POWERS[1],
            3.. => Self::WIND_BURST_POWERS[2],
            0 => 0.0,
        }
    }

    /// Checks if Wind Burst should trigger on attack (fallDistance >= 1.5 and not flying)
    #[must_use]
    pub const fn can_trigger_wind_burst(
        fall_distance: f32,
        is_fall_flying: bool,
        wind_burst_level: u32,
    ) -> bool {
        wind_burst_level > 0
            && fall_distance >= Self::WIND_BURST_FALL_THRESHOLD
            && !is_fall_flying
    }

    #[must_use]
    pub fn is_valid_repair_item(item_id: u16) -> bool {
        item_id == Item::BREEZE_ROD.id
    }

    #[must_use]
    pub fn get_smash_sound(target_on_ground: bool, fall_distance: f32) -> pumpkin_data::sound::Sound {
        if target_on_ground {
            if fall_distance > Self::SMASH_ATTACK_HEAVY_THRESHOLD {
                pumpkin_data::sound::Sound::ItemMaceSmashGroundHeavy
            } else {
                pumpkin_data::sound::Sound::ItemMaceSmashGround
            }
        } else {
            pumpkin_data::sound::Sound::ItemMaceSmashAir
        }
    }

    #[must_use]
    pub fn calculate_knockback_power(
        distance: f64,
        fall_distance: f32,
        knockback_resistance: f64,
    ) -> f64 {
        if distance > Self::SMASH_ATTACK_KNOCKBACK_RADIUS as f64 {
            return 0.0;
        }
        let multiplier = if fall_distance > Self::SMASH_ATTACK_HEAVY_THRESHOLD {
            2.0
        } else {
            1.0
        };
        let power = (Self::SMASH_ATTACK_KNOCKBACK_RADIUS as f64 - distance)
            * (Self::SMASH_ATTACK_KNOCKBACK_POWER as f64)
            * multiplier
            * (1.0 - knockback_resistance);
        power.max(0.0)
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
