use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, breeze_attack::BreezeAttackGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, revenge::RevengeGoal,
        swim::SwimGoal, water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
};

pub struct BreezeEntity {
    pub mob_entity: MobEntity,
    pub whirl_sound_ticks: AtomicI32,
}

impl BreezeEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::BREEZE_XP_REWARD;
    pub const INNER_CIRCLE_MIN_RANGE: f64 = 4.0;
    pub const INNER_CIRCLE_MAX_Y: f64 = 10.0;
    pub const FIRING_Y_OFFSET: f64 = 0.3;
    pub const JUMP_TRAIL_DURATION_TICKS: i32 = 5;
    pub const FALL_DISTANCE_SOUND_TRIGGER_THRESHOLD: f32 = 3.0;
    pub const MAX_HEAD_Y_ROT: i32 = 30;
    pub const HEAD_ROT_SPEED: i32 = 25;

    // Vanilla Shoot.java:18-24 & LongJump.java:34-41 parity constants
    pub const ATTACK_RANGE_MAX_SQRT: f64 = 256.0;
    pub const ATTACK_RANGE_MAX: f64 = 16.0;
    pub const PROJECTILE_MOVEMENT_SCALE: f32 = 0.7;
    pub const SHOOT_INITIAL_DELAY_TICKS: i32 = 15;
    pub const SHOOT_RECOVER_DELAY_TICKS: i32 = 4;
    pub const SHOOT_COOLDOWN_TICKS: i32 = 10;
    pub const UNCERTAINTY_BASE: i32 = 5;
    pub const UNCERTAINTY_MULTIPLIER: i32 = 4;

    pub const REQUIRED_AIR_BLOCKS_ABOVE: i32 = 4;
    pub const JUMP_COOLDOWN_TICKS: i32 = 10;
    pub const JUMP_COOLDOWN_WHEN_HURT_TICKS: i32 = 2;
    pub const INHALING_DURATION_TICKS: i32 = 10;
    pub const DEFAULT_FOLLOW_RANGE: f32 = 24.0;
    pub const DEFAULT_MAX_JUMP_VELOCITY: f32 = 1.4;
    pub const MAX_JUMP_VELOCITY_MULTIPLIER: f32 = 0.058_333_334;
    pub const ALLOWED_JUMP_ANGLES: &[i32] = &[40, 55, 60, 75, 80];
    pub const TOO_CLOSE_FOR_JUMP_DISTANCE: f32 = 4.0;
    pub const SPEED_MULTIPLIER_WHEN_SLIDING: f32 = 0.6;
    pub const POST_LAND_SHOOT_MEMORY_TICKS: i32 = 100;

    #[must_use]
    pub const fn calculate_shoot_uncertainty(difficulty_id: u8) -> i32 {
        Self::UNCERTAINTY_BASE - (difficulty_id as i32) * Self::UNCERTAINTY_MULTIPLIER
    }

    #[must_use]
    pub fn calculate_max_jump_velocity(follow_range: f64) -> f32 {
        Self::MAX_JUMP_VELOCITY_MULTIPLIER * (follow_range as f32)
    }

    #[must_use]
    pub const fn is_too_close_for_jump(distance: f32) -> bool {
        distance <= Self::TOO_CLOSE_FOR_JUMP_DISTANCE
    }

    #[must_use]
    pub const fn is_target_within_shoot_range(dist_sq: f64) -> bool {
        dist_sq < Self::ATTACK_RANGE_MAX_SQRT
    }

    #[must_use]
    pub const fn get_jump_cooldown(was_hurt: bool) -> i32 {
        if was_hurt {
            Self::JUMP_COOLDOWN_WHEN_HURT_TICKS
        } else {
            Self::JUMP_COOLDOWN_TICKS
        }
    }

    #[must_use]
    pub fn calculate_aim_target_y(target_base_y: f64, is_passenger: bool, firing_y: f64) -> f64 {
        let eye_offset = if is_passenger { 0.8 } else { 0.3 };
        (target_base_y + eye_offset) - firing_y
    }

    #[must_use]
    pub fn can_target_entity(entity_type: &EntityType) -> bool {
        entity_type == &EntityType::PLAYER || entity_type == &EntityType::IRON_GOLEM
    }

    #[must_use]
    pub fn calculate_firing_y_position(base_y: f64, bb_height: f32) -> f64 {
        base_y + f64::from(bb_height) / 2.0 + Self::FIRING_Y_OFFSET
    }

    #[must_use]
    pub fn is_within_inner_circle(dx: f64, dy: f64, dz: f64) -> bool {
        (dx * dx + dz * dz) <= (Self::INNER_CIRCLE_MIN_RANGE * Self::INNER_CIRCLE_MIN_RANGE)
            && dy.abs() <= Self::INNER_CIRCLE_MAX_Y
    }

    #[must_use]
    pub const fn can_be_leashed(&self) -> bool {
        false
    }

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let breeze = Self {
            mob_entity,
            whirl_sound_ticks: AtomicI32::new(rand::random_range(1..=80)),
        };
        let mob_arc = Arc::new(breeze);

        // Vanilla Breeze: MOVEMENT_SPEED = 0.63, MAX_HEALTH = 30.0, FOLLOW_RANGE = 24.0, ATTACK_DAMAGE = 3.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, f64::from(crate::entity::combat::BREEZE_MAX_HEALTH));
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MOVEMENT_SPEED, crate::entity::combat::BREEZE_BASE_MOVEMENT_SPEED);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::FOLLOW_RANGE, crate::entity::combat::BREEZE_FOLLOW_RANGE);
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, f64::from(crate::entity::combat::BREEZE_BASE_ATTACK_DAMAGE));
        mob_arc.mob_entity.living_entity.set_health(crate::entity::combat::BREEZE_MAX_HEALTH);

        let breeze_weak = Arc::downgrade(&mob_arc);
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
            goal_selector.add_goal(1, BreezeAttackGoal::new(breeze_weak));
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(0.6)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
        };

        mob_arc
    }
}

