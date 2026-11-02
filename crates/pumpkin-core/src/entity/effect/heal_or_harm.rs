use pumpkin_data::damage::DamageType;

use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;

pub struct HealOrHarmMobEffect {
    pub is_harm: bool,
}

impl HealOrHarmMobEffect {
    pub const INSTANT_HEALTH: Self = Self { is_harm: false };
    pub const INSTANT_DAMAGE: Self = Self { is_harm: true };

    pub const HEALTH_COLOR: i32 = 16262179;
    pub const DAMAGE_COLOR: i32 = 11101546;

    #[must_use]
    pub const fn calculate_direct_heal(amplifier: u8) -> f32 {
        if let Some(shl) = 4i32.checked_shl(amplifier as u32) {
            if shl < 0 { 0.0 } else { shl as f32 }
        } else {
            i32::MAX as f32
        }
    }

    #[must_use]
    pub const fn calculate_direct_harm(amplifier: u8) -> f32 {
        if let Some(shl) = 6i32.checked_shl(amplifier as u32) {
            if shl < 0 { 0.0 } else { shl as f32 }
        } else {
            i32::MAX as f32
        }
    }

    #[must_use]
    pub fn calculate_scaled_heal(amplifier: u8, scale: f32) -> f32 {
        let base = Self::calculate_direct_heal(amplifier);
        (scale * base + 0.5).floor().max(0.0)
    }

    #[must_use]
    pub fn calculate_scaled_harm(amplifier: u8, scale: f32) -> f32 {
        let base = Self::calculate_direct_harm(amplifier);
        (scale * base + 0.5).floor().max(0.0)
    }

    /// Vanilla parity: `HealOrHarmMobEffect.applyInstantaneousEffect` (HealOrHarmMobEffect.java:28-47)
    pub fn apply_instantaneous_effect(
        &self,
        living: &LivingEntity,
        amplifier: u8,
        scale: f32,
    ) {
        if self.is_harm == living.is_inverted_heal_and_harm() {
            let amount = Self::calculate_scaled_heal(amplifier, scale);
            if amount > 0.0 {
                living.heal(amount);
            }
        } else {
            let amount = Self::calculate_scaled_harm(amplifier, scale);
            if amount > 0.0 {
                let dyn_self = living
                    .entity
                    .world
                    .load()
                    .get_entity_by_id(living.entity.entity_id);
                if let Some(dyn_self) = dyn_self {
                    let _ = dyn_self.damage(&*dyn_self, amount, DamageType::MAGIC);
                }
            }
        }
    }

    /// Vanilla parity: `HealOrHarmMobEffect.applyEffectTick` (HealOrHarmMobEffect.java:17-25)
    pub fn apply_effect_tick(&self, living: &LivingEntity, amplifier: u8) -> bool {
        if self.is_harm == living.is_inverted_heal_and_harm() {
            let amount = Self::calculate_direct_heal(amplifier);
            if amount > 0.0 {
                living.heal(amount);
            }
        } else {
            let amount = Self::calculate_direct_harm(amplifier);
            if amount > 0.0 {
                let dyn_self = living
                    .entity
                    .world
                    .load()
                    .get_entity_by_id(living.entity.entity_id);
                if let Some(dyn_self) = dyn_self {
                    let _ = dyn_self.damage(&*dyn_self, amount, DamageType::MAGIC);
                }
            }
        }
        true
    }
}

impl MobEffect for HealOrHarmMobEffect {
    fn should_apply_effect_tick(&self, duration: i32, _amplifier: u8) -> bool {
        // Vanilla InstantaneousMobEffect.java:14-16: remainingDuration >= 1
        duration >= 1
    }

    fn apply_effect_tick(&self, living: &LivingEntity, amplifier: u8) -> bool {
        Self::apply_effect_tick(self, living, amplifier)
    }
}
