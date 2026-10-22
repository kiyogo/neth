use crossbeam::atomic::AtomicCell;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI32, Ordering},
};
use uuid::Uuid;

use pumpkin_data::{
    attributes::Attributes,
    data_component_impl::EquipmentSlot,
    effect::StatusEffect,
    entity::EntityStatus,
    item_stack::ItemStack,
    particle::Particle,
    potion::Effect,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    zombie_nautilus_variant::ZombieNautilusVariant,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::VarInt;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub struct ZombieNautilusEntity {
    pub mob_entity: MobEntity,
    pub variant: AtomicCell<ZombieNautilusVariant>,
    pub is_tame: AtomicBool,
    pub owner: AtomicCell<Option<Uuid>>,
    pub is_dashing: AtomicBool,
    pub dash_cooldown: AtomicI32,
    pub is_saddled: AtomicBool,
    pub inventory: Mutex<Vec<ItemStack>>,
}

impl ZombieNautilusEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);

        // Vanilla: Attributes.MOVEMENT_SPEED = 1.1F
        {
            let mut attributes = mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 1.1;
                speed.dirty.store(true, Ordering::Relaxed);
            }
        }

        let nautilus = Self {
            mob_entity,
            variant: AtomicCell::new(ZombieNautilusVariant::Temperate),
            is_tame: AtomicBool::new(false),
            owner: AtomicCell::new(None),
            is_dashing: AtomicBool::new(false),
            dash_cooldown: AtomicI32::new(0),
            is_saddled: AtomicBool::new(false),
            inventory: Mutex::new(vec![ItemStack::new(0, &pumpkin_data::item::Item::AIR); 9]),
        };

        Arc::new(nautilus)
    }

    pub fn get_variant(&self) -> ZombieNautilusVariant {
        self.variant.load()
    }

    pub fn set_variant(&self, variant: ZombieNautilusVariant) {
        self.variant.store(variant);
        self.mob_entity.living_entity.entity.set_synced_data(
            pumpkin_data::tracked_data::zombie_nautilus::DATA_VARIANT_ID,
            VarInt(variant.id() as i32),
        );
    }

    pub fn is_dashing(&self) -> bool {
        self.is_dashing.load(Ordering::Relaxed)
    }

    pub fn set_dashing(&self, dashing: bool) {
        self.is_dashing.store(dashing, Ordering::Relaxed);
        self.mob_entity.living_entity.entity.set_synced_data(
            pumpkin_data::tracked_data::zombie_nautilus::DASH,
            dashing,
        );
    }

    pub fn is_tame(&self) -> bool {
        self.is_tame.load(Ordering::Relaxed)
    }

    pub fn set_tame(&self, tame: bool, owner: Option<Uuid>) {
        self.is_tame.store(tame, Ordering::Relaxed);
        self.owner.store(owner);
    }

    pub fn get_ambient_sound(&self) -> Sound {
        let is_water = self
            .mob_entity
            .living_entity
            .entity
            .touching_water
            .load(Ordering::Relaxed);
        if is_water {
            Sound::EntityZombieNautilusAmbient
        } else {
            Sound::EntityZombieNautilusAmbientLand
        }
    }

    pub fn get_dash_sound(&self) -> Sound {
        let is_water = self
            .mob_entity
            .living_entity
            .entity
            .touching_water
            .load(Ordering::Relaxed);
        if is_water {
            Sound::EntityZombieNautilusDash
        } else {
            Sound::EntityZombieNautilusDashLand
        }
    }

    pub fn get_dash_ready_sound(&self) -> Sound {
        let is_water = self
            .mob_entity
            .living_entity
            .entity
            .touching_water
            .load(Ordering::Relaxed);
        if is_water {
            Sound::EntityZombieNautilusDashReady
        } else {
            Sound::EntityZombieNautilusDashReadyLand
        }
    }

    pub fn get_eat_sound(&self) -> Sound {
        Sound::EntityZombieNautilusEat
    }

    pub fn get_swim_sound(&self) -> Sound {
        Sound::EntityZombieNautilusSwim
    }
}

impl Animal for ZombieNautilusEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item == &pumpkin_data::item::Item::NAUTILUS_SHELL
            || item_stack.item.has_tag(&tag::Item::MINECRAFT_NAUTILUS_FOOD)
            || item_stack.item == &pumpkin_data::item::Item::PUFFERFISH
            || item_stack.item == &pumpkin_data::item::Item::COD
            || item_stack.item == &pumpkin_data::item::Item::SALMON
            || item_stack.item == &pumpkin_data::item::Item::TROPICAL_FISH
    }
}

impl CustomSound for ZombieNautilusEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityZombieNautilusHurt
        } else {
            Sound::EntityZombieNautilusHurtLand
        })
    }

    fn death_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityZombieNautilusDeath
        } else {
            Sound::EntityZombieNautilusDeathLand
        })
    }
}

impl Mob for ZombieNautilusEntity {
    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    /// Water animals never drown.
    fn can_breathe_underwater(&self) -> bool {
        true
    }

    /// WaterAnimal.handleAirSupply.
    fn dries_out_on_land(&self) -> bool {
        true
    }

