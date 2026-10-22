use super::{Controls, Goal};
use crate::entity::mob::Mob;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct SitWhenOrderedToGoal {
    goal_control: Controls,
}

impl SitWhenOrderedToGoal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            goal_control: Controls::MOVE.union(Controls::JUMP),
        }
    }
}

impl Goal for SitWhenOrderedToGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(tamable) = mob.as_tamable() else {
            return false;
        };

        if !tamable.is_ordered_to_sit() || !tamable.is_tame() {
            return false;
        }

        let entity = &mob.get_mob_entity().living_entity.entity;
        if entity.is_in_water() {
            return false;
        }

        if !entity.on_ground.load(Ordering::Relaxed) {
            return false;
        }

        if let Some(owner_uuid) = tamable.get_owner() {
            let world = entity.world.load_full();
            if let Some(owner) = world.get_player_by_uuid(owner_uuid) {
                let dist_sq = entity
                    .pos
                    .load()
                    .squared_distance_to_vec(&owner.living_entity.entity.pos.load());
                if dist_sq < 144.0
                    && owner
                        .living_entity
                        .last_attacker_id
                        .load(Ordering::Relaxed)
                        > 0
                {
                    return false;
                }
            }
        }

        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(tamable) = mob.as_tamable() else {
            return false;
        };
        tamable.is_ordered_to_sit()
    }

    fn start(&mut self, mob: &dyn Mob) {
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.stop();
        if let Some(tamable) = mob.as_tamable() {
            tamable.set_in_sitting_pose(true);
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if let Some(tamable) = mob.as_tamable() {
            tamable.set_in_sitting_pose(false);
        }
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
