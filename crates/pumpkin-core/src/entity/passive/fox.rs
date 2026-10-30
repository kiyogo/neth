use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use uuid::Uuid;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;

use crate::entity::{
    Entity, EntityBase, combat,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        avoid_entity::AvoidEntityGoal, breed::BreedGoal,
        climb_on_top_of_powder_snow::ClimbOnTopOfPowderSnowGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const FOX_FOOD: &[&Item] = &[&Item::SWEET_BERRIES, &Item::GLOW_BERRIES];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum FoxVariant {
    #[default]
    Red = 0,
    Snow = 1,
}

impl FoxVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Snow,
            _ => Self::Red,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Snow => "snow",
        }
    }

    #[must_use]
    pub fn from_name(s: &str) -> Self {
        match s {
            "snow" | "minecraft:snow" => Self::Snow,
            _ => Self::Red,
        }
    }
}

/// Represents a Fox, a passive/neutral mob.
///
/// Wiki: <https://minecraft.wiki/w/Fox>
pub struct FoxEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub flags: AtomicU8,
    pub trusted_0: AtomicCell<Option<Uuid>>,
    pub trusted_1: AtomicCell<Option<Uuid>>,
    pub ticks_since_eaten: AtomicI32,
}

impl FoxEntity {
    pub const XP_REWARD: u32 = combat::FOX_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let fox = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(FoxVariant::Red.id()),
            flags: AtomicU8::new(0),
            trusted_0: AtomicCell::new(None),
            trusted_1: AtomicCell::new(None),
            ticks_since_eaten: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(fox);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let fox_weak = Arc::downgrade(&mob_arc);

