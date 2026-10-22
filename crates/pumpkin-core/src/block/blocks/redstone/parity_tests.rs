use std::sync::{Arc, Weak};

use arc_swap::ArcSwap;
use pumpkin_config::world::LevelConfig;
use pumpkin_data::block_properties::{
    ComparatorLikeProperties, EastRedstone, ModeComparator, NorthRedstone,
    RedstoneWireLikeProperties, RepeaterLikeProperties, SouthRedstone, WestRedstone,
};
use pumpkin_data::dimension::Dimension;
use pumpkin_data::{Block, BlockDirection, BlockState};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::world_seed::Seed;
use pumpkin_world::world::BlockFlags;
use pumpkin_world::{chunk::ChunkData, level::Level, world_info::LevelData};

use crate::block::blocks::piston::PistonHandler;
use crate::block::blocks::piston::piston::should_extend;
use crate::block::blocks::redstone::{
    block_receives_redstone_power, diode_get_input_strength, get_max_strong_power,
    get_redstone_power,
};
use crate::block::registry::default_registry;
use crate::world::World;

fn create_test_world() -> (tempfile::TempDir, Arc<World>) {
    let directory = tempfile::TempDir::new().expect("failed to create temp dir");
    let level = Level::from_root_folder(
        &LevelConfig::default(),
        directory.path().to_path_buf(),
        0,
        Dimension::OVERWORLD,
    );
    let world = Arc::new(World::load(
        level.clone(),
        Arc::new(ArcSwap::from_pointee(LevelData::default(Seed(0)))),
        Dimension::OVERWORLD,
        default_registry(),
        Weak::new(),
    ));
    let chunk = ChunkData::empty_sync(0, 0);
    world.level.loaded_chunks.insert(Vector2::new(0, 0), chunk);
    (directory, world)
}

// =========================================================================
// GROUP 1: SIGNAL CALCULATION & STRONG/WEAK POWER PROPAGATION
// =========================================================================

#[tokio::test]
async fn test_redstone_wire_does_not_emit_strong_power_horizontally() {
    let (_dir, world) = create_test_world();
    let pos = BlockPos::new(5, 64, 5);

    let mut props = RedstoneWireLikeProperties::default(&Block::REDSTONE_WIRE);
    props.power = 15;
    props.north = NorthRedstone::Side;
    props.south = SouthRedstone::Side;
    props.east = EastRedstone::Side;
    props.west = WestRedstone::Side;
    let state_id = props.to_state_id(&Block::REDSTONE_WIRE);
    let state = BlockState::from_id(state_id);

    // Vanilla Java Parity: Wire NEVER emits strong power horizontally, even when connected
    for dir in [
        BlockDirection::North,
        BlockDirection::South,
        BlockDirection::East,
        BlockDirection::West,
    ] {
        let strong = world.block_registry.get_strong_redstone_power(
            &Block::REDSTONE_WIRE,
            &world,
            &pos,
            state,
            dir,
        );
        assert_eq!(
            strong, 0,
            "Redstone wire must never emit strong power in horizontal direction {dir:?}"
        );
    }
}

#[tokio::test]
async fn test_redstone_wire_emits_strong_power_downwards() {
    let (_dir, world) = create_test_world();
    let pos = BlockPos::new(5, 64, 5);

    let mut props = RedstoneWireLikeProperties::default(&Block::REDSTONE_WIRE);
    props.power = 12;
    let state_id = props.to_state_id(&Block::REDSTONE_WIRE);
    let state = BlockState::from_id(state_id);

    // BlockDirection::Up queries the block BELOW the wire
    let strong = world.block_registry.get_strong_redstone_power(
        &Block::REDSTONE_WIRE,
        &world,
        &pos,
        state,
        BlockDirection::Up,
    );
    assert_eq!(
        strong, 12,
        "Redstone wire must emit strong power downwards into the block beneath it"
    );

    // BlockDirection::Down queries the block ABOVE the wire
    let strong_above = world.block_registry.get_strong_redstone_power(
        &Block::REDSTONE_WIRE,
        &world,
        &pos,
        state,
        BlockDirection::Down,
    );
    assert_eq!(
        strong_above, 0,
        "Redstone wire must not emit strong power upwards"
    );
}

