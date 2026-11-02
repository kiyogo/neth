use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;

pub struct HungerMobEffect;

impl MobEffect for HungerMobEffect {
    fn should_apply_effect_tick(&self, _duration: i32, _amplifier: u8) -> bool {
        // Vanilla HungerMobEffect.java:22-24: shouldApplyEffectTickThisTick always returns true
        true
    }

    fn apply_effect_tick(&self, living: &LivingEntity, amplifier: u8) -> bool {
        // Vanilla HungerMobEffect.java:13-19: causeFoodExhaustion(0.005F * (amplification + 1))
        let world = living.entity.world.load();
        if let Some(entity) = world.get_entity_by_id(living.entity.entity_id)
            && let Some(player) = entity.get_player()
        {
            let exhaustion = 0.005 * (f32::from(amplifier) + 1.0);
            player.hunger_manager.add_exhaustion(exhaustion);
        }
        true
    }
}
