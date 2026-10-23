use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, equipment::RegionalDifficulty, skeleton::SkeletonEntityBase},
};
use crate::world::World;

pub struct SkeletonEntity {
    pub entity: Arc<SkeletonEntityBase>,
    pub freezing_time: AtomicI32,
    pub conversion_time: AtomicI32,
}

impl SkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let skeleton = Self {
            entity,
            freezing_time: AtomicI32::new(0),
            conversion_time: AtomicI32::new(-1),
        };
        Arc::new(skeleton)
    }

    #[must_use]
    pub fn is_converting(&self) -> bool {
        self.conversion_time.load(Ordering::Relaxed) >= 0
    }

    pub fn start_freeze_conversion(&self, time: i32) {
        self.conversion_time.store(time, Ordering::Relaxed);
        self.set_converting(true);
    }

    pub fn set_converting(&self, converting: bool) {
        self.get_entity().set_synced_data(
            tracked_data::skeleton::DATA_STRAY_CONVERSION_ID,
            converting,
        );
    }

    fn convert_to_stray(&self) {
        let entity = &self.entity.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();

        let stray = crate::entity::r#type::from_type(
            &pumpkin_data::entity::EntityType::STRAY,
            pos,
            &world,
            uuid::Uuid::new_v4(),
        );

        let stray_base = stray.get_entity();
        stray_base.set_rotation(entity.yaw.load(), entity.pitch.load());
        stray_base.head_yaw.store(entity.head_yaw.load());
        stray_base.velocity.store(entity.velocity.load());

        if let Some(living) = stray.get_living_entity() {
            living.set_health(self.entity.mob_entity.living_entity.health.load());
        }

        if let Some(custom_name) = &**entity.custom_name.load() {
            stray_base.set_custom_name(custom_name.clone());
        }

        {
            let src_equip = self
                .entity
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(living) = stray.get_living_entity() {
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
            Sound::EntitySkeletonConvertedToStray,
            SoundCategory::Hostile,
            &pos,
        );

        world.spawn_entity(stray);
        entity.remove();
    }
}

impl CustomSound for SkeletonEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonHurt)
    }
}

impl Mob for SkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn as_skeleton(&self) -> Option<&SkeletonEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonAmbient)
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        let world = self.get_entity().world.load();
        if world.get_difficulty() == pumpkin_util::Difficulty::Hard {
            Some(20)
        } else {
            Some(40)
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let base_entity = &self.entity.mob_entity.living_entity.entity;
        if !base_entity.is_alive() {
            return;
        }

        // Vanilla Skeleton: Powder snow Stray conversion
        // In powder snow: 140 ticks until conversion begins, then 300 ticks shaking conversion.
        if base_entity.is_in_powder_snow() {
            let conv = self.conversion_time.load(Ordering::Relaxed);
            if conv >= 0 {
                let remaining = conv - 1;
                self.conversion_time.store(remaining, Ordering::Relaxed);
                if remaining < 0 {
                    self.convert_to_stray();
                }
            } else {
                let freeze = self.freezing_time.fetch_add(1, Ordering::Relaxed) + 1;
                if freeze >= 140 {
                    self.start_freeze_conversion(300);
                }
            }
        } else {
            self.freezing_time.store(-1, Ordering::Relaxed);
            if self.is_converting() {
                self.conversion_time.store(-1, Ordering::Relaxed);
                self.set_converting(false);
            }
        }
    }

    fn mob_init_data_tracker(&self) {
        self.get_entity().set_synced_data(
            tracked_data::skeleton::DATA_STRAY_CONVERSION_ID,
            self.is_converting(),
        );
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("StrayConversionTime", self.conversion_time.load(Ordering::Relaxed));
        nbt.put_int("FreezingTime", self.freezing_time.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(time) = nbt.get_int("StrayConversionTime") {
            if time != -1 {
                self.start_freeze_conversion(time);
            }
        }
        if let Some(freeze) = nbt.get_int("FreezingTime") {
            self.freezing_time.store(freeze, Ordering::Relaxed);
        }
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
}
