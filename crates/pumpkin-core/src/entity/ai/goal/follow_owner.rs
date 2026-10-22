use super::{Controls, Goal};
use crate::entity::ai::pathfinder::node::PathType;
use crate::entity::ai::pathfinder::pathfinding_context::PathfindingContext;
use crate::entity::mob::Mob;
use crate::entity::player::Player;
use crate::world::World;
use pumpkin_data::Block;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use std::sync::Arc;

const TELEPORT_DISTANCE_SQ: f64 = 144.0;

pub struct FollowOwnerGoal {
    speed_modifier: f64,
    start_distance_sq: f64,
    stop_distance_sq: f64,
    time_to_recalc_path: i32,
    owner: Option<Arc<Player>>,
    old_water_cost: f32,
}

impl FollowOwnerGoal {
    #[must_use]
    pub fn new(speed_modifier: f64, start_distance: f32, stop_distance: f32) -> Box<Self> {
        Box::new(Self {
            speed_modifier,
            start_distance_sq: f64::from(start_distance) * f64::from(start_distance),
            stop_distance_sq: f64::from(stop_distance) * f64::from(stop_distance),
            time_to_recalc_path: 0,
            owner: None,
            old_water_cost: 0.0,
        })
    }

    fn unable_to_move_to_owner(mob: &dyn Mob, owner: Option<&Player>) -> bool {
        if mob.is_sitting() {
            return true;
        }
        if let Some(tamable) = mob.as_tamable()
            && tamable.is_unable_to_move_to_owner()
        {
            return true;
        }
        let mob_entity = &mob.get_mob_entity().living_entity.entity;
        if mob_entity.has_vehicle() || mob_entity.is_leashed() {
            return true;
        }
        let Some(owner) = owner else {
            return true;
        };
        if owner.is_spectator() || !owner.living_entity.entity.is_alive() {
            return true;
        }
        false
    }

    fn find_owner(mob: &dyn Mob) -> Option<Arc<Player>> {
        let owner_uuid = mob.get_owner_uuid()?;
        let world = mob.get_mob_entity().living_entity.entity.world.load_full();
        let player = world.get_player_by_uuid(owner_uuid)?;
        if player.is_spectator() {
            return None;
        }
        Some(player)
    }

    fn distance_to_owner_sq(mob: &dyn Mob, owner: &Player) -> f64 {
        let mob_pos = mob.get_mob_entity().living_entity.entity.pos.load();
        let owner_pos = owner.living_entity.entity.pos.load();
        mob_pos.squared_distance_to_vec(&owner_pos)
    }

    fn should_try_teleport_to_owner(mob: &dyn Mob, owner: &Player) -> bool {
        let dist_sq = Self::distance_to_owner_sq(mob, owner);
        dist_sq >= TELEPORT_DISTANCE_SQ
    }

    fn can_teleport_to(
        ctx: &mut PathfindingContext,
        world: &World,
        mob: &dyn Mob,
        target: BlockPos,
    ) -> bool {
        let mob_entity = &mob.get_mob_entity().living_entity.entity;

        // PathType check matching Vanilla: WalkNodeEvaluator.getPathTypeStatic(this, pos) == PathType.WALKABLE
        let path_type = ctx.get_land_node_type(target.0);
        if path_type != PathType::Walkable {
            return false;
        }

        // Block below must not be leaves
        let below_pos = BlockPos::new(target.0.x, target.0.y - 1, target.0.z);
        let below_state = world.get_block_state(&below_pos);
        let below_block = Block::from_state_id(below_state.id);
        if below_block.has_tag(&tag::Block::MINECRAFT_LEAVES) {
            return false;
        }

        // Bounding box at destination must have no collision and no liquid
        let target_pos = Vector3::new(
            target.0.x as f64 + 0.5,
            f64::from(target.0.y),
            target.0.z as f64 + 0.5,
        );
        let target_bb = BoundingBox::new_from_pos(
            target_pos.x,
            target_pos.y,
            target_pos.z,
            &mob_entity.entity_dimension.load(),
        );

        if world.check_fluid_collision(target_bb) {
            return false;
        }

        let (collisions, _) = world.get_block_collisions(target_bb, mob_entity);
        if !collisions.is_empty() {
            return false;
        }

        true
    }

