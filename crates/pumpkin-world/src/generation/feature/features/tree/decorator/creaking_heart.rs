use pumpkin_data::{
    Block, BlockDirection, BlockState,
    block_properties::{Axis, CreakingHeartLikeProperties, CreakingHeartState},
    tag,
};
use pumpkin_util::{
    math::position::BlockPos,
    random::{RandomGenerator, RandomImpl},
};

use crate::generation::proto_chunk::GenerationCache;

/// Default probability of placing Creaking Heart decorator in Pale Oak tree (1.0 = 100%).
///
/// Mirrors vanilla `data/minecraft/worldgen/feature/pale_oak_creaking.json`:
/// `{"type": "minecraft:creaking_heart", "probability": 1.0}`
pub const DEFAULT_PROBABILITY: f32 = 1.0;

/// Number of adjacent log blocks required on all 6 faces (UP, DOWN, NORTH, SOUTH, EAST, WEST).
///
/// Mirrors vanilla `CreakingHeartDecorator.java:41-47`:
/// `for (Direction dir : Direction.values()) { if (!state.is(BlockTags.LOGS)) return false; }`
pub const REQUIRED_ADJACENT_LOGS: usize = 6;

#[inline]
#[must_use]
pub const fn is_valid_creaking_heart_placement(adjacent_logs: usize) -> bool {
    adjacent_logs == REQUIRED_ADJACENT_LOGS
}

pub struct CreakingHeartTreeDecorator {
    pub probability: f32,
}

impl CreakingHeartTreeDecorator {
    pub fn generate<T: GenerationCache>(
        &self,
        chunk: &mut T,
        random: &mut RandomGenerator,
        log_positions: &[BlockPos],
        decorations: &mut Vec<BlockPos>,
    ) {
        if log_positions.is_empty() || random.next_f32() >= self.probability {
            return;
        }

        let mut shuffled = log_positions.to_vec();
        for i in (1..shuffled.len()).rev() {
            let j = random.next_bounded_i32((i + 1) as i32) as usize;
            shuffled.swap(i, j);
        }

        for pos in shuffled {
            let mut all_logs = true;
            for dir in BlockDirection::all() {
                let neighbor = pos.offset(dir.to_offset());
                let block_id = GenerationCache::get_block_state(chunk, &neighbor.0).to_block_id();
                if !block_id.has_tag(tag::Block::MINECRAFT_LOGS) {
                    all_logs = false;
                    break;
                }
            }

            if all_logs {
                let props = CreakingHeartLikeProperties {
                    r#axis: Axis::Y,
                    r#creaking_heart_state: CreakingHeartState::Dormant,
                    r#natural: true,
                };
                let state_id = props.to_state_id(&Block::CREAKING_HEART);
                chunk.set_block_state(&pos.0, BlockState::from_id(state_id));
                decorations.push(pos);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creaking_heart_decorator_parity() {
        assert_eq!(DEFAULT_PROBABILITY, 1.0);
        assert_eq!(REQUIRED_ADJACENT_LOGS, 6);
        assert!(is_valid_creaking_heart_placement(6));
        assert!(!is_valid_creaking_heart_placement(5));
        assert!(!is_valid_creaking_heart_placement(0));
    }
}
