use pumpkin_data::damage::DamageType;

use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;

pub struct PoisonMobEffect;

impl MobEffect for PoisonMobEffect {
    fn should_apply_effect_tick(&self, duration: i32, amplifier: u8) -> bool {
        // Vanilla PoisonMobEffect.java:23-26:
        // int interval = 25 >> amplification;
        // return interval > 0 ? tickCount % interval == 0 : true;
        if duration <= 0 {
            return false;
        }
        let tick_rate = 25i32.checked_shr(u32::from(amplifier)).unwrap_or(0);
        if tick_rate > 0 {
            (duration as u32).is_multiple_of(tick_rate as u32)
        } else {
            true
        }
    }

    fn apply_effect_tick(&self, living: &LivingEntity, _amplifier: u8) -> bool {
        // Vanilla PoisonMobEffect.java:14-20
        let current_health = living.health.load();
        if current_health > 1.0
            && let Some(dyn_self) = living
                .entity
                .world
                .load()
                .get_entity_by_id(living.entity.entity_id)
        {
            let damage_amount = (current_health - 1.0).min(1.0);
            if damage_amount > 0.0 {
                dyn_self.damage(&*dyn_self, damage_amount, DamageType::MAGIC);
            }
        }
        true
    }
}
