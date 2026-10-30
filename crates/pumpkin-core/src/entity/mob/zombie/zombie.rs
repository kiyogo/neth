use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::damage::DamageType;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::custom_sound::CustomSound;
use crate::entity::mob::equipment::RegionalDifficulty;
use crate::entity::mob::zombie::ZombieEntityBase;
use crate::entity::mob::{Mob, MobEntity};
use crate::entity::{Entity, EntityBase};
use crate::world::World;

use std::sync::atomic::AtomicI32;

use pumpkin_data::tracked_data;

pub struct ZombieEntity {
    pub entity: Arc<ZombieEntityBase>,
    pub in_water_time: AtomicI32,
    pub drowned_conversion_time: AtomicI32,
}

impl ZombieEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = ZombieEntityBase::new(entity);
        let zombie = Self {
            entity,
            in_water_time: AtomicI32::new(0),
            drowned_conversion_time: AtomicI32::new(-1),
        };
        Arc::new(zombie)
    }

    #[must_use]
    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let entity = ZombieEntityBase::with_can_break_doors(entity, can_break_doors);
        let zombie = Self {
            entity,
            in_water_time: AtomicI32::new(0),
            drowned_conversion_time: AtomicI32::new(-1),
        };
        Arc::new(zombie)
    }

    #[must_use]
    pub fn is_converting(&self) -> bool {
        self.drowned_conversion_time.load(Ordering::Relaxed) >= 0
    }

    pub fn start_drowned_conversion(&self, time: i32) {
        self.drowned_conversion_time.store(time, Ordering::Relaxed);
        self.set_converting(true);
    }

    pub fn set_converting(&self, converting: bool) {
        self.get_entity().set_synced_data(
            tracked_data::zombie::DATA_DROWNED_CONVERSION_ID,
            converting,
        );
    }

    fn convert_to_drowned(&self) {
        let entity = &self.entity.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();

        let drowned = crate::entity::r#type::from_type(
            &pumpkin_data::entity::EntityType::DROWNED,
            pos,
            &world,
            uuid::Uuid::new_v4(),
        );

        let drowned_base = drowned.get_entity();
        drowned_base.set_rotation(entity.yaw.load(), entity.pitch.load());
        drowned_base.head_yaw.store(entity.head_yaw.load());
        drowned_base.velocity.store(entity.velocity.load());

        if let Some(living) = drowned.get_living_entity() {
            living.set_health(self.entity.mob_entity.living_entity.health.load());
        }

        if let Some(custom_name) = &**entity.custom_name.load() {
            drowned_base.set_custom_name(custom_name.clone());
        }

        {
            let src_equip = self
                .entity
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(living) = drowned.get_living_entity() {
                let mut dst_equip = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for (slot, item) in &src_equip.equipment {
                    dst_equip.put(slot, item.clone());
                }
            }
        }

        world.play_sound(
            Sound::EntityZombieConvertedToDrowned,
            SoundCategory::Hostile,
            &pos,
        );

        world.spawn_entity(drowned);
        entity.remove();
    }
}

impl CustomSound for ZombieEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityZombieDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityZombieHurt)
    }
}

