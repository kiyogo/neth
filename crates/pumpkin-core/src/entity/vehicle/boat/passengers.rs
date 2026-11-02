use pumpkin_protocol::PositionFlag;
use pumpkin_util::math::vector3::Vector3;

use super::BoatEntity;
use crate::entity::{player::Player, EntityBase};

impl BoatEntity {
    pub(in crate::entity) fn position_rider(&self, passenger: &dyn EntityBase) {
        self.vehicle.entity.position_rider(passenger);
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
