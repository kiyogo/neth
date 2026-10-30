use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, combat,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

pub struct SkeletonHorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
    pub is_trap: AtomicBool,
    pub trap_time: AtomicI32,
}

impl SkeletonHorseEntity {
    pub const XP_REWARD: u32 = combat::ABSTRACT_HORSE_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            is_trap: AtomicBool::new(false),
            trap_time: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(horse);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla SkeletonHorse: MAX_HEALTH = 15.0, MOVEMENT_SPEED = 0.2, JUMP_STRENGTH = 0.7, STEP_HEIGHT = 1.0, SAFE_FALL_DISTANCE = 6.0, FALL_DAMAGE_MULTIPLIER = 0.5
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MAX_HEALTH,
                combat::SKELETON_HORSE_MAX_HEALTH,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::MOVEMENT_SPEED,
                combat::SKELETON_HORSE_MOVEMENT_SPEED,
            );
            mob_arc.mob_entity.living_entity.set_attribute_base(
                &Attributes::JUMP_STRENGTH,
                combat::SKELETON_HORSE_JUMP_STRENGTH,
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
            mob_arc.mob_entity.living_entity.set_health(combat::SKELETON_HORSE_MAX_HEALTH as f32);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Vanilla SkeletonHorse overrides addBehaviourGoals() empty: no SwimGoal (walks underwater) and no PanicGoal
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
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::skeleton_horse::DATA_ID_FLAGS,
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

    #[must_use]
    pub fn is_trap(&self) -> bool {
        self.is_trap.load(Ordering::Relaxed)
    }

    pub fn set_trap(&self, trap: bool) {
        self.is_trap.store(trap, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get_trap_time(&self) -> i32 {
        self.trap_time.load(Ordering::Relaxed)
    }

    pub fn set_trap_time(&self, time: i32) {
        self.trap_time.store(time, Ordering::Relaxed);
    }
}

impl AgeableMob for SkeletonHorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for SkeletonHorseEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl Mob for SkeletonHorseEntity {
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

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        nbt.put_bool("SkeletonTrap", self.is_trap());
        nbt.put_int("SkeletonTrapTime", self.get_trap_time());
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(trap) = nbt.get_bool("SkeletonTrap") {
            self.set_trap(trap);
        }
        if let Some(time) = nbt.get_int("SkeletonTrapTime") {
            self.set_trap_time(time);
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

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_skeleton_horse(&self) -> Option<&SkeletonHorseEntity> {
        Some(self)
    }

    fn can_breathe_under_water(&self) -> bool {
        true
    }

    fn can_drown(&self) -> bool {
        false
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

        if self.is_trap() {
            let time = self.trap_time.fetch_add(1, Ordering::Relaxed) + 1;
            if combat::should_skeleton_trap_despawn(true, time) {
                entity.remove();
                return;
            }

            let pos = entity.pos.load();
            let world = entity.world.load();
            let nearby_players = world.get_nearby_players(pos, 10.0);
            if !nearby_players.is_empty() {
                self.set_trap(false);
                self.set_tame(true);
                entity.set_age(0);
                world.play_sound(Sound::EntityLightningBoltThunder, SoundCategory::Weather, &pos);
            }
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if entity.touching_water.load(Ordering::Relaxed) {
                Sound::EntitySkeletonHorseAmbientWater
            } else {
                Sound::EntitySkeletonHorseAmbient
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
            entity.set_synced_data(
                pumpkin_data::tracked_data::skeleton_horse::DATA_BABY_ID,
                true,
            );
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::skeleton_horse::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if !self.is_tame() {
            return false;
        }

        let item = item_stack.get_item();

        if item == &Item::SADDLE && !self.is_saddled() && !self.is_baby() {
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

        if !self.is_baby() {
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

impl CustomSound for SkeletonHorseEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonHorseDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonHorseHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skeleton_horse_parity_constants() {
        assert_eq!(SkeletonHorseEntity::XP_REWARD, 2);
        assert_eq!(combat::SKELETON_HORSE_MAX_HEALTH, 15.0);
        assert_eq!(combat::SKELETON_HORSE_MOVEMENT_SPEED, 0.2);
        assert_eq!(combat::SKELETON_HORSE_JUMP_STRENGTH, 0.7);
        assert_eq!(combat::SKELETON_HORSE_TRAP_CHANCE, 0.2);
    }
}