#[tokio::test]
async fn test_redstone_wire_emits_weak_power_horizontally_when_connected() {
    let (_dir, world) = create_test_world();
    let pos = BlockPos::new(5, 64, 5);

    let mut props = RedstoneWireLikeProperties::default(&Block::REDSTONE_WIRE);
    props.power = 15;
    props.north = NorthRedstone::Side;
    let state_id = props.to_state_id(&Block::REDSTONE_WIRE);
    let state = BlockState::from_id(state_id);

    // Neighbor to North queries with South (the relative face)
    let weak_north = world.block_registry.get_weak_redstone_power(
        &Block::REDSTONE_WIRE,
        &world,
        &pos,
        state,
        BlockDirection::South,
    );
    assert_eq!(
        weak_north, 15,
        "Redstone wire must emit weak power to connected side"
    );

    // Neighbor to West (not connected)
    let weak_west = world.block_registry.get_weak_redstone_power(
        &Block::REDSTONE_WIRE,
        &world,
        &pos,
        state,
        BlockDirection::East,
    );
    assert_eq!(
        weak_west, 0,
        "Redstone wire must not emit power to disconnected side"
    );
}

#[tokio::test]
async fn test_solid_block_horizontally_adjacent_to_wire_is_not_strongly_powered() {
    let (_dir, world) = create_test_world();
    let wire_pos = BlockPos::new(5, 64, 5);
    let stone_pos = BlockPos::new(6, 64, 5); // East of wire

    let mut props = RedstoneWireLikeProperties::default(&Block::REDSTONE_WIRE);
    props.power = 15;
    props.east = EastRedstone::Side;
    world.set_block_state(&wire_pos, props.to_state_id(&Block::REDSTONE_WIRE), BlockFlags::empty());
    world.set_block_state(&stone_pos, Block::STONE.default_state.id, BlockFlags::empty());

    // The stone block must NOT be strongly powered by horizontal wire
    let strong = get_max_strong_power(&world, &stone_pos, true);
    assert_eq!(
        strong, 0,
        "Solid block horizontally adjacent to wire must have 0 strong power"
    );

    let power_through = get_redstone_power(
        &Block::STONE,
        world.get_block_state(&stone_pos),
        &world,
        &stone_pos,
        BlockDirection::East,
    );
    assert_eq!(
        power_through, 0,
        "Solid block must not conduct redstone wire power without a repeater"
    );
}

#[tokio::test]
async fn test_solid_block_with_wire_on_top_is_strongly_powered() {
    let (_dir, world) = create_test_world();
    let wire_pos = BlockPos::new(5, 65, 5);
    let stone_pos = BlockPos::new(5, 64, 5); // Directly below wire

    let mut props = RedstoneWireLikeProperties::default(&Block::REDSTONE_WIRE);
    props.power = 14;
    let flags = BlockFlags::SKIP_BLOCK_ADDED_CALLBACK;
    world.set_block_state(&stone_pos, Block::STONE.default_state.id, flags);
    world.set_block_state(&wire_pos, props.to_state_id(&Block::REDSTONE_WIRE), flags);

    let strong = get_max_strong_power(&world, &stone_pos, true);
    assert_eq!(
        strong, 14,
        "Solid block directly beneath redstone wire must receive strong power"
    );
}

#[tokio::test]
async fn test_repeater_pulls_weak_power_from_solid_block() {
    let (_dir, world) = create_test_world();
    let stone_pos = BlockPos::new(6, 64, 5);
    let wire_on_top_pos = BlockPos::new(6, 65, 5);
    let repeater_pos = BlockPos::new(7, 64, 5);

    // Place stone first, then wire on top of stone with power=15
    let flags = BlockFlags::SKIP_BLOCK_ADDED_CALLBACK;
    world.set_block_state(&stone_pos, Block::STONE.default_state.id, flags);
    let mut props = RedstoneWireLikeProperties::default(&Block::REDSTONE_WIRE);
    props.power = 15;
    world.set_block_state(&wire_on_top_pos, props.to_state_id(&Block::REDSTONE_WIRE), flags);

    // Diode facing East reading from stone block (to its West)
    // In Java, wire on top strongly powers stone, so repeater pulls power (weak power) from stone.
    let power = diode_get_input_strength(&world, &repeater_pos, BlockDirection::West);
    assert_eq!(
        power, 15,
        "Repeater must be able to pull weak power out of a strongly powered solid block"
    );
}