impl Mob for BreezeEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn pre_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) -> bool {
        if let Some(src) = source {
            let src_type = src.get_entity().entity_type;
            if src_type == &EntityType::BREEZE {
                return false;
            }
            if src_type != &EntityType::BREEZE_WIND_CHARGE && src_type != &EntityType::WIND_CHARGE {
                if crate::entity::projectile::is_projectile(src_type) {
                    let world = self.get_entity().world.load();
                    world.play_sound_fine(
                        Sound::EntityBreezeDeflect,
                        SoundCategory::Hostile,
                        &self.get_entity().pos.load(),
                        1.0,
                        1.0,
                    );
                    let proj = src.get_entity();
                    let velo = proj.velocity.load();
                    proj.velocity.store(Vector3::new(-velo.x * 0.5, -velo.y * 0.5, -velo.z * 0.5));
                    let rotation = 170.0 + rand::random::<f32>() * 20.0;
                    proj.yaw.store(proj.yaw.load() + rotation);
                    proj.velocity_dirty.store(true, Ordering::Relaxed);
                    return false;
                }
            }
        }
        true
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = self.get_entity();
        let silent = entity.silent.load(Ordering::Relaxed);
        let world = entity.world.load();

        if !silent && rand::random_range(0..80) == 0 {
            let sound = if entity.on_ground.load(Ordering::Relaxed) {
                Sound::EntityBreezeIdleGround
            } else {
                Sound::EntityBreezeIdleAir
            };
            world.play_sound_fine(
                sound,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }

        // Vanilla Breeze whirl sound (Breeze.java:145-148, 190-194)
        let whirl_cd = self.whirl_sound_ticks.load(Ordering::Relaxed);
        if whirl_cd <= 0 {
            self.whirl_sound_ticks
                .store(rand::random_range(1..=80), Ordering::Relaxed);
            if !silent {
                let rand_pitch = rand::random_range(0.0..1.0);
                let rand_vol = rand::random_range(0.0..1.0);
                let pitch = crate::entity::combat::calculate_breeze_whirl_pitch(rand_pitch);
                let volume = crate::entity::combat::calculate_breeze_whirl_volume(rand_vol);
                world.play_sound_fine(
                    Sound::EntityBreezeWhirl,
                    SoundCategory::Hostile,
                    &entity.pos.load(),
                    volume,
                    pitch,
                );
            }
        } else {
            self.whirl_sound_ticks
                .store(whirl_cd - 1, Ordering::Relaxed);
        }
    }

    fn on_damage(&self, damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        if damage_type.id == DamageType::FALL.id {
            let fall_dist = self.mob_entity.living_entity.fall_distance.load();
            if crate::entity::combat::should_breeze_play_fall_sound(fall_dist) {
                let entity = self.get_entity();
                if !entity.silent.load(Ordering::Relaxed) {
                    entity.world.load().play_sound_fine(
                        Sound::EntityBreezeLand,
                        SoundCategory::Hostile,
                        &entity.pos.load(),
                        1.0,
                        1.0,
                    );
                }
            }
        }
    }

    fn as_breeze(&self) -> Option<&BreezeEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn can_be_leashed(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        crate::entity::combat::BREEZE_XP_REWARD
    }
}

