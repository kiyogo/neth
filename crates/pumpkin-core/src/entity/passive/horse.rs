use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, combat,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[
    &Item::GOLDEN_APPLE,
    &Item::ENCHANTED_GOLDEN_APPLE,
    &Item::GOLDEN_CARROT,
];

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_BRED: u8 = 8;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

pub struct HorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl HorseEntity {
    #[must_use]
    pub const fn can_be_leashed(&self) -> bool {
        true
    }
    pub const XP_REWARD: u32 = combat::ABSTRACT_HORSE_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let color = rng.random_range(0..7);
        let style = rng.random_range(0..5);
        let variant = color | (style << 8);

        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant),
            flags: AtomicU8::new(0),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(horse);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla AbstractHorse/Horse: MAX_HEALTH = 53.0, MOVEMENT_SPEED = 0.225, JUMP_STRENGTH = 0.7, STEP_HEIGHT = 1.0, SAFE_FALL_DISTANCE = 6.0, FALL_DAMAGE_MULTIPLIER = 0.5
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MAX_HEALTH,
                combat::ABSTRACT_HORSE_MAX_HEALTH_BOUND,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MOVEMENT_SPEED,
                combat::ABSTRACT_HORSE_BASE_MOVEMENT_SPEED,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::JUMP_STRENGTH,
                combat::ABSTRACT_HORSE_JUMP_STRENGTH,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::STEP_HEIGHT,
                combat::ABSTRACT_HORSE_STEP_HEIGHT,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::SAFE_FALL_DISTANCE,
                combat::ABSTRACT_HORSE_SAFE_FALL_DISTANCE,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::FALL_DAMAGE_MULTIPLIER,
                combat::ABSTRACT_HORSE_FALL_DAMAGE_MULTIPLIER,
            );
            mob_arc.mob_entity.living_entity.set_health(combat::ABSTRACT_HORSE_MAX_HEALTH_BOUND as f32);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.2));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(0.7)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> i32 {
        self.variant.load(Ordering::Relaxed)
    }

    pub fn set_variant(&self, variant: i32) {
        self.variant.store(variant, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::horse::DATA_ID_TYPE_VARIANT,
            VarInt(variant),
        );
    }

    #[must_use]
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::horse::DATA_ID_FLAGS,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn is_tame(&self) -> bool {
        self.has_flag(FLAG_TAME)
    }

    pub fn set_tame(&self, val: bool) {
        self.set_flag(FLAG_TAME, val);
    }

    #[must_use]
    pub fn is_saddled(&self) -> bool {
        self.has_flag(FLAG_SADDLE)
    }

    pub fn set_saddled(&self, val: bool) {
        self.set_flag(FLAG_SADDLE, val);
    }
}

impl AgeableMob for HorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for HorseEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_HORSE_FOOD)
            || item_stack.item == &Item::WHEAT
            || item_stack.item == &Item::SUGAR
            || item_stack.item == &Item::HAY_BLOCK
            || item_stack.item == &Item::APPLE
            || item_stack.item == &Item::GOLDEN_CARROT
            || item_stack.item == &Item::GOLDEN_APPLE
            || item_stack.item == &Item::ENCHANTED_GOLDEN_APPLE
    }
}

impl Mob for HorseEntity {
    fn get_base_experience_reward(&self) -> u32 {
        if self.mob_entity.living_entity.entity.age.load(Ordering::Relaxed) < 0 {
            0
        } else {
            Self::XP_REWARD
        }
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("Variant", self.get_variant());
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(variant) = nbt.get_int("Variant") {
            self.set_variant(variant);
        }
        if let Some(tame) = nbt.get_bool("Tame") {
            self.set_tame(tame);
        }
        if let Some(temper) = nbt.get_int("Temper") {
            self.temper.store(temper, Ordering::Relaxed);
        }
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
    }

