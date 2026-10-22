use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityType;
use pumpkin_data::translation;
use pumpkin_inventory::Inventory;
use pumpkin_nbt::NbtCompound;
use pumpkin_util::GameMode;
use pumpkin_util::text::TextComponent;

use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;

use pumpkin_data::damage::DamageType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag;
use pumpkin_protocol::java::client::play::Metadata;

use pumpkin_util::math::vector3::Vector3;

use crate::entity::vehicle::vehicle::VehicleEntity;

pub mod block_effects;
pub mod chest;
pub mod container;
pub mod dismount;
pub mod passengers;
pub mod physics;

use block_effects::BoatMovement;
use chest::ChestBoat;
use physics::BoatStatus;

const TIME_TO_EJECT: f32 = 60.0;

pub struct BoatEntity {
    pub vehicle: VehicleEntity,
    kind: BoatKind,
    ticks_underwater: AtomicCell<f32>,
    status: AtomicCell<Option<BoatStatus>>,
    water_level: AtomicCell<f64>,
    land_friction: AtomicCell<f32>,
    last_yd: AtomicCell<f64>,
    movements: Mutex<VecDeque<BoatMovement>>,
    left_paddle_moving: AtomicBool,
    right_paddle_moving: AtomicBool,
}

pub enum BoatKind {
    Normal,
    Chest(ChestBoat),
}

impl BoatEntity {
    pub fn new(entity: Entity) -> Self {
        let kind = match entity.entity_type.id {
            id if id == EntityType::ACACIA_CHEST_BOAT.id
                || id == EntityType::BIRCH_CHEST_BOAT.id
                || id == EntityType::DARK_OAK_CHEST_BOAT.id
                || id == EntityType::JUNGLE_CHEST_BOAT.id
                || id == EntityType::MANGROVE_CHEST_BOAT.id
                || id == EntityType::OAK_CHEST_BOAT.id
                || id == EntityType::PALE_OAK_CHEST_BOAT.id
                || id == EntityType::SPRUCE_CHEST_BOAT.id
                || id == EntityType::BAMBOO_CHEST_RAFT.id
                || id == EntityType::CHERRY_CHEST_BOAT.id
                || id == EntityType::POPLAR_CHEST_BOAT.id =>
            {
                BoatKind::Chest(ChestBoat::new())
            }
            _ => BoatKind::Normal,
        };
        Self {
            vehicle: VehicleEntity::new(entity),
            kind,
            ticks_underwater: AtomicCell::new(0.0),
            status: AtomicCell::new(None),
            water_level: AtomicCell::new(0.0),
            land_friction: AtomicCell::new(0.0),
            last_yd: AtomicCell::new(0.0),
            movements: Mutex::new(VecDeque::new()),
            left_paddle_moving: AtomicBool::new(false),
            right_paddle_moving: AtomicBool::new(false),
        }
    }

    pub fn set_paddles(&self, left: bool, right: bool) {
        self.left_paddle_moving.store(left, Ordering::Relaxed);
        self.right_paddle_moving.store(right, Ordering::Relaxed);

        self.vehicle.entity.send_meta_data(
            &[
                Metadata::new(pumpkin_data::tracked_data::boat::ID_PADDLE_LEFT, left),
                Metadata::new(pumpkin_data::tracked_data::boat::ID_PADDLE_RIGHT, right),
            ],
            None,
        );
    }

    fn send_wobble_metadata(&self) {
        self.vehicle.send_wobble_metadata();
    }

    pub fn open_inventory(&self, player: &Arc<Player>) -> bool {
        if let BoatKind::Chest(boat) = &self.kind {
            let custom_name = self.vehicle.entity.custom_name.load().as_ref().clone();
            return boat.interact(custom_name, self.get_chest_boat_translation(), player);
        }
        false
    }