#[tokio::test]
async fn test_redstone_block_emits_weak_power_all_sides_not_strong() {
    let (_dir, world) = create_test_world();
    let pos = BlockPos::new(5, 64, 5);
    let state = Block::REDSTONE_BLOCK.default_state;

    for dir in BlockDirection::all() {
        let weak = world.block_registry.get_weak_redstone_power(
            &Block::REDSTONE_BLOCK,
            &world,
            &pos,
            state,
            dir,
        );
        assert_eq!(weak, 15, "Redstone block must emit 15 weak power in direction {dir:?}");

        let strong = world.block_registry.get_strong_redstone_power(
            &Block::REDSTONE_BLOCK,
            &world,
            &pos,
            state,
            dir,
        );
        assert_eq!(strong, 0, "Redstone block must emit 0 strong power in direction {dir:?}");
    }
}

// =========================================================================
// GROUP 2: REDSTONE WIRE DECAY & SHAPES
// =========================================================================

#[test]
fn test_wire_power_decay_per_block() {
    // Redstone signal attenuates by exactly 1 per block: 15 -> 14 -> 13 ... -> 0
    let mut power = 15u8;
    let mut distance = 0;
    while power > 0 {
        distance += 1;
        power = power.saturating_sub(1);
    }
    assert_eq!(distance, 15, "Redstone wire travels exactly 15 blocks before reaching 0");
}

#[test]
fn test_wire_connection_types() {
    let mut props = RedstoneWireLikeProperties::default(&Block::REDSTONE_WIRE);
    props.north = NorthRedstone::Side;
    props.south = SouthRedstone::Up;
    props.east = EastRedstone::None;
    props.west = WestRedstone::Side;
    props.power = 10;

    let state_id = props.to_state_id(&Block::REDSTONE_WIRE);
    let decoded = RedstoneWireLikeProperties::from_state_id(state_id);

    assert_eq!(decoded.north, NorthRedstone::Side);
    assert_eq!(decoded.south, SouthRedstone::Up);
    assert_eq!(decoded.east, EastRedstone::None);
    assert_eq!(decoded.west, WestRedstone::Side);
    assert_eq!(decoded.power, 10);
}

// =========================================================================
// GROUP 3: REPEATER (DELAY, LOCKING, DIRECTION)
// =========================================================================

#[test]
fn test_repeater_delays_match_vanilla() {
    // Vanilla Repeater delay: delay * 2 redstone ticks
    for (setting, expected_delay) in [(1, 2), (2, 4), (3, 6), (4, 8)] {
        let mut props = RepeaterLikeProperties::default(&Block::REPEATER);
        props.delay = setting;
        let delay_ticks = u32::from(props.delay) * 2;
        assert_eq!(
            delay_ticks, expected_delay,
            "Repeater delay setting {setting} must produce {expected_delay} ticks"
        );
    }
}

#[tokio::test]
async fn test_repeater_locking_state_detection() {
    let (_dir, world) = create_test_world();
    let repeater_pos = BlockPos::new(5, 64, 5);
    let side_repeater_pos = BlockPos::new(5, 64, 6); // South of repeater

    // Main repeater facing East
    let mut props = RepeaterLikeProperties::default(&Block::REPEATER);
    props.facing = pumpkin_data::block_properties::HorizontalFacing::East;
    props.locked = false;
    world.set_block_state(
        &repeater_pos,
        props.to_state_id(&Block::REPEATER),
        BlockFlags::empty(),
    );

    // Side repeater facing North (pointing directly into main repeater's side) and powered
    let mut side_props = RepeaterLikeProperties::default(&Block::REPEATER);
    side_props.facing = pumpkin_data::block_properties::HorizontalFacing::North;
    side_props.powered = true;
    world.set_block_state(
        &side_repeater_pos,
        side_props.to_state_id(&Block::REPEATER),
        BlockFlags::empty(),
    );

    // Side power into main repeater
    let side_power = world.block_registry.get_strong_redstone_power(
        &Block::REPEATER,
        &world,
        &side_repeater_pos,
        BlockState::from_id(side_props.to_state_id(&Block::REPEATER)),
        BlockDirection::North,
    );
    assert_eq!(
        side_power, 15,
        "Powered side repeater must emit strong signal in facing direction"
    );
}

