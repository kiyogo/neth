use crate::block::{
    BlockBehaviour, BlockMetadata, BrokenArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs, PathComputationType, PlacedArgs,
};
use pumpkin_data::block_properties::MangroveRootsLikeProperties;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::{BlockId, BlockState, BlockStateId};
use pumpkin_world::tick::TickPriority;

pub struct HeavyCoreBlock;

impl HeavyCoreBlock {
    /// Hardness = 10.0 (Blocks.java:5514)
    pub const HARDNESS: f32 = 10.0;
    /// Explosion resistance = 1200.0 (Blocks.java:5516)
    pub const BLAST_RESISTANCE: f32 = 1200.0;

    /// VoxelShape dimensions (HeavyCoreBlock.java:23: Block.column(8.0, 0.0, 8.0))
    pub const SHAPE_MIN_X: f64 = 0.25; // 4.0 / 16.0
    pub const SHAPE_MAX_X: f64 = 0.75; // 12.0 / 16.0
    pub const SHAPE_MIN_Y: f64 = 0.0;
    pub const SHAPE_MAX_Y: f64 = 0.5;  // 8.0 / 16.0
    pub const SHAPE_MIN_Z: f64 = 0.25; // 4.0 / 16.0
    pub const SHAPE_MAX_Z: f64 = 0.75; // 12.0 / 16.0

    /// Mace crafting recipe check: Heavy Core above Breeze Rod
    #[must_use]
    pub const fn is_valid_mace_recipe(top_item: &str, bottom_item: &str) -> bool {
        (const_str_eq(top_item, "minecraft:heavy_core") || const_str_eq(top_item, "heavy_core"))
            && (const_str_eq(bottom_item, "minecraft:breeze_rod") || const_str_eq(bottom_item, "breeze_rod"))
    }

    /// Checks if a relative coordinate falls within the 8x8x8 centered column shape
    #[must_use]
    pub const fn is_inside_bounding_box(rel_x: f64, rel_y: f64, rel_z: f64) -> bool {
        rel_x >= Self::SHAPE_MIN_X
            && rel_x <= Self::SHAPE_MAX_X
            && rel_y >= Self::SHAPE_MIN_Y
            && rel_y <= Self::SHAPE_MAX_Y
            && rel_z >= Self::SHAPE_MIN_Z
            && rel_z <= Self::SHAPE_MAX_Z
    }
}

const fn const_str_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut i = 0;
    while i < a_bytes.len() {
        if a_bytes[i] != b_bytes[i] {
            return false;
        }
        i += 1;
    }
    true
}

impl BlockMetadata for HeavyCoreBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::HEAVY_CORE].into()
    }
}

impl BlockBehaviour for HeavyCoreBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = MangroveRootsLikeProperties::default(args.block);
        props.waterlogged = args.replacing.water_source();
        props.to_state_id(args.block)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = MangroveRootsLikeProperties::from_state_id(args.state_id);
        if props.waterlogged {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed,
                TickPriority::Normal,
            );
        }
        args.state_id
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
