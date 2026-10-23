use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::Block;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::{EntityType, MobCategory};
use pumpkin_data::environment_attribute::EnvironmentAttribute;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, ranged_attack::RangedAttackGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    item::ItemEntity,
    mob::{Mob, MobEntity, RangedAttackMob},
    player::Player,
    projectile::snowball::SnowballEntity,
    shearable::{Shearable, shear_by_player, shearing_loot},
};

pub struct SnowGolemEntity {
    pub mob_entity: MobEntity,
    pub has_pumpkin: AtomicBool,
}

impl SnowGolemEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let snow_golem = Self {
            mob_entity,
            has_pumpkin: AtomicBool::new(true),
        };
        let mob_arc = Arc::new(snow_golem);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let ranged_weak: Weak<dyn RangedAttackMob> = {
            let ranged_arc: Arc<dyn RangedAttackMob> = mob_arc.clone();
            Arc::downgrade(&ranged_arc)
        };

        // Vanilla SnowGolem: MAX_HEALTH = 4.0, MOVEMENT_SPEED = 0.2
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
                speed.base_value = 0.2;
                speed.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(4.0);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(
                1,
                Box::new(RangedAttackGoal::new(ranged_weak, 1.25, 20, 10.0)),
            );
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(
                1,
                ActiveTargetGoal::predicated(&mob_arc.mob_entity, 10, true, |living, _world| {
                    living.entity.entity_type.category == &MobCategory::MONSTER
                }),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn has_pumpkin(&self) -> bool {
        self.has_pumpkin.load(Ordering::Relaxed)
    }

    pub fn set_has_pumpkin(&self, has_pumpkin: bool) {
        self.has_pumpkin.store(has_pumpkin, Ordering::Relaxed);
        let entity = self.get_entity();
        let flags = if has_pumpkin { 16u8 } else { 0u8 };
        entity.set_synced_data(
            pumpkin_data::tracked_data::snow_golem::DATA_PUMPKIN_ID,
            flags as i8,
        );
    }

    pub fn throw_snowball(&self, target: &Arc<dyn EntityBase>) {
        let entity = self.get_entity();
        let world = entity.world.load_full();

        let snowball_entity = Entity::new(world.clone(), entity.pos.load(), &EntityType::SNOWBALL);
        let snowball = SnowballEntity::new_shot(snowball_entity, entity);

        let mob_pos = entity.pos.load();
        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let target_height = f64::from(target_entity.entity_dimension.load().height);

        let dx = target_pos.x - mob_pos.x;
        let dy = (target_pos.y + target_height / 3.0) - snowball.get_entity().pos.load().y;
        let dz = target_pos.z - mob_pos.z;
        let horizontal_distance = dx.hypot(dz);
        let yo = horizontal_distance * 0.2;

        snowball.thrown.set_velocity(dx, dy + yo, dz, 1.6, 12.0);

        if !entity.silent.load(Ordering::Relaxed) {
            world.play_sound(Sound::EntitySnowballThrow, SoundCategory::Neutral, &mob_pos);
        }

        let snowball_arc: Arc<dyn EntityBase> = Arc::new(snowball);
        world.spawn_entity(snowball_arc);
    }
}

impl Shearable for SnowGolemEntity {
    fn shear(&self, sound_category: SoundCategory, tool: &ItemStack) -> bool {
        if !self.has_pumpkin.swap(false, Ordering::Relaxed) {
            return false;
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound(Sound::EntitySnowGolemShear, sound_category, &pos);
        self.set_has_pumpkin(false);

        let eye_height = f64::from(entity.entity_dimension.load().eye_height);
        let drop_pos = Vector3::new(pos.x, pos.y + eye_height, pos.z);
        for drop in shearing_loot(entity, "minecraft:shearing/snow_golem", tool) {
            world.spawn_entity(Arc::new(ItemEntity::new(
                Entity::new(world.clone(), drop_pos, &EntityType::ITEM),
                drop,
            )));
        }
        true
    }

    fn ready_for_shearing(&self) -> bool {
        self.has_pumpkin()
    }
}

impl Mob for SnowGolemEntity {
    fn as_snow_golem(&self) -> Option<&SnowGolemEntity> {
        Some(self)
    }

    fn as_shearable(&self) -> Option<&dyn Shearable> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("Pumpkin", self.has_pumpkin());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(pumpkin) = nbt.get_bool("Pumpkin") {
            self.set_has_pumpkin(pumpkin);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn is_sensitive_to_water(&self) -> bool {
        true
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let world = entity.world.load_full();
        let feet_pos = entity.block_pos.load();

        // Water/rain sensitivity (vanilla LivingEntity#aiStep)
        let raining_at_feet = world.is_raining_at(&feet_pos);
        let raining_at_head = world.is_raining_at(&entity.bounding_box.load().max_block_pos());
        if entity.touching_water.load(Ordering::Relaxed) || raining_at_feet || raining_at_head {
            caller.damage(caller, 1.0, DamageType::DROWN);
        }

        // Snow golem melts in warm biomes or dimensions (vanilla SnowGolem#aiStep)
        let melts = world.environment_attributes().get_value_bool(
            EnvironmentAttribute::GameplaySnowGolemMelts,
            &feet_pos,
        ) || {
            let biome = world.get_biome(&feet_pos);
            biome.weather.base_temperature() > 1.0
        };
        if melts {
            caller.damage(caller, 1.0, DamageType::ON_FIRE);
        }

        if !entity.is_alive() {
            return;
        }

        // Ambient sound
        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            world.play_sound_fine(
                Sound::EntitySnowGolemAmbient,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }

        // Vanilla SnowGolem#aiStep: check mob griefing before placing snow trail
        if !world.level_info.load().game_rules.mob_griefing {
            return;
        }

        let pos = entity.pos.load();
        let snow_state_id = Block::SNOW.default_state.id;

        // In vanilla, SnowGolem places snow under 4 quadrant points around its base
        for i in 0..4 {
            let offset_x = ((i % 2 * 2) as f64 - 1.0) * 0.25;
            let offset_z = ((i / 2 % 2 * 2) as f64 - 1.0) * 0.25;
            let xx = (pos.x + offset_x).floor() as i32;
            let yy = pos.y.floor() as i32;
            let zz = (pos.z + offset_z).floor() as i32;
            let snow_pos = BlockPos::new(xx, yy, zz);

            if world.get_block_state(&snow_pos).is_air()
                && crate::block::blocks::snow::can_place_at(world.as_ref(), &snow_pos)
            {
                world.set_block_state(&snow_pos, snow_state_id, BlockFlags::NOTIFY_ALL);
                world.emit_game_event(GameEvent::BlockPlace.name(), snow_pos.to_centered_f64());
            }
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let flags = if self.has_pumpkin() { 16u8 } else { 0u8 };
        entity.set_synced_data(
            pumpkin_data::tracked_data::snow_golem::DATA_PUMPKIN_ID,
            flags as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if item_stack.get_item() == &Item::SHEARS && self.ready_for_shearing() {
            return shear_by_player(self, player, item_stack);
        }
        false
    }
}

impl CustomSound for SnowGolemEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySnowGolemDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySnowGolemHurt)
    }
}

impl RangedAttackMob for SnowGolemEntity {
    fn perform_ranged_attack(&self, target: &Arc<dyn EntityBase>, _power: f32) {
        self.throw_snowball(target);
    }
}
