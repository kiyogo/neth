pub mod command_block;
mod chest;
mod container;
mod furnace;
mod hopper;
mod rideable;
pub mod spawner;
mod tnt;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam::atomic::AtomicCell;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_protocol::java::server::play::SPlayerInput;
use rand::RngExt;

use crate::{
    entity::{Entity, EntityBase, living::LivingEntity, player::Player},
    server::Server,
    world::World,
};
use pumpkin_data::block_properties::{PoweredRailLikeProperties, RailShape};
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockStateId};
use pumpkin_inventory::Inventory;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::math::wrap_degrees;

use crate::entity::vehicle::vehicle::VehicleEntity;
use crate::entity::velocity;
use chest::ChestMinecart;
use command_block::CommandBlockMinecart;
use container::MinecartInventory;
use furnace::FurnaceMinecart;
use hopper::HopperMinecart;
use rideable::RideableMinecart;
use spawner::SpawnerMinecart;
use tnt::TntMinecart;

/// Vanilla `AbstractMinecart.exits`: a slope's lower end sits one block down.
const fn get_exits(
    shape: pumpkin_data::block_properties::RailShape,
) -> (Vector3<f64>, Vector3<f64>) {
    use pumpkin_data::block_properties::RailShape;
    const WEST: Vector3<f64> = Vector3::new(-1.0, 0.0, 0.0);
    const EAST: Vector3<f64> = Vector3::new(1.0, 0.0, 0.0);
    const NORTH: Vector3<f64> = Vector3::new(0.0, 0.0, -1.0);
    const SOUTH: Vector3<f64> = Vector3::new(0.0, 0.0, 1.0);
    const fn below(exit: Vector3<f64>) -> Vector3<f64> {
        Vector3::new(exit.x, -1.0, exit.z)
    }
    match shape {
        RailShape::NorthSouth => (NORTH, SOUTH),
        RailShape::EastWest => (WEST, EAST),
        RailShape::AscendingEast => (below(WEST), EAST),
        RailShape::AscendingWest => (WEST, below(EAST)),
        RailShape::AscendingNorth => (NORTH, below(SOUTH)),
        RailShape::AscendingSouth => (below(NORTH), SOUTH),
        RailShape::SouthEast => (SOUTH, EAST),
        RailShape::SouthWest => (SOUTH, WEST),
        RailShape::NorthWest => (NORTH, WEST),
        RailShape::NorthEast => (NORTH, EAST),
    }
}

/// Vanilla `getCurrentBlockPosOrRailBelow`.
fn rail_block_pos(world: &World, pos: Vector3<f64>) -> BlockPos {
    let block_pos = BlockPos::floored_v(pos);
    let below = BlockPos(Vector3::new(
        block_pos.0.x,
        block_pos.0.y - 1,
        block_pos.0.z,
    ));
    if world
        .get_block(&below)
        .has_tag(&tag::Block::MINECRAFT_RAILS)
    {
        below
    } else {
        block_pos
    }
}

fn rail_shape_of(block: &Block, state_id: BlockStateId) -> Option<RailShape> {
    use pumpkin_data::block_properties::{BlockProperties, RailLikeProperties, RailShapeStraight};
    if block.id == Block::RAIL.id {
        return Some(RailLikeProperties::from_state_id(state_id).shape);
    }
    if !PoweredRailLikeProperties::handles_block_id(block.id) {
        return None;
    }
    Some(
        match PoweredRailLikeProperties::from_state_id(state_id).shape {
            RailShapeStraight::NorthSouth => RailShape::NorthSouth,
            RailShapeStraight::EastWest => RailShape::EastWest,
            RailShapeStraight::AscendingEast => RailShape::AscendingEast,
            RailShapeStraight::AscendingWest => RailShape::AscendingWest,
            RailShapeStraight::AscendingNorth => RailShape::AscendingNorth,
            RailShapeStraight::AscendingSouth => RailShape::AscendingSouth,
        },
    )
}

/// How far along the rail line (`x0`/`z0` + `xd`/`zd`) the point sits.
fn rail_progress(
    point: Vector3<f64>,
    block_x: f64,
    block_z: f64,
    x0: f64,
    z0: f64,
    xd: f64,
    zd: f64,
) -> f64 {
    if xd == 0.0 {
        point.z - block_z
    } else if zd == 0.0 {
        point.x - block_x
    } else {
        ((point.x - x0) * xd + (point.z - z0) * zd) * 2.0
    }
}

/// Vanilla `OldMinecartBehavior.getPos`: the point on the rail under `pos`.
fn rail_pos(world: &World, pos: Vector3<f64>) -> Option<Vector3<f64>> {
    let block_pos = rail_block_pos(world, pos);
    let (block, state_id) = world.get_block_and_state_id(&block_pos);
    let (exit0, exit1) = get_exits(rail_shape_of(block, state_id)?);
    let (block_x, block_y, block_z) = (
        f64::from(block_pos.0.x),
        f64::from(block_pos.0.y),
        f64::from(block_pos.0.z),
    );
    let x0 = block_x + 0.5 + exit0.x * 0.5;
    let y0 = block_y + RAIL_HEIGHT_OFFSET + exit0.y * 0.5;
    let z0 = block_z + 0.5 + exit0.z * 0.5;
    let xd = block_x + 0.5 + exit1.x * 0.5 - x0;
    let yd = (block_y + RAIL_HEIGHT_OFFSET + exit1.y * 0.5 - y0) * 2.0;
    let zd = block_z + 0.5 + exit1.z * 0.5 - z0;
    let progress = rail_progress(pos, block_x, block_z, x0, z0, xd, zd);
    let lift = crate::entity::combat::calculate_minecart_rail_slope_position_lift(yd);
    Some(Vector3::new(
        x0 + xd * progress,
        y0 + yd * progress + lift,
        z0 + zd * progress,
    ))
}

/// powered rail boost, after `applyNaturalSlowdown`.
fn boost(
    world: &World,
    pos: BlockPos,
    shape: RailShape,
    movement: Vector3<f64>,
) -> Vector3<f64> {
    // A standing cart starts away from a redstone conductor.
    let conductor = |x: i32, z: i32| {
        world
            .get_block_state(&BlockPos(Vector3::new(pos.0.x + x, pos.0.y, pos.0.z + z)))
            .is_solid_block()
    };
    crate::entity::combat::calculate_minecart_powered_rail_boost(
        movement,
        shape,
        conductor(-1, 0),
        conductor(1, 0),
        conductor(0, -1),
        conductor(0, 1),
    )
}

