use pumpkin_data::block_properties::{HorizontalFacing, ShelfMushroomLikeProperties};
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState};
use pumpkin_util::{
    math::position::BlockPos,
    random::{RandomGenerator, RandomImpl},
};

use crate::generation::proto_chunk::GenerationCache;

/// Minimum height offset above tree base where shelf mushrooms can generate on standing trees (1 block).
/// Mirrors vanilla `ShelfMushroomDecorator.java:18`:
/// `private static final int MIN_HEIGHT_OFFSET = 1;`
pub const MIN_HEIGHT_OFFSET: i32 = 1;

/// Maximum height offset above tree base where shelf mushrooms can generate on standing trees (4 blocks).
/// Mirrors vanilla `ShelfMushroomDecorator.java:19`:
/// `private static final int MAX_HEIGHT_OFFSET = 4;`
pub const MAX_HEIGHT_OFFSET: i32 = 4;

/// Per-side placement chance when attempting to place shelf mushroom (0.25 = 25%).
/// Mirrors vanilla `ShelfMushroomDecorator.java:20`:
/// `private static final float PER_SIDE_PLACEMENT_CHANCE = 0.25F;`
pub const PER_SIDE_PLACEMENT_CHANCE: f32 = 0.25;

/// Maximum exclusive age for generated shelf mushrooms (2 -> age 0 or 1).
/// Mirrors vanilla `ShelfMushroomDecorator.java:21`:
/// `private static final int MAX_AGE_EXCLUSIVE = 2;`
pub const MAX_AGE_EXCLUSIVE: i32 = 2;

/// Standard probability for poplar trees (0.4 = 40%).
/// Mirrors vanilla `TreeFeatures.java:670-692`: `new ShelfMushroomDecorator(0.4F)`
pub const POPLAR_SHELF_MUSHROOM_PROBABILITY: f32 = 0.4;

/// Standard probability for fallen trees (0.8 = 80%).
/// Mirrors vanilla `TreeFeatures.java:184`: `new ShelfMushroomDecorator(0.8F)`
pub const FALLEN_TREE_SHELF_MUSHROOM_PROBABILITY: f32 = 0.8;

/// Tree decorator that generates shelf mushrooms on standing trees or fallen logs.
/// Mirrors vanilla `net.minecraft.world.level.levelgen.feature.treedecorators.ShelfMushroomDecorator`.
pub struct ShelfMushroomTreeDecorator {
    pub placement_probability: f32,
}

impl ShelfMushroomTreeDecorator {
    #[must_use]
    pub const fn new(placement_probability: f32) -> Self {
        Self {
            placement_probability,
        }
    }

    pub fn generate<T: GenerationCache>(
        &self,
        chunk: &mut T,
        random: &mut RandomGenerator,
        log_positions: &[BlockPos],
        decorations: &mut Vec<BlockPos>,
    ) {
        if log_positions.is_empty() || random.next_f32() >= self.placement_probability {
            return;
        }

        if Self::is_fallen_log(log_positions) {
            Self::place_on_fallen_log(chunk, log_positions, random, decorations);
        } else {
            Self::place_on_standing_tree(chunk, log_positions, random, decorations);
        }
    }

    fn place_on_standing_tree<T: GenerationCache>(
        chunk: &mut T,
        logs: &[BlockPos],
        random: &mut RandomGenerator,
        decorations: &mut Vec<BlockPos>,
    ) {
        let directions = Self::pick_two_perpendicular_directions(random);
        let tree_base_y = logs[0].0.y;

        for &log_pos in logs {
            if Self::is_within_decoratable_height(&log_pos, tree_base_y) {
                for &facing in &directions {
                    if random.next_f32() <= PER_SIDE_PLACEMENT_CHANCE
                        && Self::try_place_mushroom_on_standing_tree(
                            chunk,
                            log_pos,
                            facing,
                            random,
                            decorations,
                        )
                    {
                        break;
                    }
                }
            }
        }
    }

    fn place_on_fallen_log<T: GenerationCache>(
        chunk: &mut T,
        logs: &[BlockPos],
        random: &mut RandomGenerator,
        decorations: &mut Vec<BlockPos>,
    ) {
        let directions = Self::perpendicular_to_fallen_log(logs);

        for &log_pos in logs {
            for &facing in &directions {
                if random.next_f32() <= PER_SIDE_PLACEMENT_CHANCE {
                    Self::try_place_mushroom_on_fallen_tree(
                        chunk,
                        log_pos,
                        facing,
                        random,
                        decorations,
                    );
                }
            }
        }
    }

