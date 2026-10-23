use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::damage::DamageType;
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
                if in_water >= 600 {
                    self.start_drowned_conversion(300);
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
