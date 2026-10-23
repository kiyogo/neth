use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::damage::DamageType;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::Difficulty;

use crate::entity::custom_sound::CustomSound;
use crate::entity::mob::equipment::RegionalDifficulty;
use crate::entity::mob::zombie::ZombieEntityBase;
use crate::entity::mob::{Mob, MobEntity};
use crate::entity::{Entity, EntityBase};
use crate::world::World;

use std::sync::atomic::AtomicI32;

use pumpkin_data::tracked_data;

pub struct HuskEntity {
    pub entity: Arc<ZombieEntityBase>,
    pub in_water_time: AtomicI32,
    pub drowned_conversion_time: AtomicI32,
}

impl HuskEntity {
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
            tracked_data::husk::DATA_DROWNED_CONVERSION_ID,
            converting,
        );
    }

    fn convert_to_zombie(&self) {
        let entity = &self.entity.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();

        let zombie = crate::entity::r#type::from_type(
            &pumpkin_data::entity::EntityType::ZOMBIE,
            pos,
            &world,
            uuid::Uuid::new_v4(),
        );

        let zombie_base = zombie.get_entity();
        zombie_base.set_rotation(entity.yaw.load(), entity.pitch.load());
        zombie_base.head_yaw.store(entity.head_yaw.load());
        zombie_base.velocity.store(entity.velocity.load());

        if let Some(living) = zombie.get_living_entity() {
            living.set_health(self.entity.mob_entity.living_entity.health.load());
        }

        if let Some(custom_name) = &**entity.custom_name.load() {
            zombie_base.set_custom_name(custom_name.clone());
        }

        {
            let src_equip = self
                .entity
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(living) = zombie.get_living_entity() {
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
            Sound::EntityHuskConvertedToZombie,
            SoundCategory::Hostile,
            &pos,
        );

        world.spawn_entity(zombie);
        entity.remove();
    }
}

impl CustomSound for HuskEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityHuskDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityHuskHurt)
    }
}

impl Mob for HuskEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn as_husk(&self) -> Option<&HuskEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    // Husks do not burn in daylight in vanilla
    fn burns_in_daylight(&self) -> bool {
        false
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        let equipment = self
            .get_mob_entity()
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let main_hand = equipment.get(&EquipmentSlot::MAIN_HAND);
        // Vanilla Husk: unarmed attacks inflict Hunger (140 * difficulty ticks)
        if main_hand.is_empty() {
            if let Some(living) = target.get_living_entity() {
                let world = self.get_entity().world.load();
                let multiplier = match world.level_info.load().difficulty {
                    Difficulty::Peaceful => 0,
                    Difficulty::Easy => 1,
                    Difficulty::Normal => 2,
                    Difficulty::Hard => 3,
                };
                if multiplier > 0 {
                    living.add_effect(Effect {
                        effect_type: &StatusEffect::HUNGER,
                        duration: 140 * multiplier,
                        amplifier: 0,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                        blend: false,
                    });
                }
            }
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let base_entity = &self.entity.mob_entity.living_entity.entity;
        if !base_entity.is_alive() {
            return;
        }

        // Vanilla Husk: Drowned conversion in water (converts to Zombie)
        // Submerged in water: 600 ticks (30s) until conversion begins, then 300 ticks shaking conversion.
        if base_entity.is_submerged_in_water() {
            let conv = self.drowned_conversion_time.load(Ordering::Relaxed);
            if conv >= 0 {
                let remaining = conv - 1;
                self.drowned_conversion_time.store(remaining, Ordering::Relaxed);
                if remaining < 0 {
                    self.convert_to_zombie();
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
                Sound::EntityHuskAmbient,
                SoundCategory::Hostile,
                &base_entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn mob_init_data_tracker(&self) {
        self.get_entity().set_synced_data(
            tracked_data::husk::DATA_DROWNED_CONVERSION_ID,
            self.is_converting(),
        );
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

    fn spawn_as_baby(&self) -> bool {
        self.entity.spawn_as_baby()
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
