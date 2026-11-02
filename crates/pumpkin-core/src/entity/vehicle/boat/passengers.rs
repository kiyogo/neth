use pumpkin_protocol::PositionFlag;
use pumpkin_util::math::vector3::Vector3;

use super::BoatEntity;
use crate::entity::{player::Player, EntityBase};

impl BoatEntity {
    pub(in crate::entity) fn position_rider(&self, passenger: &dyn EntityBase) {
        self.vehicle.entity.position_rider(passenger);
        self.clamp_rotation(passenger);
    }

    /// Clamps passenger body and head yaw inside a boat (`AbstractBoat.java:693-706`).
    pub fn clamp_rotation(&self, passenger: &dyn EntityBase) -> f32 {
        let entity = &self.vehicle.entity;
        let passenger_entity = passenger.get_entity();
        let passengers = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let total = passengers.len();
        drop(passengers);

        let boat_yaw = entity.yaw.load();
        let is_mob = passenger.get_player().is_none();
        let (new_yaw, y_delta) = crate::entity::combat::clamp_boat_passenger_rotation(
            boat_yaw,
            passenger_entity.entity_type,
            passenger_entity.entity_id,
            passenger_entity.yaw.load(),
            total,
            crate::entity::combat::BOAT_DEFAULT_MAX_PASSENGERS,
            is_mob,
        );

        let body_yaw = crate::entity::combat::calculate_boat_passenger_body_y_rot(
            boat_yaw,
            passenger_entity.entity_type,
            passenger_entity.entity_id,
            total,
            crate::entity::combat::BOAT_DEFAULT_MAX_PASSENGERS,
        );

        passenger_entity.body_yaw.store(body_yaw);
        if (passenger_entity.yaw.load() - new_yaw).abs() > 1e-4 {
            passenger_entity.yaw.store(new_yaw);
            passenger_entity.head_yaw.store(new_yaw);
        }
        y_delta
    }

    /// Called when a passenger turns their head / body while riding this boat (`AbstractBoat.java:716-719`).
    pub fn on_passenger_turned(&self, passenger: &dyn EntityBase) {
        self.clamp_rotation(passenger);
    }

    pub(in crate::entity) fn synchronize_mount(&self, player: &Player) -> bool {
        let Some(position) = self.rider_position(player) else {
            return false;
        };
        if !player.request_teleport_with_flags(
            position,
            0.0,
            0.0,
            vec![PositionFlag::YRot, PositionFlag::XRot],
        ) {
            return false;
        }
        player.get_entity().velocity.store(Vector3::default());
        true
    }

    fn rider_position(&self, passenger: &dyn EntityBase) -> Option<Vector3<f64>> {
        let entity = &self.vehicle.entity;
        let passengers = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let index = passengers
            .iter()
            .position(|rider| rider.get_entity().entity_id == passenger.get_entity().entity_id)?;
        let total = passengers.len();
        drop(passengers);
        Some(entity.calculate_passenger_pos(passenger, index, total))
    }
}
