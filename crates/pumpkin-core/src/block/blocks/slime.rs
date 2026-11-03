use pumpkin_macros::pumpkin_block;

use crate::block::{
    BlockBehaviour, BrokenArgs, OnLandedUponArgs, PlacedArgs, UpdateEntityMovementAfterFallOnArgs, bounce_entity_after_fall,
};

#[pumpkin_block("minecraft:slime_block")]
pub struct SlimeBlock;

impl SlimeBlock {
    pub const BOUNCE_FACTOR: f32 = 1.0;
    pub const DAMAGE_MULTIPLIER_NORMAL: f32 = 0.0;
    pub const DAMAGE_MULTIPLIER_SNEAKING: f32 = 1.0;
}

impl BlockBehaviour for SlimeBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

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
