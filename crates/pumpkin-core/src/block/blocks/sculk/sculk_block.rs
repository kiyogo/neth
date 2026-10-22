use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs};
use crate::entity::experience_orb::ExperienceOrbEntity;
use pumpkin_data::BlockId;
use pumpkin_util::GameMode;

pub struct SculkBlock;

impl BlockMetadata for SculkBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK].into()
    }
}

impl BlockBehaviour for SculkBlock {
    fn broken(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative {
            ExperienceOrbEntity::spawn(args.world, args.position.to_centered_f64(), 1);
        }
    }
}