        // Vanilla Fox.java:createAttributes()
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::MAX_HEALTH,
            crate::entity::combat::FOX_MAX_HEALTH,
        );
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::MOVEMENT_SPEED,
            crate::entity::combat::FOX_BASE_MOVEMENT_SPEED,
        );
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::ATTACK_DAMAGE,
            crate::entity::combat::FOX_BASE_ATTACK_DAMAGE,
        );
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::SAFE_FALL_DISTANCE,
            crate::entity::combat::FOX_SAFE_FALL_DISTANCE,
        );
        mob_arc.mob_entity.living_entity.set_attribute_base(
            &Attributes::FOLLOW_RANGE,
            crate::entity::combat::FOX_FOLLOW_RANGE,
        );
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::FOX_MAX_HEALTH as f32);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(0, Box::new(ClimbOnTopOfPowderSnowGoal));
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.2));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.2, FOX_FOOD, false)));
            // Vanilla Fox.java:187: avoid Player if !trusts(player) && !isDefending()
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::with_target_predicate(
                    &EntityType::PLAYER,
                    16.0,
                    1.6,
                    1.4,
                    {
                        let fox_weak = fox_weak.clone();
                        move |entity| {
                            if let Some(fox) = fox_weak.upgrade() {
                                let uuid = entity.get_entity().entity_uuid;
                                combat::should_fox_avoid_player(fox.is_defending(), fox.trusts(&uuid))
                            } else {
                                true
                            }
                        }
                    },
                )),
            );
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::new(&EntityType::WOLF, 8.0, 1.6, 1.4)),
            );
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::new(&EntityType::POLAR_BEAR, 8.0, 1.6, 1.4)),
            );
            goal_selector.add_goal(5, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 24.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };


        // Vanilla Fox: MOVEMENT_SPEED = 0.3, MAX_HEALTH = 10.0, ATTACK_DAMAGE = 2.0, SAFE_FALL_DISTANCE = 5.0, FOLLOW_RANGE = 32.0
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 10.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.3;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = 2.0;
                damage.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(fall) = attributes.get_mut(&Attributes::SAFE_FALL_DISTANCE.id) {
                fall.base_value = combat::get_fox_safe_fall_distance();
                fall.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(follow) = attributes.get_mut(&Attributes::FOLLOW_RANGE.id) {
                follow.base_value = 32.0;
                follow.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(10.0);
        mob_arc.mob_entity.set_can_pick_up_loot(true);

        mob_arc
    }

    #[must_use]
    pub const fn can_be_leashed(&self) -> bool {
        true
    }

    pub fn get_ticks_since_eaten(&self) -> i32 {
        self.ticks_since_eaten.load(Ordering::Relaxed)
    }

    pub fn set_ticks_since_eaten(&self, ticks: i32) {
        self.ticks_since_eaten.store(ticks, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get_variant(&self) -> FoxVariant {
        FoxVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: FoxVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::TYPE_ID,
            VarInt(variant.id()),
        );
    }

    fn get_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    fn set_flag(&self, flag: u8, value: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if value {
            current | flag
        } else {
            current & !flag
        };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::fox::FLAGS_ID, new_flags);
    }

    #[must_use]
    pub fn is_sitting(&self) -> bool {
        self.get_flag(1)
    }

    pub fn set_sitting(&self, val: bool) {
        self.set_flag(1, val);
    }

    #[must_use]
    pub fn is_crouching(&self) -> bool {
        self.get_flag(4)
    }

    pub fn set_crouching(&self, val: bool) {
        self.set_flag(4, val);
    }

    #[must_use]
    pub fn is_interested(&self) -> bool {
        self.get_flag(8)
    }

    pub fn set_interested(&self, val: bool) {
        self.set_flag(8, val);
    }

    #[must_use]
    pub fn is_pouncing(&self) -> bool {
        self.get_flag(16)
    }

    pub fn set_pouncing(&self, val: bool) {
        self.set_flag(16, val);
    }

    #[must_use]
    pub fn is_sleeping(&self) -> bool {
        self.get_flag(32)
    }

    pub fn set_sleeping(&self, val: bool) {
        self.set_flag(32, val);
    }

    #[must_use]
    pub fn is_faceplanted(&self) -> bool {
        self.get_flag(64)
    }

    pub fn set_faceplanted(&self, val: bool) {
        self.set_flag(64, val);
    }

    #[must_use]
    pub fn is_defending(&self) -> bool {
        self.get_flag(128)
    }

    pub fn set_defending(&self, val: bool) {
        self.set_flag(128, val);
    }

    pub fn clear_states(&self) {
        self.set_interested(false);
        self.set_crouching(false);
        self.set_sitting(false);
        self.set_sleeping(false);
        self.set_defending(false);
        self.set_faceplanted(false);
    }

    #[must_use]
    pub fn get_trusted_0(&self) -> Option<Uuid> {
        self.trusted_0.load()
    }

    #[must_use]
    pub fn get_trusted_1(&self) -> Option<Uuid> {
        self.trusted_1.load()
    }

    pub fn add_trusted_uuid(&self, uuid: Uuid) {
        let entity = self.get_entity();
        if self.trusted_0.load().is_some() {
            self.trusted_1.store(Some(uuid));
            entity.set_synced_data(
                pumpkin_data::tracked_data::fox::DATA_TRUSTED_ID_1,
                Some(uuid),
            );
        } else {
            self.trusted_0.store(Some(uuid));
            entity.set_synced_data(
                pumpkin_data::tracked_data::fox::DATA_TRUSTED_ID_0,
                Some(uuid),
            );
        }
    }

    pub fn clear_trusted(&self) {
        self.trusted_0.store(None);
        self.trusted_1.store(None);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::DATA_TRUSTED_ID_0,
            None as Option<Uuid>,
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::DATA_TRUSTED_ID_1,
            None as Option<Uuid>,
        );
    }

    #[must_use]
    pub fn trusts(&self, uuid: &Uuid) -> bool {
        self.trusted_0.load().as_ref() == Some(uuid) || self.trusted_1.load().as_ref() == Some(uuid)
    }
}

