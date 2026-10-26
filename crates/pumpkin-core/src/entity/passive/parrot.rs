use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_protocol::codec::var_int::VarInt;

use crate::entity::{
    Entity, EntityBase,
    ai::control::flying_move_control::FlyingMoveControl,
    ai::goal::{
        escape_danger::EscapeDangerGoal, follow_owner::FollowOwnerGoal,
        look_at_entity::LookAtEntityGoal, sit_when_ordered_to::SitWhenOrderedToGoal,
        swim::SwimGoal, water_avoiding_random_flying::WaterAvoidingRandomFlyingGoal,
    },
    ai::pathfinder::Navigator,
    combat,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::{
        animal::Animal,
        tamable::{TamableAnimal, TamableData},
    },
    player::Player,
};

/// Duration in ticks of the poison a parrot gets from eating a cookie, matching
/// vanilla `Parrot.mobInteract`.
const COOKIE_POISON_DURATION: i32 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ParrotVariant {
    #[default]
    RedBlue = 0,
    Blue = 1,
    Green = 2,
    YellowBlue = 3,
    Gray = 4,
}

impl ParrotVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Blue,
            2 => Self::Green,
            3 => Self::YellowBlue,
            4 => Self::Gray,
            _ => Self::RedBlue,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }
}

/// Represents a Parrot, a passive flying mob that can mimic nearby mob sounds.
///
/// Wiki: <https://minecraft.wiki/w/Parrot>
pub struct ParrotEntity {
    pub mob_entity: MobEntity,
    pub tamable_data: TamableData,
    pub variant: AtomicI32,
}

impl ParrotEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        *mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(FlyingMoveControl::new(10, false));
        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Navigator::flying();

        let variant = ParrotVariant::from_id(rand::random_range(0..5));
        let parrot = Self {
            mob_entity,
            tamable_data: TamableData::default(),
            variant: AtomicI32::new(variant.id()),
        };
        let mob_arc = Arc::new(parrot);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Parrot: MAX_HEALTH = 6.0, FLYING_SPEED = 0.4, MOVEMENT_SPEED = 0.2, ATTACK_DAMAGE = 3.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 6.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(flying) = attributes.get_mut(&Attributes::FLYING_SPEED.id) {
                flying.base_value = 0.4;
                flying.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.2;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = 3.0;
                damage.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(6.0);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Priority 0: PanicGoal (1.25) & FloatGoal
            goal_selector.add_goal(0, EscapeDangerGoal::new(1.25));
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            // Priority 1: LookAtPlayerGoal (range 8.0)
            goal_selector.add_goal(
                1,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            // Priority 2: SitWhenOrderedToGoal, FollowOwnerGoal, ParrotWanderGoal
            goal_selector.add_goal(2, Box::new(SitWhenOrderedToGoal::new()));
            goal_selector.add_goal(2, FollowOwnerGoal::new(1.0, 5.0, 1.0));
            goal_selector.add_goal(2, Box::new(WaterAvoidingRandomFlyingGoal::new(1.0)));
        };

        mob_arc
            .mob_entity
            .living_entity
            .entity
            .set_synced_data(
                pumpkin_data::tracked_data::parrot::DATA_VARIANT_ID,
                VarInt(variant.id()),
            );

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> ParrotVariant {
        ParrotVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: ParrotVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        self.mob_entity
            .living_entity
            .entity
            .set_synced_data(
                pumpkin_data::tracked_data::parrot::DATA_VARIANT_ID,
                VarInt(variant.id()),
            );
    }

    #[must_use]
    pub fn is_flying(&self) -> bool {
        !self
            .mob_entity
            .living_entity
            .entity
            .on_ground
            .load(Ordering::Relaxed)
    }

    /// Feeds the parrot a cookie: it is poisoned and then killed, as in vanilla
    /// `Parrot.mobInteract`.
    fn eat_cookie(&self, player: &Arc<Player>, item_stack: &mut ItemStack) {
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);

        self.mob_entity
            .living_entity
            .add_effect(pumpkin_data::potion::Effect {
                effect_type: &StatusEffect::POISON,
                duration: COOKIE_POISON_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: true,
            });

        // Vanilla guards this call with `player.isCreative() || !this.isInvulnerable()`,
        // but `hurt` re-checks invulnerability itself and `player_attack` doesn't bypass
        // it, so the guard only skips a call that would do nothing anyway.
        self.damage_with_context(
            self,
            f32::MAX,
            DamageType::PLAYER_ATTACK,
            None,
            Some(player.as_ref()),
            Some(player.as_ref()),
        );
    }
}

impl Animal for ParrotEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl TamableAnimal for ParrotEntity {
    fn get_tamable_data(&self) -> &TamableData {
        &self.tamable_data
    }
}

