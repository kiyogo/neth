use pumpkin_data::tag::{self, Taggable};
use pumpkin_util::math::{
    boundingbox::{BoundingBox, EntityDimensions},
    position::BlockPos,
    vector3::Vector3,
};

use super::BoatEntity;
use crate::entity::EntityBase;
use crate::world::World;

impl BoatEntity {
    pub(in crate::entity) fn get_dismount_location_for_passenger(
        &self,
        passenger: &dyn EntityBase,
    ) -> Vector3<f64> {
        let entity = &self.vehicle.entity;
        let passenger_entity = passenger.get_entity();
        let pos = entity.pos.load();
        let top = entity.bounding_box.load().max.y;
        let fallback = Vector3::new(pos.x, top, pos.z);
        let direction = crate::entity::combat::get_collision_horizontal_escape_vector(
            f64::from(entity.width() * std::f32::consts::SQRT_2),
            f64::from(passenger_entity.width()),
            passenger_entity.yaw.load(),
        );
        let target = fallback + direction;
        let target_pos = BlockPos::floored_v(target);
        let world = entity.world.load();
        let (fluid, _) = world.get_fluid_and_fluid_state(&target_pos.down());
        if fluid.has_tag(&tag::Fluid::MINECRAFT_WATER) {
            return fallback;
        }

        let targets = [target_pos, target_pos.down()].map(|block_pos| {
            let floor = world.get_dismount_height(&block_pos);
            (floor.is_finite() && floor < 1.0).then_some(Vector3::new(
                target.x,
                f64::from(block_pos.0.y) + floor,
                target.z,
            ))
        });
        let is_player = passenger.get_player().is_some();
        let poses = crate::entity::combat::get_entity_dismount_poses(is_player);
        for &pose in poses {
            let dimensions = if is_player {
                passenger_entity.get_dimensions(pose)
            } else {
                passenger_entity.entity_dimension.load()
            };
            for target in targets.into_iter().flatten() {
                if !can_dismount_to(&world, passenger, target, &dimensions) {
                    continue;
                }
                if is_player {
                    passenger_entity.set_pose(pose);
                    if passenger_entity.pose.load() != pose {
                        continue;
                    }
                }
                return target;
            }
        }
        fallback
    }
}

pub(crate) fn can_dismount_to(
    world: &World,
    passenger: &dyn EntityBase,
    location: Vector3<f64>,
    dimensions: &EntityDimensions,
) -> bool {
    let bounds = BoundingBox::new_from_pos(location.x, location.y, location.z, dimensions);
    if !world.get_block_collisions(bounds, passenger).0.is_empty() {
        return false;
    }
    let border = world
        .worldborder
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    border.contains(bounds.min.x, bounds.min.z)
        && border.contains(
            bounds.max.x - crate::entity::combat::DISMOUNT_EPSILON,
            bounds.max.z - crate::entity::combat::DISMOUNT_EPSILON,
        )
}
