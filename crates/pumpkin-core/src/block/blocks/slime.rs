use pumpkin_macros::pumpkin_block;

use crate::block::{
    BlockBehaviour, OnLandedUponArgs, UpdateEntityMovementAfterFallOnArgs, bounce_entity_after_fall,
};

#[pumpkin_block("minecraft:slime_block")]
pub struct SlimeBlock;

impl BlockBehaviour for SlimeBlock {
    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        if let Some(living) = args.entity.get_living_entity() {
            // Vanilla `SlimeBlock.fallOn`: if entity is suppressing bounce (sneaking),
            // it takes full fall damage (damage modifier 1.0). Otherwise, damage is negated (0.0).
            let damage_multiplier = if args.entity.get_entity().is_sneaking() {
                1.0
            } else {
                0.0
            };
            living.handle_fall_damage(args.entity, args.fall_distance, damage_multiplier);
        }
    }

    fn update_entity_movement_after_fall_on(&self, args: UpdateEntityMovementAfterFallOnArgs<'_>) {
        bounce_entity_after_fall(args.entity, 1.0);
    }
}
