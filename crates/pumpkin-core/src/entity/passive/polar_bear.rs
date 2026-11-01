use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use crate::entity::combat;
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::behavior::neutral::apply_targets,
    ai::goal::{
        active_target::{ActiveTargetGoal, TargetCondition},
        escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal,
        revenge::RevengeGoal,
        swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    custom_sound::CustomSound,
    mob::{
        Mob, MobEntity,
        neutral::{NeutralData, NeutralMob},
    },
    passive::animal::Animal,
    player::Player,
};

fn is_baby(mob: &dyn Mob) -> bool {
    mob.as_ageable().is_some_and(AgeableMob::is_baby)
}

pub struct PolarBearEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub neutral_data: NeutralData,
    pub standing: AtomicBool,
    pub warning_sound_ticks: AtomicI32,
}

impl PolarBearEntity {
    #[must_use]
    pub const fn can_be_leashed(&self) -> bool {
        true
    }
    pub const XP_REWARD: u32 = combat::POLAR_BEAR_XP_REWARD;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let polar_bear = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            neutral_data: NeutralData::default(),
            standing: AtomicBool::new(false),
            warning_sound_ticks: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(polar_bear);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla PolarBear: MAX_HEALTH = 30.0, FOLLOW_RANGE = 20.0, MOVEMENT_SPEED = 0.25, ATTACK_DAMAGE = 6.0
        {
            mob_arc.mob_entity.living_entity.set_attribute_base(&Attributes::MAX_HEALTH, combat::POLAR_BEAR_MAX_HEALTH);
            mob_arc.mob_entity.living_entity.set_attribute_base(&Attributes::FOLLOW_RANGE, combat::POLAR_BEAR_FOLLOW_RANGE);
            mob_arc.mob_entity.living_entity.set_attribute_base(&Attributes::MOVEMENT_SPEED, combat::POLAR_BEAR_BASE_MOVEMENT_SPEED);
            mob_arc.mob_entity.living_entity.set_attribute_base(&Attributes::ATTACK_DAMAGE, combat::POLAR_BEAR_BASE_ATTACK_DAMAGE);
        }
        mob_arc.mob_entity.living_entity.set_health(30.0);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(MeleeAttackGoal::new(1.25, true)));
            // Vanilla: `new PanicGoal(this, 2.0, bear -> bear.isBaby() ? PANIC_CAUSES : PANIC_ENVIRONMENTAL_CAUSES)`
            goal_selector.add_goal(
                1,
                EscapeDangerGoal::new_with_tag(2.0, |mob| {
                    if mob
                        .get_mob_entity()
                        .living_entity
                        .entity
                        .age
                        .load(Ordering::Relaxed)
                        < 0
                    {
                        &tag::DamageType::MINECRAFT_PANIC_CAUSES
                    } else {
                        &tag::DamageType::MINECRAFT_PANIC_ENVIRONMENTAL_CAUSES
                    }
                }),
            );
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            target_selector.add_goal(
                1,
                Box::new(
                    RevengeGoal::new(true)
                        .raising_alarm_when(is_baby)
                        .alerting_only(|other| !is_baby(other)),
                ),
            );
            // Vanilla PolarBear.PolarBearAttackPlayersGoal at priority 2:
            // Adults charge and target players when a baby cub is nearby (within 8x4x8 blocks).
            target_selector.add_goal(
                2,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::PLAYER,
                    20,
                    true,
                    true,
                    None::<fn(&crate::entity::living::LivingEntity, &crate::world::World) -> bool>,
                ))
                .gated_by(|mob| {
                    if is_baby(mob) {
                        return false;
                    }
                    let mob_entity = mob.get_mob_entity();
                    let entity = &mob_entity.living_entity.entity;
                    let world = entity.world.load();
                    let bb = entity.bounding_box.load().expand(8.0, 4.0, 8.0);
                    let nearby = world.get_entities_at_box(&bb);
                    nearby.iter().any(|other| {
                        other.get_entity().entity_type == &EntityType::POLAR_BEAR
                            && other.get_entity().age.load(Ordering::Relaxed) < 0
                    })
                }),
            );
            apply_targets(&mut target_selector, &mob_arc.mob_entity, 3, 5, false);
            target_selector.add_goal(
                4,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::FOX, true)
                    .when(TargetCondition::Adult),
            );
        };

        mob_arc.refresh_dimensions();
        mob_arc
    }

    #[must_use]
    pub fn is_standing(&self) -> bool {
        self.standing.load(Ordering::Relaxed)
    }

    pub fn refresh_dimensions(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let is_baby = self.is_baby();
        let is_standing = self.is_standing();
        let (w, h, eye) = combat::calculate_polar_bear_dimensions(
            is_baby,
            is_standing,
            entity.entity_type.dimension[0],
            entity.entity_type.dimension[1],
            entity.entity_type.eye_height,
        );
        let dimensions = EntityDimensions {
            width: w,
            height: h,
            eye_height: eye,
        };
        let pos = entity.pos.load();
        entity.bounding_box.store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    pub fn age_boundary_reached(&self) {
        let mob = self.get_mob_entity();
        let entity = &mob.living_entity.entity;
        self.refresh_dimensions();
        crate::entity::ageable::handle_age_boundary_vehicle_dismount(self.is_baby(), entity);
    }

    pub fn set_standing(&self, standing: bool) {
        self.standing.store(standing, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::polar_bear::DATA_STANDING_ID,
            standing,
        );
        self.refresh_dimensions();
    }

    pub fn play_warning_sound(&self) {
        if self.warning_sound_ticks.load(Ordering::Relaxed) <= 0 {
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound_fine(
                Sound::EntityPolarBearWarning,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
            self.warning_sound_ticks.store(combat::POLAR_BEAR_WARNING_SOUND_TICKS, Ordering::Relaxed);
        }
    }
}

impl AgeableMob for PolarBearEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn set_baby(&self, baby: bool) {
        self.set_age(if baby { self.get_baby_start_age() } else { 0 });
        self.refresh_dimensions();
    }
}

