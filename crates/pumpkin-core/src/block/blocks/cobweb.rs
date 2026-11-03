use crate::block::{BlockBehaviour, BrokenArgs, OnEntityCollisionArgs, PlacedArgs};
use crate::entity::EntityBase;
use pumpkin_data::effect::StatusEffect;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::vector3::Vector3;

#[pumpkin_block(Block::COBWEB)]
pub struct CobwebBlock;

impl CobwebBlock {
    pub const SLOW_MULTIPLIER_NORMAL: (f64, f64, f64) = (0.25, 0.05, 0.25);
    pub const SLOW_MULTIPLIER_WEAVING: (f64, f64, f64) = (0.5, 0.25, 0.5);
}

impl BlockBehaviour for CobwebBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        let entity = args.entity.get_entity();
        let vec = if let Some(living) = entity.get_living_entity()
            && living.has_effect(&StatusEffect::WEAVING)
        {
            Vector3::new(0.5, 0.25, 0.5)
        } else {
            Vector3::new(0.25, 0.05, 0.25)
        };
        entity.slow_movement(args.state, vec);
    }
}
