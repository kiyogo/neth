pub mod absorption;
pub mod bad_omen;
pub mod heal_or_harm;
pub mod hunger;
pub mod infested;
pub mod oozing;
pub mod poison;
pub mod raid_omen;
pub mod regeneration;
pub mod saturation;
pub mod trial_omen;
pub mod weaving;
pub mod wind_charged;
pub mod wither;

use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use tracing::warn;

use crate::entity::living::LivingEntity;
use crate::entity::{NBTStorage, NBTStorageInit};

pub trait MobEffect: Send + Sync {
    /// Returns true if `apply_effect_tick` should be called for the current tick and duration.
    fn should_apply_effect_tick(&self, _duration: i32, _amplifier: u8) -> bool {
        false
    }

    /// Applies periodic/tick-based effect logic on a living entity.
    ///
    /// Vanilla parity: `MobEffect.applyEffectTick` returns `boolean` (MobEffect.java:81).
    /// When returning `false`, the effect is immediately terminated and removed from `active_effects`
    /// (e.g. `Absorption` when absorption hearts reach 0, `BadOmen` when converting into `RaidOmen`,
    /// or `RaidOmen` when duration reaches 1 and creates/extends a raid).
    fn apply_effect_tick(&self, _living: &LivingEntity, _amplifier: u8) -> bool {
        true
    }

    /// Called when an effect is started on an entity (Vanilla `MobEffect.onEffectStarted`).
    fn on_effect_started(&self, _living: &LivingEntity, _amplifier: u8) {}

    /// Called when an entity carrying this effect is hurt.
    fn on_mob_hurt(
        &self,
        _living: &LivingEntity,
        _amplifier: u8,
        _damage_type: &DamageType,
        _damage_amount: f32,
    ) {
    }

    /// Called when an entity carrying this effect dies.
    fn on_mob_death(&self, _living: &LivingEntity, _amplifier: u8, _damage_type: &DamageType) {}
}

pub static REGENERATION: regeneration::RegenerationMobEffect = regeneration::RegenerationMobEffect;
pub static POISON: poison::PoisonMobEffect = poison::PoisonMobEffect;
pub static WITHER: wither::WitherMobEffect = wither::WitherMobEffect;
pub static HUNGER: hunger::HungerMobEffect = hunger::HungerMobEffect;
pub static SATURATION: saturation::SaturationMobEffect = saturation::SaturationMobEffect;
pub static BAD_OMEN: bad_omen::BadOmenMobEffect = bad_omen::BadOmenMobEffect;
pub static TRIAL_OMEN: trial_omen::TrialOmenMobEffect = trial_omen::TrialOmenMobEffect;
pub static RAID_OMEN: raid_omen::RaidOmenMobEffect = raid_omen::RaidOmenMobEffect;
pub static INFESTED: infested::InfestedMobEffect = infested::InfestedMobEffect;
pub static OOZING: oozing::OozingMobEffect = oozing::OozingMobEffect;
pub static WEAVING: weaving::WeavingMobEffect = weaving::WeavingMobEffect;
pub static WIND_CHARGED: wind_charged::WindChargedMobEffect = wind_charged::WindChargedMobEffect;
pub static ABSORPTION: absorption::AbsorptionMobEffect = absorption::AbsorptionMobEffect;
pub static INSTANT_HEALTH: heal_or_harm::HealOrHarmMobEffect = heal_or_harm::HealOrHarmMobEffect::INSTANT_HEALTH;
pub static INSTANT_DAMAGE: heal_or_harm::HealOrHarmMobEffect = heal_or_harm::HealOrHarmMobEffect::INSTANT_DAMAGE;

