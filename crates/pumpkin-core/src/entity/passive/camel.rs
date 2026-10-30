use crate::entity::combat;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::CACTUS];

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_BRED: u8 = 8;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;

pub struct CamelEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub dashing: AtomicBool,
    pub dash_cooldown: AtomicI32,
}

impl CamelEntity {
    pub const XP_REWARD: u32 = combat::CAMEL_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let camel = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            dashing: AtomicBool::new(false),
            dash_cooldown: AtomicI32::new(0),
        };

        // Initialize attributes: Max Health = 32.0, Movement Speed = 0.09, Step Height = 1.5, Jump Strength = 0.42
        {
            let mut attributes = camel
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 32.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.09;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(step) = attributes.get_mut(&Attributes::STEP_HEIGHT.id) {
                step.base_value = 1.5;
                step.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(jump) = attributes.get_mut(&Attributes::JUMP_STRENGTH.id) {
                jump.base_value = 0.42;
                jump.dirty.store(true, Ordering::Relaxed);
            }
        }
        camel.mob_entity.living_entity.health.store(32.0);

        let mob_arc = Arc::new(camel);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new(4.0));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(2.5, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(2.5)));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(2.0)));
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
            pumpkin_data::tracked_data::camel::DATA_ID_FLAGS,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn is_saddled(&self) -> bool {
        self.has_flag(FLAG_SADDLE)
    }

    pub fn set_saddled(&self, val: bool) {
        self.set_flag(FLAG_SADDLE, val);
    }

    #[must_use]
    pub fn is_dashing(&self) -> bool {
        self.dashing.load(Ordering::Relaxed)
    }

    pub fn set_dashing(&self, dashing: bool) {
        self.dashing.store(dashing, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::camel::DASH, dashing);
    }

    #[must_use]
    pub fn get_dash_cooldown(&self) -> i32 {
        self.dash_cooldown.load(Ordering::Relaxed)
    }

    pub fn set_dash_cooldown(&self, cooldown: i32) {
        self.dash_cooldown.store(cooldown, Ordering::Relaxed);
    }

    pub fn execute_dash(&self, jump_amount: f32) {
        let entity = self.get_entity();
        let yaw = entity.yaw.load();
        let movement_speed = self
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::MOVEMENT_SPEED);
        let jump_power = self
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::JUMP_STRENGTH);
        let block_speed_factor = 1.0;
        let delta = crate::entity::combat::calculate_camel_dash_velocity(
            yaw.to_radians(),
            jump_amount,
            movement_speed,
            jump_power,
            block_speed_factor,
        );
        entity.velocity.store(entity.velocity.load() + delta);
        self.dash_cooldown.store(55, Ordering::Relaxed);
        self.set_dashing(true);
    }
}

impl AgeableMob for CamelEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for CamelEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_CAMEL_FOOD)
            || item_stack.item == &Item::CACTUS
    }
}

impl Mob for CamelEntity {
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

    fn as_camel(&self) -> Option<&CamelEntity> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Saddle", self.is_saddled());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(saddle) = nbt.get_bool("Saddle") {
            self.set_saddled(saddle);
        }
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

        // Vanilla Camel.tick:
        // if (this.isDashing() && this.dashCooldown < 50 && (this.onGround() || this.isInLiquid() || this.isPassenger()))
        //    this.setDashing(false);
        if self.is_dashing()
            && self.dash_cooldown.load(Ordering::Relaxed) < 50
            && (entity.on_ground.load(Ordering::Relaxed)
                || entity.touching_water.load(Ordering::Relaxed))
        {
            self.set_dashing(false);
        }

        let cd = self.dash_cooldown.load(Ordering::Relaxed);
        if cd > 0 {
            let new_cd = cd - 1;
            self.dash_cooldown.store(new_cd, Ordering::Relaxed);
            if new_cd == 0 {
                let pos = entity.pos.load();
                entity.world.load().play_sound_fine(
                    Sound::EntityCamelDashReady,
                    SoundCategory::Neutral,
                    &pos,
                    1.0,
                    1.0,
                );
            }
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            entity.world.load().play_sound_fine(
                Sound::EntityCamelAmbient,
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
            entity.set_synced_data(pumpkin_data::tracked_data::camel::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::camel::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
        entity.set_synced_data(pumpkin_data::tracked_data::camel::DASH, self.is_dashing());
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if self.is_food(item_stack) {
            let living = &self.mob_entity.living_entity;
            let current_health = living.get_health();
            let max_health = living.get_max_health();
            if current_health < max_health {
                living.heal(2.0);
            }
        }

        if item == &Item::SADDLE && !self.is_saddled() && !self.is_baby() {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityCamelSaddle,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if self.is_saddled() && !self.is_baby() && !self.is_food(item_stack) {
            let ent = &self.mob_entity.living_entity.entity;
            let passengers = ent
                .passengers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if crate::entity::combat::can_camel_accept_passenger(passengers.len()) {
                drop(passengers);
                let world = player.world();
                if let Some(vehicle) = world.get_entity_by_id(ent.entity_id)
                    && let Some(passenger) = world.get_player_by_id(player.entity_id())
                {
                    ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                    return true;
                }
            }
        }

        self.animal_interact(player, item_stack, Sound::EntityCamelAmbient)
    }

    fn get_max_head_rotation(&self) -> f32 {
        30.0
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        false
    }
}

impl CustomSound for CamelEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityCamelDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityCamelHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camel_parity_constants() {
        assert_eq!(CamelEntity::XP_REWARD, 2);
        assert_eq!(combat::CAMEL_MAX_HEALTH, 32.0);
        assert_eq!(combat::CAMEL_MOVEMENT_SPEED, 0.09);
        assert_eq!(combat::CAMEL_STEP_HEIGHT, 1.5);
        assert_eq!(combat::CAMEL_JUMP_STRENGTH, 0.42);
        assert_eq!(combat::CAMEL_DASH_COOLDOWN_TICKS, 55);
    }
}
