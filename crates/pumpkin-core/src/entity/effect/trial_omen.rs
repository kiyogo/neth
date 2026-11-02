use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;

pub struct TrialOmenMobEffect;

impl TrialOmenMobEffect {
    pub const COLOR: u32 = 1484454;
    pub const TRIAL_OMEN_PER_BAD_OMEN_LEVEL: i32 = 18_000;
    pub const EVENT_BECOME_OMINOUS: i32 = 3020;

    #[must_use]
    pub const fn calculate_duration_from_bad_omen(bad_omen_amplifier: u8) -> i32 {
        ((bad_omen_amplifier as i32) + 1) * Self::TRIAL_OMEN_PER_BAD_OMEN_LEVEL
    }
}

impl MobEffect for TrialOmenMobEffect {
    fn should_apply_effect_tick(&self, _duration: i32, _amplifier: u8) -> bool {
        false
    }

    fn apply_effect_tick(&self, _living: &LivingEntity, _amplifier: u8) -> bool {
        true
    }
}
