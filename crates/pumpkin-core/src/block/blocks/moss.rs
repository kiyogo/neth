use std::sync::Arc;

use pumpkin_data::block_properties::{
    DoubleBlockHalf, PaleMossCarpetLikeProperties, TallSeagrassLikeProperties,
};
use pumpkin_data::{Block, BlockDirection, BlockId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::block::blocks::carpet::{
    create_topper_with_side_chance, update_pale_moss_carpet_state,
    PALE_MOSS_CARPET_TOPPER_SIDE_CHANCE,
};
use crate::block::{BlockBehaviour, BlockMetadata, BonemealArgs, BrokenArgs, PlacedArgs};
use crate::world::World;

/// Depth of the moss patch replacement layer, mirroring vanilla `ConstantInt.of(1)`
/// in `CaveFeatures.java:418` and `VegetationFeatures.java:328`.
pub const MOSS_PATCH_BONEMEAL_DEPTH: i32 = 1;

/// Vertical range to search upwards/downwards for ground surface, mirroring
/// `CaveFeatures.java:420` and `VegetationFeatures.java:330` (5 blocks).
pub const MOSS_PATCH_BONEMEAL_VERTICAL_RANGE: i32 = 5;

/// Chance (0.60 = 60%) to place vegetation on each valid surface block, mirroring
/// `CaveFeatures.java:421` and `VegetationFeatures.java:331`.
pub const MOSS_PATCH_BONEMEAL_VEGETATION_CHANCE: f32 = 0.60;

/// Chance (0.75 = 75%) to include edge columns that are not corners, mirroring
/// `CaveFeatures.java:423` and `VegetationFeatures.java:333`.
pub const MOSS_PATCH_BONEMEAL_EXTRA_EDGE_COLUMN_CHANCE: f32 = 0.75;

/// Min and max base radius sampled from `UniformInt.of(1, 2)`, plus 1 giving 2..=3 blocks,
/// mirroring vanilla `CaveFeatures.java:422`, `VegetationFeatures.java:332`, and
/// `VegetationPatchFeature.java:92-93`.
pub const MOSS_PATCH_BONEMEAL_XZ_RADIUS_MIN: i32 = 1;
pub const MOSS_PATCH_BONEMEAL_XZ_RADIUS_MAX: i32 = 2;

// --- Weights for MOSS_VEGETATION (CaveFeatures.java:387-393) ---
pub const MOSS_VEG_WEIGHT_FLOWERING_AZALEA: u32 = 4;
pub const MOSS_VEG_WEIGHT_AZALEA: u32 = 7;
pub const MOSS_VEG_WEIGHT_MOSS_CARPET: u32 = 25;
pub const MOSS_VEG_WEIGHT_SHORT_GRASS: u32 = 50;
pub const MOSS_VEG_WEIGHT_TALL_GRASS: u32 = 10;
pub const MOSS_VEG_TOTAL_WEIGHT: u32 = 96;

// --- Weights for PALE_MOSS_VEGETATION (VegetationFeatures.java:299-303) ---
pub const PALE_MOSS_VEG_WEIGHT_PALE_MOSS_CARPET: u32 = 25;
pub const PALE_MOSS_VEG_WEIGHT_SHORT_GRASS: u32 = 25;
pub const PALE_MOSS_VEG_WEIGHT_TALL_GRASS: u32 = 10;
pub const PALE_MOSS_VEG_TOTAL_WEIGHT: u32 = 60;

pub struct MossBlock;

impl MossBlock {
    pub const BONEMEAL_DEPTH: i32 = MOSS_PATCH_BONEMEAL_DEPTH;
    pub const BONEMEAL_VERTICAL_RANGE: i32 = MOSS_PATCH_BONEMEAL_VERTICAL_RANGE;
    pub const BONEMEAL_VEGETATION_CHANCE: f32 = MOSS_PATCH_BONEMEAL_VEGETATION_CHANCE;
    pub const BONEMEAL_EXTRA_EDGE_COLUMN_CHANCE: f32 = MOSS_PATCH_BONEMEAL_EXTRA_EDGE_COLUMN_CHANCE;
}

impl BlockMetadata for MossBlock {
    fn ids() -> Box<[BlockId]> {
        Box::new([BlockId::MOSS_BLOCK, BlockId::PALE_MOSS_BLOCK])
    }
}

/// Checks whether a block is in the `#minecraft:moss_replaceable` tag,
/// mirroring `moss_replaceable.json` (`BlockTags.java:MOSS_REPLACEABLE`).
#[must_use]
pub const fn is_moss_replaceable(id: BlockId) -> bool {
    matches!(
        id,
        // #minecraft:base_stone_overworld
        BlockId::STONE
            | BlockId::GRANITE
            | BlockId::DIORITE
            | BlockId::ANDESITE
            | BlockId::TUFF
            | BlockId::DEEPSLATE
            // #minecraft:dirt
            | BlockId::DIRT
            | BlockId::COARSE_DIRT
            | BlockId::ROOTED_DIRT
            // #minecraft:mud
            | BlockId::MUD
            | BlockId::MUDDY_MANGROVE_ROOTS
            // #minecraft:grass_blocks
            | BlockId::GRASS_BLOCK
            | BlockId::PODZOL
            | BlockId::MYCELIUM
            // #minecraft:moss_blocks
            | BlockId::MOSS_BLOCK
            | BlockId::PALE_MOSS_BLOCK
            // #minecraft:cave_vines
            | BlockId::CAVE_VINES
            | BlockId::CAVE_VINES_PLANT
    )
}

/// Determines if an edge column in the patch grid should be included, mirroring
/// `VegetationPatchFeature.java:121`:
/// `!isCorner && (!isEdgeButNotCorner || this.extraEdgeColumnChance != 0.0F && !(random.nextFloat() > this.extraEdgeColumnChance))`
#[must_use]
pub fn should_include_patch_column(is_corner: bool, is_edge_but_not_corner: bool, roll: f32) -> bool {
    if is_corner {
        false
    } else if is_edge_but_not_corner {
        roll <= MOSS_PATCH_BONEMEAL_EXTRA_EDGE_COLUMN_CHANCE
    } else {
        true
    }
}

/// Chooses which vegetation block to place for normal Moss Block bonemeal,
/// mirroring vanilla `CaveFeatures.java:387-393`.
#[must_use]
pub fn pick_moss_vegetation(roll: u32) -> &'static Block {
    if roll < MOSS_VEG_WEIGHT_FLOWERING_AZALEA {
        &Block::FLOWERING_AZALEA
    } else if roll < MOSS_VEG_WEIGHT_FLOWERING_AZALEA + MOSS_VEG_WEIGHT_AZALEA {
        &Block::AZALEA
    } else if roll < MOSS_VEG_WEIGHT_FLOWERING_AZALEA + MOSS_VEG_WEIGHT_AZALEA + MOSS_VEG_WEIGHT_MOSS_CARPET {
        &Block::MOSS_CARPET
    } else if roll < MOSS_VEG_WEIGHT_FLOWERING_AZALEA + MOSS_VEG_WEIGHT_AZALEA + MOSS_VEG_WEIGHT_MOSS_CARPET + MOSS_VEG_WEIGHT_SHORT_GRASS {
        &Block::SHORT_GRASS
    } else {
        &Block::TALL_GRASS
    }
}