    fn as_horse(&self) -> Option<&HorseEntity> {
        Some(self)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if entity.age.load(Ordering::Relaxed) < 0 {
                Sound::EntityBabyHorseAmbient
            } else {
                Sound::EntityHorseAmbient
            };
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::horse::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::horse::DATA_ID_TYPE_VARIANT,
            VarInt(self.get_variant()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::horse::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if self.is_tame() && item == &Item::SADDLE && !self.is_saddled() && !self.is_baby() {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityHorseSaddle,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if let Some(props) = combat::get_equine_food_properties(item.id) {
            let entity = self.get_entity();
            let is_baby = entity.age.load(Ordering::Relaxed) < 0;
            let current_health = self.mob_entity.living_entity.health.load();
            let max_health = self.mob_entity.living_entity.get_max_health();
            let is_tame = self.is_tame();
            let in_love = self.mob_entity.is_in_love();
            let current_temper = self.temper.load(Ordering::Relaxed);

            let mut item_used = false;

            // 1. Healing
            if current_health < max_health && props.heal > 0.0 {
                self.mob_entity.living_entity.heal(props.heal);
                item_used = true;
            }

            // 2. Baby age up
            if is_baby && props.age_up_seconds > 0 {
                let age = entity.age.load(Ordering::Relaxed);
                let new_age = (age + props.age_up_seconds * 20).min(0);
                entity.set_age(new_age);
                let world = entity.world.load();
                world.spawn_particle(
                    entity.pos.load(),
                    pumpkin_util::math::vector3::Vector3::new(0.0, 0.5, 0.0),
                    1.0,
                    5,
                    pumpkin_data::particle::Particle::HappyVillager,
                );
                item_used = true;
            }

            // 3. Temper modification
            if props.temper_increase > 0 && (!is_tame || item_used) && current_temper < 100 {
                self.temper.store((current_temper + props.temper_increase).min(100), Ordering::Relaxed);
                item_used = true;
            }

            // 4. In love breeding
            if props.can_breed && is_tame && !is_baby && !in_love {
                self.mob_entity.set_love_ticks(600, Some(player.gameprofile.id));
                item_used = true;
            }

            if item_used {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                let world = entity.world.load();
                world.play_sound(
                    Sound::EntityHorseEat,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                );
                return true;
            }
        }

        if !self.is_baby() && !self.is_food(item_stack) {
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if let Some(vehicle) = world.get_entity_by_id(ent.entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }

        false
    }
}

impl CustomSound for HorseEntity {
    fn death_sound(&self) -> Option<Sound> {
        let is_baby = self.mob_entity.living_entity.entity.age.load(Ordering::Relaxed) < 0;
        Some(if is_baby {
            Sound::EntityBabyHorseDeath
        } else {
            Sound::EntityHorseDeath
        })
    }

    fn hurt_sound(&self) -> Option<Sound> {
        let is_baby = self.mob_entity.living_entity.entity.age.load(Ordering::Relaxed) < 0;
        Some(if is_baby {
            Sound::EntityBabyHorseHurt
        } else {
            Sound::EntityHorseHurt
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horse_parity_constants() {
        assert_eq!(HorseEntity::XP_REWARD, 2);
        assert_eq!(combat::ABSTRACT_HORSE_JUMP_STRENGTH, 0.7);
        assert_eq!(combat::ABSTRACT_HORSE_MAX_HEALTH_BOUND, 53.0);
        assert_eq!(combat::ABSTRACT_HORSE_BASE_MOVEMENT_SPEED, 0.225);
        assert_eq!(combat::ABSTRACT_HORSE_STEP_HEIGHT, 1.0);
        assert_eq!(combat::ABSTRACT_HORSE_SAFE_FALL_DISTANCE, 6.0);
        assert_eq!(combat::ABSTRACT_HORSE_FALL_DAMAGE_MULTIPLIER, 0.5);
        assert_eq!(combat::ABSTRACT_HORSE_MIN_NATURAL_HEALTH, 15.0);
        assert_eq!(combat::ABSTRACT_HORSE_MAX_NATURAL_HEALTH, 30.0);
    }
}
