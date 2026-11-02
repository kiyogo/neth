use std::sync::atomic::{AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{Entity, EntityBase, player::Player};

/// Vanilla `MinecartFurnace`: powered by coal/charcoal, pushes minecarts along rails.
pub(super) struct FurnaceMinecart {
    fuel: AtomicI32,
    push: AtomicCell<Vector3<f64>>,
}

impl FurnaceMinecart {
    pub const FUEL_PER_ITEM: i32 = 3_600;
    pub const MAX_FUEL: i32 = 32_000;
    pub const DEFAULT_FUEL: i32 = 0;
    pub const DEFAULT_PUSH: Vector3<f64> = Vector3::new(0.0, 0.0, 0.0);

    pub(super) const fn new() -> Self {
        Self {
            fuel: AtomicI32::new(Self::DEFAULT_FUEL),
            push: AtomicCell::new(Self::DEFAULT_PUSH),
        }
    }

    fn set_fueled(entity: &Entity, fueled: bool) {
        entity.set_synced_data(
            pumpkin_data::tracked_data::furnace_minecart::ID_FUEL,
            fueled,
        );
    }

    pub(super) fn has_fuel(&self) -> bool {
        self.fuel.load(Ordering::Relaxed) > 0
    }

    pub(super) fn get_fuel(&self) -> i32 {
        self.fuel.load(Ordering::Relaxed)
    }

    pub(super) fn get_push(&self) -> Vector3<f64> {
        self.push.load()
    }

    pub(super) fn set_push(&self, push: Vector3<f64>) {
        self.push.store(push);
    }

    /// Vanilla `MinecartFurnace.tick()`: decays fuel by 1 tick, resets push when empty, spawns smoke.
    pub(super) fn tick(&self, entity: &Entity) {
        let fuel = self.fuel.load(Ordering::Relaxed);
        if fuel > 0 {
            let remaining = fuel - 1;
            self.fuel.store(remaining, Ordering::Relaxed);
            if remaining <= 0 {
                self.push.store(Self::DEFAULT_PUSH);
                Self::set_fueled(entity, false);
            }
        } else {
            self.push.store(Self::DEFAULT_PUSH);
            Self::set_fueled(entity, false);
        }

        if self.has_fuel() && rand::rng().random_range(0..4) == 0 {
            let mut pos = entity.pos.load();
            pos.y += 0.8;
            entity.world.load().spawn_particle(
                pos,
                Vector3::new(0.0, 0.0, 0.0),
                0.0,
                1,
                Particle::LargeSmoke,
            );
        }
    }

    /// Vanilla `MinecartFurnace.applyNaturalSlowdown()`:
    /// Applies 0.8 dampening + push along rail, then minecart natural slowdown (0.96 on land, 0.1 * 0.95 in water).
    pub(super) fn velocity(&self, entity: &Entity, velocity: Vector3<f64>) -> Vector3<f64> {
        let in_water = entity.touching_water.load(Ordering::Relaxed);
        let (new_push, new_movement) = calculate_furnace_natural_slowdown(
            self.push.load(),
            velocity,
            in_water,
        );
        self.push.store(new_push);
        new_movement
    }

    /// Vanilla `MinecartFurnace.addFuel(Vec3 interactingPos, ItemStack itemStack)`:
    /// Only updates fuel and push when the item is valid fuel and fuel won't exceed MAX_FUEL (32000).
    pub(super) fn add_fuel(
        &self,
        entity: &Entity,
        interacting_pos: Vector3<f64>,
        item_stack: &ItemStack,
    ) -> bool {
        let fuel = self.fuel.load(Ordering::Relaxed);
        if item_stack
            .get_item()
            .has_tag(&tag::Item::MINECRAFT_FURNACE_MINECART_FUEL)
            && fuel + Self::FUEL_PER_ITEM <= Self::MAX_FUEL
        {
            let new_fuel = fuel + Self::FUEL_PER_ITEM;
            self.fuel.store(new_fuel, Ordering::Relaxed);
            if new_fuel > 0 {
                let cart_pos = entity.pos.load();
                self.push.store(Vector3::new(
                    cart_pos.x - interacting_pos.x,
                    0.0,
                    cart_pos.z - interacting_pos.z,
                ));
                Self::set_fueled(entity, true);
            }
            true
        } else {
            false
        }
    }

    /// Vanilla `MinecartFurnace.interact()`: consumes 1 fuel item and sets push direction away from player.
    pub(super) fn interact(
        &self,
        entity: &Entity,
        player: &Player,
        item_stack: &mut ItemStack,
    ) -> bool {
        let player_pos = player.get_entity().pos.load();
        if self.add_fuel(entity, player_pos, item_stack) {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
        }
        true
    }

    pub(super) fn init_data_tracker(&self, entity: &Entity) {
        Self::set_fueled(entity, self.has_fuel());
    }

    pub(super) fn write_nbt(&self, nbt: &mut NbtCompound) {
        let push = self.push.load();
        nbt.put_double("PushX", push.x);
        nbt.put_double("PushZ", push.z);
        nbt.put_short("Fuel", self.fuel.load(Ordering::Relaxed) as i16);
    }

    pub(super) fn read_nbt(&self, nbt: &NbtCompound) {
        self.push.store(Vector3::new(
            nbt.get_double("PushX").unwrap_or(0.0),
            0.0,
            nbt.get_double("PushZ").unwrap_or(0.0),
        ));
        self.fuel.store(
            i32::from(nbt.get_short("Fuel").unwrap_or(0)),
            Ordering::Relaxed,
        );
    }
}

/// Vanilla `MinecartFurnace.calculateNewPushAlong(Vec3 deltaMovement)`.
/// Projects the push vector onto the current movement direction along track.
#[must_use]
pub fn calculate_new_push_along(push: Vector3<f64>, delta_movement: Vector3<f64>) -> Vector3<f64> {
    let push_h_sq = push.x * push.x + push.z * push.z;
    let move_h_sq = delta_movement.x * delta_movement.x + delta_movement.z * delta_movement.z;
    if push_h_sq > 1.0e-4 && move_h_sq > 0.001 {
        let move_h = Vector3::new(delta_movement.x, 0.0, delta_movement.z);
        let move_len = move_h.length();
        if move_len > 0.0 {
            let move_dir = move_h / move_len;
            let push_len = (push.x * push.x + push.y * push.y + push.z * push.z).sqrt();
            let dot = push.dot(&move_h);
            let scale = if dot < 0.0 {
                -push_len
            } else if dot > 0.0 {
                push_len
            } else {
                0.0
            };
            move_dir * scale
        } else {
            push
        }
    } else {
        push
    }
}

/// Vanilla `MinecartFurnace.applyNaturalSlowdown(Vec3 deltaMovement)`.
/// Returns `(updated_push, new_delta_movement)`.
#[must_use]
pub fn calculate_furnace_natural_slowdown(
    mut push: Vector3<f64>,
    delta_movement: Vector3<f64>,
    in_water: bool,
) -> (Vector3<f64>, Vector3<f64>) {
    let push_len_sq = push.x * push.x + push.y * push.y + push.z * push.z;
    let mut new_delta_movement = if push_len_sq > 1.0e-7 {
        push = calculate_new_push_along(push, delta_movement);
        let mut m = Vector3::new(delta_movement.x * 0.8, 0.0, delta_movement.z * 0.8).add(&push);
        if in_water {
            m = m * 0.1;
        }
        m
    } else {
        delta_movement.multiply(0.98, 0.0, 0.98)
    };

    // super.applyNaturalSlowdown:
    // slowdownFactor = 0.96 (no passengers)
    new_delta_movement = new_delta_movement.multiply(0.96, 0.0, 0.96);
    if in_water {
        new_delta_movement = new_delta_movement * f64::from(0.95f32);
    }

    (push, new_delta_movement)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn furnace_minecart_default_state() {
        let cart = FurnaceMinecart::new();
        assert_eq!(cart.get_fuel(), 0);
        assert!(!cart.has_fuel());
        assert_eq!(cart.get_push(), Vector3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn furnace_minecart_nbt_read_write() {
        let cart = FurnaceMinecart::new();
        cart.fuel.store(1800, Ordering::Relaxed);
        cart.set_push(Vector3::new(0.5, 0.0, -0.25));

        let mut nbt = NbtCompound::new();
        cart.write_nbt(&mut nbt);
        assert_eq!(nbt.get_short("Fuel"), Some(1800));
        assert_eq!(nbt.get_double("PushX"), Some(0.5));
        assert_eq!(nbt.get_double("PushZ"), Some(-0.25));

        let cart2 = FurnaceMinecart::new();
        cart2.read_nbt(&nbt);
        assert_eq!(cart2.get_fuel(), 1800);
        assert_eq!(cart2.get_push().x, 0.5);
        assert_eq!(cart2.get_push().z, -0.25);
    }

    #[test]
    fn furnace_minecart_projection_along_movement() {
        let push = Vector3::new(1.0, 0.0, 0.0);
        let movement = Vector3::new(0.2, 0.0, 0.0);
        let projected = calculate_new_push_along(push, movement);
        assert!((projected.x - 1.0).abs() < 1e-6);
        assert_eq!(projected.z, 0.0);

        // Turn around a corner onto Z axis:
        let movement_z = Vector3::new(0.0, 0.0, 0.3);
        let projected_z = calculate_new_push_along(push, movement_z);
        // Push aligns with Z axis because dot product with (0,0) is 0 -> maintains length along new axis
        assert_eq!(projected_z.y, 0.0);
    }

    #[test]
    fn furnace_minecart_slowdown_with_and_without_push() {
        // Without push: 0.98 * 0.96 = 0.9408
        let delta = Vector3::new(0.2, 0.0, 0.0);
        let (push0, res0) = calculate_furnace_natural_slowdown(Vector3::new(0.0, 0.0, 0.0), delta, false);
        assert_eq!(push0, Vector3::new(0.0, 0.0, 0.0));
        assert!((res0.x - 0.2 * 0.98 * 0.96).abs() < 1e-6);

        // In water without push: 0.2 * 0.98 * 0.96 * 0.95
        let (_, res_water) = calculate_furnace_natural_slowdown(Vector3::new(0.0, 0.0, 0.0), delta, true);
        assert!((res_water.x - 0.2 * 0.98 * 0.96 * 0.95).abs() < 1e-6);
    }
}
