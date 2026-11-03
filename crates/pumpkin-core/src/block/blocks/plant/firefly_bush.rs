use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockId, BlockStateId, tag, tag::Taggable};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

use crate::block::blocks::plant::PlantBlockBase;
use crate::block::{BlockBehaviour, BlockMetadata, BonemealArgs, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, PlacedArgs, RandomTickArgs};

/// Per-tick probability that a firefly particle is spawned when conditions are met,
/// mirroring vanilla `FireflyBushBlock.java:17`: `FIREFLY_CHANCE_PER_TICK = 0.7`.
pub const FIREFLY_CHANCE_PER_TICK: f64 = 0.7;

/// Total horizontal spread range around the bush center (-5.0 to +5.0 blocks),
/// mirroring vanilla `FireflyBushBlock.java:18`: `FIREFLY_HORIZONTAL_RANGE = 10.0`.
pub const FIREFLY_HORIZONTAL_RANGE: f64 = 10.0;

/// Total vertical spread range above the bush (0.0 to +5.0 blocks),
/// mirroring vanilla `FireflyBushBlock.java:19`: `FIREFLY_VERTICAL_RANGE = 5.0`.
pub const FIREFLY_VERTICAL_RANGE: f64 = 5.0;

/// Maximum raw block/sky brightness level allowing firefly particle spawning,
/// mirroring vanilla `FireflyBushBlock.java:20`: `FIREFLY_SPAWN_MAX_BRIGHTNESS_LEVEL = 13`.
pub const FIREFLY_SPAWN_MAX_BRIGHTNESS_LEVEL: u8 = 13;

/// 1-in-N chance per tick to play the ambient idle sound,
/// mirroring vanilla `FireflyBushBlock.java:21`: `FIREFLY_AMBIENT_SOUND_CHANCE_ONE_IN = 30`.
pub const FIREFLY_AMBIENT_SOUND_CHANCE_ONE_IN: u32 = 30;

pub struct FireflyBushBlock;

impl BlockMetadata for FireflyBushBlock {
    fn ids() -> Box<[BlockId]> {
        Box::new([BlockId::FIREFLY_BUSH])
    }
}

impl FireflyBushBlock {
    pub const CHANCE_PER_TICK: f64 = FIREFLY_CHANCE_PER_TICK;
    pub const HORIZONTAL_RANGE: f64 = FIREFLY_HORIZONTAL_RANGE;
    pub const VERTICAL_RANGE: f64 = FIREFLY_VERTICAL_RANGE;
    pub const MAX_BRIGHTNESS: u8 = FIREFLY_SPAWN_MAX_BRIGHTNESS_LEVEL;
    pub const AMBIENT_SOUND_CHANCE: u32 = FIREFLY_AMBIENT_SOUND_CHANCE_ONE_IN;

    /// Checks if at least one horizontal neighbouring block is air and can support vegetation,
    /// mirroring vanilla `BonemealableBlock.hasSpreadableNeighbourPos(level, pos, state)`
    /// in `FireflyBushBlock.java:45` and `BonemealableBlock.java:20-22`.
    #[must_use]
    pub fn has_spreadable_neighbour_pos(world: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        Self::get_spreadable_neighbour_positions(world, pos).next().is_some()
    }