impl Mob for ZombieEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn as_zombie(&self) -> Option<&ZombieEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        let base_entity = &self.entity.mob_entity.living_entity.entity;
        if !base_entity.is_on_fire() {
            return;
        }
        let equipment = self
            .entity
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let main_hand = equipment.get(&EquipmentSlot::MAIN_HAND);
        if main_hand.is_empty() {
            let world = base_entity.world.load();
            let difficulty = match world.level_info.load().difficulty {
                pumpkin_util::Difficulty::Peaceful => 0.0,
                pumpkin_util::Difficulty::Easy => 1.0,
                pumpkin_util::Difficulty::Normal => 2.0,
                pumpkin_util::Difficulty::Hard => 3.0,
            };
            let rand_val = rand::random::<f32>();
            if let Some(seconds) = crate::entity::combat::calculate_zombie_fire_attack_seconds(
                true,
                true,
                difficulty,
                rand_val,
            ) {
                target.set_on_fire_for(seconds as f32);
            }
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let base_entity = &self.entity.mob_entity.living_entity.entity;
        if !base_entity.is_alive() {
            return;
        }

        // Vanilla Zombie: Drowned conversion in water
        // Submerged in water: 600 ticks (30s) until conversion begins, then 300 ticks shaking conversion.
        if base_entity.is_submerged_in_water() {
            let conv = self.drowned_conversion_time.load(Ordering::Relaxed);
            if conv >= 0 {
                let remaining = conv - 1;
                self.drowned_conversion_time.store(remaining, Ordering::Relaxed);
                if remaining < 0 {
                    self.convert_to_drowned();
                }
            } else {
                let in_water = self.in_water_time.fetch_add(1, Ordering::Relaxed) + 1;
                if in_water >= crate::entity::combat::ZOMBIE_DROWN_CONVERSION_SUBMERGED_TICKS {
                    self.start_drowned_conversion(crate::entity::combat::ZOMBIE_DROWN_CONVERSION_SHAKING_TICKS);
                }
            }
        } else {
            self.in_water_time.store(-1, Ordering::Relaxed);
            if self.is_converting() {
                self.drowned_conversion_time.store(-1, Ordering::Relaxed);
                self.set_converting(false);
            }
        }

        // Ambient sound
        if rand::random_range(0..100) == 0 && !base_entity.silent.load(Ordering::Relaxed) {
            base_entity.world.load().play_sound_fine(
                Sound::EntityZombieAmbient,
                SoundCategory::Hostile,
                &base_entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn mob_init_data_tracker(&self) {
        self.get_entity().set_synced_data(
            tracked_data::zombie::DATA_DROWNED_CONVERSION_ID,
            self.is_converting(),
        );
    }

    fn spawn_as_baby(&self) -> bool {
        self.entity.spawn_as_baby()
    }

    fn populate_default_equipment_slots(
        &self,
        world: &Arc<World>,
        difficulty: &RegionalDifficulty,
    ) {
        self.entity
            .populate_default_equipment_slots(world, difficulty);
    }

    fn populate_default_equipment_enchantments(&self, difficulty: &RegionalDifficulty) {
        self.entity
            .populate_default_equipment_enchantments(difficulty);
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.entity.mob_write_nbt(nbt);
        nbt.put_int(
            "DrownedConversionTime",
            self.drowned_conversion_time.load(Ordering::Relaxed),
        );
        nbt.put_int("InWaterTime", self.in_water_time.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.entity.mob_read_nbt(nbt);
        if let Some(time) = nbt.get_int("DrownedConversionTime") {
            if time != -1 {
                self.start_drowned_conversion(time);
            }
        }
        if let Some(in_water) = nbt.get_int("InWaterTime") {
            self.in_water_time.store(in_water, Ordering::Relaxed);
        }
    }

    fn on_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) {
        self.entity.try_spawn_reinforcements(source);
    }

    fn get_base_experience_reward(&self) -> u32 {
        Mob::get_base_experience_reward(self.entity.as_ref())
    }
}

impl ZombieEntity {
    #[must_use]
    pub fn can_break_doors(&self) -> bool {
        self.entity
            .can_break_doors
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_can_break_doors(&self, can_break: bool) {
        self.entity
            .can_break_doors
            .store(can_break, std::sync::atomic::Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_baby(&self) -> bool {
        self.entity.is_baby()
    }

    pub fn set_baby(&self, baby: bool) {
        self.entity.set_baby(baby);
    }
}

#[cfg(test)]
mod tests {
    use crate::entity::combat::{
        ZOMBIE_BABY_SPEED_MODIFIER_AMOUNT, ZOMBIE_BABY_SPEED_MODIFIER_ID,
        ZOMBIE_BASE_ARMOR, ZOMBIE_BASE_ATTACK_DAMAGE, ZOMBIE_BASE_MOVEMENT_SPEED,
        ZOMBIE_DROWN_CONVERSION_SHAKING_TICKS, ZOMBIE_DROWN_CONVERSION_SUBMERGED_TICKS,
        ZOMBIE_FOLLOW_RANGE, ZOMBIE_MAX_HEALTH, ZOMBIE_XP_REWARD,
    };

    #[test]
    fn test_zombie_drowning_conversion_and_baby_speed_parity() {
        assert_eq!(ZOMBIE_MAX_HEALTH, 20.0);
        assert_eq!(ZOMBIE_FOLLOW_RANGE, 35.0);
        assert_eq!(ZOMBIE_BASE_MOVEMENT_SPEED, 0.23);
        assert_eq!(ZOMBIE_BASE_ATTACK_DAMAGE, 3.0);
        assert_eq!(ZOMBIE_BASE_ARMOR, 2.0);
        assert_eq!(ZOMBIE_XP_REWARD, 5);

        assert_eq!(ZOMBIE_BABY_SPEED_MODIFIER_ID, "minecraft:baby");
        assert_eq!(ZOMBIE_BABY_SPEED_MODIFIER_AMOUNT, 0.5);

        assert_eq!(ZOMBIE_DROWN_CONVERSION_SUBMERGED_TICKS, 600);
        assert_eq!(ZOMBIE_DROWN_CONVERSION_SHAKING_TICKS, 300);
    }

    #[test]
    fn test_zombie_fire_attack_and_leader_parity() {
        use crate::entity::combat::{
            calculate_zombie_fire_attack_seconds, ZOMBIE_BREAK_DOOR_CHANCE,
            ZOMBIE_FIRE_ATTACK_CHANCE_FACTOR, ZOMBIE_FIRE_ATTACK_DURATION_MULTIPLIER,
            ZOMBIE_LEADER_CHANCE, ZOMBIE_REINFORCEMENT_ATTEMPTS,
            ZOMBIE_REINFORCEMENT_CALLEE_CHARGE, ZOMBIE_REINFORCEMENT_CALLER_CHARGE,
            ZOMBIE_REINFORCEMENT_RANGE_MAX, ZOMBIE_REINFORCEMENT_RANGE_MIN,
            ZOMBIE_TO_DROWNED_CONVERSION_SOUND_EVENT_ID,
            ZOMBIE_TO_ZOMBIE_VILLAGER_CONVERSION_SOUND_EVENT_ID,
        };

        assert_eq!(ZOMBIE_LEADER_CHANCE, 0.05);
        assert_eq!(ZOMBIE_REINFORCEMENT_ATTEMPTS, 50);
        assert_eq!(ZOMBIE_REINFORCEMENT_RANGE_MAX, 40);
        assert_eq!(ZOMBIE_REINFORCEMENT_RANGE_MIN, 7);
        assert_eq!(ZOMBIE_BREAK_DOOR_CHANCE, 0.1);
        assert_eq!(ZOMBIE_REINFORCEMENT_CALLEE_CHARGE, -0.05);
        assert_eq!(ZOMBIE_REINFORCEMENT_CALLER_CHARGE, -0.05);
        assert_eq!(ZOMBIE_TO_ZOMBIE_VILLAGER_CONVERSION_SOUND_EVENT_ID, 1026);
        assert_eq!(ZOMBIE_TO_DROWNED_CONVERSION_SOUND_EVENT_ID, 1040);

        assert_eq!(ZOMBIE_FIRE_ATTACK_CHANCE_FACTOR, 0.3);
        assert_eq!(ZOMBIE_FIRE_ATTACK_DURATION_MULTIPLIER, 2);

        // Not on fire -> no ignition
        assert_eq!(calculate_zombie_fire_attack_seconds(false, true, 2.0, 0.1), None);
        // Holding weapon -> no ignition
        assert_eq!(calculate_zombie_fire_attack_seconds(true, false, 2.0, 0.1), None);
        // Peaceful difficulty -> no ignition
        assert_eq!(calculate_zombie_fire_attack_seconds(true, true, 0.0, 0.1), None);

        // Easy (difficulty = 1.0, threshold = 0.3):
        assert_eq!(calculate_zombie_fire_attack_seconds(true, true, 1.0, 0.1), Some(2));
        assert_eq!(calculate_zombie_fire_attack_seconds(true, true, 1.0, 0.35), None);

        // Normal (difficulty = 2.0, threshold = 0.6):
        assert_eq!(calculate_zombie_fire_attack_seconds(true, true, 2.0, 0.5), Some(4));
        assert_eq!(calculate_zombie_fire_attack_seconds(true, true, 2.0, 0.7), None);

        // Hard (difficulty = 3.0, threshold = 0.9):
        assert_eq!(calculate_zombie_fire_attack_seconds(true, true, 3.0, 0.8), Some(6));
        assert_eq!(calculate_zombie_fire_attack_seconds(true, true, 3.0, 0.95), None);
    }
}