impl CustomSound for BreezeEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBreezeHurt)
    }

    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityBreezeDeath)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_breeze_fall_and_whirl_parity() {
        assert_eq!(crate::entity::combat::BREEZE_FALL_DISTANCE_SOUND_THRESHOLD, 3.0);
        assert!(crate::entity::combat::should_breeze_play_fall_sound(3.1));
        assert!(!crate::entity::combat::should_breeze_play_fall_sound(3.0));
        assert!((crate::entity::combat::calculate_breeze_whirl_pitch(0.0) - 0.7).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_breeze_whirl_pitch(1.0) - 1.1).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_breeze_whirl_volume(0.0) - 0.8).abs() < 1e-6);
        assert!((crate::entity::combat::calculate_breeze_whirl_volume(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_breeze_shoot_and_jump_constants_and_formulas_parity() {
        use super::BreezeEntity;

        // 1. Shoot constants parity (Shoot.java:18-24)
        assert_eq!(BreezeEntity::ATTACK_RANGE_MAX_SQRT, 256.0);
        assert_eq!(BreezeEntity::ATTACK_RANGE_MAX, 16.0);
        assert_eq!(BreezeEntity::PROJECTILE_MOVEMENT_SCALE, 0.7);
        assert_eq!(BreezeEntity::SHOOT_INITIAL_DELAY_TICKS, 15);
        assert_eq!(BreezeEntity::SHOOT_RECOVER_DELAY_TICKS, 4);
        assert_eq!(BreezeEntity::SHOOT_COOLDOWN_TICKS, 10);
        assert_eq!(BreezeEntity::UNCERTAINTY_BASE, 5);
        assert_eq!(BreezeEntity::UNCERTAINTY_MULTIPLIER, 4);

        // 2. Shoot uncertainty formula: 5 - difficulty_id * 4 (Shoot.java:82)
        assert_eq!(BreezeEntity::calculate_shoot_uncertainty(0), 5);  // Peaceful (id 0)
        assert_eq!(BreezeEntity::calculate_shoot_uncertainty(1), 1);  // Easy (id 1)
        assert_eq!(BreezeEntity::calculate_shoot_uncertainty(2), -3); // Normal (id 2)
        assert_eq!(BreezeEntity::calculate_shoot_uncertainty(3), -7); // Hard (id 3)

        // 3. Aim target Y calculation (Shoot.java:80)
        // Standing target (offset 0.3): (64.0 + 0.3) - 64.885 = -0.585
        let target_standing_dy = BreezeEntity::calculate_aim_target_y(64.0, false, 64.885);
        assert!((target_standing_dy - (-0.585)).abs() < 1e-6);
        // Passenger target (offset 0.8): (64.0 + 0.8) - 64.885 = -0.085
        let target_passenger_dy = BreezeEntity::calculate_aim_target_y(64.0, true, 64.885);
        assert!((target_passenger_dy - (-0.085)).abs() < 1e-6);

        // 4. LongJump constants parity (LongJump.java:34-41)
        assert_eq!(BreezeEntity::REQUIRED_AIR_BLOCKS_ABOVE, 4);
        assert_eq!(BreezeEntity::JUMP_COOLDOWN_TICKS, 10);
        assert_eq!(BreezeEntity::JUMP_COOLDOWN_WHEN_HURT_TICKS, 2);
        assert_eq!(BreezeEntity::INHALING_DURATION_TICKS, 10);
        assert_eq!(BreezeEntity::DEFAULT_FOLLOW_RANGE, 24.0);
        assert_eq!(BreezeEntity::DEFAULT_MAX_JUMP_VELOCITY, 1.4);
        assert_eq!(BreezeEntity::ALLOWED_JUMP_ANGLES, &[40, 55, 60, 75, 80]);
        assert_eq!(BreezeEntity::TOO_CLOSE_FOR_JUMP_DISTANCE, 4.0);
        assert_eq!(BreezeEntity::SPEED_MULTIPLIER_WHEN_SLIDING, 0.6);
        assert_eq!(BreezeEntity::POST_LAND_SHOOT_MEMORY_TICKS, 100);

        // 5. Jump velocity calculation: 0.058333334 * follow_range (LongJump.java:229)
        let max_jump_vel = BreezeEntity::calculate_max_jump_velocity(24.0);
        assert!((max_jump_vel - 1.4).abs() < 1e-5);

        // 6. Target distance checks
        assert!(BreezeEntity::is_too_close_for_jump(4.0));
        assert!(BreezeEntity::is_too_close_for_jump(2.5));
        assert!(!BreezeEntity::is_too_close_for_jump(4.01));
        assert!(BreezeEntity::is_target_within_shoot_range(255.9));
        assert!(!BreezeEntity::is_target_within_shoot_range(256.0));

        // 7. Jump cooldown calculation
        assert_eq!(BreezeEntity::get_jump_cooldown(false), 10);
        assert_eq!(BreezeEntity::get_jump_cooldown(true), 2);
    }
}