    fn try_teleport_to_owner(mob: &dyn Mob, owner: &Player) {
        let owner_pos = owner.living_entity.entity.pos.load();
        let mob_entity = &mob.get_mob_entity().living_entity.entity;
        let world = mob_entity.world.load_full();

        let base_x = owner_pos.x.floor() as i32;
        let base_y = owner_pos.y.floor() as i32;
        let base_z = owner_pos.z.floor() as i32;

        let mut ctx = PathfindingContext::new(Vector3::new(base_x, base_y, base_z), world.clone());

        let mut rng = mob.get_random();
        for _ in 0..10 {
            let dx: i32 = rng.random_range(-3..=3);
            let dz: i32 = rng.random_range(-3..=3);
            if dx.abs() < 2 && dz.abs() < 2 {
                continue;
            }

            let dy = rng.random_range(-1..=1);
            let target = BlockPos::new(base_x + dx, base_y + dy, base_z + dz);

            if Self::can_teleport_to(&mut ctx, &world, mob, target) {
                mob_entity.teleport(
                    Vector3::new(
                        target.0.x as f64 + 0.5,
                        f64::from(target.0.y),
                        target.0.z as f64 + 0.5,
                    ),
                    None,
                    None,
                    &world,
                );

                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.stop();
                return;
            }
        }
    }
}

impl Goal for FollowOwnerGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(owner) = Self::find_owner(mob) else {
            return false;
        };

        if Self::unable_to_move_to_owner(mob, Some(&owner)) {
            return false;
        }

        let dist_sq = Self::distance_to_owner_sq(mob, &owner);
        if dist_sq < self.start_distance_sq {
            return false;
        }

        self.owner = Some(owner);
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        if Self::unable_to_move_to_owner(mob, self.owner.as_deref()) {
            return false;
        }

        let Some(owner) = &self.owner else {
            return false;
        };

        if Self::should_try_teleport_to_owner(mob, owner) {
            return true;
        }

        let is_done = mob
            .get_mob_entity()
            .navigator
            .lock()
            .map_or(false, |n| n.is_done());
        if is_done {
            return false;
        }

        let dist_sq = Self::distance_to_owner_sq(mob, owner);
        dist_sq > self.stop_distance_sq
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.time_to_recalc_path = 0;
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.old_water_cost = navigator.get_pathfinding_malus(PathType::Water);
        navigator.set_pathfinding_malus(PathType::Water, 0.0);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.owner = None;
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.stop();
        navigator.set_pathfinding_malus(PathType::Water, self.old_water_cost);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(owner) = &self.owner else {
            return;
        };

        let is_owner_far_away = Self::should_try_teleport_to_owner(mob, owner);

        if !is_owner_far_away {
            let mob_entity = mob.get_mob_entity();
            let owner_eye_pos = owner.living_entity.entity.get_eye_pos();
            let mut look_control = mob_entity
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            look_control.look_at_with_range(
                owner_eye_pos.x,
                owner_eye_pos.y,
                owner_eye_pos.z,
                10.0,
                mob.get_max_look_pitch_change(),
            );
        }

        self.time_to_recalc_path -= 1;
        if self.time_to_recalc_path <= 0 {
            self.time_to_recalc_path = self.get_tick_count(10);
            if is_owner_far_away {
                Self::try_teleport_to_owner(mob, owner);
            } else {
                let mut navigator = mob
                    .get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.move_to_entity(
                    &owner.living_entity,
                    self.speed_modifier,
                    &mob.get_mob_entity().living_entity,
                );
            }
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
