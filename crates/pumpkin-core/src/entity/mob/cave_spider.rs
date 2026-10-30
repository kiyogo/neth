use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};

use crate::entity::combat::{
    CAVE_SPIDER_BASE_ATTACK_DAMAGE, CAVE_SPIDER_BASE_MOVEMENT_SPEED, CAVE_SPIDER_MAX_HEALTH,
    CAVE_SPIDER_XP_REWARD, calculate_cave_spider_poison_duration,
};

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    ai::goal::{
        active_target::{ActiveTargetGoal, TargetCondition},
        avoid_entity::AvoidEntityGoal,
        leap_at_target::LeapAtTargetGoal,
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        revenge::RevengeGoal,
        spider_attack::SpiderAttackGoal,
        swim::SwimGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    mob::{Mob, MobEntity},
};

pub struct CaveSpiderEntity {
    pub mob_entity: MobEntity,
    pub is_climbing: AtomicBool,
}

impl CaveSpiderEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let cave_spider = Self {
            mob_entity,
            is_climbing: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(cave_spider);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        // Vanilla CaveSpider: MAX_HEALTH = 12.0, MOVEMENT_SPEED = 0.3, ATTACK_DAMAGE = 2.0 (CaveSpider.java:26)
        {
            let mut attributes = mob_arc
                .mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = CAVE_SPIDER_MAX_HEALTH as f64;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = CAVE_SPIDER_BASE_MOVEMENT_SPEED;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = CAVE_SPIDER_BASE_ATTACK_DAMAGE as f64;
                damage.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_arc.mob_entity.living_entity.set_health(CAVE_SPIDER_MAX_HEALTH);

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

            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                2,
                Box::new(AvoidEntityGoal::with_target_predicate(
                    &EntityType::ARMADILLO,
                    6.0,
                    1.0,
                    1.2,
                    |target| {
                        target
                            .get_mob()
                            .and_then(Mob::as_armadillo)
                            .is_some_and(|armadillo| !armadillo.is_scared())
                    },
                )),
            );
            goal_selector.add_goal(3, Box::new(LeapAtTargetGoal::new(0.4)));
            goal_selector.add_goal(4, SpiderAttackGoal::new(1.0, false));
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(0.8)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true)
                    .when(TargetCondition::NoDaylight),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true)
                    .when(TargetCondition::NoDaylight),
            );
        };

        mob_arc
    }

    pub fn is_climbing(&self) -> bool {
        self.is_climbing.load(Ordering::Relaxed)
    }

    pub fn set_climbing(&self, climbing: bool) {
        if self.is_climbing.swap(climbing, Ordering::Relaxed) != climbing {
            let flags = i8::from(climbing);
            self.mob_entity
                .living_entity
                .entity
                .set_synced_data(pumpkin_data::tracked_data::cave_spider::DATA_FLAGS_ID, flags);
        }
    }
}

impl CustomSound for CaveSpiderEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySpiderDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySpiderHurt)
    }
}

impl Mob for CaveSpiderEntity {
    fn finalize_spawn(
        &self,
        _world: &Arc<crate::world::World>,
        group_data: Option<super::spawn::SpawnGroupData>,
    ) -> Option<super::spawn::SpawnGroupData> {
        // Vanilla CaveSpider.java:52-56:
        // Cave spiders do not spawn skeleton jockeys or random hard difficulty effects.
        self.mob_entity.finalize_spawn_base();
        group_data
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_cave_spider(&self) -> Option<&CaveSpiderEntity> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        if let Some(living) = target.get_living_entity() {
            let world = self.mob_entity.living_entity.entity.world.load();
            let duration = calculate_cave_spider_poison_duration(world.level_info.load().difficulty);
            if duration > 0 {
                living.add_effect(Effect {
                    effect_type: &StatusEffect::POISON,
                    duration,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
        }
    }

    fn get_base_experience_reward(&self) -> u32 {
        CAVE_SPIDER_XP_REWARD
    }

    fn mob_on_climbable(&self, _caller: &dyn EntityBase) -> bool {
        self.is_climbing()
            && self
                .mob_entity
                .living_entity
                .entity
                .horizontal_collision
                .load(Ordering::SeqCst)
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let is_colliding_horizontally = entity.horizontal_collision.load(Ordering::SeqCst);
        self.set_climbing(is_colliding_horizontally);
        self.mob_entity
            .living_entity
            .climbing
            .store(is_colliding_horizontally, Ordering::Relaxed);

        // Ambient sound
        if rand::random_range(0..100) == 0 && !entity.silent.load(Ordering::Relaxed) {
            entity.world.load().play_sound_fine(
                Sound::EntitySpiderAmbient,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cave_spider_poison_and_attributes_parity() {
        assert_eq!(CAVE_SPIDER_MAX_HEALTH, 12.0);
        assert_eq!(CAVE_SPIDER_BASE_ATTACK_DAMAGE, 2.0);
        assert_eq!(CAVE_SPIDER_BASE_MOVEMENT_SPEED, 0.3);
        assert_eq!(CAVE_SPIDER_XP_REWARD, 5);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Peaceful), 0);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Easy), 0);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Normal), 140);
        assert_eq!(calculate_cave_spider_poison_duration(pumpkin_util::Difficulty::Hard), 300);
    }
}
