use crate::block::{
    BlockActionResult, BlockBehaviour, BlockIsReplacing, BlockMetadata, BonemealArgs, BrokenArgs,
    CanPlaceAtArgs, CanUpdateAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs, PlacedArgs,
    UseWithItemArgs,
};
use pumpkin_data::block_properties::GlowLichenLikeProperties;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use rustc_hash::FxHashSet;
use crate::entity::{EntityBase, player::Player};
use pumpkin_data::FacingExt;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

/// Thickness of resin clump bounding box on a surface (1 pixel: 1/16 = 0.0625).
/// Mirroring vanilla `MultifaceBlock.java:49`: `Block.boxZ(16.0, 0.0, 1.0)`.
pub const RESIN_CLUMP_THICKNESS: f64 = 1.0 / 16.0;

/// Cooking time in ticks for smelting resin clump into resin brick (200 ticks = 10s).
/// Mirroring vanilla `resin_brick.json`: `"cookingtime": 200`.
pub const RESIN_SMELTING_COOKING_TIME_TICKS: u32 = 200;

/// Experience awarded for smelting resin clump into resin brick (0.1 XP).
/// Mirroring vanilla `resin_brick.json`: `"experience": 0.1`.
pub const RESIN_SMELTING_XP: f32 = 0.1;

/// Number of resin clumps required to craft 1 resin block (9 clumps).
/// Mirroring vanilla `resin_block.json`.
pub const RESIN_BLOCK_CRAFT_COUNT: u32 = 9;

/// Number of resin bricks required to craft 1 resin bricks block (4 bricks).
/// Mirroring vanilla `resin_bricks.json`.
pub const RESIN_BRICKS_CRAFT_COUNT: u32 = 4;

pub struct MultifaceBlock;

impl MultifaceBlock {
    pub const THICKNESS: f64 = RESIN_CLUMP_THICKNESS;
    pub const COOKING_TIME: u32 = RESIN_SMELTING_COOKING_TIME_TICKS;
    pub const SMELT_XP: f32 = RESIN_SMELTING_XP;
    pub const BLOCK_CRAFT_COUNT: u32 = RESIN_BLOCK_CRAFT_COUNT;
    pub const BRICKS_CRAFT_COUNT: u32 = RESIN_BRICKS_CRAFT_COUNT;

    #[must_use]
    pub fn calculate_drop_count(props: &GlowLichenLikeProperties) -> u32 {
        calculate_resin_clump_drop_count(props)
    }

    #[must_use]
    pub fn has_faces(props: &GlowLichenLikeProperties) -> bool {
        has_active_faces(props)
    }

    #[must_use]
    pub fn can_attach(state: &BlockState, face_dir: BlockDirection) -> bool {
        can_attach_to_face(state, face_dir)
    }
}
impl BlockMetadata for MultifaceBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::SCULK_VEIN,
            BlockId::GLOW_LICHEN,
            BlockId::RESIN_CLUMP,
        ]
        .into()
    }
}