impl AgeableMob for FoxEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for FoxEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FOX_FOOD)
            || FOX_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for FoxEntity {
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

    fn as_fox(&self) -> Option<&FoxEntity> {
        Some(self)
    }

    /// Vanilla `SweetBerryBushBlock`: foxes are immune to sweet berry bush damage.
    fn pre_damage(
        &self,
        damage_type: DamageType,
        _source: Option<&dyn EntityBase>,
    ) -> bool {
        if damage_type == DamageType::SWEET_BERRY_BUSH {
            return false;
        }
        true
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("Type", self.get_variant().as_str().to_string());
        nbt.put_int("TicksSinceEaten", self.get_ticks_since_eaten());
        nbt.put_bool("Sleeping", self.is_sleeping());
        nbt.put_bool("Sitting", self.is_sitting());
        nbt.put_bool("Crouching", self.is_crouching());

        let mut list = Vec::new();
        if let Some(t0) = self.trusted_0.load() {
            nbt.put_uuid("Trusted0", t0);
            let u = t0.as_u128();
            list.push(pumpkin_nbt::tag::NbtTag::IntArray(vec![
                (u >> 96) as i32,
                (u >> 64) as i32,
                (u >> 32) as i32,
                u as i32,
            ]));
        }
        if let Some(t1) = self.trusted_1.load() {
            nbt.put_uuid("Trusted1", t1);
            let u = t1.as_u128();
            list.push(pumpkin_nbt::tag::NbtTag::IntArray(vec![
                (u >> 96) as i32,
                (u >> 64) as i32,
                (u >> 32) as i32,
                u as i32,
            ]));
        }
        if !list.is_empty() {
            nbt.put("Trusted", pumpkin_nbt::tag::NbtTag::List(list));
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_str) = nbt.get_string("Type") {
            self.set_variant(FoxVariant::from_name(variant_str));
        }
        if let Some(ticks) = nbt.get_int("TicksSinceEaten") {
            self.set_ticks_since_eaten(ticks);
        }
        if let Some(sleeping) = nbt.get_bool("Sleeping") {
            self.set_sleeping(sleeping);
        }
        if let Some(sitting) = nbt.get_bool("Sitting") {
            self.set_sitting(sitting);
        }
        if let Some(crouching) = nbt.get_bool("Crouching") {
            self.set_crouching(crouching);
        }

        self.clear_trusted();
        if let Some(t0) = nbt.get_uuid("Trusted0") {
            self.add_trusted_uuid(t0);
        }
        if let Some(t1) = nbt.get_uuid("Trusted1") {
            self.add_trusted_uuid(t1);
        }
        if let Some(pumpkin_nbt::tag::NbtTag::List(list)) = nbt.get("Trusted") {
            for tag in list {
                if let pumpkin_nbt::tag::NbtTag::IntArray(ints) = tag {
                    if ints.len() == 4 {
                        let u128_val = ((ints[0] as u32 as u128) << 96)
                            | ((ints[1] as u32 as u128) << 64)
                            | ((ints[2] as u32 as u128) << 32)
                            | (ints[3] as u32 as u128);
                        self.add_trusted_uuid(Uuid::from_u128(u128_val));
                    }
                }
            }
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        self.set_variant(FoxVariant::from_name(name));
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla Fox.java:213-220: held food consumption after 600 ticks
        let item_in_mouth = {
            let equip = self
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equip
                .equipment
                .get(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND)
                .cloned()
                .unwrap_or_else(|| ItemStack::EMPTY.clone())
        };

        if !item_in_mouth.is_empty()
            && (self.is_food(&item_in_mouth) || item_in_mouth.item.has_tag(&tag::Item::MINECRAFT_FOX_FOOD))
        {
            let ticks = self.ticks_since_eaten.fetch_add(1, Ordering::Relaxed) + 1;
            if combat::can_fox_eat_item(true, ticks) {
                self.ticks_since_eaten.store(0, Ordering::Relaxed);
                self.mob_entity.living_entity.heal(2.0);
                let mut remaining = item_in_mouth;
                remaining.decrement(1);
                {
                    let mut equip = self
                        .mob_entity
                        .living_entity
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if remaining.is_empty() {
                        equip.equipment.remove(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND);
                    } else {
                        equip.equipment.insert(pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND, remaining.clone());
                    }
                }
                self.mob_entity.living_entity.send_equipment_changes(&[(
                    pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
                    remaining,
                )]);
                let entity = self.get_entity();
                if !entity.is_silent() {
                    entity.world.load().play_sound_fine(
                        Sound::EntityFoxEat,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                        1.0,
                        1.0,
                    );
                }
            }
        } else {
            self.ticks_since_eaten.store(0, Ordering::Relaxed);
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if self.is_sleeping() {
                Sound::EntityFoxSleep
            } else {
                Sound::EntityFoxAmbient
            };
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                if entity.age.load(Ordering::Relaxed) < 0 {
                    1.25
                } else {
                    1.0
                },
            );
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::fox::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::TYPE_ID,
            VarInt(self.get_variant().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::FLAGS_ID,
            self.flags.load(Ordering::Relaxed),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::DATA_TRUSTED_ID_0,
            self.trusted_0.load(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::DATA_TRUSTED_ID_1,
            self.trusted_1.load(),
        );
    }


    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityFoxAmbient)
    }
}

