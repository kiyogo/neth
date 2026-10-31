use pumpkin_data::{Block, BlockState, block_properties::PaleHangingMossLikeProperties};
use pumpkin_util::{
    math::position::BlockPos,
    random::{RandomGenerator, RandomImpl},
};

use crate::generation::proto_chunk::GenerationCache;

/// Default leaves hanging moss probability (0.15).
///
/// Mirrors vanilla `data/minecraft/worldgen/feature/pale_oak.json`:
/// `{"leaves_probability": 0.15, ...}`
pub const DEFAULT_LEAVES_PROBABILITY: f32 = 0.15;

/// Default trunk hanging moss probability (0.4).
///
/// Mirrors vanilla `data/minecraft/worldgen/feature/pale_oak.json`:
/// `{"trunk_probability": 0.4, ...}`
pub const DEFAULT_TRUNK_PROBABILITY: f32 = 0.4;

/// Default ground pale moss patch probability (0.8).
///
/// Mirrors vanilla `data/minecraft/worldgen/feature/pale_oak.json`:
/// `{"ground_probability": 0.8, ...}`
pub const DEFAULT_GROUND_PROBABILITY: f32 = 0.8;

/// Chance for hanging moss to continue downwards per segment (0.5 = 50%).
///
/// Mirrors vanilla `PaleMossDecorator.java:77`:
/// `!(context.random().nextFloat() < 0.5)`
pub const CONTINUATION_CHANCE: f32 = 0.5;

#[inline]
#[must_use]
pub const fn should_continue_hanging_moss(roll: f32) -> bool {
    roll >= CONTINUATION_CHANCE
}

pub struct PaleMossTreeDecorator {
    pub leaves_probability: f32,
    pub trunk_probability: f32,
    pub ground_probability: f32,
}

impl PaleMossTreeDecorator {
    pub fn generate<T: GenerationCache>(
        &self,
        chunk: &mut T,
        random: &mut RandomGenerator,
        log_positions: &[BlockPos],
        foliage_positions: &[BlockPos],
        decorations: &mut Vec<BlockPos>,
    ) {
        if log_positions.is_empty() {
            return;
        }

        for &pos in log_positions {
            if random.next_f32() < self.trunk_probability {
                let down = pos.down();
                if chunk.is_air(&down.0) {
                    Self::add_moss_hanger(chunk, random, down, decorations);
                }
            }
        }

        for &pos in foliage_positions {
            if random.next_f32() < self.leaves_probability {
                let down = pos.down();
                if chunk.is_air(&down.0) {
                    Self::add_moss_hanger(chunk, random, down, decorations);
                }
            }
        }
    }

    fn add_moss_hanger<T: GenerationCache>(
        chunk: &mut T,
        random: &mut RandomGenerator,
        mut pos: BlockPos,
        decorations: &mut Vec<BlockPos>,
    ) {
        while chunk.is_air(&pos.down().0) && should_continue_hanging_moss(random.next_f32()) {
            let props = PaleHangingMossLikeProperties { tip: false };
            let state_id = props.to_state_id(&Block::PALE_HANGING_MOSS);
            chunk.set_block_state(&pos.0, BlockState::from_id(state_id));
            decorations.push(pos);
            pos = pos.down();
        }

        let props = PaleHangingMossLikeProperties { tip: true };
        let state_id = props.to_state_id(&Block::PALE_HANGING_MOSS);
        chunk.set_block_state(&pos.0, BlockState::from_id(state_id));
        decorations.push(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pale_moss_decorator_parity() {
        assert_eq!(DEFAULT_LEAVES_PROBABILITY, 0.15);
        assert_eq!(DEFAULT_TRUNK_PROBABILITY, 0.4);
        assert_eq!(DEFAULT_GROUND_PROBABILITY, 0.8);
        assert_eq!(CONTINUATION_CHANCE, 0.5);
        assert!(should_continue_hanging_moss(0.5));
        assert!(should_continue_hanging_moss(0.7));
        assert!(!should_continue_hanging_moss(0.49));
    }
}
