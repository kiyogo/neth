use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, BrokenArgs, EmitsRedstonePowerArgs, GetRedstonePowerArgs, PlacedArgs};

#[pumpkin_block("minecraft:redstone_block")]
pub struct RedstoneBlock;

impl RedstoneBlock {
    pub const POWER: u8 = 15;

    #[must_use]
    pub const fn emits_weak_power() -> u8 {
        Self::POWER
    }

    #[must_use]
    pub const fn emits_strong_power() -> u8 {
        0
    }
}

impl BlockBehaviour for RedstoneBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn get_weak_redstone_power(&self, _args: GetRedstonePowerArgs<'_>) -> u8 {
        Self::POWER
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }
}
