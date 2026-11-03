use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, PlacedArgs};
use crate::entity::experience_orb::ExperienceOrbEntity;
use pumpkin_data::BlockId;
use pumpkin_util::GameMode;

pub struct SculkBlock;

impl SculkBlock {
    pub const XP_AMOUNT: u32 = 1;
    pub const GROWTH_INHIBITOR_RANGE: i32 = 4;
}

impl BlockMetadata for SculkBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK].into()
    }
}

impl BlockBehaviour for SculkBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative {
            ExperienceOrbEntity::spawn(
                args.world,
                args.position.to_centered_f64(),
                Self::XP_AMOUNT,
            );
        }
        args.world.update_neighbors(args.position, None);
    }
}