    fn get_chest_boat_translation(&self) -> TextComponent {
        match self.get_entity().entity_type.id {
            id if id == EntityType::ACACIA_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_ACACIA_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_ACACIA_NAME,
            ),
            id if id == EntityType::BIRCH_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_BIRCH_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_BIRCH_NAME,
            ),
            id if id == EntityType::DARK_OAK_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_DARK_OAK_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_BIG_OAK_NAME,
            ),
            id if id == EntityType::JUNGLE_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_JUNGLE_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_JUNGLE_NAME,
            ),
            id if id == EntityType::MANGROVE_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_MANGROVE_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_MANGROVE_NAME,
            ),
            id if id == EntityType::OAK_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_OAK_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_OAK_NAME,
            ),
            id if id == EntityType::PALE_OAK_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_PALE_OAK_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_PALE_OAK_NAME,
            ),
            id if id == EntityType::SPRUCE_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_SPRUCE_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_SPRUCE_NAME,
            ),
            id if id == EntityType::BAMBOO_CHEST_RAFT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_BAMBOO_CHEST_RAFT,
                translation::bedrock::ITEM_CHEST_BOAT_BAMBOO_NAME,
            ),
            id if id == EntityType::CHERRY_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_CHERRY_CHEST_BOAT,
                translation::bedrock::ITEM_CHEST_BOAT_CHERRY_NAME,
            ),
            id if id == EntityType::POPLAR_CHEST_BOAT.id => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_POPLAR_CHEST_BOAT,
                // Missing Poplar Bedrock translation
                translation::bedrock::ENTITY_CHEST_BOAT_NAME,
            ),
            _ => pumpkin_macros::translate_cross!(
                translation::java::ENTITY_MINECRAFT_CHEST_BOAT,
                translation::bedrock::ENTITY_CHEST_BOAT_NAME,
            ),
        }
    }

    fn tick_underwater(&self, status: BoatStatus) {
        let ticks = if status.is_underwater() {
            self.ticks_underwater.load() + 1.0
        } else {
            0.0
        };
        self.ticks_underwater.store(ticks);
        if ticks < TIME_TO_EJECT {
            return;
        }

        let passengers = self
            .vehicle
            .entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for passenger in passengers.iter().rev() {
            self.vehicle
                .entity
                .remove_passenger(passenger.get_entity().entity_id);
        }
    }

    fn is_client_controlled(&self) -> bool {
        self.vehicle
            .entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .is_some_and(|passenger| passenger.get_player().is_some())
    }

    fn get_max_passengers(&self) -> usize {
        if self.is_chest_boat() { 1 } else { 2 }
    }

    fn is_chest_boat(&self) -> bool {
        tag::Item::MINECRAFT_CHEST_BOATS
            .0
            .contains(&self.vehicle.entity.entity_type.resource_name)
    }

    fn tick_passengers(&self) {
        let entity = &self.vehicle.entity;
        let passengers = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for passenger in passengers {
            if let Some(player) = passenger.get_player() {
                player.ride_tick();
            }
            if entity.has_passenger(passenger.get_entity().entity_id) {
                self.position_rider(passenger.as_ref());
            }
        }
    }

    pub(in crate::entity) fn can_push(&self, other: &Entity, is_boat: bool) -> bool {
        let bounds = self.vehicle.entity.bounding_box.load();
        let other_bottom = other.bounding_box.load().min.y;
        if is_boat {
            other_bottom < bounds.max.y
        } else {
            other_bottom <= bounds.min.y
        }
    }
}

impl EntityBase for BoatEntity {
    fn get_entity(&self) -> &Entity {
        &self.vehicle.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn get_vehicle_entity(&self) -> Option<&VehicleEntity> {
        Some(&self.vehicle)
    }
    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        let status = self.get_status();
        let old_status = self.status.swap(Some(status));
        self.tick_underwater(status);
        self.vehicle.tick();
        let entity = &self.vehicle.entity;
        entity.tick(caller, server);
        if entity.is_removed() {
            return;
        }

        // Vanilla leaves ridden boat movement to its controlling player's client.
        if self.is_client_controlled() {
            entity.velocity.store(Vector3::default());
        } else {
            if self.left_paddle_moving.load(Ordering::Relaxed)
                || self.right_paddle_moving.load(Ordering::Relaxed)
            {
                self.set_paddles(false, false);
            }
            let surface_offset = self.float_boat(old_status, status);
            self.move_boat(caller, surface_offset);
        }
        self.apply_effects_from_blocks();
        entity.tick_block_collisions(caller);
        self.tick_passengers();
    }

    fn init_data_tracker(&self) {
        self.send_wobble_metadata();
    }

    fn can_hit(&self) -> bool {
        self.vehicle.entity.is_alive()
    }

    fn is_collidable(&self, _entity: Option<Box<dyn EntityBase>>) -> bool {
        true
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        let is_creative = source
            .and_then(EntityBase::get_player)
            .is_some_and(|player| player.gamemode.load() == GameMode::Creative);
        let will_break = self.vehicle.entity.is_alive()
            && (is_creative || self.vehicle.get_damage() + amount * 10.0 > 40.0);

        let damaged = self.vehicle.damage_with_context(amount, source);

        if let BoatKind::Chest(boat) = &self.kind
            && will_break
            && self.vehicle.entity.is_removed()
        {
            let world = self.vehicle.entity.world.load();
            if world.level_info.load().game_rules.entity_drops {
                let position = self.vehicle.entity.block_pos.load();
                let inventory: Arc<dyn Inventory> = boat.inventory().clone();
                world.scatter_inventory(&position, &inventory);
            }
        }
        damaged
    }

    fn interact(&self, player: &Arc<Player>, _item_stack: &mut ItemStack) -> bool {
        if player.get_entity().is_sneaking() {
            // Try opening inventory (for chest boats)
            return self.open_inventory(player);
        }

        if self.ticks_underwater.load() >= TIME_TO_EJECT || self.is_eye_in_water() {
            return false;
        }

        if self
            .vehicle
            .entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            >= self.get_max_passengers()
        {
            return false;
        }

        if player.get_entity().has_vehicle() {
            return false;
        }

        let world = self.vehicle.entity.world.load();
        let Some(vehicle) = world.get_entity_by_id(self.vehicle.entity.entity_id) else {
            return false;
        };

        let Some(passenger) = world.get_player_by_id(player.entity_id()) else {
            return false;
        };

        self.vehicle
            .entity
            .add_passenger(vehicle, passenger as Arc<dyn EntityBase>);

        self.vehicle.entity.has_passenger(player.entity_id())
    }

    fn set_paddle_state(&self, left: bool, right: bool) {
        self.set_paddles(left, right);
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    fn is_pushable(&self) -> bool {
        true
    }

    fn get_gravity(&self) -> f64 {
        if self.vehicle.entity.has_no_gravity() {
            0.0
        } else {
            physics::DEFAULT_GRAVITY
        }
    }
    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        if let BoatKind::Chest(boat) = &self.kind {
            boat.write_nbt(nbt);
        }
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        if let BoatKind::Chest(boat) = &self.kind {
            boat.read_nbt(nbt);
        }
    }
}
