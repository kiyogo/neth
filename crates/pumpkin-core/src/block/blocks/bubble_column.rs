use std::sync::Arc;

use pumpkin_data::block_properties::BubbleColumnLikeProperties;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockId, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BlockMetadata, BrokenArgs, OnEntityCollisionArgs, OnNeighborUpdateArgs,
    OnScheduledTickArgs, PlacedArgs,
};
use crate::world::World;

const CREATE_DELAY_TICKS: u32 = 20;
const REMOVE_DELAY_TICKS: u32 = 5;

// Vanilla Entity.java:2945-2994 constants
const UPWARD_ACCELERATION: f64 = 0.06;
const UPWARD_MAX_SPEED: f64 = 0.7;
const SURFACE_UPWARD_ACCELERATION: f64 = 0.1;
const SURFACE_UPWARD_MAX_SPEED: f64 = 1.8;
const DOWNWARD_ACCELERATION: f64 = -0.03;
const DOWNWARD_MIN_SPEED: f64 = -0.3;
const SURFACE_DOWNWARD_ACCELERATION: f64 = -0.03;
const SURFACE_DOWNWARD_MIN_SPEED: f64 = -0.9;

pub struct BubbleColumnBlock;

impl BubbleColumnBlock {
    pub const CREATE_DELAY_TICKS: u32 = 20;
    pub const REMOVE_DELAY_TICKS: u32 = 5;
    pub const UPWARD_ACCELERATION: f64 = 0.06;
    pub const UPWARD_MAX_SPEED: f64 = 0.7;
    pub const SURFACE_UPWARD_ACCELERATION: f64 = 0.1;
    pub const SURFACE_UPWARD_MAX_SPEED: f64 = 1.8;
    pub const DOWNWARD_ACCELERATION: f64 = -0.03;
    pub const DOWNWARD_MIN_SPEED: f64 = -0.3;
    pub const SURFACE_DOWNWARD_ACCELERATION: f64 = -0.03;
    pub const SURFACE_DOWNWARD_MIN_SPEED: f64 = -0.9;
}

impl BlockMetadata for BubbleColumnBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BUBBLE_COLUMN, BlockId::WATER].into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BubbleColumnKind {
    Upward,
    Downward,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReconcileAction {
    SetBubble(BubbleColumnKind),
    RestoreWater,
    Stop,
}

fn source_water_state() -> BlockStateId {
    Fluid::WATER
        .states
        .iter()
        .find(|state| state.is_still && state.is_source)
        .map_or(BlockStateId::new_or_air(0), |state| state.block_state_id)
}

fn bubble_column_state(kind: BubbleColumnKind) -> BlockStateId {
    BubbleColumnLikeProperties {
        r#drag: matches!(kind, BubbleColumnKind::Downward),
    }
    .to_state_id(&Block::BUBBLE_COLUMN)
}

fn kind_from_support(block: &Block) -> Option<BubbleColumnKind> {
    if block.has_tag(&tag::Block::MINECRAFT_ENABLES_BUBBLE_COLUMN_PUSH_UP) {
        Some(BubbleColumnKind::Upward)
    } else if block.has_tag(&tag::Block::MINECRAFT_ENABLES_BUBBLE_COLUMN_DRAG_DOWN) {
        Some(BubbleColumnKind::Downward)
    } else {
        None
    }
}

fn kind_from_state(state: BlockStateId) -> BubbleColumnKind {
    let props = BubbleColumnLikeProperties::from_state_id(state);
    if props.r#drag {
        BubbleColumnKind::Downward
    } else {
        BubbleColumnKind::Upward
    }
}

fn kind_from_below(block: &Block, state: BlockStateId) -> Option<BubbleColumnKind> {
    if block == &Block::BUBBLE_COLUMN {
        Some(kind_from_state(state))
    } else {
        kind_from_support(block)
    }
}

fn is_source_water_state(state: BlockStateId) -> bool {
    let Some(fluid) = Fluid::from_state_id(state) else {
        return false;
    };

    fluid.has_tag(&tag::Fluid::MINECRAFT_BUBBLE_COLUMN_CAN_OCCUPY)
        && fluid.is_source(state)
        && fluid.states.iter().any(|fluid_state| {
            fluid_state.block_state_id == state && fluid_state.is_still && fluid_state.is_source
        })
}

fn is_source_water(world: &World, position: BlockPos) -> bool {
    is_source_water_state(world.get_block_state_id(&position))
}

