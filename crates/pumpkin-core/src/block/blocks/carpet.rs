use crate::block::{
    BlockBehaviour, BonemealArgs, BrokenArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
    OnScheduledTickArgs, PlacedArgs,
};
use pumpkin_data::block_properties::is_air;
use pumpkin_data::block_properties::{EastWall, NorthWall, PaleMossCarpetLikeProperties, SouthWall, WestWall};
use pumpkin_data::{Block, BlockDirection, BlockStateId};
use pumpkin_macros::{pumpkin_block, pumpkin_block_from_tag};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::{BlockAccessor, BlockFlags};
use rand::RngExt;

#[pumpkin_block_from_tag("minecraft:wool_carpets")]
pub struct CarpetBlock;

impl CarpetBlock {
    pub const HEIGHT: f64 = 1.0 / 16.0;
}

impl BlockBehaviour for CarpetBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !can_place_at(args.world, args.position) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
        }
        args.state_id
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !can_place_at(args.world.as_ref(), args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }
}

#[pumpkin_block("minecraft:moss_carpet")]
pub struct MossCarpetBlock;

impl MossCarpetBlock {
    pub const HEIGHT: f64 = 1.0 / 16.0;
}

impl BlockBehaviour for MossCarpetBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_at(args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if !can_place_at(args.world, args.position) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
        }
        args.state_id
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !can_place_at(args.world.as_ref(), args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }
}

/// Carpet base thickness: 1 pixel (1/16 = 0.0625), mirroring vanilla `Block.boxZ(16.0, 0.0, 1.0)`.
pub const PALE_MOSS_CARPET_BASE_HEIGHT: f64 = 1.0 / 16.0;

/// Low wall-side height: 10 pixels (10/16 = 0.625), mirroring vanilla `Block.boxZ(16.0, 0.0, 10.0, 0.0, 1.0)`.
pub const PALE_MOSS_CARPET_LOW_SIDE_HEIGHT: f64 = 10.0 / 16.0;

/// Tall wall-side height: 16 pixels (16/16 = 1.0), mirroring vanilla `Block.boxZ(16.0, 0.0, 1.0)`.
pub const PALE_MOSS_CARPET_TALL_SIDE_HEIGHT: f64 = 1.0;

/// Side wall thickness: 1 pixel (1/16 = 0.0625).
pub const PALE_MOSS_CARPET_SIDE_THICKNESS: f64 = 1.0 / 16.0;

/// Probability for each supporting wall side to spawn a topper carpet when placed by a player (50%).
pub const PALE_MOSS_CARPET_TOPPER_SIDE_CHANCE: f64 = 0.5;

#[pumpkin_block("minecraft:pale_moss_carpet")]
pub struct PaleMossCarpetBlock;

impl PaleMossCarpetBlock {
    pub const BASE_HEIGHT: f64 = PALE_MOSS_CARPET_BASE_HEIGHT;
    pub const LOW_SIDE_HEIGHT: f64 = PALE_MOSS_CARPET_LOW_SIDE_HEIGHT;
    pub const TALL_SIDE_HEIGHT: f64 = PALE_MOSS_CARPET_TALL_SIDE_HEIGHT;
    pub const SIDE_THICKNESS: f64 = PALE_MOSS_CARPET_SIDE_THICKNESS;
    pub const TOPPER_SIDE_CHANCE: f64 = PALE_MOSS_CARPET_TOPPER_SIDE_CHANCE;

    #[must_use]
    pub fn has_faces(props: &PaleMossCarpetLikeProperties) -> bool {
        pale_moss_carpet_has_faces(props)
    }

    #[must_use]
    pub fn can_survive(world: &dyn BlockAccessor, pos: &BlockPos, is_bottom: bool) -> bool {
        pale_moss_carpet_can_survive(world, pos, is_bottom)
    }
}

pub fn pale_moss_carpet_has_faces(props: &PaleMossCarpetLikeProperties) -> bool {
    props.r#bottom
        || props.r#north != NorthWall::None
        || props.r#east != EastWall::None
        || props.r#south != SouthWall::None
        || props.r#west != WestWall::None
}

pub fn pale_moss_carpet_can_survive(
    world: &dyn BlockAccessor,
    pos: &BlockPos,
    is_bottom: bool,
) -> bool {
    let below_pos = pos.down();
    let (below_block, below_state) = world.get_block_and_state(&below_pos);
    if is_bottom {
        !below_state.is_air()
    } else if below_block == &Block::PALE_MOSS_CARPET {
        let below_props = PaleMossCarpetLikeProperties::from_state_id(below_state.id);
        below_props.r#bottom
    } else {
        false
    }
}

fn get_wall_side_for_dir(props: &PaleMossCarpetLikeProperties, dir: BlockDirection) -> EastWall {
    match dir {
        BlockDirection::North => match props.r#north {
            NorthWall::None => EastWall::None,
            NorthWall::Low => EastWall::Low,
            NorthWall::Tall => EastWall::Tall,
        },
        BlockDirection::East => props.r#east,
        BlockDirection::South => match props.r#south {
            SouthWall::None => EastWall::None,
            SouthWall::Low => EastWall::Low,
            SouthWall::Tall => EastWall::Tall,
        },
        BlockDirection::West => match props.r#west {
            WestWall::None => EastWall::None,
            WestWall::Low => EastWall::Low,
            WestWall::Tall => EastWall::Tall,
        },
        _ => EastWall::None,
    }
}