    fn try_place_mushroom_on_standing_tree<T: GenerationCache>(
        chunk: &mut T,
        log_pos: BlockPos,
        facing: BlockDirection,
        random: &mut RandomGenerator,
        decorations: &mut Vec<BlockPos>,
    ) -> bool {
        let mushroom_pos = Self::mushroom_pos_for(log_pos, facing);
        if !Self::is_block_replaceable_with_shelf_mushroom(chunk, &mushroom_pos) {
            return false;
        }

        if Self::has_shelf_mushroom_at(chunk, &mushroom_pos.down()) {
            return false;
        }

        Self::place_mushroom(chunk, mushroom_pos, facing, random, decorations);
        true
    }

    fn try_place_mushroom_on_fallen_tree<T: GenerationCache>(
        chunk: &mut T,
        log_pos: BlockPos,
        facing: BlockDirection,
        random: &mut RandomGenerator,
        decorations: &mut Vec<BlockPos>,
    ) {
        let mushroom_pos = Self::mushroom_pos_for(log_pos, facing);
        if Self::is_block_replaceable_with_shelf_mushroom(chunk, &mushroom_pos)
            && !Self::has_horizontally_adjacent_shelf_mushroom(chunk, &mushroom_pos)
            && !Self::has_horizontally_adjacent_shelf_mushroom(chunk, &log_pos)
        {
            Self::place_mushroom(chunk, mushroom_pos, facing, random, decorations);
        }
    }

    #[must_use]
    pub fn is_fallen_log(logs: &[BlockPos]) -> bool {
        if logs.is_empty() {
            return false;
        }
        logs[0].0.y == logs[logs.len() - 1].0.y
    }

    #[must_use]
    pub fn pick_two_perpendicular_directions(random: &mut RandomGenerator) -> [BlockDirection; 2] {
        let first =
            BlockDirection::from_cardinal_direction(BlockDirection::random_horizontal(random));
        let second = first.rotate_clockwise();
        [first, second]
    }

    #[must_use]
    pub fn perpendicular_to_fallen_log(logs: &[BlockPos]) -> [BlockDirection; 2] {
        if logs.len() < 2 {
            return [BlockDirection::North, BlockDirection::South];
        }
        let first = logs[0];
        let last = logs[logs.len() - 1];
        let is_x_axis = first.0.x != last.0.x;
        if is_x_axis {
            [BlockDirection::North, BlockDirection::South]
        } else {
            [BlockDirection::East, BlockDirection::West]
        }
    }

    #[must_use]
    pub const fn is_within_decoratable_height(pos: &BlockPos, tree_base_y: i32) -> bool {
        let dy = pos.0.y - tree_base_y;
        dy >= MIN_HEIGHT_OFFSET && dy <= MAX_HEIGHT_OFFSET
    }

    #[must_use]
    pub fn mushroom_pos_for(log_pos: BlockPos, facing: BlockDirection) -> BlockPos {
        log_pos.offset(facing.to_offset())
    }

    fn place_mushroom<T: GenerationCache>(
        chunk: &mut T,
        pos: BlockPos,
        facing: BlockDirection,
        random: &mut RandomGenerator,
        decorations: &mut Vec<BlockPos>,
    ) {
        let horizontal_facing = match facing {
            BlockDirection::North => HorizontalFacing::North,
            BlockDirection::South => HorizontalFacing::South,
            BlockDirection::East => HorizontalFacing::East,
            BlockDirection::West => HorizontalFacing::West,
            _ => HorizontalFacing::North,
        };
        let age = random.next_bounded_i32(MAX_AGE_EXCLUSIVE) as u8;
        let props = ShelfMushroomLikeProperties {
            age,
            facing: horizontal_facing,
        };
        let state_id = props.to_state_id(&Block::SHELF_MUSHROOM);
        chunk.set_block_state(&pos.0, BlockState::from_id(state_id));
        decorations.push(pos);
    }

    fn has_shelf_mushroom_at<T: GenerationCache>(chunk: &T, pos: &BlockPos) -> bool {
        let block_id = GenerationCache::get_block_state(chunk, &pos.0).to_block_id();
        block_id == BlockId::SHELF_MUSHROOM
    }

