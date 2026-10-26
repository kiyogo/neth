use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    item_steerable::{ItemBasedSteering, ItemSteerable},
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::WARPED_FUNGUS, &Item::WARPED_FUNGUS_ON_A_STICK];

pub struct StriderEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub steering: ItemBasedSteering,
    pub saddled: AtomicBool,
    pub suffocating: AtomicBool,
}

impl StriderEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let strider = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            steering: ItemBasedSteering::default(),
            saddled: AtomicBool::new(false),
            suffocating: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(strider);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla Strider: MAX_HEALTH = 20.0, MOVEMENT_SPEED = 0.175
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 20.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.175;
                speed.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(20.0);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Vanilla Strider.registerGoals: Panic(1.65), Breed(1.0), Tempt(1.4), FollowParent(1.0), RandomStroll(1.0), LookAtPlayer(8.0), RandomLookAround
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.65));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.4, TEMPT_ITEMS, false)));
            goal_selector.add_goal(5, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_ridden_speed(&self) -> f32 {
        let base_speed = self
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::MOVEMENT_SPEED);
        crate::entity::combat::calculate_strider_ridden_speed(
            base_speed,
            self.is_suffocating(),
            self.steering.boost_factor(),
        )
    }

    #[must_use]
    pub fn is_being_tempted(&self) -> bool {
        self.mob_entity
            .goals_selector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_running::<TemptGoal>()
    }

    #[must_use]
    pub fn is_panicking(&self) -> bool {
        self.mob_entity
            .goals_selector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_running::<EscapeDangerGoal>()
    }

    #[must_use]
    pub fn is_suffocating(&self) -> bool {
        self.suffocating.load(Ordering::Relaxed)
    }

    pub fn set_suffocating(&self, suffocating: bool) {
        let prev = self.suffocating.swap(suffocating, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::strider::DATA_SUFFOCATING,
            suffocating,
        );
        if prev != suffocating {
            // Vanilla Strider.java:164-174: SUFFOCATING_MODIFIER = -0.34F
            // Base speed is 0.175. When suffocating (out of lava/warm blocks): 0.175 * 0.66 = 0.1155.
            let mut attributes = self
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = if suffocating {
                    0.175 * 0.66
                } else {
                    0.175
                };
                speed.dirty.store(true, Ordering::Relaxed);
            }
        }
    }
}

impl AgeableMob for StriderEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for StriderEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_STRIDER_FOOD)
            || item_stack.item == &Item::WARPED_FUNGUS
    }
}

impl Mob for StriderEntity {
    fn as_strider(&self) -> Option<&StriderEntity> {
        Some(self)
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
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
        } else if let Some(saddle_byte) = nbt.get_byte("Saddle") {
            self.set_saddled(saddle_byte == 1);
        }
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn is_sensitive_to_water(&self) -> bool {
        true
    }

    fn get_item_steerable(&self) -> Option<&dyn ItemSteerable> {
        Some(self)
    }

    fn is_saddled(&self) -> bool {
        self.saddled.load(Ordering::Relaxed)
    }

    fn can_be_saddled(&self) -> bool {
        self.mob_entity.living_entity.entity.is_alive() && !self.is_baby()
    }

    fn set_saddled(&self, saddled: bool) {
        self.saddled.store(saddled, Ordering::Relaxed);
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla Strider.tick: checks if standing in lava or on STRIDER_WARM_BLOCKS
        let in_warm_blocks = entity.touching_lava.load(Ordering::Relaxed)
            || entity.lava_height.load() > 0.0
            || {
                let pos = entity.block_pos.load();
                let world = entity.world.load();
                let block_pos_down = pos.down();
                let block = world.get_block(&pos);
                let block_down = world.get_block(&block_pos_down);
                block.has_tag(&tag::Block::MINECRAFT_STRIDER_WARM_BLOCKS)
                    || block_down.has_tag(&tag::Block::MINECRAFT_STRIDER_WARM_BLOCKS)
                    || block.id == pumpkin_data::Block::LAVA.id
                    || block_down.id == pumpkin_data::Block::LAVA.id
            };
        self.set_suffocating(!in_warm_blocks);
        self.steering.tick_boost();

        let is_tempted = self.is_being_tempted();
        let is_panicking = self.is_panicking();

        if !entity.silent.load(Ordering::Relaxed) {
            let mut rng = rand::rng();
            // Vanilla Strider.java:303-307: happy when tempted (1/140), retreat when panicking (1/60)
            if crate::entity::combat::should_strider_be_happy(is_tempted, rng.random_range(0..140)) {
                entity.world.load().play_sound_fine(
                    Sound::EntityStriderHappy,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            } else if crate::entity::combat::should_strider_retreat(is_panicking, rng.random_range(0..60)) {
                entity.world.load().play_sound_fine(
                    Sound::EntityStriderRetreat,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            } else if !is_panicking && !is_tempted && rng.random_range(0..100) == 0 {
                // Vanilla Strider.java:354-356: ambient only when not panicking and not tempted
                entity.world.load().play_sound_fine(
                    Sound::EntityStriderAmbient,
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
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::strider::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::strider::DATA_SUFFOCATING,
            self.is_suffocating(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if item == &Item::SADDLE && self.can_be_saddled() && !self.is_saddled() {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            let pos = entity.pos.load();
            world.play_sound(Sound::EntityStriderSaddle, SoundCategory::Neutral, &pos);
            return true;
        }

        if self.is_saddled() && !self.is_food(item_stack) {
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if let Some(vehicle) = world.get_entity_by_id(ent.entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }

        self.animal_interact(player, item_stack, Sound::EntityStriderEat)
    }
}

impl ItemSteerable for StriderEntity {
    fn boost(&self) -> bool {
        self.steering.boost()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_steerable_speed(&self) -> f32 {
        self.get_ridden_speed()
    }
}

impl CustomSound for StriderEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityStriderDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityStriderHurt)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_strider_ridden_speed_and_sounds() {
        let warm_speed = crate::entity::combat::calculate_strider_ridden_speed(0.175, false, 1.0);
        let cold_speed = crate::entity::combat::calculate_strider_ridden_speed(0.175, true, 1.0);
        assert!(warm_speed > cold_speed);
        assert!((warm_speed - 0.09625).abs() < 1e-5);
        assert!((cold_speed - 0.06125).abs() < 1e-5);

        assert!(crate::entity::combat::should_strider_be_happy(true, 0));
        assert!(!crate::entity::combat::should_strider_be_happy(false, 0));
        assert!(crate::entity::combat::should_strider_retreat(true, 0));
        assert!(!crate::entity::combat::should_strider_retreat(false, 0));
    }
}