const RAIL_HEIGHT_OFFSET: f64 = crate::entity::combat::MINECART_RAIL_HEIGHT_OFFSET;
const AIR_DRAG: f32 = crate::entity::combat::MINECART_AIR_DRAG;
/// Pumpkin: vanilla never zeroes a sliding cart. Below this the rest of the slide
/// is under 0.04 blocks, even with the 0.997 rider slowdown.
const STOP_SPEED: f64 = 1.0E-4;

pub struct MinecartEntity {
    pub vehicle: VehicleEntity,
    kind: MinecartKind,
    flipped: AtomicBool,
    pub fall_distance: AtomicCell<f64>,
    pub custom_display_block_state: AtomicCell<Option<BlockStateId>>,
    pub custom_display_offset: AtomicCell<Option<i32>>,
    pub has_ticked: AtomicBool,
}

enum MinecartKind {
    Rideable(RideableMinecart),
    Chest(ChestMinecart),
    Furnace(FurnaceMinecart),
    Hopper(HopperMinecart),
    Tnt(TntMinecart),
    CommandBlock(CommandBlockMinecart),
    Spawner(SpawnerMinecart),
    Other,
}

impl MinecartEntity {
    pub fn new(entity: Entity) -> Self {
        let kind = match entity.entity_type.id {
            id if id == EntityType::MINECART.id => MinecartKind::Rideable(RideableMinecart),
            id if id == EntityType::CHEST_MINECART.id => MinecartKind::Chest(ChestMinecart::new()),
            id if id == EntityType::FURNACE_MINECART.id => {
                MinecartKind::Furnace(FurnaceMinecart::new())
            }
            id if id == EntityType::HOPPER_MINECART.id => {
                MinecartKind::Hopper(HopperMinecart::new())
            }
            id if id == EntityType::TNT_MINECART.id => MinecartKind::Tnt(TntMinecart::new()),
            id if id == EntityType::COMMAND_BLOCK_MINECART.id => {
                MinecartKind::CommandBlock(CommandBlockMinecart::new())
            }
            id if id == EntityType::SPAWNER_MINECART.id => {
                MinecartKind::Spawner(SpawnerMinecart::new())
            }
            _ => MinecartKind::Other,
        };
        Self {
            vehicle: VehicleEntity::new(entity),
            kind,
            flipped: AtomicBool::new(false),
            fall_distance: AtomicCell::new(0.0),
            custom_display_block_state: AtomicCell::new(None),
            custom_display_offset: AtomicCell::new(None),
            has_ticked: AtomicBool::new(false),
        }
    }

    #[must_use]
    pub fn get_default_display_block_state(&self) -> Option<BlockStateId> {
        match &self.kind {
            MinecartKind::Rideable(_) | MinecartKind::Other => None,
            MinecartKind::Chest(_) => Block::CHEST
                .state_from_properties(&[("facing", "north")])
                .map(|s| s.id)
                .or(Some(Block::CHEST.default_state.id)),
            MinecartKind::Furnace(f) => {
                let lit_str = if f.has_fuel() { "true" } else { "false" };
                Block::FURNACE
                    .state_from_properties(&[("facing", "north"), ("lit", lit_str)])
                    .map(|s| s.id)
                    .or(Some(Block::FURNACE.default_state.id))
            }
            MinecartKind::Hopper(_) => Some(Block::HOPPER.default_state.id),
            MinecartKind::Tnt(_) => Some(Block::TNT.default_state.id),
            MinecartKind::CommandBlock(_) => Some(Block::COMMAND_BLOCK.default_state.id),
            MinecartKind::Spawner(_) => Some(Block::SPAWNER.default_state.id),
        }
    }

    #[must_use]
    pub fn get_display_block_state(&self) -> Option<BlockStateId> {
        self.custom_display_block_state
            .load()
            .or_else(|| self.get_default_display_block_state())
    }

    #[must_use]
    pub fn get_custom_display_block_state(&self) -> Option<BlockStateId> {
        self.custom_display_block_state.load()
    }

    pub fn set_custom_display_block_state(&self, state: Option<BlockStateId>) {
        self.custom_display_block_state.store(state);
        self.send_display_metadata();
    }

    #[must_use]
    pub fn get_default_display_offset(&self) -> i32 {
        crate::entity::combat::get_minecart_default_display_offset(
            self.vehicle.entity.entity_type.id,
        )
    }

    #[must_use]
    pub fn get_display_offset(&self) -> i32 {
        self.custom_display_offset
            .load()
            .unwrap_or_else(|| self.get_default_display_offset())
    }

    pub fn set_display_offset(&self, offset: i32) {
        self.custom_display_offset.store(Some(offset));
        self.send_display_metadata();
    }

    #[must_use]
    pub fn has_ticked(&self) -> bool {
        self.has_ticked.load(Ordering::Relaxed)
    }

    pub fn set_has_ticked(&self, ticked: bool) {
        self.has_ticked.store(ticked, Ordering::Relaxed);
    }

    pub fn send_display_metadata(&self) {
        let block_state_val = self
            .custom_display_block_state
            .load()
            .map(|id| VarInt(id.as_u16() as i32))
            .unwrap_or(VarInt(0));
        let offset_val = VarInt(self.get_display_offset());

        self.vehicle.entity.send_meta_data(
            &[
                Metadata::new(
                    pumpkin_data::tracked_data::abstract_minecart::ID_CUSTOM_DISPLAY_BLOCK,
                    block_state_val,
                ),
                Metadata::new(
                    pumpkin_data::tracked_data::abstract_minecart::ID_DISPLAY_OFFSET,
                    offset_val,
                ),
            ],
            None,
        );
    }