#[must_use]
pub fn get_mob_effect(effect: &'static StatusEffect) -> Option<&'static dyn MobEffect> {
    if effect == &StatusEffect::REGENERATION {
        Some(&REGENERATION)
    } else if effect == &StatusEffect::POISON {
        Some(&POISON)
    } else if effect == &StatusEffect::WITHER {
        Some(&WITHER)
    } else if effect == &StatusEffect::HUNGER {
        Some(&HUNGER)
    } else if effect == &StatusEffect::SATURATION {
        Some(&SATURATION)
    } else if effect == &StatusEffect::RAID_OMEN {
        Some(&RAID_OMEN)
    } else if effect == &StatusEffect::INFESTED {
        Some(&INFESTED)
    } else if effect == &StatusEffect::OOZING {
        Some(&OOZING)
    } else if effect == &StatusEffect::WEAVING {
        Some(&WEAVING)
    } else if effect == &StatusEffect::WIND_CHARGED {
        Some(&WIND_CHARGED)
    } else if effect == &StatusEffect::BAD_OMEN {
        Some(&BAD_OMEN)
    } else if effect == &StatusEffect::TRIAL_OMEN {
        Some(&TRIAL_OMEN)
    } else if effect == &StatusEffect::ABSORPTION {
        Some(&ABSORPTION)
    } else if effect == &StatusEffect::INSTANT_HEALTH {
        Some(&INSTANT_HEALTH)
    } else if effect == &StatusEffect::INSTANT_DAMAGE {
        Some(&INSTANT_DAMAGE)
    } else {
        None
    }
}

impl NBTStorage for pumpkin_data::potion::Effect {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put("id", self.effect_type.minecraft_name);
        if self.amplifier > 0 {
            nbt.put("amplifier", NbtTag::Int(i32::from(self.amplifier)));
        }
        nbt.put("duration", NbtTag::Int(self.duration));
        if self.ambient {
            nbt.put("ambient", NbtTag::Byte(1));
        }
        if !self.show_particles {
            nbt.put("show_particles", NbtTag::Byte(0));
        }
        let show_icon: i8 = i8::from(self.show_icon);
        nbt.put("show_icon", NbtTag::Byte(show_icon));
    }
}