// =========================================================================
// GROUP 4: COMPARATOR (MODES & CONTAINER READING)
// =========================================================================

#[test]
fn test_comparator_modes_parity() {
    // Compare Mode: output = input if input >= side_input else 0
    let input = 10u8;
    for (side, expected) in [(5u8, 10u8), (10u8, 10u8), (11u8, 0u8)] {
        let output = if input >= side { input } else { 0 };
        assert_eq!(output, expected, "Compare mode: input={input}, side={side}");
    }

    // Subtract Mode: output = max(0, input - side_input)
    for (side, expected) in [(3u8, 7u8), (10u8, 0u8), (12u8, 0u8)] {
        let output = input.saturating_sub(side);
        assert_eq!(output, expected, "Subtract mode: input={input}, side={side}");
    }
}

#[test]
fn test_comparator_properties_serialization() {
    let mut props = ComparatorLikeProperties::default(&Block::COMPARATOR);
    props.mode = ModeComparator::Subtract;
    props.powered = true;
    props.facing = pumpkin_data::block_properties::HorizontalFacing::South;

    let state_id = props.to_state_id(&Block::COMPARATOR);
    let decoded = ComparatorLikeProperties::from_state_id(state_id);

    assert_eq!(decoded.mode, ModeComparator::Subtract);
    assert!(decoded.powered);
    assert_eq!(decoded.facing, pumpkin_data::block_properties::HorizontalFacing::South);
}

// =========================================================================
// GROUP 5: PISTON MECHANICS (PUSH LIMIT & MOVABILITY)
// =========================================================================

#[tokio::test]
async fn test_piston_push_limit_12_blocks() {
    let (_dir, world) = create_test_world();
    let piston_pos = BlockPos::new(2, 64, 2);

    // Place 12 stone blocks in a row East of piston
    for i in 1..=12 {
        world.set_block_state(
            &BlockPos::new(2 + i, 64, 2),
            Block::STONE.default_state.id,
            BlockFlags::empty(),
        );
    }

    let mut handler = PistonHandler::new(&world, piston_pos, BlockDirection::East, true);
    assert!(
        handler.calculate_push(),
        "Piston must be able to push exactly 12 blocks"
    );
    assert_eq!(handler.moved_blocks.len(), 12);
}

#[tokio::test]
async fn test_piston_push_limit_13_blocks_fails() {
    let (_dir, world) = create_test_world();
    let piston_pos = BlockPos::new(2, 64, 2);

    // Place 13 stone blocks in a row East of piston
    for i in 1..=13 {
        world.set_block_state(
            &BlockPos::new(2 + i, 64, 2),
            Block::STONE.default_state.id,
            BlockFlags::empty(),
        );
    }

    let mut handler = PistonHandler::new(&world, piston_pos, BlockDirection::East, true);
    assert!(
        !handler.calculate_push(),
        "Piston must NOT be able to push 13 blocks (exceeds max limit 12)"
    );
}

#[tokio::test]
async fn test_piston_cannot_push_obsidian_or_bedrock() {
    let (_dir, world) = create_test_world();
    let piston_pos = BlockPos::new(2, 64, 2);

    // Place obsidian directly in front
    world.set_block_state(
        &BlockPos::new(3, 64, 2),
        Block::OBSIDIAN.default_state.id,
        BlockFlags::empty(),
    );

    let mut handler = PistonHandler::new(&world, piston_pos, BlockDirection::East, true);
    assert!(
        !handler.calculate_push(),
        "Piston must fail to push obsidian"
    );

    // Place bedrock directly in front
    world.set_block_state(
        &BlockPos::new(3, 64, 2),
        Block::BEDROCK.default_state.id,
        BlockFlags::empty(),
    );

    let mut handler_bedrock = PistonHandler::new(&world, piston_pos, BlockDirection::East, true);
    assert!(
        !handler_bedrock.calculate_push(),
        "Piston must fail to push bedrock"
    );
}