impl BlockBehaviour for MultifaceBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if let BlockIsReplacing::Itself(state_id) = args.replacing {
            let (Some(direction), _) = get_attach_direction(
                args.world,
                args.position,
                args.block,
                Some(args.player),
                args.direction,
                true,
            ) else {
                return Block::AIR.default_state.id;
            };
            let mut props = GlowLichenLikeProperties::from_state_id(state_id);
            set_face(&mut props, direction);
            props.waterlogged = args.replacing.water_source();
            return props.to_state_id(args.block);
        }
        let (Some(direction), _) = get_attach_direction(
            args.world,
            args.position,
            args.block,
            Some(args.player),
            args.direction,
            false,
        ) else {
            return Block::AIR.default_state.id;
        };
        let mut props = GlowLichenLikeProperties::default(args.block);
        set_face(&mut props, direction);
        props.waterlogged = args.replacing.water_source();
        props.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        get_attach_direction(
            args.block_accessor,
            args.position,
            args.block,
            args.player,
            args.direction.unwrap_or(BlockDirection::Down),
            false,
        )
        .0
        .is_some()
    }

    fn can_update_at(&self, args: CanUpdateAtArgs<'_>) -> bool {
        get_attach_direction(
            args.world,
            args.position,
            args.block,
            Some(args.player),
            args.direction,
            true,
        )
        .0
        .is_some()
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let old_props = GlowLichenLikeProperties::from_state_id(args.state_id);
        if old_props.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed,
                TickPriority::Normal,
            );
        }

        let mut new_directions = active_directions(old_props);
        let support_pos = args.position.offset(args.direction.to_offset());
        if !is_solid_face_at(args.world, &support_pos, args.direction) {
            new_directions.remove(&args.direction);
        }

        if new_directions.is_empty() {
            return Block::AIR.default_state.id;
        }
        let mut new_props = GlowLichenLikeProperties::default(args.block);
        for dir in new_directions {
            set_face(&mut new_props, dir);
        }
        new_props.waterlogged = old_props.waterlogged;
        new_props.to_state_id(args.block)
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        if args.item_stack.item.id != args.block.id.as_u16() {
            return BlockActionResult::Pass;
        }
        let state = args.world.get_block_state(args.position);
        let mut props = GlowLichenLikeProperties::from_state_id(state.id);

        let (Some(accurate_dir), _) = get_attach_direction(
            args.world.as_ref(),
            args.position,
            args.block,
            Some(args.player),
            *args.hit.face,
            true,
        ) else {
            return BlockActionResult::Fail;
        };
        set_face(&mut props, accurate_dir);

        args.world.set_block_state(
            args.position,
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
        BlockActionResult::Consume
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        if args.block != &Block::GLOW_LICHEN {
            return false;
        }
        let props = GlowLichenLikeProperties::from_state_id(args.state_id);
        let active = active_directions(props);
        active.len() < 6
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        if args.block != &Block::GLOW_LICHEN {
            return;
        }
        let mut props = GlowLichenLikeProperties::from_state_id(args.state_id);
        for dir in BlockDirection::all() {
            let support_pos = args.position.offset(dir.to_offset());
            if is_solid_face_at(args.world.as_ref(), &support_pos, dir) {
                set_face(&mut props, dir);
            }
        }
        args.world.set_block_state(
            args.position,
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
    }
}

fn get_attach_direction(
    block_accessor: &dyn BlockAccessor,
    block_pos: &BlockPos,
    target_block: &Block,
    player_wrapper: Option<&Player>,
    click_direction: BlockDirection,
    replacing: bool,
) -> (Option<BlockDirection>, bool) {
    let clicked_block = block_accessor.get_block(&block_pos.offset(click_direction.to_offset()));

    if !replacing && clicked_block == target_block {
        return (None, false);
    }

    let clicked_pos = block_pos.offset(click_direction.to_offset());
    if is_solid_face_at(block_accessor, &clicked_pos, click_direction) {
        return (Some(click_direction), false);
    }

    let (replacing_block, replacing_block_state) = block_accessor.get_block_and_state(block_pos);
    let already_active = if replacing_block == target_block {
        active_directions(GlowLichenLikeProperties::from_state_id(
            replacing_block_state.id,
        ))
    } else {
        FxHashSet::default()
    };

    if let Some(player) = player_wrapper {
        let fs = player.get_entity().get_entity_facing_order();
        let directions = [
            fs[0].to_block_direction(),
            fs[1].to_block_direction(),
            fs[2].to_block_direction(),
            fs[3].to_block_direction(),
            fs[4].to_block_direction(),
            fs[5].to_block_direction(),
        ];
        for dir in directions {
            if !already_active.contains(&dir) {
                let support_pos = block_pos.offset(dir.to_offset());
                if is_solid_face_at(block_accessor, &support_pos, dir) {
                    return (Some(dir), false);
                }
            }
        }
    }
    (None, false)
}

#[must_use]
pub fn can_attach_to_face(support_state: &BlockState, face_dir: BlockDirection) -> bool {
    support_state.is_side_solid(face_dir.opposite()) || support_state.is_full_cube()
}

fn is_solid_face_at(block_accessor: &dyn BlockAccessor, support_pos: &BlockPos, face_dir: BlockDirection) -> bool {
    let (_, state) = block_accessor.get_block_and_state(support_pos);
    can_attach_to_face(state, face_dir)
}