fn reconcile_action(
    current_block: &Block,
    current_state: BlockStateId,
    below_block: &Block,
    below_state: BlockStateId,
) -> ReconcileAction {
    let current_is_bubble = current_block == &Block::BUBBLE_COLUMN;
    let current_is_source_water = is_source_water_state(current_state);

    if let Some(kind) = kind_from_below(below_block, below_state)
        && (current_is_bubble || current_is_source_water)
    {
        return ReconcileAction::SetBubble(kind);
    }

    if current_is_bubble {
        ReconcileAction::RestoreWater
    } else {
        ReconcileAction::Stop
    }
}

fn schedule_reconcile(world: &Arc<World>, position: BlockPos, delay: u32) {
    world.schedule_block_tick(&Block::BUBBLE_COLUMN, position, delay, TickPriority::Normal);
}

pub fn bubble_column_vertical_velocity(
    current_y: f64,
    kind: BubbleColumnKind,
    at_surface: bool,
) -> f64 {
    match (kind, at_surface) {
        (BubbleColumnKind::Upward, true) => {
            (current_y + SURFACE_UPWARD_ACCELERATION).min(SURFACE_UPWARD_MAX_SPEED)
        }
        (BubbleColumnKind::Upward, false) => {
            (current_y + UPWARD_ACCELERATION).min(UPWARD_MAX_SPEED)
        }
        (BubbleColumnKind::Downward, true) => {
            (current_y + SURFACE_DOWNWARD_ACCELERATION).max(SURFACE_DOWNWARD_MIN_SPEED)
        }
        (BubbleColumnKind::Downward, false) => {
            (current_y + DOWNWARD_ACCELERATION).max(DOWNWARD_MIN_SPEED)
        }
    }
}

pub fn bubble_column_velocity(
    current: Vector3<f64>,
    kind: BubbleColumnKind,
    at_surface: bool,
) -> Vector3<f64> {
    Vector3::new(
        current.x,
        bubble_column_vertical_velocity(current.y, kind, at_surface),
        current.z,
    )
}

impl BlockBehaviour for BubbleColumnBlock {
    /// Moves entities in the column and refills their air (`BubbleColumnBlock.java:45-58`).
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if args.block != &Block::BUBBLE_COLUMN {
            return;
        }

        let kind = kind_from_state(args.state.id);
        let block_above = args.world.get_block(&args.position.up());
        let (fluid_above, fluid_state_above) = args.world.get_fluid_and_fluid_state(&args.position.up());
        let at_surface = block_above == &Block::AIR
            && (fluid_above.id == pumpkin_data::fluid::Fluid::EMPTY.id || fluid_state_above.is_empty);

        let entity = args.entity.get_entity();
        let is_drag_down = matches!(kind, BubbleColumnKind::Downward);

        // Vanilla BubbleColumnBlock.java:53: onAboveBubbleColumn for boat
        if at_surface
            && let Some(boat) = args
                .entity
                .cast_any()
                .downcast_ref::<crate::entity::vehicle::boat::BoatEntity>()
        {
            boat.on_above_bubble_column(is_drag_down);
        }

        entity.velocity.store(bubble_column_velocity(
            entity.velocity.load(),
            kind,
            at_surface,
        ));