    /// Iterates all horizontal neighboring positions where the firefly bush can spread.
    pub fn get_spreadable_neighbour_positions<'a>(
        world: &'a dyn BlockAccessor,
        pos: &'a BlockPos,
    ) -> impl Iterator<Item = BlockPos> + 'a {
        const HORIZONTAL_OFFSETS: [(i32, i32); 4] = [(0, -1), (0, 1), (1, 0), (-1, 0)];
        HORIZONTAL_OFFSETS.into_iter().filter_map(move |(dx, dz)| {
            let neighbor = BlockPos::new(pos.0.x + dx, pos.0.y, pos.0.z + dz);
            if world.get_block_state(&neighbor).is_air() {
                let below = neighbor.down();
                let below_block = world.get_block(&below);
                if below_block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION) {
                    return Some(neighbor);
                }
            }
            None
        })
    }

    /// Selects a random valid horizontal neighbour position to spread to,
    /// mirroring vanilla `BonemealableBlock.findSpreadableNeighbourPos(level, pos, state)`
    /// in `FireflyBushBlock.java:55` and `BonemealableBlock.java:24-26`.
    pub fn find_spreadable_neighbour_pos(
        world: &dyn BlockAccessor,
        pos: &BlockPos,
        rng: &mut impl RngExt,
    ) -> Option<BlockPos> {
        let mut dirs = [(0, -1), (0, 1), (1, 0), (-1, 0)];
        // Fisher-Yates shuffle matching Direction.Plane.HORIZONTAL.shuffledCopy
        for i in (1..dirs.len()).rev() {
            let j = rng.random_range(0..=i);
            dirs.swap(i, j);
        }
        for (dx, dz) in dirs {
            let neighbor = BlockPos::new(pos.0.x + dx, pos.0.y, pos.0.z + dz);
            if world.get_block_state(&neighbor).is_air() {
                let below = neighbor.down();
                let below_block = world.get_block(&below);
                if below_block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_VEGETATION) {
                    return Some(neighbor);
                }
            }
        }
        None
    }

    /// Validates whether a firefly particle should spawn at the given brightness and roll,
    /// mirroring `FireflyBushBlock.java:35`:
    /// `level.getMaxLocalRawBrightness(pos) <= 13 && random.nextDouble() <= 0.7`.
    #[must_use]
    pub fn should_spawn_firefly_particle(brightness: u8, roll: f64) -> bool {
        brightness <= FIREFLY_SPAWN_MAX_BRIGHTNESS_LEVEL && roll <= FIREFLY_CHANCE_PER_TICK
    }

    /// Validates whether the ambient idle sound should play,
    /// mirroring `FireflyBushBlock.java:29`: `random.nextInt(30) == 0`.
    #[must_use]
    pub fn should_play_ambient_sound(roll: u32) -> bool {
        roll == 0
    }
}

impl BlockBehaviour for FireflyBushBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        Self::has_spreadable_neighbour_pos(args.world.as_ref(), args.position)
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        let mut rng = rand::rng();
        if let Some(target_pos) =
            Self::find_spreadable_neighbour_pos(args.world.as_ref(), args.position, &mut rng)
        {
            args.world.set_block_state(
                &target_pos,
                Block::FIREFLY_BUSH.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let mut rng = rand::rng();
        if Self::should_play_ambient_sound(rng.random_range(0..FIREFLY_AMBIENT_SOUND_CHANCE_ONE_IN)) {
            args.world.play_sound(
                Sound::BlockFireflyBushIdle,
                SoundCategory::Ambient,
                &args.position.to_centered_f64(),
            );
        }
    }
}

impl PlantBlockBase for FireflyBushBlock {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firefly_bush_constants_and_probabilities_parity() {
        assert_eq!(FIREFLY_CHANCE_PER_TICK, 0.7);
        assert_eq!(FIREFLY_HORIZONTAL_RANGE, 10.0);
        assert_eq!(FIREFLY_VERTICAL_RANGE, 5.0);
        assert_eq!(FIREFLY_SPAWN_MAX_BRIGHTNESS_LEVEL, 13);
        assert_eq!(FIREFLY_AMBIENT_SOUND_CHANCE_ONE_IN, 30);

        // Particle conditions
        assert!(FireflyBushBlock::should_spawn_firefly_particle(0, 0.0));
        assert!(FireflyBushBlock::should_spawn_firefly_particle(13, 0.7));
        assert!(!FireflyBushBlock::should_spawn_firefly_particle(14, 0.5));
        assert!(!FireflyBushBlock::should_spawn_firefly_particle(10, 0.71));

        // Ambient sound conditions
        assert!(FireflyBushBlock::should_play_ambient_sound(0));
        assert!(!FireflyBushBlock::should_play_ambient_sound(1));
        assert!(!FireflyBushBlock::should_play_ambient_sound(29));
    }
}