impl CustomSound for FoxEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityFoxDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityFoxHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fox_ticks_since_eaten() {
        assert_eq!(combat::get_fox_min_ticks_before_eat(), 600);
        assert!(!combat::can_fox_eat_item(true, 600));
        assert!(combat::can_fox_eat_item(true, 601));
        assert!(!combat::can_fox_eat_item(false, 601));

        let mut nbt = NbtCompound::new();
        nbt.put_int("TicksSinceEaten", 350);
        assert_eq!(nbt.get_int("TicksSinceEaten"), Some(350));
    }

    #[test]
    fn test_fox_variants_and_ids() {
        assert_eq!(FoxVariant::Red.id(), 0);
        assert_eq!(FoxVariant::Snow.id(), 1);
        assert_eq!(FoxVariant::from_id(0), FoxVariant::Red);
        assert_eq!(FoxVariant::from_id(1), FoxVariant::Snow);
        assert_eq!(FoxVariant::from_name("red"), FoxVariant::Red);
        assert_eq!(FoxVariant::from_name("snow"), FoxVariant::Snow);
    }

    #[test]
    fn test_fox_parity_constants() {
        assert_eq!(FoxEntity::XP_REWARD, 1);
        assert_eq!(combat::FOX_MAX_HEALTH, 10.0);
        assert_eq!(combat::FOX_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(combat::FOX_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(combat::FOX_SAFE_FALL_DISTANCE, 5.0);
        assert_eq!(combat::FOX_FOLLOW_RANGE, 32.0);
        assert_eq!(combat::FOX_PANIC_SPEED, 2.2);
        assert_eq!(combat::FOX_BREED_SPEED, 1.0);
        assert_eq!(combat::FOX_SEEK_SHELTER_SPEED, 1.25);
        assert_eq!(combat::FOX_MELEE_ATTACK_SPEED, 1.2);
        assert_eq!(combat::FOX_AVOID_PLAYER_DISTANCE, 16.0);
        assert_eq!(combat::FOX_AVOID_PLAYER_WALK_SPEED, 1.6);
        assert_eq!(combat::FOX_AVOID_PLAYER_SPRINT_SPEED, 1.4);
        assert_eq!(combat::FOX_AVOID_WOLF_DISTANCE, 8.0);
        assert_eq!(combat::FOX_AVOID_POLAR_BEAR_DISTANCE, 8.0);
        assert_eq!(combat::FOX_FLAG_SITTING, 1);
        assert_eq!(combat::FOX_FLAG_CROUCHING, 4);
        assert_eq!(combat::FOX_FLAG_INTERESTED, 8);
        assert_eq!(combat::FOX_FLAG_POUNCING, 16);
        assert_eq!(combat::FOX_FLAG_SLEEPING, 32);
        assert_eq!(combat::FOX_FLAG_FACEPLANTED, 64);
        assert_eq!(combat::FOX_FLAG_DEFENDING, 128);
    }
}