fn set_wall_side_for_dir(props: &mut PaleMossCarpetLikeProperties, dir: BlockDirection, side: EastWall) {
    match dir {
        BlockDirection::North => {
            props.r#north = match side {
                EastWall::None => NorthWall::None,
                EastWall::Low => NorthWall::Low,
                EastWall::Tall => NorthWall::Tall,
            };
        }
        BlockDirection::East => {
            props.r#east = side;
        }
        BlockDirection::South => {
            props.r#south = match side {
                EastWall::None => SouthWall::None,
                EastWall::Low => SouthWall::Low,
                EastWall::Tall => SouthWall::Tall,
            };
        }
        BlockDirection::West => {
            props.r#west = match side {
                EastWall::None => WestWall::None,
                EastWall::Low => WestWall::Low,
                EastWall::Tall => WestWall::Tall,
            };
        }
        _ => {}
    }
}

pub fn update_pale_moss_carpet_state(
    world: &dyn BlockAccessor,
    pos: &BlockPos,
    mut props: PaleMossCarpetLikeProperties,
    create_sides: bool,
) -> PaleMossCarpetLikeProperties {
    let create_sides = create_sides || props.r#bottom;
    let above_pos = pos.up();
    let (above_block, above_state) = world.get_block_and_state(&above_pos);
    let below_pos = pos.down();
    let (below_block, below_state) = world.get_block_and_state(&below_pos);

    for dir in [
        BlockDirection::North,
        BlockDirection::East,
        BlockDirection::South,
        BlockDirection::West,
    ] {
        let wall_pos = match dir {
            BlockDirection::North => pos.north(),
            BlockDirection::East => pos.east(),
            BlockDirection::South => pos.south(),
            BlockDirection::West => pos.west(),
            _ => *pos,
        };
        let (_, wall_state) = world.get_block_and_state(&wall_pos);
        let can_support = wall_state.is_side_solid(dir.opposite());

        let mut side = if can_support {
            if create_sides {
                EastWall::Low
            } else {
                get_wall_side_for_dir(&props, dir)
            }
        } else {
            EastWall::None
        };

        if side == EastWall::Low {
            if above_block == &Block::PALE_MOSS_CARPET {
                let above_props = PaleMossCarpetLikeProperties::from_state_id(above_state.id);
                if !above_props.r#bottom && get_wall_side_for_dir(&above_props, dir) != EastWall::None {
                    side = EastWall::Tall;
                }
            }

            if !props.r#bottom && below_block == &Block::PALE_MOSS_CARPET {
                let below_props = PaleMossCarpetLikeProperties::from_state_id(below_state.id);
                if get_wall_side_for_dir(&below_props, dir) == EastWall::None {
                    side = EastWall::None;
                }
            }
        }

        set_wall_side_for_dir(&mut props, dir, side);
    }

    props
}

