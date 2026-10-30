use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ai::control::fish_move_control::FishMoveControl,
    ai::goal::{
        avoid_entity::AvoidEntityGoal, escape_danger::EscapeDangerGoal,
        random_swimming::RandomSwimmingGoal,
    },
    ai::pathfinder::{node::PathType, Navigator},
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::fish,
};

pub struct TropicalFishEntity {
    pub mob_entity: MobEntity,
    pub air_supply: AtomicI32,
}

impl TropicalFishEntity {
    pub const XP_REWARD: u32 = crate::entity::combat::FISH_XP_REWARD;
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        *mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(FishMoveControl::default());

        let mut navigator = Navigator::water_bound(false);
        navigator.set_pathfinding_malus(PathType::Water, 0.0);
        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;

        let tropical_fish = Self {
            mob_entity,
            air_supply: AtomicI32::new(300),
        };
        let mob_arc = Arc::new(tropical_fish);

        // Vanilla AbstractFish: MAX_HEALTH = 3.0
        mob_arc
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::MAX_HEALTH, crate::entity::combat::FISH_MAX_HEALTH as f64);
        mob_arc
            .mob_entity
            .living_entity
            .set_health(crate::entity::combat::FISH_MAX_HEALTH);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Priority 0: PanicGoal (EscapeDangerGoal 1.25)
            goal_selector.add_goal(0, EscapeDangerGoal::new(1.25));
            // Priority 2: AvoidEntityGoal (Player, 8.0, 1.6, 1.4)
            goal_selector.add_goal(
                2,
                Box::new(AvoidEntityGoal::new(&EntityType::PLAYER, 8.0, 1.6, 1.4)),
            );
            // Priority 4: FishSwimGoal (RandomSwimmingGoal 1.0)
            goal_selector.add_goal(4, Box::new(RandomSwimmingGoal::new(1.0)));
        };

        mob_arc
    }
}

impl Mob for TropicalFishEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_tropical_fish(&self) -> Option<&TropicalFishEntity> {
        Some(self)
    }

    fn mob_travel_in_water(&self, caller: &dyn EntityBase) -> bool {
        fish::travel_in_water(self, caller)
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn can_be_leashed(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        Self::XP_REWARD
    }

    fn can_breathe_underwater(&self) -> bool {
        true
    }

    fn dries_out_on_land(&self) -> bool {
        true
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        let in_water =
            entity.touching_water.load(Ordering::Relaxed) || entity.is_in_water();

        if !in_water && entity.on_ground.load(Ordering::Relaxed) {
            let mut rng = rand::rng();
            let dx = (rng.random::<f32>() * 2.0 - 1.0) * 0.05;
            let dz = (rng.random::<f32>() * 2.0 - 1.0) * 0.05;
            let velo = entity.velocity.load();
            entity.set_velocity(Vector3::new(velo.x + dx as f64, 0.4, velo.z + dz as f64));
            entity.on_ground.store(false, Ordering::Relaxed);
            if !entity.silent.load(Ordering::Relaxed) {
                entity.world.load().play_sound_fine(
                    Sound::EntityTropicalFishFlop,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                    1.0,
                    1.0,
                );
            }
        } else if in_water
            && !entity.silent.load(Ordering::Relaxed)
            && rand::random_range(0..100) == 0
        {
            entity.world.load().play_sound_fine(
                Sound::EntityTropicalFishAmbient,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
        }
    }
}

impl CustomSound for TropicalFishEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityTropicalFishDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityTropicalFishHurt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::combat;

    #[test]
    fn test_tropical_fish_parity_constants() {
        assert_eq!(TropicalFishEntity::XP_REWARD, 2);
        assert_eq!(combat::FISH_MAX_HEALTH, 3.0);
        assert_eq!(combat::FISH_AMBIENT_SOUND_INTERVAL, 120);
    }
}