impl Animal for PolarBearEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl NeutralMob for PolarBearEntity {
    fn get_neutral_data(&self) -> &NeutralData {
        &self.neutral_data
    }
}

impl Mob for PolarBearEntity {
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

    fn as_neutral(&self) -> Option<&dyn NeutralMob> {
        Some(self)
    }

    fn as_polar_bear(&self) -> Option<&PolarBearEntity> {
        Some(self)
    }

    fn can_dispenser_equip_into_slot(
        &self,
        _slot: &pumpkin_data::data_component_impl::EquipmentSlot,
    ) -> bool {
        false
    }

    fn mob_write_nbt(&self, _nbt: &mut NbtCompound) {}

    fn mob_read_nbt(&self, _nbt: &NbtCompound) {
        self.refresh_dimensions();
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

        // Vanilla PolarBear.java:205-207: warningSoundTicks decrement
        let warn_ticks = self.warning_sound_ticks.load(Ordering::Relaxed);
        if warn_ticks > 0 {
            self.warning_sound_ticks.store(warn_ticks - 1, Ordering::Relaxed);
        }

        // Vanilla PolarBear.PolarBearMeleeAttackGoal:
        // When approaching target, rear up on hind legs and growl warning
        let is_adult = entity.age.load(Ordering::Relaxed) >= 0;
        if is_adult {
            if let Some(target) = self.mob_entity.get_target() {
                let my_pos = entity.pos.load();
                let t_pos = target.get_entity().pos.load();
                let dist_sq = my_pos.squared_distance_to(t_pos.x, t_pos.y, t_pos.z);
                let bb = target.get_entity().bounding_box.load();
                let target_width = (bb.max.x - bb.min.x) as f32;
                if crate::entity::combat::should_polar_bear_stand_and_warn(dist_sq, target_width, 10) {
                    self.set_standing(true);
                    self.play_warning_sound();
                } else if dist_sq > (target_width as f64 + 3.0).powi(2) && self.is_standing() {
                    self.set_standing(false);
                }
            } else if self.is_standing() {
                self.set_standing(false);
            }
        }

        if !entity.silent.load(Ordering::Relaxed) && rand::random_range(0..100) == 0 {
            let sound = if entity.age.load(Ordering::Relaxed) < 0 {
                Sound::EntityPolarBearAmbientBaby
            } else {
                Sound::EntityPolarBearAmbient
            };
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }

    fn on_attack(&self, _target: &dyn EntityBase) {
        // Vanilla PolarBear.java:314, 317, 327: lower hind legs on hit
        self.set_standing(false);
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::polar_bear::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::polar_bear::DATA_STANDING_ID,
            self.is_standing(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityPolarBearAmbient)
    }
}

impl CustomSound for PolarBearEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPolarBearDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityPolarBearHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::combat;

