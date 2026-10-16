use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pumpkin_data::data_component_impl::{PotionContentsImpl, StatusEffectInstance};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    item::ItemEntity,
    mob::{Mob, MobEntity, equipment::RegionalDifficulty, skeleton::SkeletonEntityBase},
    player::Player,
    shearable::{Shearable, shear_by_player, shearing_loot},
};
use crate::world::World;

pub struct BoggedSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
    sheared: AtomicBool,
}

impl BoggedSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let bogged = Self {
            entity,
            sheared: AtomicBool::new(false),
        };
        Arc::new(bogged)
    }

    #[must_use]
    pub fn is_sheared(&self) -> bool {
        self.sheared.load(Ordering::Relaxed)
    }

    pub fn set_sheared(&self, sheared: bool) {
        self.sheared.store(sheared, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(tracked_data::bogged::DATA_SHEARED, sheared);
    }
}

impl CustomSound for BoggedSkeletonEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBoggedDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBoggedHurt)
    }
}

impl Shearable for BoggedSkeletonEntity {
    fn shear(&self, sound_category: SoundCategory, tool: &ItemStack) -> bool {
        if self.sheared.swap(true, Ordering::Relaxed) {
            return false;
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound(Sound::EntityBoggedShear, sound_category, &pos);

        let drop_pos = Vector3::new(pos.x, pos.y + f64::from(entity.height()), pos.z);
        for drop in shearing_loot(entity, "minecraft:shearing/bogged", tool) {
            world.spawn_entity(Arc::new(ItemEntity::new(
                Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
                drop,
            )));
        }
        self.set_sheared(true);
        true
    }

    fn ready_for_shearing(&self) -> bool {
        !self.is_sheared()
    }
}

impl Mob for BoggedSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_shearable(&self) -> Option<&dyn Shearable> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBoggedAmbient)
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        let world = self.get_entity().world.load();
        if world.get_difficulty() == pumpkin_util::Difficulty::Hard {
            Some(50)
        } else {
            Some(70)
        }
    }

    fn get_arrow_projectile(&self) -> Option<ItemStack> {
        // Plain arrow avoids PotionDurationScale reducing the 100-tick poison duration.
        let mut arrow = ItemStack::new(1, &Item::ARROW);

        let poison_effect = StatusEffectInstance {
            effect_id: Cow::Borrowed("minecraft:poison"),
            amplifier: 0,
            duration: 100,
            ambient: false,
            show_particles: true,
            show_icon: true,
        };

        let potion_contents = PotionContentsImpl {
            potion_id: None,
            custom_color: None,
            custom_effects: vec![poison_effect],
            custom_name: None,
        };

        arrow.set_data_component(potion_contents);
        Some(arrow)
    }

    fn mob_init_data_tracker(&self) {
        self.get_entity()
            .set_synced_data(tracked_data::bogged::DATA_SHEARED, self.is_sheared());
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("sheared", self.is_sheared());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.set_sheared(nbt.get_bool("sheared").unwrap_or(false));
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if item_stack.get_item() == &Item::SHEARS && self.ready_for_shearing() {
            return shear_by_player(self, player, item_stack);
        }
        self.entity.mob_entity.mob_interact(player, item_stack)
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

impl EntityBase for BoggedSkeletonEntity {
    fn get_entity(&self) -> &Entity {
        &self.entity.mob_entity.living_entity.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(&self.entity.mob_entity.living_entity)
    }

    fn get_mob(&self) -> Option<&dyn Mob> {
        Some(self)
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