    fn has_horizontally_adjacent_shelf_mushroom<T: GenerationCache>(
        chunk: &T,
        pos: &BlockPos,
    ) -> bool {
        let horizontal_dirs = [
            BlockDirection::North,
            BlockDirection::South,
            BlockDirection::East,
            BlockDirection::West,
        ];
        for dir in horizontal_dirs {
            let neighbor = pos.offset(dir.to_offset());
            if Self::has_shelf_mushroom_at(chunk, &neighbor) {
                return true;
            }
        }
        false
    }

    pub fn is_block_replaceable_with_shelf_mushroom<T: GenerationCache>(
        chunk: &T,
        pos: &BlockPos,
    ) -> bool {
        chunk.is_air(&pos.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shelf_mushroom_decorator_constants_parity() {
        assert_eq!(MIN_HEIGHT_OFFSET, 1);
        assert_eq!(MAX_HEIGHT_OFFSET, 4);
        assert_eq!(PER_SIDE_PLACEMENT_CHANCE, 0.25);
        assert_eq!(MAX_AGE_EXCLUSIVE, 2);
        assert_eq!(POPLAR_SHELF_MUSHROOM_PROBABILITY, 0.4);
        assert_eq!(FALLEN_TREE_SHELF_MUSHROOM_PROBABILITY, 0.8);
    }

    #[test]
    fn test_shelf_mushroom_fallen_log_detection() {
        let standing_logs = vec![
            BlockPos::new(0, 64, 0),
            BlockPos::new(0, 65, 0),
            BlockPos::new(0, 66, 0),
        ];
        assert!(!ShelfMushroomTreeDecorator::is_fallen_log(&standing_logs));

        let fallen_logs_x = vec![
            BlockPos::new(0, 64, 0),
            BlockPos::new(1, 64, 0),
            BlockPos::new(2, 64, 0),
        ];
        assert!(ShelfMushroomTreeDecorator::is_fallen_log(&fallen_logs_x));

        let fallen_logs_z = vec![
            BlockPos::new(0, 64, 0),
            BlockPos::new(0, 64, 1),
            BlockPos::new(0, 64, 2),
        ];
        assert!(ShelfMushroomTreeDecorator::is_fallen_log(&fallen_logs_z));
    }

    #[test]
    fn test_shelf_mushroom_decoratable_height() {
        let base_y = 64;
        assert!(!ShelfMushroomTreeDecorator::is_within_decoratable_height(&BlockPos::new(0, 64, 0), base_y));
        assert!(ShelfMushroomTreeDecorator::is_within_decoratable_height(&BlockPos::new(0, 65, 0), base_y));
        assert!(ShelfMushroomTreeDecorator::is_within_decoratable_height(&BlockPos::new(0, 66, 0), base_y));
        assert!(ShelfMushroomTreeDecorator::is_within_decoratable_height(&BlockPos::new(0, 67, 0), base_y));
        assert!(ShelfMushroomTreeDecorator::is_within_decoratable_height(&BlockPos::new(0, 68, 0), base_y));
        assert!(!ShelfMushroomTreeDecorator::is_within_decoratable_height(&BlockPos::new(0, 69, 0), base_y));
    }

    #[test]
    fn test_shelf_mushroom_perpendicular_directions() {
        let fallen_x = vec![BlockPos::new(0, 64, 0), BlockPos::new(3, 64, 0)];
        let perp_x = ShelfMushroomTreeDecorator::perpendicular_to_fallen_log(&fallen_x);
        assert_eq!(perp_x, [BlockDirection::North, BlockDirection::South]);

        let fallen_z = vec![BlockPos::new(0, 64, 0), BlockPos::new(0, 64, 3)];
        let perp_z = ShelfMushroomTreeDecorator::perpendicular_to_fallen_log(&fallen_z);
        assert_eq!(perp_z, [BlockDirection::East, BlockDirection::West]);
    }

    #[test]
    fn test_shelf_mushroom_properties_roundtrip() {
        for facing in [
            HorizontalFacing::North,
            HorizontalFacing::South,
            HorizontalFacing::East,
            HorizontalFacing::West,
        ] {
            for age in 0..=1 {
                let props = ShelfMushroomLikeProperties { age, facing };
                let state_id = props.to_state_id(&Block::SHELF_MUSHROOM);
                let parsed = ShelfMushroomLikeProperties::from_state_id(state_id);
                assert_eq!(parsed.age, age);
                assert_eq!(parsed.facing, facing);
            }
        }
    }
}
