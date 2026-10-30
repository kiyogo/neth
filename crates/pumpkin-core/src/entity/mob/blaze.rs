use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::vector3::Vector3;

use crate::entity::custom_sound::CustomSound;
use crate::entity::util::RandomExt;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, move_towards_restriction::MoveTowardsRestrictionGoal,
        revenge::RevengeGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    mob::{Mob, MobEntity},
};

use crate::entity::combat::{
    BLAZE_ASCENT_ACCELERATION, BLAZE_ATTACK_DAMAGE, BLAZE_BASE_MOVEMENT_SPEED,
    BLAZE_FALL_SPEED_MULTIPLIER, BLAZE_FOLLOW_RANGE, BLAZE_HEIGHT_OFFSET_INTERVAL_TICKS,
    BLAZE_HEIGHT_OFFSET_SPREAD, BLAZE_MAX_HEALTH, BLAZE_XP_REWARD,
};

const INITIAL_ALLOWED_HEIGHT_OFFSET: f32 = 0.5;
const HEIGHT_OFFSET_CHANGE_INTERVAL: i32 = BLAZE_HEIGHT_OFFSET_INTERVAL_TICKS;
const HEIGHT_OFFSET_SPREAD: f64 = BLAZE_HEIGHT_OFFSET_SPREAD;
const ASCENT_ACCELERATION: f64 = BLAZE_ASCENT_ACCELERATION;

pub struct BlazeEntity {
    pub entity: Arc<MobEntity>,
    pub is_charged: AtomicBool,
    pub allowed_height_offset: AtomicCell<f32>,
    pub next_height_offset_change_tick: AtomicI32,
}

impl BlazeEntity {
    pub const XP_REWARD: u32 = BLAZE_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, f64::from(BLAZE_MAX_HEALTH));
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, f64::from(BLAZE_ATTACK_DAMAGE));
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, BLAZE_BASE_MOVEMENT_SPEED);
        mob_entity
            .living_entity
            .set_attribute_base(&Attributes::FOLLOW_RANGE, BLAZE_FOLLOW_RANGE);
        mob_entity.living_entity.health.store(BLAZE_MAX_HEALTH);

        let entity = Arc::new(mob_entity);
        let blaze = Self {
            entity,
            is_charged: AtomicBool::new(false),
            allowed_height_offset: AtomicCell::new(INITIAL_ALLOWED_HEIGHT_OFFSET),
            next_height_offset_change_tick: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(blaze);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        {
            let mut goal_selector = mob_arc
                .entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(
                4,
                Box::new(
                    crate::entity::ai::goal::blaze_attack::BlazeShootFireballGoal::new(
                        Arc::downgrade(&mob_arc),
                    ),
                ),
            );
            goal_selector.add_goal(5, Box::new(MoveTowardsRestrictionGoal::new(1.0)));

            goal_selector.add_goal(
                7,
                Box::new(WaterAvoidingRandomStrollGoal::with_probability(1.0, 0.0)),
            );
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_charged(&self) -> bool {
        self.is_charged.load(Ordering::Relaxed)
    }

    pub fn set_charged(&self, charged: bool) {
        self.is_charged.store(charged, Ordering::Relaxed);
        let flags = i8::from(charged);
        self.entity
            .living_entity
            .entity
            .set_synced_data(pumpkin_data::tracked_data::blaze::DATA_FLAGS_ID, flags);
    }
}

impl CustomSound for BlazeEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBlazeDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBlazeHurt)
    }
}

