pub mod hunger;
pub mod infested;
pub mod oozing;
pub mod poison;
pub mod raid_omen;
pub mod regeneration;
pub mod saturation;
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
    fn apply_effect_tick(&self, _living: &LivingEntity, _amplifier: u8) {}

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
pub static RAID_OMEN: raid_omen::RaidOmenMobEffect = raid_omen::RaidOmenMobEffect;
pub static INFESTED: infested::InfestedMobEffect = infested::InfestedMobEffect;
pub static OOZING: oozing::OozingMobEffect = oozing::OozingMobEffect;
pub static WEAVING: weaving::WeavingMobEffect = weaving::WeavingMobEffect;
pub static WIND_CHARGED: wind_charged::WindChargedMobEffect = wind_charged::WindChargedMobEffect;

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
    use crate::entity::effect::wind_charged::WindChargedMobEffect;
    use crate::entity::effect::weaving::WeavingMobEffect;
    use crate::entity::effect::oozing::OozingMobEffect;
    use crate::entity::effect::infested::InfestedMobEffect;
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

        // 5. get_mob_effect registry lookup parity
        assert!(get_mob_effect(&StatusEffect::WIND_CHARGED).is_some());
        assert!(get_mob_effect(&StatusEffect::WEAVING).is_some());
        assert!(get_mob_effect(&StatusEffect::OOZING).is_some());
        assert!(get_mob_effect(&StatusEffect::INFESTED).is_some());
    }
}
