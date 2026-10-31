use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::item::Item;
use pumpkin_util::Difficulty;

/// Ominous Bottle parity with Vanilla Minecraft 1.21.4:
/// - Vanilla Java: `OminousBottleAmplifier.java`, `Items.java:OMINOUS_BOTTLE`,
///   `BadOmenMobEffect.java`, `RaidOmenMobEffect.java`, `TrialSpawnerStateData.java`.
pub struct OminousBottleItem;

impl ItemMetadata for OminousBottleItem {
    fn ids() -> Box<[u16]> {
        [Item::OMINOUS_BOTTLE.id].into()
    }
}

impl OminousBottleItem {
    /// Vanilla: `OminousBottleAmplifier.EFFECT_DURATION = 120000;` (100 minutes = 120,000 ticks)
    pub const EFFECT_DURATION: i32 = 120_000;

    /// Vanilla: `OminousBottleAmplifier.MIN_AMPLIFIER = 0;` (Bad Omen I)
    pub const MIN_AMPLIFIER: i32 = 0;

    /// Vanilla: `OminousBottleAmplifier.MAX_AMPLIFIER = 4;` (Bad Omen V)
    pub const MAX_AMPLIFIER: i32 = 4;

    /// Vanilla: `TrialSpawnerStateData.TRIAL_OMEN_PER_BAD_OMEN_LEVEL = 18000;` (15 minutes per level)
    pub const TRIAL_OMEN_PER_BAD_OMEN_LEVEL: i32 = 18_000;

    /// Vanilla: `BadOmenMobEffect.applyEffectTick` adds RAID_OMEN with 600 ticks (30 seconds)
    pub const RAID_OMEN_DURATION: i32 = 600;

    /// Clamps amplifier from NBT / component within [0, 4]
    pub const fn calculate_bad_omen_amplifier(stored_amplifier: i32) -> u8 {
        if stored_amplifier < Self::MIN_AMPLIFIER {
            Self::MIN_AMPLIFIER as u8
        } else if stored_amplifier > Self::MAX_AMPLIFIER {
            Self::MAX_AMPLIFIER as u8
        } else {
            stored_amplifier as u8
        }
    }

    /// Vanilla: `int amplifier = badOmen.getAmplifier() + 1; int duration = 18000 * amplifier;`
    pub const fn calculate_trial_omen_duration(bad_omen_amplifier: u8) -> i32 {
        let level = (bad_omen_amplifier as i32) + 1;
        Self::TRIAL_OMEN_PER_BAD_OMEN_LEVEL * level
    }

    /// Vanilla: `BadOmenMobEffect.applyEffectTick`:
    /// Checks `mob instanceof ServerPlayer player && !player.isSpectator() && level.getDifficulty() != Difficulty.PEACEFUL && level.isVillage(player.blockPosition())`
    pub fn can_convert_bad_omen_to_raid_omen(
        is_spectator: bool,
        difficulty: Difficulty,
        in_village: bool,
    ) -> bool {
        !is_spectator && difficulty != Difficulty::Peaceful && in_village
    }

    /// Vanilla: `RaidOmenMobEffect.shouldApplyEffectTickThisTick(remainingDuration, amplification)`:
    /// Returns true when `remainingDuration == 1`, creating or extending the raid.
    pub const fn should_trigger_raid_from_raid_omen(remaining_duration: i32) -> bool {
        remaining_duration == 1
    }
}

impl ItemBehaviour for OminousBottleItem {
    fn normal_use(&self, _item: &Item, _player: &Player) {
        // Drinking is handled by the consumable flow in living entity
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