/// Chooses which vegetation block to place for Pale Moss Block bonemeal,
/// mirroring vanilla `VegetationFeatures.java:299-303`.
#[must_use]
pub fn pick_pale_moss_vegetation(roll: u32) -> &'static Block {
    if roll < PALE_MOSS_VEG_WEIGHT_PALE_MOSS_CARPET {
        &Block::PALE_MOSS_CARPET
    } else if roll < PALE_MOSS_VEG_WEIGHT_PALE_MOSS_CARPET + PALE_MOSS_VEG_WEIGHT_SHORT_GRASS {
        &Block::SHORT_GRASS
    } else {
        &Block::TALL_GRASS
    }
}

impl BlockBehaviour for MossBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        // Vanilla BonemealableFeaturePlacerBlock.java:24:
        // level.getBlockState(pos.above()).isAir()
        let above = args.position.up();
        args.world.is_in_height_limit(above.0.y)
            && args.world.is_loaded(&above)
            && args.world.get_block_state(&above).is_air()
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        // Vanilla BonemealableFeaturePlacerBlock.java:29: returns true
        true
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        perform_moss_patch_bonemeal(args.world, args.block, *args.position);
    }
}

pub fn perform_moss_patch_bonemeal(world: &Arc<World>, block: &Block, origin_pos: BlockPos) {
    let is_pale = block == &Block::PALE_MOSS_BLOCK;
    let ground_block = if is_pale {
        &Block::PALE_MOSS_BLOCK
    } else {
        &Block::MOSS_BLOCK
    };

    let mut rng = rand::rng();
    let x_radius = rng.random_range(MOSS_PATCH_BONEMEAL_XZ_RADIUS_MIN..=MOSS_PATCH_BONEMEAL_XZ_RADIUS_MAX) + 1;
    let z_radius = rng.random_range(MOSS_PATCH_BONEMEAL_XZ_RADIUS_MIN..=MOSS_PATCH_BONEMEAL_XZ_RADIUS_MAX) + 1;

    let origin = origin_pos.up();
    let mut surface_positions = Vec::new();

    for dx in -x_radius..=x_radius {
        let is_x_edge = dx == -x_radius || dx == x_radius;
        for dz in -z_radius..=z_radius {
            let is_z_edge = dz == -z_radius || dz == z_radius;
            let is_edge = is_x_edge || is_z_edge;
            let is_corner = is_x_edge && is_z_edge;
            let is_edge_but_not_corner = is_edge && !is_corner;

            let roll: f32 = rng.random();
            if !should_include_patch_column(is_corner, is_edge_but_not_corner, roll) {
                continue;
            }

            let mut col_pos = BlockPos::new(origin.0.x + dx, origin.0.y, origin.0.z + dz);

            // Move downwards while air, up to verticalRange (5) times
            let mut offset = 0;
            while world.is_in_height_limit(col_pos.0.y)
                && world.get_block_state(&col_pos).is_air()
                && offset < MOSS_PATCH_BONEMEAL_VERTICAL_RANGE
            {
                col_pos = col_pos.down();
                offset += 1;
            }

            // Move upwards while not air, up to verticalRange (5) times
            let mut up_count = 0;
            while world.is_in_height_limit(col_pos.0.y)
                && !world.get_block_state(&col_pos).is_air()
                && up_count < MOSS_PATCH_BONEMEAL_VERTICAL_RANGE
            {
                col_pos = col_pos.up();
                up_count += 1;
            }

            let below_pos = col_pos.down();
            if !world.is_in_height_limit(col_pos.0.y) || !world.is_in_height_limit(below_pos.0.y) {
                continue;
            }

            if world.get_block_state(&col_pos).is_air() {
                let (below_block, below_state) = world.get_block_and_state(&below_pos);
                if below_state.is_side_solid(BlockDirection::Up) && is_moss_replaceable(below_block.id) {
                    if below_block != ground_block {
                        world.set_block_state(
                            &below_pos,
                            ground_block.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        );
                    }
                    surface_positions.push(below_pos);
                }
            }
        }
    }

    // Distribute vegetation on surface
    for surface_pos in surface_positions {
        if rng.random::<f32>() < MOSS_PATCH_BONEMEAL_VEGETATION_CHANCE {
            let veg_pos = surface_pos.up();
            if !world.is_in_height_limit(veg_pos.0.y) || !world.get_block_state(&veg_pos).is_air() {
                continue;
            }

            if is_pale {
                let roll = rng.random_range(0..PALE_MOSS_VEG_TOTAL_WEIGHT);
                let chosen = pick_pale_moss_vegetation(roll);
                if chosen == &Block::PALE_MOSS_CARPET {
                    let default_carpet = PaleMossCarpetLikeProperties::default(&Block::PALE_MOSS_CARPET);
                    let updated = update_pale_moss_carpet_state(world.as_ref(), &veg_pos, default_carpet, true);
                    world.set_block_state(
                        &veg_pos,
                        updated.to_state_id(&Block::PALE_MOSS_CARPET),
                        BlockFlags::NOTIFY_ALL,
                    );
                    if let Some(topper) = create_topper_with_side_chance(world.as_ref(), &veg_pos, || rng.random_bool(PALE_MOSS_CARPET_TOPPER_SIDE_CHANCE)) {
                        let topper_id = topper.to_state_id(&Block::PALE_MOSS_CARPET);
                        world.set_block_state(&veg_pos.up(), topper_id, BlockFlags::NOTIFY_ALL);
                    }
                } else if chosen == &Block::SHORT_GRASS {
                    world.set_block_state(
                        &veg_pos,
                        Block::SHORT_GRASS.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    );
                } else {
                    // Tall grass
                    let above_veg = veg_pos.up();
                    if world.is_in_height_limit(above_veg.0.y) && world.get_block_state(&above_veg).is_air() {
                        let lower_state = Block::TALL_GRASS.default_state.id;
                        world.set_block_state(&veg_pos, lower_state, BlockFlags::NOTIFY_ALL);
                        let mut props = TallSeagrassLikeProperties::from_state_id(lower_state);
                        props.half = DoubleBlockHalf::Upper;
                        world.set_block_state(
                            &above_veg,
                            props.to_state_id(&Block::TALL_GRASS),
                            BlockFlags::NOTIFY_ALL,
                        );
                    } else {
                        world.set_block_state(
                            &veg_pos,
                            Block::SHORT_GRASS.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        );
                    }
                }
            } else {
                let roll = rng.random_range(0..MOSS_VEG_TOTAL_WEIGHT);
                let chosen = pick_moss_vegetation(roll);
                if chosen == &Block::TALL_GRASS {
                    let above_veg = veg_pos.up();
                    if world.is_in_height_limit(above_veg.0.y) && world.get_block_state(&above_veg).is_air() {
                        let lower_state = Block::TALL_GRASS.default_state.id;
                        world.set_block_state(&veg_pos, lower_state, BlockFlags::NOTIFY_ALL);
                        let mut props = TallSeagrassLikeProperties::from_state_id(lower_state);
                        props.half = DoubleBlockHalf::Upper;
                        world.set_block_state(
                            &above_veg,
                            props.to_state_id(&Block::TALL_GRASS),
                            BlockFlags::NOTIFY_ALL,
                        );
                    } else {
                        world.set_block_state(
                            &veg_pos,
                            Block::SHORT_GRASS.default_state.id,
                            BlockFlags::NOTIFY_ALL,
                        );
                    }
                } else {
                    world.set_block_state(
                        &veg_pos,
                        chosen.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    );
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::BlockId;

    #[test]
    fn test_moss_replaceable_tag_blocks_parity() {
        // Base stone overworld
        assert!(is_moss_replaceable(BlockId::STONE));
        assert!(is_moss_replaceable(BlockId::GRANITE));
        assert!(is_moss_replaceable(BlockId::DIORITE));
        assert!(is_moss_replaceable(BlockId::ANDESITE));
        assert!(is_moss_replaceable(BlockId::TUFF));
        assert!(is_moss_replaceable(BlockId::DEEPSLATE));

        // Dirt
        assert!(is_moss_replaceable(BlockId::DIRT));
        assert!(is_moss_replaceable(BlockId::COARSE_DIRT));
        assert!(is_moss_replaceable(BlockId::ROOTED_DIRT));

        // Mud
        assert!(is_moss_replaceable(BlockId::MUD));
        assert!(is_moss_replaceable(BlockId::MUDDY_MANGROVE_ROOTS));

        // Grass blocks
        assert!(is_moss_replaceable(BlockId::GRASS_BLOCK));
        assert!(is_moss_replaceable(BlockId::PODZOL));
        assert!(is_moss_replaceable(BlockId::MYCELIUM));

        // Moss blocks
        assert!(is_moss_replaceable(BlockId::MOSS_BLOCK));
        assert!(is_moss_replaceable(BlockId::PALE_MOSS_BLOCK));

        // Cave vines
        assert!(is_moss_replaceable(BlockId::CAVE_VINES));
        assert!(is_moss_replaceable(BlockId::CAVE_VINES_PLANT));

        // Non-replaceable blocks
        assert!(!is_moss_replaceable(BlockId::BEDROCK));
        assert!(!is_moss_replaceable(BlockId::OBSIDIAN));
        assert!(!is_moss_replaceable(BlockId::OAK_LOG));
        assert!(!is_moss_replaceable(BlockId::GLASS));
        assert!(!is_moss_replaceable(BlockId::DIAMOND_BLOCK));
    }

    #[test]
    fn test_moss_bonemeal_constants_and_patch_rules_parity() {
        assert_eq!(MOSS_PATCH_BONEMEAL_DEPTH, 1);
        assert_eq!(MOSS_PATCH_BONEMEAL_VERTICAL_RANGE, 5);
        assert!((MOSS_PATCH_BONEMEAL_VEGETATION_CHANCE - 0.60).abs() < 1e-6);
        assert!((MOSS_PATCH_BONEMEAL_EXTRA_EDGE_COLUMN_CHANCE - 0.75).abs() < 1e-6);
        assert_eq!(MOSS_PATCH_BONEMEAL_XZ_RADIUS_MIN, 1);
        assert_eq!(MOSS_PATCH_BONEMEAL_XZ_RADIUS_MAX, 2);

        // Edge column inclusion
        assert!(!should_include_patch_column(true, false, 0.0));
        assert!(!should_include_patch_column(true, true, 0.0));
        assert!(should_include_patch_column(false, false, 1.0));
        assert!(should_include_patch_column(false, true, 0.74));
        assert!(should_include_patch_column(false, true, 0.75));
        assert!(!should_include_patch_column(false, true, 0.76));
    }

    #[test]
    fn test_moss_vegetation_weights_parity() {
        assert_eq!(MOSS_VEG_WEIGHT_FLOWERING_AZALEA, 4);
        assert_eq!(MOSS_VEG_WEIGHT_AZALEA, 7);
        assert_eq!(MOSS_VEG_WEIGHT_MOSS_CARPET, 25);
        assert_eq!(MOSS_VEG_WEIGHT_SHORT_GRASS, 50);
        assert_eq!(MOSS_VEG_WEIGHT_TALL_GRASS, 10);
        assert_eq!(MOSS_VEG_TOTAL_WEIGHT, 96);

        assert_eq!(pick_moss_vegetation(0), &Block::FLOWERING_AZALEA);
        assert_eq!(pick_moss_vegetation(3), &Block::FLOWERING_AZALEA);
        assert_eq!(pick_moss_vegetation(4), &Block::AZALEA);
        assert_eq!(pick_moss_vegetation(10), &Block::AZALEA);
        assert_eq!(pick_moss_vegetation(11), &Block::MOSS_CARPET);
        assert_eq!(pick_moss_vegetation(35), &Block::MOSS_CARPET);
        assert_eq!(pick_moss_vegetation(36), &Block::SHORT_GRASS);
        assert_eq!(pick_moss_vegetation(85), &Block::SHORT_GRASS);
        assert_eq!(pick_moss_vegetation(86), &Block::TALL_GRASS);
        assert_eq!(pick_moss_vegetation(95), &Block::TALL_GRASS);

        assert_eq!(PALE_MOSS_VEG_WEIGHT_PALE_MOSS_CARPET, 25);
        assert_eq!(PALE_MOSS_VEG_WEIGHT_SHORT_GRASS, 25);
        assert_eq!(PALE_MOSS_VEG_WEIGHT_TALL_GRASS, 10);
        assert_eq!(PALE_MOSS_VEG_TOTAL_WEIGHT, 60);

        assert_eq!(pick_pale_moss_vegetation(0), &Block::PALE_MOSS_CARPET);
        assert_eq!(pick_pale_moss_vegetation(24), &Block::PALE_MOSS_CARPET);
        assert_eq!(pick_pale_moss_vegetation(25), &Block::SHORT_GRASS);
        assert_eq!(pick_pale_moss_vegetation(49), &Block::SHORT_GRASS);
        assert_eq!(pick_pale_moss_vegetation(50), &Block::TALL_GRASS);
        assert_eq!(pick_pale_moss_vegetation(59), &Block::TALL_GRASS);
    }
}