impl Mob for ParrotEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_parrot(&self) -> Option<&ParrotEntity> {
        Some(self)
    }

    fn as_tamable(&self) -> Option<&dyn TamableAnimal> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("Variant", self.get_variant().id());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_id) = nbt.get_int("Variant") {
            self.set_variant(ParrotVariant::from_id(variant_id));
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            entity.world.load().play_sound_fine(
                Sound::EntityParrotAmbient,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        let entity = &self.mob_entity.living_entity.entity;

        // 1. Taming with parrot food (seeds) when untamed
        if !self.is_tame() && item.has_tag(&tag::Item::MINECRAFT_PARROT_FOOD) {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);

            if !entity.silent.load(Ordering::Relaxed) {
                let pitch = combat::get_parrot_eat_sound_pitch(rand::random(), rand::random());
                entity.world.load().play_sound_fine(
                    Sound::EntityParrotEat,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    pitch,
                );
            }

            let rand_10 = rand::random_range(0..10);
            if combat::calculate_parrot_tame_success(rand_10) {
                Mob::tame(self, player);
                TamableAnimal::tame(self, player.gameprofile.id);
                self.spawn_taming_particles(true);
                entity.world.load().send_entity_status(
                    entity,
                    EntityStatus::TamingSucceeded,
                    Some(ActorEventID::TamingSucceeded),
                );
            } else {
                self.spawn_taming_particles(false);
                entity.world.load().send_entity_status(
                    entity,
                    EntityStatus::TamingFailed,
                    Some(ActorEventID::TamingFailed),
                );
            }

            return true;
        }

        // 2. Sitting/standing toggle when tamed and owned by player
        if !item.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD) {
            if !self.is_flying() && self.is_tame() && self.is_owned_by(&player.gameprofile.id) {
                self.set_ordered_to_sit(!self.is_ordered_to_sit());
                return true;
            }
            return self.mob_entity.mob_interact(player, item_stack);
        }

        // 3. Poisonous food (Cookie) kills parrot
        self.eat_cookie(player, item_stack);
        true
    }

    /// Vanilla `Parrot.java:293-294`: Parrots take zero fall damage.
    fn calculate_fall_damage(&self, _fall_distance: f64, _damage_modifier: f32) -> i32 {
        combat::calculate_parrot_fall_damage()
    }

    fn adjust_damage_amount(
        &self,
        amount: f32,
        damage_type: &DamageType,
    ) -> f32 {
        if *damage_type == DamageType::FALL {
            0.0
        } else {
            amount
        }
    }
}

impl CustomSound for ParrotEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityParrotDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityParrotHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::item::Item;
    use pumpkin_data::tag::{self, Taggable};

    /// The interaction is gated on the vanilla `parrot_poisonous_food` tag rather than
    /// on a hardcoded cookie id, so check the tag actually resolves the way the
    /// interaction assumes.
    #[test]
    fn cookie_is_poisonous_parrot_food() {
        assert!(Item::COOKIE.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
    }

    /// Seeds tame a parrot in vanilla and must not reach the poison branch.
    #[test]
    fn parrot_food_is_not_poisonous() {
        assert!(!Item::WHEAT_SEEDS.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
        assert!(!Item::COOKED_CHICKEN.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
        assert!(Item::WHEAT_SEEDS.has_tag(&tag::Item::MINECRAFT_PARROT_FOOD));
    }

    #[test]
    fn poison_lasts_45_seconds() {
        assert_eq!(COOKIE_POISON_DURATION, 900);
    }

    #[test]
    fn test_parrot_variants_and_ids() {
        assert_eq!(ParrotVariant::from_id(0), ParrotVariant::RedBlue);
        assert_eq!(ParrotVariant::from_id(1), ParrotVariant::Blue);
        assert_eq!(ParrotVariant::from_id(2), ParrotVariant::Green);
        assert_eq!(ParrotVariant::from_id(3), ParrotVariant::YellowBlue);
        assert_eq!(ParrotVariant::from_id(4), ParrotVariant::Gray);
        assert_eq!(ParrotVariant::RedBlue.id(), 0);
        assert_eq!(ParrotVariant::Blue.id(), 1);
        assert_eq!(ParrotVariant::Green.id(), 2);
        assert_eq!(ParrotVariant::YellowBlue.id(), 3);
        assert_eq!(ParrotVariant::Gray.id(), 4);
    }

    #[test]
    fn test_parrot_fall_damage_immunity() {
        assert_eq!(combat::calculate_parrot_fall_damage(), 0);
        assert!(!combat::should_parrot_take_fall_damage());
    }
}