impl Mob for BlazeEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity
    }

    fn as_blaze(&self) -> Option<&BlazeEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn is_sensitive_to_water(&self) -> bool {
        true
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let base_entity = &self.entity.living_entity.entity;
        if !base_entity.is_alive() {
            return;
        }

        let on_ground = base_entity.on_ground.load(Ordering::Relaxed);
        let vel = base_entity.velocity.load();
        if !on_ground && vel.y < 0.0 {
            base_entity
                .velocity
                .store(Vector3::new(vel.x, vel.y * BLAZE_FALL_SPEED_MULTIPLIER, vel.z));
        }

        let world = base_entity.world.load_full();
        let raining_at_feet = world.is_raining_at(&base_entity.block_pos.load());
        let raining_at_head = world.is_raining_at(&base_entity.bounding_box.load().max_block_pos());
        let is_raining = raining_at_feet || raining_at_head;
        let touching_water = base_entity.touching_water.load(Ordering::Relaxed);

        if crate::entity::combat::should_blaze_take_water_damage(touching_water, is_raining) {
            caller.damage_with_context(caller, 1.0, DamageType::DROWN, None, None, None);
        }

        // Ambient sound
        if rand::random_range(0..100) == 0 && !base_entity.silent.load(Ordering::Relaxed) {
            base_entity.world.load().play_sound_fine(
                Sound::EntityBlazeAmbient,
                SoundCategory::Hostile,
                &base_entity.pos.load(),
                1.0,
                1.0,
            );
        }

        // Crackling burn sound (Blaze.java:91-102)
        if crate::entity::combat::should_blaze_play_burn_sound(
            rand::random_range(0..24),
            base_entity.silent.load(Ordering::Relaxed),
        ) {
            let pos = base_entity.pos.load();
            let sound_pos = Vector3::new(pos.x + 0.5, pos.y + 0.5, pos.z + 0.5);
            let rand_vol = rand::random_range(0.0..1.0);
            let rand_pitch = rand::random_range(0.0..1.0);
            base_entity.world.load().play_sound_fine(
                Sound::EntityBlazeBurn,
                SoundCategory::Hostile,
                &sound_pos,
                crate::entity::combat::calculate_blaze_burn_volume(rand_vol),
                crate::entity::combat::calculate_blaze_burn_pitch(rand_pitch),
            );
        }
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn custom_server_ai_step(&self, _caller: &dyn EntityBase) {
        if self
            .next_height_offset_change_tick
            .fetch_sub(1, Ordering::Relaxed)
            <= 1
        {
            self.next_height_offset_change_tick
                .store(HEIGHT_OFFSET_CHANGE_INTERVAL, Ordering::Relaxed);
            let mut rng = self.get_random();
            self.allowed_height_offset.store(rng.triangle(
                f64::from(INITIAL_ALLOWED_HEIGHT_OFFSET),
                HEIGHT_OFFSET_SPREAD,
            ) as f32);
        }

        let Some(target) = self.entity.get_target() else {
            return;
        };
        let entity = &self.entity.living_entity.entity;
        if target.get_entity().get_eye_y()
            > entity.get_eye_y() + f64::from(self.allowed_height_offset.load())
            && self.can_attack(target.as_ref())
        {
            let velocity = entity.velocity.load();
            entity.velocity.store(Vector3::new(
                velocity.x,
                velocity.y + (ASCENT_ACCELERATION - velocity.y) * ASCENT_ACCELERATION,
                velocity.z,
            ));
            entity.velocity_dirty.store(true, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blaze_burn_sound_and_fireball_spread_parity() {
        assert_eq!(BlazeEntity::XP_REWARD, BLAZE_XP_REWARD);
        assert_eq!(BLAZE_MAX_HEALTH, 20.0);
        assert_eq!(BLAZE_BASE_MOVEMENT_SPEED, 0.23);
        assert_eq!(BLAZE_ATTACK_DAMAGE, 6.0);
        assert_eq!(BLAZE_FOLLOW_RANGE, 48.0);
        assert_eq!(BLAZE_FALL_SPEED_MULTIPLIER, 0.6);
        assert_eq!(BLAZE_HEIGHT_OFFSET_INTERVAL_TICKS, 100);
        assert_eq!(BLAZE_HEIGHT_OFFSET_SPREAD, 6.891);
        assert_eq!(BLAZE_ASCENT_ACCELERATION, 0.3);
        assert!((crate::entity::combat::calculate_blaze_burn_pitch(0.0) - 0.3).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_blaze_burn_pitch(1.0) - 1.0).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_blaze_burn_volume(0.0) - 1.0).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_blaze_burn_volume(1.0) - 2.0).abs() < 1e-6);
        assert!(crate::entity::combat::should_blaze_play_burn_sound(0, false));
        assert!(!crate::entity::combat::should_blaze_play_burn_sound(1, false));
        assert!(!crate::entity::combat::should_blaze_play_burn_sound(0, true));
        let spread = crate::entity::combat::calculate_blaze_small_fireball_spread(16.0);
        assert!((spread - 2.297).abs() < 1e-6);
    }
}