        if let Some(living) = args.entity.get_living_entity() {
            living.breath.reset(args.entity);
            living.fall_distance.store(0.0);
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block == &Block::WATER && is_source_water(args.world, *args.position) {
            schedule_reconcile(args.world, *args.position, CREATE_DELAY_TICKS);
        }
        args.world.update_neighbors(args.position, None);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        args.world.update_neighbors(args.position, None);
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let state = args.world.get_block_state_id(args.position);
        if args.block == &Block::BUBBLE_COLUMN {
            schedule_reconcile(args.world, *args.position, REMOVE_DELAY_TICKS);
        } else if args.block == &Block::WATER && is_source_water_state(state) {
            let below_pos = args.position.down();
            let below_state = args.world.get_block_state_id(&below_pos);
            let below_block = Block::from_state_id(below_state);
            if kind_from_below(below_block, below_state).is_some() {
                schedule_reconcile(args.world, *args.position, CREATE_DELAY_TICKS);
            }
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state_id(args.position);
        let block = Block::from_state_id(state);
        if block != &Block::BUBBLE_COLUMN && block != &Block::WATER {
            return;
        }

        let below_pos = args.position.down();
        let below_state = args.world.get_block_state_id(&below_pos);
        let below_block = Block::from_state_id(below_state);

        match reconcile_action(block, state, below_block, below_state) {
            ReconcileAction::SetBubble(kind) => {
                let new_state = bubble_column_state(kind);
                args.world
                    .set_block_state(args.position, new_state, BlockFlags::NOTIFY_ALL);
                schedule_reconcile(args.world, args.position.up(), CREATE_DELAY_TICKS);
            }
            ReconcileAction::RestoreWater => {
                args.world.set_block_state(
                    args.position,
                    source_water_state(),
                    BlockFlags::NOTIFY_ALL,
                );
                schedule_reconcile(args.world, args.position.up(), REMOVE_DELAY_TICKS);
            }
            ReconcileAction::Stop => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use pumpkin_data::BlockStateId;
    use pumpkin_data::fluid::{Falling, FlowingWaterLikeFluidProperties, FluidProperties, Level};

    use super::*;

    fn flowing_water_state() -> BlockStateId {
        FlowingWaterLikeFluidProperties {
            r#falling: Falling::False,
            r#level: Level::L1,
        }
        .to_state_id(&Fluid::FLOWING_WATER)
    }

    #[test]
    fn support_tags_map_to_expected_kinds() {
        assert_eq!(
            kind_from_support(&Block::SOUL_SAND),
            Some(BubbleColumnKind::Upward)
        );
        assert_eq!(
            kind_from_support(&Block::MAGMA_BLOCK),
            Some(BubbleColumnKind::Downward)
        );
        assert_eq!(kind_from_support(&Block::STONE), None);
    }

    #[test]
    fn reconciliation_creates_from_upward_support() {
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::SOUL_SAND,
                Block::SOUL_SAND.default_state.id,
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Upward)
        );
    }

    #[test]
    fn reconciliation_creates_from_downward_support() {
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::MAGMA_BLOCK,
                Block::MAGMA_BLOCK.default_state.id,
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Downward)
        );
    }

    #[test]
    fn reconciliation_inherits_direction_from_lower_column() {
        let upward_state = bubble_column_state(BubbleColumnKind::Upward);
        let downward_state = bubble_column_state(BubbleColumnKind::Downward);

        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::BUBBLE_COLUMN,
                upward_state
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Upward)
        );
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                source_water_state(),
                &Block::BUBBLE_COLUMN,
                downward_state,
            ),
            ReconcileAction::SetBubble(BubbleColumnKind::Downward)
        );
    }

    #[test]
    fn reconciliation_rejects_flowing_water_and_air() {
        assert_eq!(
            reconcile_action(
                &Block::WATER,
                flowing_water_state(),
                &Block::SOUL_SAND,
                Block::SOUL_SAND.default_state.id,
            ),
            ReconcileAction::Stop
        );
        assert_eq!(
            reconcile_action(
                &Block::AIR,
                BlockStateId::AIR,
                &Block::SOUL_SAND,
                Block::SOUL_SAND.default_state.id,
            ),
            ReconcileAction::Stop
        );
    }

    #[test]
    fn reconciliation_restores_invalidated_column() {
        assert_eq!(
            reconcile_action(
                &Block::BUBBLE_COLUMN,
                bubble_column_state(BubbleColumnKind::Upward),
                &Block::STONE,
                Block::STONE.default_state.id,
            ),
            ReconcileAction::RestoreWater
        );
    }

    #[test]
    fn test_bubble_column_velocity_and_surface_parity_gap432() {
        // 1. Upward column inside (Entity.java:2987: min(0.7, y + 0.06))
        assert!((bubble_column_vertical_velocity(0.0, BubbleColumnKind::Upward, false) - 0.06).abs() < 1e-6);
        assert!((bubble_column_vertical_velocity(0.68, BubbleColumnKind::Upward, false) - 0.7).abs() < 1e-6);

        // 2. Upward column at surface (Entity.java:2952: min(1.8, y + 0.1))
        assert!((bubble_column_vertical_velocity(0.0, BubbleColumnKind::Upward, true) - 0.1).abs() < 1e-6);
        assert!((bubble_column_vertical_velocity(1.75, BubbleColumnKind::Upward, true) - 1.8).abs() < 1e-6);

        // 3. Downward column inside (Entity.java:2985: max(-0.3, y - 0.03))
        assert!((bubble_column_vertical_velocity(0.0, BubbleColumnKind::Downward, false) - (-0.03)).abs() < 1e-6);
        assert!((bubble_column_vertical_velocity(-0.29, BubbleColumnKind::Downward, false) - (-0.3)).abs() < 1e-6);

        // 4. Downward column at surface (Entity.java:2950: max(-0.9, y - 0.03))
        assert!((bubble_column_vertical_velocity(0.0, BubbleColumnKind::Downward, true) - (-0.03)).abs() < 1e-6);
        assert!((bubble_column_vertical_velocity(-0.89, BubbleColumnKind::Downward, true) - (-0.9)).abs() < 1e-6);
        assert!((bubble_column_vertical_velocity(-0.5, BubbleColumnKind::Downward, true) - (-0.53)).abs() < 1e-6);
    }
}