    #[test]
    fn test_polar_bear_parity_constants() {
        assert_eq!(PolarBearEntity::XP_REWARD, 2);
        assert_eq!(combat::POLAR_BEAR_MAX_HEALTH, 30.0);
        assert_eq!(combat::POLAR_BEAR_FOLLOW_RANGE, 20.0);
        assert_eq!(combat::POLAR_BEAR_BASE_MOVEMENT_SPEED, 0.25);
        assert_eq!(combat::POLAR_BEAR_BASE_ATTACK_DAMAGE, 6.0);
        assert_eq!(combat::POLAR_BEAR_STAND_ANIMATION_TICKS, 6.0);
        assert_eq!(combat::POLAR_BEAR_WARNING_SOUND_TICKS, 40);
        assert_eq!(combat::POLAR_BEAR_PERSISTENT_ANGER_MIN_TICKS, 400);
        assert_eq!(combat::POLAR_BEAR_PERSISTENT_ANGER_MAX_TICKS, 780);
    }

    #[test]
    fn test_polar_bear_warning_and_standing_conditions() {
        // Vanilla PolarBear.PolarBearMeleeAttackGoal:
        // canAttack: dist_sq > min_dist_sq && dist_sq <= max_dist_sq
        // should_polar_bear_stand_and_warn: target_width = 1.0, wait_delay = 10
        let target_width = 1.0f32;

        // reach = (1.0 + 3.0)^2 = 16.0
        assert!(crate::entity::combat::should_polar_bear_stand_and_warn(15.0, target_width, 10));
        assert!(crate::entity::combat::should_polar_bear_stand_and_warn(10.0, target_width, 5));
        assert!(!crate::entity::combat::should_polar_bear_stand_and_warn(17.0, target_width, 10));
        assert!(!crate::entity::combat::should_polar_bear_stand_and_warn(15.0, target_width, 11));
    }

    #[test]
    fn test_polar_bear_baby_dimensions_and_standing_height_scaling_parity() {
        // 1. Adult quad dimensions
        let adult_quad = combat::calculate_polar_bear_dimensions(false, false, 1.4, 1.4, 1.19);
        assert_eq!(adult_quad.0, 1.4);
        assert_eq!(adult_quad.1, 1.4);
        assert_eq!(adult_quad.2, 1.19);

        // 2. Adult standing dimensions (doubled height and eye_height)
        let adult_standing = combat::calculate_polar_bear_dimensions(false, true, 1.4, 1.4, 1.19);
        assert_eq!(adult_standing.0, 1.4);
        assert_eq!(adult_standing.1, 2.8);
        assert!((adult_standing.2 - 2.38).abs() < 1e-4);

        // 3. Baby quad dimensions (PolarBear.java:67)
        let baby_quad = combat::calculate_polar_bear_dimensions(true, false, 1.4, 1.4, 1.19);
        assert_eq!(baby_quad.0, combat::POLAR_BEAR_BABY_WIDTH);
        assert_eq!(baby_quad.1, combat::POLAR_BEAR_BABY_HEIGHT);
        assert_eq!(baby_quad.2, combat::POLAR_BEAR_BABY_EYE_HEIGHT);
        assert_eq!(combat::POLAR_BEAR_BABY_WIDTH, 0.7);
        assert_eq!(combat::POLAR_BEAR_BABY_HEIGHT, 0.7);
        assert_eq!(combat::POLAR_BEAR_BABY_EYE_HEIGHT, 0.34375);

        // 4. Baby standing dimensions (doubled height)
        let baby_standing = combat::calculate_polar_bear_dimensions(true, true, 1.4, 1.4, 1.19);
        assert_eq!(baby_standing.0, 0.7);
        assert_eq!(baby_standing.1, 1.4);
        assert!((baby_standing.2 - 0.6875).abs() < 1e-4);

        // 5. Standing scale constants
        assert_eq!(combat::POLAR_BEAR_MAX_STAND_HEIGHT_SCALE, 2.0);
        assert_eq!(combat::POLAR_BEAR_STAND_ANIMATION_TICKS, 6.0);
    }

}
