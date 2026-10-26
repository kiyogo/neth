use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        active_target::ActiveTargetGoal, avoid_entity::AvoidEntityGoal, breed::BreedGoal,
        climb_on_top_of_powder_snow::ClimbOnTopOfPowderSnowGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    combat,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::CARROT, &Item::GOLDEN_CARROT, &Item::DANDELION];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum RabbitVariant {
    #[default]
    Brown = 0,
    White = 1,
    Black = 2,
    WhiteSplotched = 3,
    Gold = 4,
    Salt = 5,
    Evil = 99,
}

impl RabbitVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::White,
            2 => Self::Black,
            3 => Self::WhiteSplotched,
            4 => Self::Gold,
            5 => Self::Salt,
            99 => Self::Evil,
            _ => Self::Brown,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub fn random_variant() -> Self {
        let mut rng = rand::rng();
        match rng.random_range(0..6) {
            1 => Self::White,
            2 => Self::Black,
            3 => Self::WhiteSplotched,
            4 => Self::Gold,
            5 => Self::Salt,
            _ => Self::Brown,
        }
    }
}

pub struct RabbitEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub more_carrot_ticks: AtomicI32,
}

impl RabbitEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let variant = RabbitVariant::random_variant();
        let rabbit = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant.id()),
            more_carrot_ticks: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(rabbit);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Rabbit: MAX_HEALTH = 3.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 3.0 (Evil: ARMOR = 8.0, ATTACK_DAMAGE = 8.0)
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 3.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.3;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = if variant == RabbitVariant::Evil { 8.0 } else { 3.0 };
                damage.dirty.store(true, Ordering::Relaxed);
            }
            if variant == RabbitVariant::Evil {
                if let Some(armor) = attributes.get_mut(&Attributes::ARMOR.id) {
                    armor.base_value = 8.0;
                    armor.dirty.store(true, Ordering::Relaxed);
                }
            }
        }
        mob_arc.mob_entity.living_entity.set_health(3.0);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(ClimbOnTopOfPowderSnowGoal));
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.2));
            goal_selector.add_goal(2, BreedGoal::new(0.8));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.0, TEMPT_ITEMS, false)));
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::new(&EntityType::PLAYER, 8.0, 2.2, 2.2)),
            );
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::new(&EntityType::WOLF, 10.0, 2.2, 2.2)),
            );
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::new(&EntityType::FOX, 10.0, 2.2, 2.2)),
            );
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::for_monsters(4.0, 2.2, 2.2)),
            );
            goal_selector.add_goal(5, Box::new(FollowParentGoal::new(0.8)));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(0.6)));
            goal_selector.add_goal(
                11,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 10.0),
            );
            goal_selector.add_goal(11, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> RabbitVariant {
        RabbitVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    /// Vanilla `Rabbit.java:324-326`: Killer Bunny uses HOSTILE sound source, others use NEUTRAL.
    #[must_use]
    pub fn get_sound_category(&self) -> SoundCategory {
        if combat::is_rabbit_evil(self.variant.load(Ordering::Relaxed)) {
            SoundCategory::Hostile
        } else {
            SoundCategory::Neutral
        }
    }

    pub fn set_variant(&self, variant: RabbitVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::rabbit::DATA_TYPE_ID,
            VarInt(variant.id()),
        );

        // Parity with Vanilla Rabbit.setVariant: EVIL rabbit gets +8 armor and +5 attack damage (total 8.0)
        {
            let mut attributes = self
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if variant == RabbitVariant::Evil {
                if let Some(armor) = attributes.get_mut(&Attributes::ARMOR.id) {
                    armor.base_value = 8.0;
                    armor.dirty.store(true, Ordering::Relaxed);
                }
                if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                    damage.base_value = 8.0;
                    damage.dirty.store(true, Ordering::Relaxed);
                }
            } else {
                if let Some(armor) = attributes.get_mut(&Attributes::ARMOR.id) {
                    armor.base_value = 0.0;
                    armor.dirty.store(true, Ordering::Relaxed);
                }
                if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                    damage.base_value = 3.0;
                    damage.dirty.store(true, Ordering::Relaxed);
                }
            }
        }

        if variant == RabbitVariant::Evil {
            let mut goal_selector = self
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.4, true)));

            let mut target_selector = self
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&self.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&self.mob_entity, &EntityType::WOLF, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&self.mob_entity, &EntityType::FOX, true),
            );
        }
    }
}

impl AgeableMob for RabbitEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for RabbitEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_RABBIT_FOOD)
            || TEMPT_ITEMS.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for RabbitEntity {
    fn as_rabbit(&self) -> Option<&RabbitEntity> {
        Some(self)
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("RabbitType", self.get_variant().id());
        nbt.put_int(
            "MoreCarrotTicks",
            self.more_carrot_ticks.load(Ordering::Relaxed),
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(rabbit_type) = nbt.get_int("RabbitType") {
            self.set_variant(RabbitVariant::from_id(rabbit_type));
        }
        if let Some(ticks) = nbt.get_int("MoreCarrotTicks") {
            self.more_carrot_ticks.store(ticks, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            entity.world.load().play_sound_fine(
                Sound::EntityRabbitAmbient,
                self.get_sound_category(),
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    /// Vanilla `Rabbit.java:317-321`: Killer Bunny plays rabbit attack sound.
    fn on_attack(&self, _target: &dyn EntityBase) {
        if self.get_variant() == RabbitVariant::Evil {
            let entity = self.get_entity();
            let pitch = combat::calculate_rabbit_attack_pitch(rand::random(), rand::random());
            entity.world.load().play_sound_fine(
                Sound::EntityRabbitAttack,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                pitch,
            );
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::rabbit::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::rabbit::DATA_TYPE_ID,
            VarInt(self.get_variant().id()),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityRabbitAmbient)
    }
}

impl CustomSound for RabbitEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityRabbitDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityRabbitHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rabbit_variants_and_evil_sound_category() {
        assert_eq!(RabbitVariant::from_id(0), RabbitVariant::Brown);
        assert_eq!(RabbitVariant::from_id(1), RabbitVariant::White);
        assert_eq!(RabbitVariant::from_id(2), RabbitVariant::Black);
        assert_eq!(RabbitVariant::from_id(3), RabbitVariant::WhiteSplotched);
        assert_eq!(RabbitVariant::from_id(4), RabbitVariant::Gold);
        assert_eq!(RabbitVariant::from_id(5), RabbitVariant::Salt);
        assert_eq!(RabbitVariant::from_id(99), RabbitVariant::Evil);

        assert!(combat::is_rabbit_evil(RabbitVariant::Evil.id()));
        assert!(!combat::is_rabbit_evil(RabbitVariant::Brown.id()));
        assert!(!combat::is_rabbit_evil(RabbitVariant::White.id()));

        // Speed constants
        assert_eq!(combat::RABBIT_STROLL_SPEED_MOD, 0.6);
        assert_eq!(combat::RABBIT_BREED_SPEED_MOD, 0.8);
        assert_eq!(combat::RABBIT_FOLLOW_SPEED_MOD, 1.0);
        assert_eq!(combat::RABBIT_FLEE_SPEED_MOD, 2.2);
        assert_eq!(combat::RABBIT_ATTACK_SPEED_MOD, 1.4);
    }
}
