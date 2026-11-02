use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;

pub struct RegenerationMobEffect;

impl MobEffect for RegenerationMobEffect {
    fn should_apply_effect_tick(&self, duration: i32, amplifier: u8) -> bool {
        // Vanilla RegenerationMobEffect.java:21-24:
        // int interval = 50 >> amplification;
        // return interval > 0 ? tickCount % interval == 0 : true;
        if duration <= 0 {
            return false;
        }
        let tick_rate = 50i32.checked_shr(u32::from(amplifier)).unwrap_or(0);
        if tick_rate > 0 {
            (duration as u32).is_multiple_of(tick_rate as u32)
        } else {
            true
        }
    }

    fn apply_effect_tick(&self, living: &LivingEntity, _amplifier: u8) -> bool {
        // Vanilla RegenerationMobEffect.java:12-18
        let current_health = living.health.load();
        let max_health = living.get_max_health();
        if current_health < max_health && current_health > 0.0 {
            living.heal(1.0);
        }
        true
    }
}
