use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;

pub struct AbsorptionMobEffect;

impl AbsorptionMobEffect {
    pub const COLOR: i32 = 2445989;
    pub const ABSORPTION_PER_LEVEL: f32 = 4.0;
}

impl MobEffect for AbsorptionMobEffect {
    fn should_apply_effect_tick(&self, _duration: i32, _amplifier: u8) -> bool {
        // Vanilla AbsorptionMobEffect.java:17-19: shouldApplyEffectTickThisTick always returns true
        true
    }

    fn apply_effect_tick(&self, living: &LivingEntity, _amplifier: u8) -> bool {
        // Vanilla AbsorptionMobEffect.java:12-14:
        // Returns true as long as the entity has absorption hearts remaining (> 0.0F).
        // Returning false causes tickServer to remove the effect immediately.
        living.get_absorption() > 0.0
    }

    fn on_effect_started(&self, living: &LivingEntity, amplifier: u8) {
        // Vanilla AbsorptionMobEffect.java:22-25:
        // mob.setAbsorptionAmount(Math.max(mob.getAbsorptionAmount(), 4 * (1 + amplifier)))
        let current = living.get_absorption();
        let target = Self::ABSORPTION_PER_LEVEL * (f32::from(amplifier) + 1.0);
        living.set_absorption(current.max(target));
    }
}