    fn parse_display_state(nbt: &NbtCompound) -> Option<BlockStateId> {
        if let Some(compound) = nbt.get_compound("DisplayState") {
            if let Some(name) = compound.get_string("Name") {
                let stripped = name.strip_prefix("minecraft:").unwrap_or(name);
                if let Some(block) = Block::from_registry_key(stripped) {
                    if let Some(props_compound) = compound.get_compound("Properties") {
                        let props_vec: Vec<(&str, &str)> = props_compound
                            .child_tags
                            .iter()
                            .filter_map(|(k, v)| {
                                if let pumpkin_nbt::tag::NbtTag::String(s) = v {
                                    Some((k.as_ref(), s.as_ref()))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if let Some(state) = block.state_from_properties(&props_vec) {
                            return Some(state.id);
                        }
                    }
                    return Some(block.default_state.id);
                }
            }
        } else if let Some(name) = nbt.get_string("DisplayState") {
            let stripped = name.strip_prefix("minecraft:").unwrap_or(name);
            if let Some(block) = Block::from_registry_key(stripped) {
                return Some(block.default_state.id);
            }
        } else if let Some(raw_id) = nbt.get_int("DisplayState") {
            return BlockStateId::from_raw(raw_id as u16);
        }
        None
    }

    fn write_display_state(state_id: BlockStateId, nbt: &mut NbtCompound) {
        let block = state_id.to_block();
        let mut compound = NbtCompound::new();
        compound.put_string("Name", format!("minecraft:{}", block.name));
        if let Some(props) = block.properties(state_id) {
            let props_vec = props.to_props();
            if !props_vec.is_empty() {
                let mut props_compound = NbtCompound::new();
                for (k, v) in props_vec {
                    props_compound.put_string(k, v.to_string());
                }
                compound.put_compound("Properties", props_compound);
            }
        }
        nbt.put_compound("DisplayState", compound);
    }

    #[must_use]
    pub fn is_flipped(&self) -> bool {
        self.flipped.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn get_motion_direction(&self) -> pumpkin_data::BlockDirection {
        let yaw = self.vehicle.entity.yaw.load();
        let flipped = self.is_flipped();
        crate::entity::combat::get_minecart_motion_direction(yaw, flipped)
    }

    #[must_use]
    pub fn command_block(&self) -> Option<&CommandBlockMinecart> {
        match &self.kind {
            MinecartKind::CommandBlock(cmd) => Some(cmd),
            _ => None,
        }
    }

    #[must_use]
    pub fn spawner(&self) -> Option<&SpawnerMinecart> {
        match &self.kind {
            MinecartKind::Spawner(spawner) => Some(spawner),
            _ => None,
        }
    }

    const fn container(&self) -> Option<&Arc<MinecartInventory>> {
        match &self.kind {
            MinecartKind::Chest(minecart) => Some(minecart.inventory()),
            MinecartKind::Hopper(minecart) => Some(minecart.inventory()),
            _ => None,
        }
    }

    /// Vanilla `getMaxSpeed` (old behaviour): the per-axis cap of one step.
    fn max_speed(&self) -> f64 {
        let in_water = self.vehicle.entity.touching_water.load(Ordering::Relaxed);
        let base = if in_water {
            crate::entity::combat::MINECART_MAX_SPEED_WATER
        } else {
            crate::entity::combat::MINECART_MAX_SPEED_LAND
        };
        match self.kind {
            MinecartKind::Furnace(_) if in_water => base * 0.75,
            MinecartKind::Furnace(_) => base * 0.5,
            _ => base,
        }
    }

    /// Vanilla `ServerPlayer.getLastClientMoveIntent` of a player in the front seat.
    fn rider_move_intent(&self) -> Vector3<f64> {
        let passengers = self
            .vehicle
            .entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(player) = passengers
            .first()
            .and_then(|passenger| passenger.get_player())
        else {
            return Vector3::default();
        };
        let input = player.last_input.load(Ordering::Relaxed);
        let axis = |positive: i8, negative: i8| match (input & positive != 0, input & negative != 0)
        {
            (true, false) => 1.0,
            (false, true) => -1.0,
            _ => 0.0,
        };
        let intent = Vector3::new(
            axis(SPlayerInput::LEFT, SPlayerInput::RIGHT),
            0.0,
            axis(SPlayerInput::FORWARD, SPlayerInput::BACKWARD),
        );
        // Vanilla `Entity.getInputVector` with speed 1.
        let length = intent.length_squared();
        if length < 1.0E-7 {
            return Vector3::default();
        }
        let intent = if length > 1.0 {
            intent.normalize()
        } else {
            intent
        };
        let yaw = player.get_entity().yaw.load().to_radians();
        let (sin, cos) = (f64::from(yaw.sin()), f64::from(yaw.cos()));
        Vector3::new(
            intent.x * cos - intent.z * sin,
            0.0,
            intent.z * cos + intent.x * sin,
        )
    }

    /// Vanilla `OldMinecartBehavior.moveAlongTrack`.
    fn move_along_track(
        &self,
        caller: &dyn EntityBase,
        world: &World,
        pos: BlockPos,
        shape: RailShape,
        power_track: bool,
        mut halt_track: bool,
    ) {
        let entity = &self.vehicle.entity;
        let start = entity.pos.load();
        let old_rail_pos = rail_pos(world, start);
        let in_water = entity.touching_water.load(Ordering::Relaxed);
        let mut movement = entity.velocity.load();

        let slide = crate::entity::combat::calculate_minecart_slope_slide_speed(in_water);
        match shape {
            RailShape::AscendingEast => movement.x -= slide,
            RailShape::AscendingWest => movement.x += slide,
            RailShape::AscendingNorth => movement.z += slide,
            RailShape::AscendingSouth => movement.z -= slide,
            _ => {}
        }
        let y = f64::from(pos.0.y) + if shape.is_ascending() { 1.0 } else { 0.0 };

        // Redirect along the rail.
        let (exit0, exit1) = get_exits(shape);
        let (mut xd, mut zd) = (exit1.x - exit0.x, exit1.z - exit0.z);
        let length = xd.hypot(zd);
        if movement.x * xd + movement.z * zd < 0.0 {
            (xd, zd) = (-xd, -zd);
        }
        let pow = movement
            .horizontal_length()
            .min(crate::entity::combat::MINECART_MAX_HORIZONTAL_MOVEMENT);
        movement = Vector3::new(pow * xd / length, movement.y, pow * zd / length);

        // A rider only nudges a (nearly) standing cart.
        let move_intent = self.rider_move_intent();
        if move_intent.length_squared() > 0.0
            && movement.horizontal_length_squared()
                < crate::entity::combat::MINECART_RIDER_NUDGE_SPEED_SQ_THRESHOLD
        {
            movement.x += move_intent.x * crate::entity::combat::MINECART_RIDER_NUDGE_FACTOR;
            movement.z += move_intent.z * crate::entity::combat::MINECART_RIDER_NUDGE_FACTOR;
            halt_track = false;
        }
        if halt_track {
            movement = crate::entity::combat::apply_minecart_unpowered_rail_brake(movement);
        }

        // Snap onto the rail line.
        let (block_x, block_z) = (f64::from(pos.0.x), f64::from(pos.0.z));
        let x0 = block_x + 0.5 + exit0.x * 0.5;
        let z0 = block_z + 0.5 + exit0.z * 0.5;
        let (xd, zd) = (
            block_x + 0.5 + exit1.x * 0.5 - x0,
            block_z + 0.5 + exit1.z * 0.5 - z0,
        );
        let progress = rail_progress(start, block_x, block_z, x0, z0, xd, zd);
        entity.set_pos(Vector3::new(x0 + xd * progress, y, z0 + zd * progress));

        // The step is capped, `deltaMovement` is not: an overspeed cart keeps its speed.
        let scale = if entity.has_passengers() { 0.75 } else { 1.0 };
        let max_speed = self.max_speed();
        let step = Vector3::new(
            (scale * movement.x).clamp(-max_speed, max_speed),
            0.0,
            (scale * movement.z).clamp(-max_speed, max_speed),
        );
        if step.length_squared() > 0.0 {
            self.move_entity(caller, step);
            // Vanilla makeStepAlongTrack calls tryConsumeItems() for Hopper minecart
            if let MinecartKind::Hopper(minecart) = &self.kind {
                minecart.try_consume_items(&self.vehicle.entity);
            }
        }

        // Climb onto the higher end of a slope.
        let moved = entity.pos.load();
        let cell = |exit: Vector3<f64>| {
            exit.y != 0.0
                && moved.x.floor() as i32 - pos.0.x == exit.x as i32
                && moved.z.floor() as i32 - pos.0.z == exit.z as i32
        };
        if let Some(exit) = [exit0, exit1].into_iter().find(|exit| cell(*exit)) {
            entity.set_pos(Vector3::new(moved.x, moved.y + exit.y, moved.z));
        }

        movement = self.apply_natural_slowdown(movement);

        // Height lost on the rail becomes speed.
        let moved = entity.pos.load();
        if let (Some(old), Some(new)) = (old_rail_pos, rail_pos(world, moved)) {
            let speed =
                crate::entity::combat::calculate_minecart_slope_acceleration(old.y, new.y);
            let other_pow = movement.horizontal_length();
            if other_pow > 0.0 {
                let factor = (other_pow + speed) / other_pow;
                movement = movement.multiply(factor, 1.0, factor);
            }
            entity.set_pos(Vector3::new(moved.x, new.y, moved.z));
        }

        // Entering the next cell: head straight into it.
        let moved = entity.pos.load();
        let (xn, zn) = (moved.x.floor() as i32, moved.z.floor() as i32);
        movement = crate::entity::combat::calculate_minecart_next_cell_redirection(
            movement,
            xn,
            zn,
            pos.0.x,
            pos.0.z,
        );

        if power_track {
            movement = boost(world, pos, shape, movement);
        }
        self.fall_distance.store(0.0);
        self.store_movement(movement);
    }

    /// Vanilla `AbstractMinecart.comeOffTrack`.
    fn come_off_track(&self, caller: &dyn EntityBase, world: &World, pos: BlockPos) {
        let entity = &self.vehicle.entity;
        let mut movement = entity.velocity.load();
        if !entity.on_ground.load(Ordering::Relaxed) {
            movement.y -= self.get_gravity();
        }
        let max_speed = self.max_speed();
        movement.x = movement.x.clamp(-max_speed, max_speed);
        movement.z = movement.z.clamp(-max_speed, max_speed);
        if self.grounded(world, pos) {
            movement = movement * crate::entity::combat::MINECART_OFF_RAIL_GROUND_DRAG;
        }
        if movement.length_squared() > 0.0 {
            self.move_entity(caller, movement);
            // Pumpkin `move_entity` leaves the collided step as velocity.
            movement = entity.velocity.load();
        }
        let is_grounded = self.grounded(world, pos);
        if !is_grounded {
            movement = movement * f64::from(AIR_DRAG);
            if movement.y < 0.0 {
                let current_fall = self.fall_distance.load();
                self.fall_distance.store(current_fall - movement.y);
            }
        } else {
            let fallen = self.fall_distance.swap(0.0);
            if fallen >= crate::entity::combat::MINECART_TNT_FALL_EXPLOSION_MIN_DISTANCE {
                if let MinecartKind::Tnt(minecart) = &self.kind {
                    minecart.cause_fall_damage(&self.vehicle.entity, fallen);
                    if self.vehicle.entity.is_removed() {
                        return;
                    }
                }
            }
        }
        self.store_movement(movement);
    }

    /// `onGround`, or resting on a block the flag missed.
    fn grounded(&self, world: &World, pos: BlockPos) -> bool {
        let below = world.get_block(&BlockPos(Vector3::new(pos.0.x, pos.0.y - 1, pos.0.z)));
        self.vehicle.entity.on_ground.load(Ordering::Relaxed)
            || (below.id != Block::AIR.id
                && below.id != Block::WATER.id
                && below.id != Block::LAVA.id)
    }

    /// Vanilla `applyNaturalSlowdown` of each minecart kind. Drops vertical speed.
    fn apply_natural_slowdown(&self, movement: Vector3<f64>) -> Vector3<f64> {
        let entity = &self.vehicle.entity;
        match &self.kind {
            MinecartKind::Furnace(minecart) => minecart.velocity(entity, movement),
            _ if let Some(inventory) = self.container() => {
                container::velocity(entity, inventory, movement)
            }
            _ => {
                // Vanilla `getSlowdownFactor`
                let mut factor = if entity.has_passengers() { 0.997 } else { 0.96 };
                if entity.touching_water.load(Ordering::Relaxed) {
                    factor *= f64::from(0.95f32);
                }
                movement.multiply(factor, 0.0, factor)
            }
        }
    }

    fn store_movement(&self, mut movement: Vector3<f64>) {
        if movement.length() < STOP_SPEED {
            movement = Vector3::default();
        }
        self.vehicle.entity.velocity.store(movement);
    }

    /// face the movement, flip on reversal
    fn update_rotation(&self, start: Vector3<f64>, start_yaw: f32) {
        let entity = &self.vehicle.entity;
        let pos = entity.pos.load();
        let (x_diff, z_diff) = (start.x - pos.x, start.z - pos.z);
        let mut yaw = entity.yaw.load();
        if x_diff * x_diff + z_diff * z_diff > 0.001 {
            yaw = z_diff.atan2(x_diff).to_degrees() as f32;
            if self.flipped.load(Ordering::Relaxed) {
                yaw += 180.0;
            }
        }
        let rot_diff = wrap_degrees(yaw - start_yaw);
        if !(-170.0..170.0).contains(&rot_diff) {
            yaw += 180.0;
            self.flipped.fetch_xor(true, Ordering::Relaxed);
        }
        entity.pitch.store(0.0);
        entity.yaw.store(yaw % 360.0);
    }

    /// Vanilla `OldMinecartBehavior.pushAndPickupEntities`.
    fn push_and_pickup_entities(&self, caller: &dyn EntityBase) {
        let entity = &self.vehicle.entity;
        let world = entity.world.load();
        let hitbox = entity.bounding_box.load().expand(
            crate::entity::combat::MINECART_PUSH_HORIZONTAL_INFLATION,
            0.0,
            crate::entity::combat::MINECART_PUSH_HORIZONTAL_INFLATION,
        );
        let is_minecart = |other: &dyn EntityBase| other.cast_any().is::<Self>();

        if !(matches!(self.kind, MinecartKind::Rideable(_))
            && entity.velocity.load().horizontal_length_squared()
                >= crate::entity::combat::MINECART_PUSH_MIN_HORIZONTAL_SPEED_SQ)
        {
            for other in world.get_all_at_box(&hitbox) {
                if other.get_entity().entity_id != entity.entity_id
                    && !entity.has_passenger(other.get_entity().entity_id)
                    && other.is_pushable()
                    && is_minecart(other.as_ref())
                {
                    other.push(caller);
                }
            }
            return;
        }

        let pushable = velocity::pushable_entities(caller, &hitbox);
        for other in &pushable {
            let other_entity = other.get_entity();
            if other.get_player().is_none()
                && other_entity.entity_type.id != EntityType::IRON_GOLEM.id
                && !is_minecart(other.as_ref())
                && !entity.has_passengers()
                && !other.is_passenger()
            {
                // `startRiding` fails on boarding cooldown, and then nothing happens.
                if other_entity.riding_cooldown.load(Ordering::Relaxed) == 0
                    && self.vehicle.collide_entity(other_entity.entity_id)
                    && let Some(this) = world.get_entity_by_id(entity.entity_id)
                {
                    entity.add_passenger(this, other.clone());
                }
            } else if is_minecart(other.as_ref())
                || self.vehicle.collide_entity(other_entity.entity_id)
            {
                // A minecart fires the collision event in its own `push`.
                caller.push(other.as_ref());
            }
        }
    }

    /// Vanilla `AbstractMinecart.pushOtherMinecart`, old movement.
    fn push_other_minecart(&self, other: &Self, xa: f64, za: f64) {
        let this = &self.vehicle.entity;
        let that = &other.vehicle.entity;
        let (pos, other_pos) = (this.pos.load(), that.pos.load());
        let dir = Vector3::new(other_pos.x - pos.x, 0.0, other_pos.z - pos.z).normalize();
        let yaw = this.yaw.load().to_radians();
        let facing = Vector3::new(f64::from(yaw.cos()), 0.0, f64::from(yaw.sin())).normalize();
        if dir.dot(&facing).abs() < crate::entity::combat::MINECART_COLLISION_DIRECTION_DOT_THRESHOLD {
            return;
        }

        let movement = this.velocity.load();
        let other_movement = that.velocity.load();
        let slow_down = |entity: &Entity, velocity: Vector3<f64>, factor: f64| {
            entity
                .velocity
                .store(velocity.multiply(factor, 1.0, factor));
        };
        match (
            matches!(self.kind, MinecartKind::Furnace(_)),
            matches!(other.kind, MinecartKind::Furnace(_)),
        ) {
            (false, true) => {
                slow_down(this, movement, 0.2);
                this.push_velocity(other_movement.x - xa, other_movement.z - za);
                slow_down(that, other_movement, 0.95);
            }
            (true, false) => {
                slow_down(that, other_movement, 0.2);
                that.push_velocity(movement.x + xa, movement.z + za);
                slow_down(this, movement, 0.95);
            }
            _ => {
                #[expect(clippy::manual_midpoint, reason = "vanilla rounding")]
                let (xdd, zdd) = (
                    (other_movement.x + movement.x) / 2.0,
                    (other_movement.z + movement.z) / 2.0,
                );
                slow_down(this, movement, 0.2);
                this.push_velocity(xdd - xa, zdd - za);
                slow_down(that, other_movement, 0.2);
                that.push_velocity(xdd + xa, zdd + za);
            }
        }
    }

    /// Mirrors vanilla `activateMinecart` in `OldMinecartBehavior.java:59-61` and subclasses.
    fn activate_minecart(&self, powered: bool) {
        match &self.kind {
            MinecartKind::Hopper(minecart) => {
                minecart.set_enabled(!powered);
            }
            MinecartKind::CommandBlock(minecart) => {
                minecart.activate(
                    &self.vehicle.entity,
                    powered,
                    self.vehicle.entity.age.load(Ordering::Relaxed),
                );
            }
            MinecartKind::Tnt(minecart) => {
                if powered {
                    minecart.prime(
                        &self.vehicle.entity,
                        crate::entity::combat::MINECART_TNT_DEFAULT_FUSE,
                    );
                }
            }
            MinecartKind::Rideable(_) => {
                if powered {
                    if let Ok(passengers) = self.vehicle.entity.passengers.try_lock() {
                        let p_ids: Vec<i32> = passengers
                            .iter()
                            .map(|p| p.get_entity().entity_id)
                            .collect();
                        if !p_ids.is_empty() {
                            let world = self.vehicle.entity.world.load();
                            let vid = self.vehicle.entity.entity_id;
                            if let Some(v) = world.get_entity_by_id(vid) {
                                for pid in p_ids {
                                    v.get_entity().remove_passenger_sync(pid);
                                }
                            }
                        }
                    }
                    if self.vehicle.get_hurt_time() == 0 {
                        self.vehicle.set_hurt_dir(-self.vehicle.get_hurt_dir());
                        self.vehicle.set_hurt_time(10);
                        self.vehicle.set_damage(50.0);
                        self.vehicle.send_wobble_metadata();
                    }
                }
            }
            _ => {}
        }
    }

    const fn drop_item(&self) -> Option<&'static Item> {
        match &self.kind {
            MinecartKind::Chest(_) => Some(&Item::CHEST_MINECART),
            MinecartKind::Furnace(_) => Some(&Item::FURNACE_MINECART),
            MinecartKind::Hopper(_) => Some(&Item::HOPPER_MINECART),
            MinecartKind::Tnt(_) => Some(&Item::TNT_MINECART),
            MinecartKind::Rideable(_)
            | MinecartKind::CommandBlock(_)
            | MinecartKind::Spawner(_)
            | MinecartKind::Other => Some(&Item::MINECART),
        }
    }
}

impl EntityBase for MinecartEntity {
    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(state_id) = self.custom_display_block_state.load() {
            Self::write_display_state(state_id, nbt);
        }
        let display_offset = self.get_display_offset();
        if crate::entity::combat::should_serialize_minecart_display_offset(
            display_offset,
            self.get_default_display_offset(),
        ) {
            nbt.put_int("DisplayOffset", display_offset);
        }
        nbt.put_bool("FlippedRotation", self.is_flipped());
        nbt.put_bool("HasTicked", self.has_ticked());

        match &self.kind {
            MinecartKind::Chest(minecart) => minecart.write_nbt(nbt),
            MinecartKind::Furnace(minecart) => minecart.write_nbt(nbt),
            MinecartKind::Hopper(minecart) => minecart.write_nbt(nbt),
            MinecartKind::Tnt(minecart) => minecart.write_nbt(nbt),
            MinecartKind::CommandBlock(minecart) => minecart.write_nbt(nbt),
            MinecartKind::Spawner(minecart) => minecart.write_nbt(nbt),
            MinecartKind::Rideable(_) | MinecartKind::Other => {}
        }
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        if let Some(state_id) = Self::parse_display_state(nbt) {
            self.custom_display_block_state.store(Some(state_id));
        }
        if let Some(offset) = nbt.get_int("DisplayOffset") {
            self.custom_display_offset.store(Some(offset));
        }
        if let Some(flipped) = nbt.get_bool("FlippedRotation") {
            self.flipped.store(flipped, Ordering::Relaxed);
        }
        if let Some(has_ticked) = nbt.get_bool("HasTicked") {
            self.has_ticked.store(has_ticked, Ordering::Relaxed);
        }

        match &self.kind {
            MinecartKind::Chest(minecart) => minecart.read_nbt(nbt),
            MinecartKind::Furnace(minecart) => minecart.read_nbt(nbt),
            MinecartKind::Hopper(minecart) => minecart.read_nbt(nbt),
            MinecartKind::Tnt(minecart) => minecart.read_nbt(nbt),
            MinecartKind::CommandBlock(minecart) => {
                minecart.read_nbt(&self.vehicle.entity, nbt);
            }
            MinecartKind::Spawner(minecart) => minecart.read_nbt(nbt),
            MinecartKind::Rideable(_) | MinecartKind::Other => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        self.vehicle.tick();
        self.has_ticked.store(true, Ordering::Relaxed);
        let from = self.vehicle.entity.pos.load();
        if let MinecartKind::Furnace(minecart) = &self.kind {
            minecart.tick(&self.vehicle.entity);
        }
        if let MinecartKind::Hopper(minecart) = &self.kind {
            minecart.reset_consumed_item_this_frame();
        }
        if let MinecartKind::Spawner(minecart) = &self.kind {
            minecart.tick(&self.vehicle.entity);
        }

        let world = self.vehicle.entity.world.load();
        let start = self.vehicle.entity.pos.load();
        let start_yaw = self.vehicle.entity.yaw.load();
        let block_pos = rail_block_pos(&world, self.vehicle.entity.pos.load());
        let (block, state_id) = world.get_block_and_state_id(&block_pos);
        let is_powered_rail = block.id == Block::POWERED_RAIL.id;
        let is_activator_rail = block.id == Block::ACTIVATOR_RAIL.id;
        let rail_shape = rail_shape_of(block, state_id);

        let mut power_track = false;
        let mut halt_track = false;
        let mut activator_powered = false;

        if is_powered_rail {
            let props = PoweredRailLikeProperties::from_state_id(state_id);
            power_track = props.powered;
            halt_track = !props.powered;
        } else if is_activator_rail {
            let props = PoweredRailLikeProperties::from_state_id(state_id);
            activator_powered = props.powered;
        } else if block.id == Block::DETECTOR_RAIL.id
            && let Some(server) = world.server.upgrade()
        {
            world.block_registry.on_entity_collision(
                block,
                &world,
                self,
                &block_pos,
                world.get_block_state(&block_pos),
                &server,
            );
        }

        if let MinecartKind::Tnt(minecart) = &self.kind
            && minecart.tick(&self.vehicle.entity)
        {
            return;
        }

        match rail_shape {
            Some(shape) => {
                self.move_along_track(caller, &world, block_pos, shape, power_track, halt_track);
                if is_activator_rail {
                    self.activate_minecart(activator_powered);
                }
            }
            None => self.come_off_track(caller, &world, block_pos),
        }

        self.vehicle.entity.position_riders();

        self.update_rotation(start, start_yaw);
        self.push_and_pickup_entities(caller);

        // Vanilla `MinecartTNT.tick`, after the pushes.
        let velocity = self.vehicle.entity.velocity.load();
        if let MinecartKind::Tnt(minecart) = &self.kind
            && self
                .vehicle
                .entity
                .horizontal_collision
                .load(Ordering::Relaxed)
            && velocity.horizontal_length_squared()
                >= crate::entity::combat::MINECART_TNT_COLLISION_EXPLOSION_SPEED_THRESHOLD_SQ
        {
            minecart.explode(&self.vehicle.entity, velocity.horizontal_length_squared());
            return;
        }

        let attempted_to = self.vehicle.entity.pos.load();
        let move_result = self.vehicle.move_vehicle(from, attempted_to);
        let to = move_result.position;
        self.vehicle.entity.set_pos(to);

        if move_result.cancelled {
            self.vehicle
                .entity
                .velocity
                .store(Vector3::new(0.0, 0.0, 0.0));
            self.vehicle.entity.send_velocity();
        }

        if to != attempted_to {
            self.vehicle.entity.teleport(to, None, None, &world);
        }

        if attempted_to != from
            && let Ok(passengers) = self.vehicle.entity.passengers.try_lock()
        {
            for passenger in passengers.iter() {
                passenger.get_entity().set_pos(to);
            }
        }

        if let MinecartKind::Hopper(minecart) = &self.kind {
            minecart.try_consume_items(&self.vehicle.entity);
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.vehicle.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn get_vehicle_entity(&self) -> Option<&VehicleEntity> {
        Some(&self.vehicle)
    }

    fn get_motion_direction(&self) -> pumpkin_data::BlockDirection {
        self.get_motion_direction()
    }

    fn get_container_inventory(&self) -> Option<Arc<dyn Inventory>> {
        match &self.kind {
            MinecartKind::Chest(c) => Some(c.inventory().clone() as Arc<dyn Inventory>),
            MinecartKind::Hopper(h) => Some(h.inventory().clone() as Arc<dyn Inventory>),
            _ => None,
        }
    }

    fn is_pushable(&self) -> bool {
        true
    }

    fn get_gravity(&self) -> f64 {
        crate::entity::combat::get_minecart_gravity(
            self.vehicle.entity.touching_water.load(Ordering::Relaxed),
            self.vehicle.entity.has_no_gravity(),
        )
    }

    /// Bedrock minecart origin sits 0.35 above the Java one.
    fn bedrock_y_offset(&self) -> f64 {
        0.35
    }

    /// Vanilla `AbstractMinecart.push`.
    fn push(&self, entity: &dyn EntityBase) {
        let self_entity = self.get_entity();
        let other_entity = entity.get_entity();

        if self_entity.no_physics.load(Ordering::Relaxed)
            || other_entity.no_physics.load(Ordering::Relaxed)
            || self_entity.has_passenger(other_entity.entity_id)
        {
            return;
        }

        let dx = other_entity.pos.load().x - self_entity.pos.load().x;
        let dz = other_entity.pos.load().z - self_entity.pos.load().z;
        let Some((xa, za)) = crate::entity::combat::calculate_minecart_push_impulse(dx, dz) else {
            return;
        };
        // Plugin hook: only for pushes that happen.
        if !self.vehicle.collide_entity(other_entity.entity_id) {
            return;
        }

        if let Some(other) = entity.cast_any().downcast_ref::<Self>() {
            self.push_other_minecart(other, xa, za);
        } else {
            self_entity.push_velocity(-xa, -za);
            other_entity.push_velocity(
                xa * crate::entity::combat::MINECART_PASSENGER_PUSH_REDUCTION_FACTOR,
                za * crate::entity::combat::MINECART_PASSENGER_PUSH_REDUCTION_FACTOR,
            );
        }
    }

    fn is_collidable(&self, _entity: Option<Box<dyn EntityBase>>) -> bool {
        true
    }

    fn has_entity_collisions(&self) -> bool {
        true
    }

    /// Vanilla `AbstractBoat.canVehicleCollide`.
    fn can_collide_with(&self, other: &dyn EntityBase) -> bool {
        crate::entity::combat::can_vehicle_collide(
            other.is_collidable(None),
            other.is_pushable(),
            self.vehicle
                .entity
                .is_passenger_of_same_vehicle(other.get_entity()),
        )
    }

    fn init_data_tracker(&self) {
        self.vehicle.send_wobble_metadata();
        if let MinecartKind::Furnace(minecart) = &self.kind {
            minecart.init_data_tracker(&self.vehicle.entity);
        }
        if let MinecartKind::CommandBlock(minecart) = &self.kind {
            minecart.init_data_tracker(&self.vehicle.entity);
        }
    }

    fn can_hit(&self) -> bool {
        self.vehicle.entity.is_alive()
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        let creative = source
            .and_then(EntityBase::get_player)
            .or_else(|| cause.and_then(EntityBase::get_player))
            .is_some_and(|player| player.gamemode.load() == GameMode::Creative);

        // Vanilla MinecartTNT.java:68-75:
        // if (source.getDirectEntity() instanceof AbstractArrow projectile && projectile.isOnFire()) {
        //     DamageSource damageSource = this.damageSources().explosion(this, source.getEntity());
        //     this.explode(damageSource, projectile.getDeltaMovement().lengthSqr());
        // }
        if let MinecartKind::Tnt(minecart) = &self.kind {
            let direct_is_flaming_arrow = source
                .map(|s| {
                    let ent = s.get_entity();
                    let is_arrow = ent.entity_type == &EntityType::ARROW
                        || ent.entity_type == &EntityType::SPECTRAL_ARROW;
                    let is_fire = ent.is_on_fire() || ent.fire_ticks.load(Ordering::Relaxed) > 0;
                    is_arrow && is_fire
                })
                .unwrap_or(false);

            if direct_is_flaming_arrow {
                let projectile_speed_squared = source
                    .map(|entity| entity.get_entity().velocity.load().length_squared())
                    .unwrap_or_default();
                let source_id = cause.or(source).map(|e| e.get_entity().entity_id);
                minecart.explode_with_source(
                    &self.vehicle.entity,
                    projectile_speed_squared,
                    source_id,
                );
                if self.vehicle.entity.is_removed() {
                    return true;
                }
            }
        }

        // Vanilla VehicleEntity.java:49-56 & MinecartTNT.java:78-88, 210-218
        let direct_on_fire = source
            .map(|s| s.get_entity().is_on_fire() || s.get_entity().fire_ticks.load(Ordering::Relaxed) > 0)
            .unwrap_or(false);
        let is_tnt = matches!(&self.kind, MinecartKind::Tnt(_));
        let is_igniting = is_tnt
            && crate::entity::combat::is_damage_source_igniting_tnt(damage_type, direct_on_fire);

        let current_damage = self.vehicle.get_damage();
        let (_new_damage, will_break_step) = crate::entity::combat::calculate_vehicle_damage_step(
            current_damage,
            amount,
            creative,
        );
        let should_source_destroy = is_igniting;
        let will_break = self.vehicle.entity.is_alive() && (will_break_step || should_source_destroy);

        if let MinecartKind::Tnt(minecart) = &self.kind
            && will_break
            && (!creative || should_source_destroy)
        {
            let velocity = self.vehicle.entity.velocity.load();
            let speed_squared = velocity.x.mul_add(velocity.x, velocity.z * velocity.z);
            if crate::entity::combat::should_tnt_minecart_prime_on_destroy(is_igniting, speed_squared) {
                self.vehicle.apply_damage_wobble(amount);
                if minecart.get_fuse() < 0 {
                    let rand_a = rand::rng().random_range(0..crate::entity::combat::MINECART_TNT_RANDOM_FUSE_RANGE);
                    let rand_b = rand::rng().random_range(0..crate::entity::combat::MINECART_TNT_RANDOM_FUSE_RANGE);
                    let fuse = crate::entity::combat::calculate_tnt_minecart_destroyed_fuse(rand_a, rand_b);
                    let source_id = cause.or(source).map(|e| e.get_entity().entity_id);
                    minecart.prime_with_fuse_and_source(&self.vehicle.entity, fuse, source_id);
                }
                return true;
            }
        }

        let damaged = self.vehicle.damage_with_context(amount, source.or(cause));

        if will_break && !creative && self.vehicle.entity.is_removed() {
            let world = self.vehicle.entity.world.load();
            if world.level_info.load().game_rules.entity_drops {
                let position = self.vehicle.entity.block_pos.load();
                if let Some(container) = self.container()
                    && container.claim_drops()
                {
                    container.unpack_loot();
                    let inventory: Arc<dyn Inventory> = container.clone();
                    world.scatter_inventory(&position, &inventory);
                }
                if let Some(item) = self.drop_item() {
                    world.drop_stack(&position, ItemStack::new(1, item));
                }
            }
        }

        damaged
    }

    fn interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        match &self.kind {
            MinecartKind::Chest(minecart) => {
                let custom_name = self.vehicle.entity.custom_name.load().as_ref().clone();
                minecart.interact(custom_name, player);
                true
            }
            MinecartKind::Furnace(minecart) => {
                minecart.interact(&self.vehicle.entity, player, item_stack)
            }
            MinecartKind::Hopper(minecart) => {
                let custom_name = self.vehicle.entity.custom_name.load().as_ref().clone();
                minecart.interact(custom_name, player);
                true
            }
            MinecartKind::Rideable(_) => RideableMinecart::interact(&self.vehicle.entity, player),
            MinecartKind::CommandBlock(minecart) => {
                minecart.interact(&self.vehicle.entity, player)
            }
            MinecartKind::Spawner(_) | MinecartKind::Tnt(_) | MinecartKind::Other => false,
        }
    }

    fn get_command_block_success_count(&self) -> Option<u32> {
        match &self.kind {
            MinecartKind::CommandBlock(cmd) => Some(cmd.get_success_count()),
            _ => None,
        }
    }

    fn get_minecart_display_block_state(&self) -> Option<BlockStateId> {
        self.get_display_block_state()
    }

    fn get_minecart_display_offset(&self) -> Option<i32> {
        Some(self.get_display_offset())
    }

    fn set_minecart_display_block(
        &self,
        state: Option<BlockStateId>,
        offset: Option<i32>,
    ) {
        self.custom_display_block_state.store(state);
        if let Some(off) = offset {
            self.custom_display_offset.store(Some(off));
        }
        self.send_display_metadata();
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_nbt::compound::NbtCompound;

    #[test]
    fn test_display_state_parsing_and_writing() {
        // Block without properties (e.g. redstone_block)
        let redstone = Block::REDSTONE_BLOCK.default_state.id;
        let mut nbt = NbtCompound::new();
        MinecartEntity::write_display_state(redstone, &mut nbt);

        let parsed = MinecartEntity::parse_display_state(&nbt);
        assert_eq!(parsed, Some(redstone));

        // Block with properties (e.g. chest facing north)
        let chest_state = Block::CHEST
            .state_from_properties(&[("facing", "north")])
            .map(|s| s.id)
            .unwrap_or(Block::CHEST.default_state.id);
        let mut chest_nbt = NbtCompound::new();
        MinecartEntity::write_display_state(chest_state, &mut chest_nbt);

        let parsed_chest = MinecartEntity::parse_display_state(&chest_nbt);
        assert_eq!(parsed_chest, Some(chest_state));

        // Legacy string format
        let mut str_nbt = NbtCompound::new();
        str_nbt.put_string("DisplayState", "minecraft:diamond_block".to_string());
        assert_eq!(
            MinecartEntity::parse_display_state(&str_nbt),
            Some(Block::DIAMOND_BLOCK.default_state.id)
        );

        // Legacy int format
        let mut int_nbt = NbtCompound::new();
        int_nbt.put_int("DisplayState", Block::GOLD_BLOCK.default_state.id.as_u16() as i32);
        assert_eq!(
            MinecartEntity::parse_display_state(&int_nbt),
            Some(Block::GOLD_BLOCK.default_state.id)
        );
    }

    #[test]
    fn test_display_offset_and_flags() {
        assert_eq!(
            crate::entity::combat::get_minecart_default_display_offset(EntityType::CHEST_MINECART.id),
            8
        );
        assert_eq!(
            crate::entity::combat::get_minecart_default_display_offset(EntityType::HOPPER_MINECART.id),
            1
        );
        assert_eq!(
            crate::entity::combat::get_minecart_default_display_offset(EntityType::MINECART.id),
            6
        );
        assert!(!crate::entity::combat::should_serialize_minecart_display_offset(6, 6));
        assert!(crate::entity::combat::should_serialize_minecart_display_offset(10, 6));
    }
}
