use crate::entity::effect::MobEffect;
use crate::entity::living::LivingEntity;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};

pub struct BadOmenMobEffect;

impl BadOmenMobEffect {
    pub const COLOR: u32 = 745784;
    pub const RAID_OMEN_DURATION: i32 = 600;
    pub const OMINOUS_BOTTLE_EFFECT_DURATION: i32 = 120_000;
    pub const MIN_AMPLIFIER: u8 = 0;
    pub const MAX_AMPLIFIER: u8 = 4;

    #[must_use]
    pub const fn calculate_trial_omen_duration(bad_omen_amplifier: u8) -> i32 {
        ((bad_omen_amplifier as i32) + 1) * 18_000
    }
}

impl MobEffect for BadOmenMobEffect {
    fn should_apply_effect_tick(&self, _duration: i32, _amplifier: u8) -> bool {
        true
    }

    fn apply_effect_tick(&self, living: &LivingEntity, amplifier: u8) -> bool {
        let world = living.entity.world.load();
        if !world.dimension.can_start_raid {
            return true;
        }
        if let Some(entity) = world.get_entity_by_id(living.entity.entity_id)
            && let Some(player) = entity.get_player()
            && !player.is_spectator()
        {
            if player.has_effect(&StatusEffect::RAID_OMEN) {
                return true;
            }
            let player_pos = living.entity.block_pos.load();
            let pos_f64 = living.entity.pos.load();

            let village_pos = world
                .villager_poi
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get_nearest_job_site(player_pos, 64)
                .or_else(|| {
                    world.raids.try_lock().ok().and_then(|raids| {
                        raids
                            .get_nearby_raid(&player_pos, 64.0 * 64.0)
                            .map(|r| r.center)
                    })
                });

            if let Some(pos) = village_pos {
                player.set_raid_omen_position(pos);
                let effect = Effect {
                    effect_type: &StatusEffect::RAID_OMEN,
                    duration: Self::RAID_OMEN_DURATION,
                    amplifier,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: true,
                };
                living.add_effect(effect);
                world.play_sound(Sound::BlockBellResonate, SoundCategory::Neutral, &pos_f64);
                return false;
            }
        }
        true
    }
}