    /// Monster category despawn behavior.
    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        true
    }

    /// Vanilla: ZombieNautilus sun protection is EquipmentSlot.BODY.
    fn sun_protection_slot(&self) -> EquipmentSlot {
        EquipmentSlot::BODY
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("Variant", self.variant.load().to_name().to_string());
        nbt.put_bool("IsTame", self.is_tame.load(Ordering::Relaxed));
        nbt.put_bool("Saddled", self.is_saddled.load(Ordering::Relaxed));
        nbt.put_int("DashCooldown", self.dash_cooldown.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_name) = nbt.get_string("Variant")
            && let Some(variant) = ZombieNautilusVariant::from_name(variant_name)
        {
            self.set_variant(variant);
        }
        if let Some(is_tame) = nbt.get_bool("IsTame") {
            self.is_tame.store(is_tame, Ordering::Relaxed);
        }
        if let Some(saddled) = nbt.get_bool("Saddled") {
            self.is_saddled.store(saddled, Ordering::Relaxed);
        }
        if let Some(dash) = nbt.get_int("DashCooldown") {
            self.dash_cooldown.store(dash, Ordering::Relaxed);
        }
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_init_data_tracker(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        entity.set_synced_data(
            pumpkin_data::tracked_data::zombie_nautilus::DASH,
            self.is_dashing(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::zombie_nautilus::DATA_VARIANT_ID,
            VarInt(self.variant.load().id() as i32),
        );
    }

    fn mob_set_variant_name(&self, name: &str) {
        if let Some(variant) = ZombieNautilusVariant::from_name(name) {
            self.set_variant(variant);
        }
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(self.get_ambient_sound())
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;

        let Ok(passengers) = entity.passengers.try_lock() else {
            return;
        };
        if let Some(passenger) = passengers.first()
            && let Some(player) = passenger.cast_any().downcast_ref::<Player>()
        {
            let world = entity.world.load();
            let game_time = world.get_world_age();
            if game_time % 40 == 0 {
                let player_arc = world.get_player_by_uuid(player.gameprofile.id);
                if let Some(p) = player_arc {
                    p.add_effect(Effect {
                        effect_type: &StatusEffect::BREATH_OF_THE_NAUTILUS,
                        duration: 60,
                        amplifier: 0,
                        ambient: true,
                        show_particles: true,
                        show_icon: true,
                        blend: true,
                    });
                }
            }
        }

        if self.is_dashing() && self.dash_cooldown.load(Ordering::Relaxed) < 35 {
            self.set_dashing(false);
        }

        let cooldown = self.dash_cooldown.load(Ordering::Relaxed);
        if cooldown > 0 {
            let next = cooldown - 1;
            self.dash_cooldown.store(next, Ordering::Relaxed);
            if next == 0 {
                let world = entity.world.load();
                world.play_sound(
                    self.get_dash_ready_sound(),
                    SoundCategory::Hostile,
                    &entity.pos.load(),
                );
            }
        }

        if entity.touching_water.load(Ordering::Relaxed) {
            let velo = entity.velocity.load();
            let speed = velo.length();
            let prob = (speed * 2.0).clamp(0.15, 1.0);
            if rand::random::<f64>() < prob {
                let world = entity.world.load();
                let pos = entity.pos.load();
                world.spawn_particle(
                    pos + Vector3::new(0.0, 0.25, 0.0),
                    Vector3::new(0.4, 0.4, 0.4),
                    0.5,
                    2,
                    Particle::Bubble,
                );
            }
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let mob_entity = &self.mob_entity;
        let entity = &mob_entity.living_entity.entity;

        if !self.is_tame() && self.is_food(item_stack) {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            if rand::random::<u32>().is_multiple_of(3) {
                self.set_tame(true, Some(player.gameprofile.id));
                let world = entity.world.load();
                world.send_entity_status(entity, EntityStatus::TamingSucceeded, None);
            } else {
                let world = entity.world.load();
                world.send_entity_status(entity, EntityStatus::TamingFailed, None);
            }
            let world = entity.world.load();
            world.play_sound(
                self.get_eat_sound(),
                SoundCategory::Hostile,
                &entity.pos.load(),
            );
            return true;
        }

        if self.is_tame() && !player.get_entity().is_sneaking() {
            if !self.is_saddled.load(Ordering::Relaxed)
                && item_stack.item == &pumpkin_data::item::Item::SADDLE
            {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.is_saddled.store(true, Ordering::Relaxed);
                let world = entity.world.load();
                world.play_sound(
                    Sound::ItemNautilusSaddleEquip,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                );
                return true;
            }

            let world = player.world();
            if let Some(vehicle) = world.get_entity_by_id(entity.entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                entity.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }

        self.animal_interact(player, item_stack, self.get_ambient_sound())
    }

    fn is_saddled(&self) -> bool {
        self.is_saddled.load(Ordering::Relaxed)
    }

    fn can_be_saddled(&self) -> bool {
        self.mob_entity.living_entity.entity.is_alive()
    }

    fn set_saddled(&self, saddled: bool) {
        self.is_saddled.store(saddled, Ordering::Relaxed);
    }
}