/// Computes the exact number of resin clumps dropped when broken, mirroring vanilla
/// `resin_clump.json`: 1 base + 1 per each active face (down, up, north, south, west, east) - 1
/// = count of active faces.
#[must_use]
pub fn calculate_resin_clump_drop_count(props: &GlowLichenLikeProperties) -> u32 {
    let mut count = 0;
    if props.down {
        count += 1;
    }
    if props.up {
        count += 1;
    }
    if props.north {
        count += 1;
    }
    if props.south {
        count += 1;
    }
    if props.west {
        count += 1;
    }
    if props.east {
        count += 1;
    }
    count
}

/// Checks whether at least one face is active on the multiface block.
#[must_use]
pub fn has_active_faces(props: &GlowLichenLikeProperties) -> bool {
    props.down || props.up || props.north || props.south || props.west || props.east
}

fn active_directions(props: GlowLichenLikeProperties) -> FxHashSet<BlockDirection> {
    let mut set = FxHashSet::default();
    if props.down {
        set.insert(BlockDirection::Down);
    }
    if props.up {
        set.insert(BlockDirection::Up);
    }
    if props.north {
        set.insert(BlockDirection::North);
    }
    if props.south {
        set.insert(BlockDirection::South);
    }
    if props.east {
        set.insert(BlockDirection::East);
    }
    if props.west {
        set.insert(BlockDirection::West);
    }
    set
}

const fn set_face(props: &mut GlowLichenLikeProperties, direction: BlockDirection) {
    match direction {
        BlockDirection::Down => props.down = true,
        BlockDirection::Up => props.up = true,
        BlockDirection::North => props.north = true,
        BlockDirection::South => props.south = true,
        BlockDirection::West => props.west = true,
        BlockDirection::East => props.east = true,
    }
}


#[cfg(test)]
mod tests {
    use super::{
        GlowLichenLikeProperties, MultifaceBlock, RESIN_BLOCK_CRAFT_COUNT,
        RESIN_BRICKS_CRAFT_COUNT, RESIN_CLUMP_THICKNESS, RESIN_SMELTING_COOKING_TIME_TICKS,
        RESIN_SMELTING_XP, calculate_resin_clump_drop_count, has_active_faces,
    };

    #[test]
    fn test_resin_clump_and_multiface_parity() {
        // Dimensions & Recipe Constants
        assert_eq!(RESIN_CLUMP_THICKNESS, 1.0 / 16.0);
        assert_eq!(MultifaceBlock::THICKNESS, 1.0 / 16.0);
        assert_eq!(RESIN_SMELTING_COOKING_TIME_TICKS, 200);
        assert_eq!(MultifaceBlock::COOKING_TIME, 200);
        assert_eq!(RESIN_SMELTING_XP, 0.1);
        assert_eq!(MultifaceBlock::SMELT_XP, 0.1);
        assert_eq!(RESIN_BLOCK_CRAFT_COUNT, 9);
        assert_eq!(MultifaceBlock::BLOCK_CRAFT_COUNT, 9);
        assert_eq!(RESIN_BRICKS_CRAFT_COUNT, 4);
        assert_eq!(MultifaceBlock::BRICKS_CRAFT_COUNT, 4);

        // Drop count parity mirroring vanilla resin_clump.json loot table:
        // 0 faces -> 0 drops
        let mut props = GlowLichenLikeProperties {
            down: false,
            up: false,
            north: false,
            south: false,
            west: false,
            east: false,
            waterlogged: false,
        };
        assert_eq!(calculate_resin_clump_drop_count(&props), 0);
        assert!(!has_active_faces(&props));

        // 1 face (down) -> 1 drop
        props.down = true;
        assert_eq!(calculate_resin_clump_drop_count(&props), 1);
        assert!(has_active_faces(&props));

        // 2 faces (down + north) -> 2 drops
        props.north = true;
        assert_eq!(calculate_resin_clump_drop_count(&props), 2);

        // 3 faces (down + north + up) -> 3 drops
        props.up = true;
        assert_eq!(calculate_resin_clump_drop_count(&props), 3);

        // All 6 faces -> 6 drops
        props.south = true;
        props.west = true;
        props.east = true;
        assert_eq!(calculate_resin_clump_drop_count(&props), 6);
        assert_eq!(MultifaceBlock::calculate_drop_count(&props), 6);
        assert!(MultifaceBlock::has_faces(&props));
    }
}
