//! Shared utilities for fish entities (Cod, Salmon, Pufferfish, Tropical Fish).

use pumpkin_util::math::vector3::Vector3;

use crate::entity::EntityBase;
use crate::entity::mob::Mob;

/// Vanilla `AbstractFish.travel` in water: a slow sink instead of gravity, and no climbing out at
/// the shore the way walking mobs do.
pub fn travel_in_water(mob: &dyn Mob, caller: &dyn EntityBase) -> bool {
    let living = &mob.get_mob_entity().living_entity;
    let entity = &living.entity;
    let input = living.movement_input.load();
    entity.update_velocity_from_input(Vector3::new(input.x, 0.0, input.z), 0.01);
    entity.move_entity(caller, entity.velocity.load());

    // A fish never has a target, so it always takes vanilla's targetless sink.
    let velocity = entity.velocity.load() * 0.9;
    entity
        .velocity
        .store(Vector3::new(velocity.x, velocity.y - 0.005, velocity.z));
    true
}