impl NBTStorageInit for pumpkin_data::potion::Effect {
    fn create_from_nbt(nbt: &mut NbtCompound) -> Option<Self> {
        let Some(effect_id) = nbt.get_string("id") else {
            warn!("Unable to read effect. Effect id is not present");
            return None;
        };
        let Some(effect_type) = StatusEffect::from_minecraft_name(effect_id) else {
            warn!("Unable to read effect. Unknown effect type: {effect_id}");
            return None;
        };
        let Some(show_icon) = nbt.get_byte("show_icon") else {
            warn!("Unable to read effect. Show icon is not present");
            return None;
        };
        let amplifier = nbt.get_int("amplifier").unwrap_or(0) as u8;
        let duration = nbt.get_int("duration").unwrap_or(0);
        let ambient = nbt.get_byte("ambient").unwrap_or(0) == 1;
        let show_particles = nbt.get_byte("show_particles").unwrap_or(1) == 1;
        let show_icon = show_icon == 1;
        Some(Self {
            effect_type,
            duration,
            amplifier,
            ambient,
            show_particles,
            show_icon,
            blend: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::effect::absorption::AbsorptionMobEffect;
    use crate::entity::effect::heal_or_harm::HealOrHarmMobEffect;
    use crate::entity::effect::infested::InfestedMobEffect;
    use crate::entity::effect::oozing::OozingMobEffect;
    use crate::entity::effect::weaving::WeavingMobEffect;
    use crate::entity::effect::wind_charged::WindChargedMobEffect;
    use pumpkin_data::entity::EntityType;

    #[test]
    fn test_combat_mob_effects_constants_formulas_and_triggers_parity() {
        // 1. Wind Charged parity (WindChargedMobEffect.java:12-38, MobEffects.java:WIND_CHARGED)
        assert_eq!(WindChargedMobEffect::GUST_BASE_STRENGTH, 3.0);
        assert_eq!(WindChargedMobEffect::GUST_RANDOM_STRENGTH, 2.0);
        assert_eq!(WindChargedMobEffect::COLOR, 12438015);
        assert_eq!(StatusEffect::WIND_CHARGED.color, 12438015);
        assert_eq!(WindChargedMobEffect::calculate_gust_strength(0.0), 3.0);
        assert_eq!(WindChargedMobEffect::calculate_gust_strength(1.0), 5.0);
        assert_eq!(WindChargedMobEffect::calculate_gust_strength(0.5), 4.0);

        // 2. Weaving parity (WeavingMobEffect.java:17-52, MobEffects.java:WEAVING)
        assert_eq!(WeavingMobEffect::MIN_COBWEBS, 2);
        assert_eq!(WeavingMobEffect::MAX_COBWEBS, 3);
        assert_eq!(WeavingMobEffect::CUBE_SEARCH_RADIUS, 1);
        assert_eq!(WeavingMobEffect::MAX_CUBE_SEARCH_ATTEMPTS, 15);
        assert_eq!(WeavingMobEffect::COLOR, 7891290);
        assert_eq!(StatusEffect::WEAVING.color, 7891290);
        assert_eq!(WeavingMobEffect::EVENT_SPAWN_COBWEB, 3018);
        assert_eq!(WeavingMobEffect::calculate_cobweb_count(0), 2);
        assert_eq!(WeavingMobEffect::calculate_cobweb_count(1), 3);
        assert_eq!(WeavingMobEffect::calculate_cobweb_count(4), 2);

        // 3. Oozing parity (OozingMobEffect.java:19-67, MobEffects.java:OOZING)
        assert_eq!(OozingMobEffect::SLIME_SIZE, 2);
        assert_eq!(OozingMobEffect::SLIMES_COUNT, 2);
        assert_eq!(OozingMobEffect::SPAWN_Y_OFFSET, 0.5);
        assert_eq!(OozingMobEffect::COLOR, 10092451);
        assert_eq!(StatusEffect::OOZING.color, 10092451);

        // Max entity cramming calculation
        assert_eq!(OozingMobEffect::number_of_slimes_to_spawn(0, 0, 2), 2);
        assert_eq!(OozingMobEffect::number_of_slimes_to_spawn(24, 20, 2), 2);
        assert_eq!(OozingMobEffect::number_of_slimes_to_spawn(24, 23, 2), 1);
        assert_eq!(OozingMobEffect::number_of_slimes_to_spawn(24, 24, 2), 0);
        assert_eq!(OozingMobEffect::number_of_slimes_to_spawn(24, 25, 2), 0);

        // 4. Infested parity (InfestedMobEffect.java:17-50, MobEffects.java:INFESTED)
        assert_eq!(InfestedMobEffect::CHANCE_TO_SPAWN, 0.1);
        assert_eq!(InfestedMobEffect::MIN_SILVERFISH, 1);
        assert_eq!(InfestedMobEffect::MAX_SILVERFISH, 2);
        assert_eq!(InfestedMobEffect::COLOR, 9214860);
        assert_eq!(StatusEffect::INFESTED.color, 9214860);

        assert!(InfestedMobEffect::should_spawn_silverfish(0.05));
        assert!(InfestedMobEffect::should_spawn_silverfish(0.10));
        assert!(!InfestedMobEffect::should_spawn_silverfish(0.1001));
        assert!(!InfestedMobEffect::should_spawn_silverfish(0.5));

        assert_eq!(InfestedMobEffect::calculate_silverfish_count(0), 1);
        assert_eq!(InfestedMobEffect::calculate_silverfish_count(1), 2);

        // Immunity check
        assert!(InfestedMobEffect::is_immune(&EntityType::WITHER));
        assert!(InfestedMobEffect::is_immune(&EntityType::ENDER_DRAGON));
        assert!(InfestedMobEffect::is_immune(&EntityType::SILVERFISH));
        assert!(!InfestedMobEffect::is_immune(&EntityType::ZOMBIE));
        assert!(!InfestedMobEffect::is_immune(&EntityType::PLAYER));

        // 5. Absorption parity (AbsorptionMobEffect.java:6-26, MobEffects.java:ABSORPTION)
        assert_eq!(AbsorptionMobEffect::COLOR, 2445989);
        assert_eq!(StatusEffect::ABSORPTION.color, 2445989);
        assert_eq!(AbsorptionMobEffect::ABSORPTION_PER_LEVEL, 4.0);
        assert!(ABSORPTION.should_apply_effect_tick(1, 0));
        assert!(ABSORPTION.should_apply_effect_tick(600, 2));

        // 6. HealOrHarm parity (HealOrHarmMobEffect.java:8-48, MobEffects.java:INSTANT_HEALTH, INSTANT_DAMAGE)
        assert_eq!(HealOrHarmMobEffect::HEALTH_COLOR, 16262179);
        assert_eq!(StatusEffect::INSTANT_HEALTH.color, 16262179);
        assert_eq!(HealOrHarmMobEffect::DAMAGE_COLOR, 11101546);
        assert_eq!(StatusEffect::INSTANT_DAMAGE.color, 11101546);

        // Direct heal: 4 << amp
        assert_eq!(HealOrHarmMobEffect::calculate_direct_heal(0), 4.0);
        assert_eq!(HealOrHarmMobEffect::calculate_direct_heal(1), 8.0);
        assert_eq!(HealOrHarmMobEffect::calculate_direct_heal(2), 16.0);

        // Direct harm: 6 << amp
        assert_eq!(HealOrHarmMobEffect::calculate_direct_harm(0), 6.0);
        assert_eq!(HealOrHarmMobEffect::calculate_direct_harm(1), 12.0);
        assert_eq!(HealOrHarmMobEffect::calculate_direct_harm(2), 24.0);

        // Scaled instantaneous: (int)(scale * amount + 0.5)
        assert_eq!(HealOrHarmMobEffect::calculate_scaled_heal(0, 1.0), 4.0);
        assert_eq!(HealOrHarmMobEffect::calculate_scaled_heal(0, 0.5), 2.0);
        assert_eq!(HealOrHarmMobEffect::calculate_scaled_heal(1, 0.5), 4.0);
        assert_eq!(HealOrHarmMobEffect::calculate_scaled_harm(0, 1.0), 6.0);
        assert_eq!(HealOrHarmMobEffect::calculate_scaled_harm(0, 0.5), 3.0);
        assert_eq!(HealOrHarmMobEffect::calculate_scaled_harm(1, 0.5), 6.0);

        // Instantaneous should_apply_effect_tick: duration >= 1
        assert!(INSTANT_HEALTH.should_apply_effect_tick(1, 0));
        assert!(!INSTANT_HEALTH.should_apply_effect_tick(0, 0));

        // 7. Poison, Regeneration, Wither tick interval parity
        // Poison: 25 >> amp (25, 12, 6, 3, 1, 0 -> true)
        assert!(POISON.should_apply_effect_tick(25, 0));
        assert!(!POISON.should_apply_effect_tick(24, 0));
        assert!(POISON.should_apply_effect_tick(12, 1));
        assert!(!POISON.should_apply_effect_tick(11, 1));
        assert!(POISON.should_apply_effect_tick(6, 2));
        assert!(POISON.should_apply_effect_tick(3, 3));
        assert!(POISON.should_apply_effect_tick(1, 4));
        assert!(POISON.should_apply_effect_tick(1, 5)); // 25 >> 5 == 0 -> every tick
        assert!(POISON.should_apply_effect_tick(7, 5));

        // Regeneration: 50 >> amp (50, 25, 12, 6, 3, 1, 0 -> true)
        assert!(REGENERATION.should_apply_effect_tick(50, 0));
        assert!(!REGENERATION.should_apply_effect_tick(49, 0));
        assert!(REGENERATION.should_apply_effect_tick(25, 1));
        assert!(REGENERATION.should_apply_effect_tick(12, 2));
        assert!(REGENERATION.should_apply_effect_tick(6, 3));
        assert!(REGENERATION.should_apply_effect_tick(3, 4));
        assert!(REGENERATION.should_apply_effect_tick(1, 5));
        assert!(REGENERATION.should_apply_effect_tick(1, 6)); // 50 >> 6 == 0 -> every tick

        // Wither: 40 >> amp (40, 20, 10, 5, 2, 1, 0 -> true)
        assert!(WITHER.should_apply_effect_tick(40, 0));
        assert!(!WITHER.should_apply_effect_tick(39, 0));
        assert!(WITHER.should_apply_effect_tick(20, 1));
        assert!(WITHER.should_apply_effect_tick(10, 2));
        assert!(WITHER.should_apply_effect_tick(5, 3));
        assert!(WITHER.should_apply_effect_tick(2, 4));
        assert!(WITHER.should_apply_effect_tick(1, 5));
        assert!(WITHER.should_apply_effect_tick(1, 6)); // 40 >> 6 == 0 -> every tick

        // Hunger: ticks every tick
        assert!(HUNGER.should_apply_effect_tick(1, 0));
        assert!(HUNGER.should_apply_effect_tick(17, 0));

        // 8. get_mob_effect registry lookup parity
        assert!(get_mob_effect(&StatusEffect::WIND_CHARGED).is_some());
        assert!(get_mob_effect(&StatusEffect::WEAVING).is_some());
        assert!(get_mob_effect(&StatusEffect::OOZING).is_some());
        assert!(get_mob_effect(&StatusEffect::INFESTED).is_some());
        assert!(get_mob_effect(&StatusEffect::BAD_OMEN).is_some());
        assert!(get_mob_effect(&StatusEffect::TRIAL_OMEN).is_some());
        assert!(get_mob_effect(&StatusEffect::RAID_OMEN).is_some());
        assert!(get_mob_effect(&StatusEffect::ABSORPTION).is_some());
        assert!(get_mob_effect(&StatusEffect::INSTANT_HEALTH).is_some());
        assert!(get_mob_effect(&StatusEffect::INSTANT_DAMAGE).is_some());
    }

    #[test]
    fn test_omen_effects_and_transformation_parity() {
        use crate::entity::effect::bad_omen::BadOmenMobEffect;
        use crate::entity::effect::trial_omen::TrialOmenMobEffect;
        use crate::entity::effect::raid_omen::RaidOmenMobEffect;

        // 1. Bad Omen parity (BadOmenMobEffect.java:1-36, MobEffects.java:BAD_OMEN)
        assert_eq!(BadOmenMobEffect::COLOR, 745784);
        assert_eq!(StatusEffect::BAD_OMEN.color, 745784);
        assert_eq!(BadOmenMobEffect::RAID_OMEN_DURATION, 600);
        assert_eq!(BadOmenMobEffect::OMINOUS_BOTTLE_EFFECT_DURATION, 120_000);
        assert_eq!(BadOmenMobEffect::MIN_AMPLIFIER, 0);
        assert_eq!(BadOmenMobEffect::MAX_AMPLIFIER, 4);

        // 2. Trial Omen transformation parity (TrialSpawnerStateData.java:46, 202-210)
        // Duration: 18000 ticks (15 min) * (bad_omen_amplifier + 1)
        assert_eq!(TrialOmenMobEffect::COLOR, 1484454);
        assert_eq!(StatusEffect::TRIAL_OMEN.color, 1484454);
        assert_eq!(TrialOmenMobEffect::TRIAL_OMEN_PER_BAD_OMEN_LEVEL, 18_000);
        assert_eq!(TrialOmenMobEffect::EVENT_BECOME_OMINOUS, 3020);

        assert_eq!(TrialOmenMobEffect::calculate_duration_from_bad_omen(0), 18_000);   // Level I: 15 min
        assert_eq!(TrialOmenMobEffect::calculate_duration_from_bad_omen(1), 36_000);   // Level II: 30 min
        assert_eq!(TrialOmenMobEffect::calculate_duration_from_bad_omen(2), 54_000);   // Level III: 45 min
        assert_eq!(TrialOmenMobEffect::calculate_duration_from_bad_omen(3), 72_000);   // Level IV: 60 min
        assert_eq!(TrialOmenMobEffect::calculate_duration_from_bad_omen(4), 90_000);   // Level V: 75 min

        // 3. Raid Omen parity (RaidOmenMobEffect.java:1-33, MobEffects.java:RAID_OMEN)
        assert_eq!(RaidOmenMobEffect::COLOR, 14565464);
        assert_eq!(StatusEffect::RAID_OMEN.color, 14565464);
        assert_eq!(RaidOmenMobEffect::DURATION, 600); // 30 seconds
        assert!(RAID_OMEN.should_apply_effect_tick(1, 0));
        assert!(!RAID_OMEN.should_apply_effect_tick(2, 0));
        assert!(!RAID_OMEN.should_apply_effect_tick(600, 0));

        // Bad Omen ticks every tick to check village proximity
        assert!(BAD_OMEN.should_apply_effect_tick(1, 0));
        assert!(BAD_OMEN.should_apply_effect_tick(100, 0));
    }
}
