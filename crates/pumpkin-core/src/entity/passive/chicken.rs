use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering, Ordering::Relaxed},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::chicken_sound_variant::ChickenSoundVariant;
use pumpkin_data::chicken_variant::ChickenVariant;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::{entity::EntityType, item::Item};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use rand::RngExt;

use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ageable::AgeableMob,
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[
    &Item::WHEAT_SEEDS,
    &Item::MELON_SEEDS,
    &Item::PUMPKIN_SEEDS,
    &Item::BEETROOT_SEEDS,
    &Item::TORCHFLOWER_SEEDS,
    &Item::PITCHER_POD,
];

/// Represents a Chicken, a passive mob that lays eggs and is immune to fall damage.
///
/// Wiki: <https://minecraft.wiki/w/Chicken>
pub struct ChickenEntity {
    pub mob_entity: MobEntity,
    pub variant: AtomicU8,
    pub sound_variant: AtomicU8,
    pub is_chicken_jockey: AtomicBool,
    egg_lay_time: AtomicI32,
    pub ageable_data: crate::entity::ageable::AgeableData,
}

impl ChickenEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let world = entity.world.load();
        let biome = world.get_biome(&entity.block_pos.load());
        let variant = ChickenVariant::select_for_biome(biome.registry_id);
        let mob_entity = MobEntity::new(entity);
        let egg_lay_time = rand::rng().random_range(6000..12000);
        let chicken = Self {
            mob_entity,
            variant: AtomicU8::new(variant.id()),
            sound_variant: AtomicU8::new(ChickenSoundVariant::Classic as u8),
            is_chicken_jockey: AtomicBool::new(false),
            egg_lay_time: AtomicI32::new(egg_lay_time),
            ageable_data: crate::entity::ageable::AgeableData::default(),
        };
        let mob_arc = Arc::new(chicken);

        // Vanilla Chicken attributes: MAX_HEALTH = 4.0, MOVEMENT_SPEED = 0.25
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 4.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.25;
                speed.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(4.0);

        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.4));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.0, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> ChickenVariant {
        ChickenVariant::from_id(self.variant.load(Ordering::Relaxed)).unwrap_or_default()
    }

    #[must_use]
    pub fn is_chicken_jockey(&self) -> bool {
        self.is_chicken_jockey.load(Ordering::Relaxed)
    }

    pub fn set_chicken_jockey(&self, is_jockey: bool) {
        self.is_chicken_jockey.store(is_jockey, Ordering::Relaxed);
    }

    pub fn set_variant(&self, variant: ChickenVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::chicken::DATA_VARIANT_ID,
            VarInt(variant.id() as i32),
        );
    }

    pub fn set_sound_variant(&self, sound_variant: ChickenSoundVariant) {
        self.sound_variant
            .store(sound_variant as u8, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::chicken::DATA_SOUND_VARIANT_ID,
            VarInt(sound_variant as u8 as i32),
        );
    }
}

impl CustomSound for ChickenEntity {
    fn death_sound(&self) -> Option<Sound> {
        let is_baby = self.is_baby();
        let sound_variant =
            ChickenSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
                .unwrap_or_default();
        Some(sound_variant.death_sound(is_baby))
    }

    fn hurt_sound(&self) -> Option<Sound> {
        let is_baby = self.is_baby();
        let sound_variant =
            ChickenSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
                .unwrap_or_default();
        Some(sound_variant.hurt_sound(is_baby))
    }
}

impl AgeableMob for ChickenEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl Animal for ChickenEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        use pumpkin_data::tag::Taggable;
        item_stack
            .item
            .has_tag(&pumpkin_data::tag::Item::MINECRAFT_CHICKEN_FOOD)
            || TEMPT_ITEMS.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for ChickenEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_chicken(&self) -> Option<&ChickenEntity> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("IsChickenJockey", self.is_chicken_jockey());
        nbt.put_int("EggLayTime", self.egg_lay_time.load(Ordering::Relaxed));
        let variant =
            ChickenVariant::from_id(self.variant.load(Ordering::Relaxed)).unwrap_or_default();
        nbt.put_string("variant", format!("minecraft:{}", variant.to_name()));
        let sound_variant =
            ChickenSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
                .unwrap_or_default();
        nbt.put_string(
            "sound_variant",
            format!("minecraft:{}", sound_variant.to_name()),
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(jockey) = nbt.get_bool("IsChickenJockey") {
            self.set_chicken_jockey(jockey);
        }
        self.egg_lay_time
            .store(nbt.get_int("EggLayTime").unwrap_or(6000), Ordering::Relaxed);
        if let Some(variant_str) = nbt.get_string("variant")
            && let Some(variant) = ChickenVariant::from_name(variant_str)
        {
            self.set_variant(variant);
        }
        if let Some(sound_str) = nbt.get_string("sound_variant")
            && let Some(sound_variant) = ChickenSoundVariant::from_name(sound_str)
        {
            self.set_sound_variant(sound_variant);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        if let Some(v) = ChickenVariant::from_name(name) {
            self.set_variant(v);
        }
    }

    fn mob_set_sound_variant_name(&self, name: &str) {
        if let Some(v) = ChickenSoundVariant::from_name(name) {
            self.set_sound_variant(v);
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::chicken::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::chicken::DATA_VARIANT_ID,
            VarInt(self.variant.load(Ordering::Relaxed) as i32),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::chicken::DATA_SOUND_VARIANT_ID,
            VarInt(self.sound_variant.load(Ordering::Relaxed) as i32),
        );
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        if self.mob_entity.living_entity.dead.load(Relaxed) {
            return;
        }
        let entity = &self.mob_entity.living_entity.entity;
        let current_velocity = entity.velocity.load();
        let on_ground = entity.on_ground.load(Ordering::Relaxed);

        // TODO: move velocity logic to physics tick when implemented
        if (!on_ground) && current_velocity.y < 0.0 {
            entity.set_velocity(current_velocity.multiply(1.0, 0.6, 1.0));
        }
        let egg_time = self.egg_lay_time.fetch_sub(1, Ordering::Relaxed) - 1;
        if crate::entity::combat::should_chicken_lay_egg(
            self.is_baby(),
            self.is_chicken_jockey(),
            egg_time,
        ) {
            let next_time = rand::rng().random_range(6000..12000);
            let world = entity.world.load_full();
            let pos = entity.block_pos.load();
            let entity_id = entity.entity_id;
            let mut drop_event =
                crate::plugin::api::events::entity::entity_drop_item::EntityDropItemEvent::new(
                    entity_id,
                    "minecraft:egg".to_string(),
                    1,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut drop_event);
            }
            if !drop_event.cancelled {
                let pitch = crate::entity::combat::get_chicken_egg_lay_sound_pitch(
                    rand::random(),
                    rand::random(),
                );
                world.play_sound_fine(
                    Sound::EntityChickenEgg,
                    pumpkin_data::sound::SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    pitch,
                );
                world.drop_stack(&pos, ItemStack::new(1, &Item::EGG));
            }
            self.egg_lay_time.store(next_time, Ordering::Relaxed);
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let sound_variant =
            ChickenSoundVariant::from_id(self.sound_variant.load(Ordering::Relaxed))
                .unwrap_or_default();
        self.animal_interact(
            player,
            item_stack,
            sound_variant.ambient_sound(self.is_baby()),
        )
    }
}