pub fn create_topper_with_side_chance(
    world: &dyn BlockAccessor,
    pos: &BlockPos,
    mut side_chance: impl FnMut() -> bool,
) -> Option<PaleMossCarpetLikeProperties> {
    let above_pos = pos.up();
    let (above_block, above_state) = world.get_block_and_state(&above_pos);
    let is_mossy_above = above_block == &Block::PALE_MOSS_CARPET;
    let above_props = if is_mossy_above {
        Some(PaleMossCarpetLikeProperties::from_state_id(above_state.id))
    } else {
        None
    };

    let can_place_topper = (!is_mossy_above || !above_props.unwrap().r#bottom)
        && (is_mossy_above || above_state.is_air());
    if !can_place_topper {
        return None;
    }

    let no_base_props = PaleMossCarpetLikeProperties {
        r#bottom: false,
        r#north: NorthWall::None,
        r#east: EastWall::None,
        r#south: SouthWall::None,
        r#west: WestWall::None,
    };
    let mut updated = update_pale_moss_carpet_state(world, &above_pos, no_base_props, true);

    for dir in [
        BlockDirection::North,
        BlockDirection::East,
        BlockDirection::South,
        BlockDirection::West,
    ] {
        if get_wall_side_for_dir(&updated, dir) != EastWall::None && !side_chance() {
            set_wall_side_for_dir(&mut updated, dir, EastWall::None);
        }
    }

    if pale_moss_carpet_has_faces(&updated) {
        Some(updated)
    } else {
        None
    }
}

impl BlockBehaviour for PaleMossCarpetBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let default_props = PaleMossCarpetLikeProperties::default(args.block);
        let updated = update_pale_moss_carpet_state(args.world, args.position, default_props, true);
        updated.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let mut rng = rand::rng();
        if let Some(topper) = create_topper_with_side_chance(args.world.as_ref(), args.position, || rng.random_bool(PALE_MOSS_CARPET_TOPPER_SIDE_CHANCE)) {
            let topper_id = topper.to_state_id(args.block);
            args.world.set_block_state(&args.position.up(), topper_id, BlockFlags::NOTIFY_ALL);
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let props = PaleMossCarpetLikeProperties::from_state_id(args.state.id);
        pale_moss_carpet_can_survive(args.block_accessor, args.position, props.r#bottom)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = PaleMossCarpetLikeProperties::from_state_id(args.state_id);
        if !pale_moss_carpet_can_survive(args.world, args.position, props.r#bottom) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            return args.state_id;
        }

        let updated = update_pale_moss_carpet_state(args.world, args.position, props, false);
        if !pale_moss_carpet_has_faces(&updated) {
            args.world
                .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            return args.state_id;
        }

        updated.to_state_id(args.block)
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let (block, state) = args.world.get_block_and_state(args.position);
        if block == args.block {
            let props = PaleMossCarpetLikeProperties::from_state_id(state.id);
            if !pale_moss_carpet_can_survive(args.world.as_ref(), args.position, props.r#bottom)
                || !pale_moss_carpet_has_faces(&update_pale_moss_carpet_state(args.world.as_ref(), args.position, props, false))
            {
                args.world
                    .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
            }
        }
    }

    fn is_valid_bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        let props = PaleMossCarpetLikeProperties::from_state_id(args.state_id);
        props.r#bottom && create_topper_with_side_chance(args.world.as_ref(), args.position, || true).is_some()
    }

    fn is_bonemeal_success(&self, _args: BonemealArgs<'_>) -> bool {
        true
    }

    fn perform_bonemeal(&self, args: BonemealArgs<'_>) {
        if let Some(topper) = create_topper_with_side_chance(args.world.as_ref(), args.position, || true) {
            let topper_id = topper.to_state_id(args.block);
            args.world.set_block_state(&args.position.up(), topper_id, BlockFlags::NOTIFY_ALL);
        }
    }
}

fn can_place_at(block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
    !is_air(block_accessor.get_block_state_id(&block_pos.down()))
}

#[cfg(test)]
mod tests {
    use super::{
        EastWall, NorthWall, PaleMossCarpetBlock, PaleMossCarpetLikeProperties,
        SouthWall, WestWall, PALE_MOSS_CARPET_BASE_HEIGHT,
        PALE_MOSS_CARPET_LOW_SIDE_HEIGHT, PALE_MOSS_CARPET_SIDE_THICKNESS,
        PALE_MOSS_CARPET_TALL_SIDE_HEIGHT, PALE_MOSS_CARPET_TOPPER_SIDE_CHANCE,
        pale_moss_carpet_has_faces,
    };

    #[test]
    fn test_pale_moss_carpet_faces_and_dimensions_parity() {
        assert_eq!(PALE_MOSS_CARPET_BASE_HEIGHT, 1.0 / 16.0);
        assert_eq!(PALE_MOSS_CARPET_LOW_SIDE_HEIGHT, 10.0 / 16.0);
        assert_eq!(PALE_MOSS_CARPET_TALL_SIDE_HEIGHT, 1.0);
        assert_eq!(PALE_MOSS_CARPET_SIDE_THICKNESS, 1.0 / 16.0);
        assert_eq!(PALE_MOSS_CARPET_TOPPER_SIDE_CHANCE, 0.5);

        assert_eq!(PaleMossCarpetBlock::BASE_HEIGHT, 1.0 / 16.0);
        assert_eq!(PaleMossCarpetBlock::LOW_SIDE_HEIGHT, 10.0 / 16.0);
        assert_eq!(PaleMossCarpetBlock::TALL_SIDE_HEIGHT, 1.0);
        assert_eq!(PaleMossCarpetBlock::SIDE_THICKNESS, 1.0 / 16.0);
        assert_eq!(PaleMossCarpetBlock::TOPPER_SIDE_CHANCE, 0.5);

        // Face test: bottom=true has faces
        let base_carpet = PaleMossCarpetLikeProperties {
            r#bottom: true,
            r#north: NorthWall::None,
            r#east: EastWall::None,
            r#south: SouthWall::None,
            r#west: WestWall::None,
        };
        assert!(pale_moss_carpet_has_faces(&base_carpet));

        // Face test: bottom=false with side has faces
        let side_topper = PaleMossCarpetLikeProperties {
            r#bottom: false,
            r#north: NorthWall::Low,
            r#east: EastWall::None,
            r#south: SouthWall::None,
            r#west: WestWall::None,
        };
        assert!(pale_moss_carpet_has_faces(&side_topper));

        // Face test: bottom=false and no sides has no faces
        let no_faces = PaleMossCarpetLikeProperties {
            r#bottom: false,
            r#north: NorthWall::None,
            r#east: EastWall::None,
            r#south: SouthWall::None,
            r#west: WestWall::None,
        };
        assert!(!pale_moss_carpet_has_faces(&no_faces));
    }
}