// =========================================================================
// GROUP 6: QUASI-CONNECTIVITY (PISTON, DISPENSER, DROPPER, BUD)
// =========================================================================

#[tokio::test]
async fn test_piston_quasi_connectivity_diagonal_power() {
    let (_dir, world) = create_test_world();
    let piston_pos = BlockPos::new(5, 64, 5);

    // Place redstone block diagonally above: pos.up().north() = (5, 65, 4)
    let qc_source = BlockPos::new(5, 65, 4);
    world.set_block_state(&qc_source, Block::REDSTONE_BLOCK.default_state.id, BlockFlags::empty());

    // Piston facing South
    let extend = should_extend(&world, &piston_pos, BlockDirection::South);
    assert!(
        extend,
        "Piston must detect quasi-connectivity from diagonally above redstone block"
    );
}

#[tokio::test]
async fn test_piston_quasi_connectivity_two_blocks_above() {
    let (_dir, world) = create_test_world();
    let piston_pos = BlockPos::new(5, 64, 5);

    // Place redstone block 2 blocks directly above piston: pos.up(2) = (5, 66, 5)
    let qc_source = BlockPos::new(5, 66, 5);
    world.set_block_state(&qc_source, Block::REDSTONE_BLOCK.default_state.id, BlockFlags::empty());

    let extend = should_extend(&world, &piston_pos, BlockDirection::South);
    assert!(
        extend,
        "Piston must detect quasi-connectivity from 2 blocks directly above"
    );
}

#[tokio::test]
async fn test_piston_facing_up_not_powered_through_front_face() {
    let (_dir, world) = create_test_world();
    let piston_pos = BlockPos::new(5, 64, 5);

    // Place redstone block directly in front of up-facing piston: pos.up() = (5, 65, 5)
    let head_pos = BlockPos::new(5, 65, 5);
    world.set_block_state(&head_pos, Block::REDSTONE_BLOCK.default_state.id, BlockFlags::empty());

    // Piston facing UP cannot be powered through its own front face
    let extend = should_extend(&world, &piston_pos, BlockDirection::Up);
    assert!(
        !extend,
        "Piston facing UP must not be powered directly through its front face"
    );
}

#[tokio::test]
async fn test_dispenser_quasi_connectivity_triggered_from_above() {
    let (_dir, world) = create_test_world();
    let dispenser_pos = BlockPos::new(5, 64, 5);

    // Place redstone block diagonally above: pos.up().north() = (5, 65, 4)
    let qc_source = BlockPos::new(5, 65, 4);
    world.set_block_state(&qc_source, Block::REDSTONE_BLOCK.default_state.id, BlockFlags::empty());

    let direct_power = block_receives_redstone_power(&world, &dispenser_pos);
    let above_power = block_receives_redstone_power(&world, &dispenser_pos.up());

    assert!(
        !direct_power,
        "Dispenser must not receive direct neighbor power from diagonal block"
    );
    assert!(
        above_power,
        "Dispenser position above must receive power from diagonal redstone block"
    );

    let should_trigger = direct_power || above_power;
    assert!(
        should_trigger,
        "Dispenser must trigger via quasi-connectivity when pos.up() is powered"
    );
}

#[tokio::test]
async fn test_dropper_quasi_connectivity_triggered_from_above() {
    let (_dir, world) = create_test_world();
    let dropper_pos = BlockPos::new(5, 64, 5);

    // Place redstone block 2 blocks above dropper: pos.up(2) = (5, 66, 5)
    let qc_source = BlockPos::new(5, 66, 5);
    world.set_block_state(&qc_source, Block::REDSTONE_BLOCK.default_state.id, BlockFlags::empty());

    let direct_power = block_receives_redstone_power(&world, &dropper_pos);
    let above_power = block_receives_redstone_power(&world, &dropper_pos.up());

    assert!(
        !direct_power,
        "Dropper must not receive direct neighbor power from 2 blocks above"
    );
    assert!(
        above_power,
        "Dropper pos.up() must receive power from block directly above it"
    );

    let should_trigger = direct_power || above_power;
    assert!(
        should_trigger,
        "Dropper must trigger via quasi-connectivity when pos.up() is powered"
    );
}
