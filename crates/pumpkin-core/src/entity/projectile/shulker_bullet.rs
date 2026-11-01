use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;
use pumpkin_nbt::compound::NbtCompound;

use crossbeam::atomic::AtomicCell;
use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::particle::Particle;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::mob::shulker::Axis;
use crate::entity::{Entity, EntityBase};
use crate::server::Server;

// Direction ordinal constants
const DIR_DOWN: u8 = 0;
const DIR_UP: u8 = 1;
const DIR_NORTH: u8 = 2;
const DIR_SOUTH: u8 = 3;
const DIR_WEST: u8 = 4;
const DIR_EAST: u8 = 5;
const DIR_NONE: u8 = 255;

const SPEED: f64 = 0.15;

pub struct ShulkerBulletEntity {
    pub entity: Entity,
    pub owner_id: i32,
    owner_uuid: Mutex<Option<Uuid>>,
    /// Entity id of the final target; -1 = no target, 0+ = valid target
    target_id: AtomicI32,
    target_uuid: Mutex<Option<Uuid>>,
    /// Current movement direction (direction ordinal, or `DIR_NONE`)
    current_dir: AtomicU8,
    /// How many ticks remain on the current axis segment
    flight_steps: AtomicI32,
    /// Steering velocity components (updated each tick)
    target_delta_x: AtomicCell<f64>,
    target_delta_y: AtomicCell<f64>,
    target_delta_z: AtomicCell<f64>,
    has_hit: AtomicBool,
    /// Tick counter; bullet is discarded after ~150 ticks if it never reaches its target
    age: AtomicI32,
}

use std::sync::atomic::AtomicU8;

impl ShulkerBulletEntity {
    pub fn new(
        owner: &Entity,
        target_id: i32,
        target_pos: Vector3<f64>,
        invalid_axis: Axis,
    ) -> Self {
        let world = owner.world.load();

        // Position at the center of the shulker's bounding box
        let bb = owner.bounding_box.load();
        let origin = Vector3::new(
            f64::midpoint(bb.min.x, bb.max.x),
            f64::midpoint(bb.min.y, bb.max.y),
            f64::midpoint(bb.min.z, bb.max.z),
        );

        let target_uuid = if target_id >= 0 {
            world.get_entity_by_id(target_id).map(|e| e.get_entity().entity_uuid)
        } else {
            None
        };

        let entity = Entity::from_uuid(
            Uuid::new_v4(),
            (*world).clone(),
            origin,
            &EntityType::SHULKER_BULLET,
        );

        let bullet = Self {
            entity,
            owner_id: owner.entity_id,
            owner_uuid: Mutex::new(Some(owner.entity_uuid)),
            target_id: AtomicI32::new(target_id),
            target_uuid: Mutex::new(target_uuid),
            current_dir: AtomicU8::new(DIR_UP),
            flight_steps: AtomicI32::new(0),
            target_delta_x: AtomicCell::new(0.0),
            target_delta_y: AtomicCell::new(0.0),
            target_delta_z: AtomicCell::new(0.0),
            has_hit: AtomicBool::new(false),
            age: AtomicI32::new(0),
        };

        // Prime target_delta and flight_steps so movement begins on tick 1.
        let avoid_axis = invalid_axis as u8;
        bullet.select_next_dir(avoid_axis, Some(target_pos));

        bullet
    }

    #[allow(clippy::similar_names)]
    fn select_next_dir(&self, avoid_axis: u8, target_pos_opt: Option<Vector3<f64>>) {
        const Y_OFFSET: f64 = 0.5;

        let pos = self.entity.pos.load();
        let cur_bp = self.entity.block_pos.load();
        let world = self.entity.world.load();

        // Compute the target block position
        let tbp = target_pos_opt.map_or_else(
            || BlockPos::new(cur_bp.0.x, cur_bp.0.y - 1, cur_bp.0.z), // Default: below
            |tp| {
                BlockPos::new(
                    // Target provided
                    tp.x.floor() as i32,
                    (tp.y + Y_OFFSET).floor() as i32,
                    tp.z.floor() as i32,
                )
            },
        );

        // ... and the "steering" target point.
        let target_x = f64::from(tbp.0.x) + 0.5;
        let target_y = f64::from(tbp.0.y) + Y_OFFSET;
        let target_z = f64::from(tbp.0.z) + 0.5;

        // If target block center is more than 2 blocks from bullet -> pick axis step.
        let tbp_cx = f64::from(tbp.0.x) + 0.5;
        let tbp_cy = f64::from(tbp.0.y) + 0.5;
        let tbp_cz = f64::from(tbp.0.z) + 0.5;
        let center_dist_sq =
            (tbp_cx - pos.x).powi(2) + (tbp_cy - pos.y).powi(2) + (tbp_cz - pos.z).powi(2);
        let target_is_far = center_dist_sq > 4.0;

        let (chosen_tx, chosen_ty, chosen_tz, selected_dir) = if target_is_far {
            // Pick an axis-aligned direction toward the target
            let mut options: Vec<u8> = Vec::new();

            if avoid_axis != 0 {
                if cur_bp.0.x < tbp.0.x && is_empty_block(&world, &cur_bp, DIR_EAST) {
                    options.push(DIR_EAST);
                } else if cur_bp.0.x > tbp.0.x && is_empty_block(&world, &cur_bp, DIR_WEST) {
                    options.push(DIR_WEST);
                }
            }
            if avoid_axis != 1 {
                if cur_bp.0.y < tbp.0.y && is_empty_block(&world, &cur_bp, DIR_UP) {
                    options.push(DIR_UP);
                } else if cur_bp.0.y > tbp.0.y && is_empty_block(&world, &cur_bp, DIR_DOWN) {
                    options.push(DIR_DOWN);
                }
            }
            if avoid_axis != 2 {
                if cur_bp.0.z < tbp.0.z && is_empty_block(&world, &cur_bp, DIR_SOUTH) {
                    options.push(DIR_SOUTH);
                } else if cur_bp.0.z > tbp.0.z && is_empty_block(&world, &cur_bp, DIR_NORTH) {
                    options.push(DIR_NORTH);
                }
            }

            // If options empty, pick a random direction (up to 5 tries for an
            // unobstructed one); ignore avoidAxis in this fallback.
            let sel = if options.is_empty() {
                let mut r = Self::random_dir();
                for _ in 0..4 {
                    if is_empty_block(&world, &cur_bp, r) {
                        break;
                    }
                    r = Self::random_dir();
                }
                r
            } else {
                options[rand::rng().random_range(0..options.len())]
            };

            (
                pos.x + f64::from(dir_step_x_s(sel)),
                pos.y + f64::from(dir_step_y_s(sel)),
                pos.z + f64::from(dir_step_z_s(sel)),
                sel,
            )
        } else {
            // If close, set selection=null and aim at the actual target position directly.
            (target_x, target_y, target_z, DIR_NONE)
        };

        // Compute normalized steering delta scaled by SPEED.
        let xa = chosen_tx - pos.x;
        let ya = chosen_ty - pos.y;
        let za = chosen_tz - pos.z;
        let dist = (xa * xa + ya * ya + za * za).sqrt();

        if dist == 0.0 {
            self.target_delta_x.store(0.0);
            self.target_delta_y.store(0.0);
            self.target_delta_z.store(0.0);
        } else {
            self.target_delta_x.store(xa / dist * SPEED);
            self.target_delta_y.store(ya / dist * SPEED);
            self.target_delta_z.store(za / dist * SPEED);
        }

        self.current_dir.store(selected_dir, Ordering::Relaxed);
        let steps = 10 + rand::rng().random_range(0..5) * 10;
        self.flight_steps.store(steps, Ordering::Relaxed);
    }

    fn random_dir() -> u8 {
        rand::rng().random_range(0u8..6)
    }
}

// Free-standing step helpers (used inside async closures that can't capture self.*)
const fn dir_step_x_s(dir: u8) -> i32 {
    match dir {
        DIR_EAST => 1,
        DIR_WEST => -1,
        _ => 0,
    }
}
const fn dir_step_y_s(dir: u8) -> i32 {
    match dir {
        DIR_UP => 1,
        DIR_DOWN => -1,
        _ => 0,
    }
}
const fn dir_step_z_s(dir: u8) -> i32 {
    match dir {
        DIR_SOUTH => 1,
        DIR_NORTH => -1,
        _ => 0,
    }
}

/// Returns the axis ordinal (0=X, 1=Y, 2=Z) of a direction.
/// Returns 255 for `DIR_NONE` or any unknown value.
const fn dir_axis(dir: u8) -> u8 {
    match dir {
        DIR_EAST | DIR_WEST => 0,
        DIR_UP | DIR_DOWN => 1,
        DIR_NORTH | DIR_SOUTH => 2,
        _ => 255,
    }
}

/// `true` if the block one step in `dir` from `from` is air (the bullet
/// can pass through it). Unloaded chunks are treated as empty.
fn is_empty_block(world: &crate::world::World, from: &BlockPos, dir: u8) -> bool {
    let nb = BlockPos::new(
        from.0.x + dir_step_x_s(dir),
        from.0.y + dir_step_y_s(dir),
        from.0.z + dir_step_z_s(dir),
    );
    world
        .get_block_state_if_loaded(&nb)
        .is_none_or(pumpkin_data::BlockState::is_air)
}

impl ShulkerBulletEntity {
    /// Create a `ShulkerBulletEntity` from a pre-existing Entity (e.g. loaded from
    /// disk or spawned via the /summon command with no context).
    pub fn orphan(entity: Entity) -> Self {
        Self {
            entity,
            owner_id: 0,
            owner_uuid: Mutex::new(None),
            target_id: AtomicI32::new(-1),
            target_uuid: Mutex::new(None),
            current_dir: AtomicU8::new(DIR_NONE),
            flight_steps: AtomicI32::new(0),
            target_delta_x: AtomicCell::new(0.0),
            target_delta_y: AtomicCell::new(0.0),
            target_delta_z: AtomicCell::new(0.0),
            has_hit: AtomicBool::new(false),
            age: AtomicI32::new(0),
        }
    }

    #[must_use]
    pub fn get_owner_uuid(&self) -> Option<Uuid> {
        *self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_owner_uuid(&self, uuid: Option<Uuid>) {
        *self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = uuid;
    }

    #[must_use]
    pub fn get_target_uuid(&self) -> Option<Uuid> {
        *self
            .target_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_target_uuid(&self, uuid: Option<Uuid>) {
        *self
            .target_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = uuid;
    }

    #[must_use]
    pub fn get_flight_steps(&self) -> i32 {
        self.flight_steps.load(Ordering::Relaxed)
    }

    pub fn set_flight_steps(&self, steps: i32) {
        self.flight_steps.store(steps, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get_target_delta(&self) -> Vector3<f64> {
        Vector3::new(
            self.target_delta_x.load(),
            self.target_delta_y.load(),
            self.target_delta_z.load(),
        )
    }

    pub fn set_target_delta(&self, delta: Vector3<f64>) {
        self.target_delta_x.store(delta.x);
        self.target_delta_y.store(delta.y);
        self.target_delta_z.store(delta.z);
    }

    #[must_use]
    pub fn get_current_dir(&self) -> u8 {
        self.current_dir.load(Ordering::Relaxed)
    }

    pub fn set_current_dir(&self, dir: u8) {
        self.current_dir.store(dir, Ordering::Relaxed);
    }
}

impl EntityBase for ShulkerBulletEntity {
    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(target) = *self
            .target_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_uuid("Target", target);
        }
        if let Some(owner) = *self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_uuid("Owner", owner);
        }
        let dir = self.current_dir.load(Ordering::Relaxed);
        if dir != DIR_NONE {
            nbt.put_byte("Dir", dir as i8);
        }
        nbt.put_int("Steps", self.flight_steps.load(Ordering::Relaxed));
        nbt.put_double("TXD", self.target_delta_x.load());
        nbt.put_double("TYD", self.target_delta_y.load());
        nbt.put_double("TZD", self.target_delta_z.load());
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        if let Some(target) = nbt.get_uuid("Target").or_else(|| nbt.get_uuid("target")) {
            *self
                .target_uuid
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(target);
        }
        if let Some(owner) = nbt.get_uuid("Owner").or_else(|| nbt.get_uuid("owner")) {
            *self
                .owner_uuid
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(owner);
        }
        if let Some(steps) = nbt.get_int("Steps").or_else(|| nbt.get_int("steps")) {
            self.flight_steps.store(steps, Ordering::Relaxed);
        }
        if let Some(txd) = nbt.get_double("TXD").or_else(|| nbt.get_double("txd")) {
            self.target_delta_x.store(txd);
        }
        if let Some(tyd) = nbt.get_double("TYD").or_else(|| nbt.get_double("tyd")) {
            self.target_delta_y.store(tyd);
        }
        if let Some(tzd) = nbt.get_double("TZD").or_else(|| nbt.get_double("tzd")) {
            self.target_delta_z.store(tzd);
        }
        let dir_val = nbt
            .get_byte("Dir")
            .or_else(|| nbt.get_byte("dir"))
            .map(|b| b as u8)
            .or_else(|| {
                nbt.get_int("Dir")
                    .or_else(|| nbt.get_int("dir"))
                    .map(|i| i as u8)
            });
        if let Some(d) = dir_val {
            if d <= DIR_EAST {
                self.current_dir.store(d, Ordering::Relaxed);
            }
        }
    }

    fn can_hit(&self) -> bool {
        !self.entity.is_removed()
    }

    fn get_owner_id(&self) -> Option<i32> {
        Some(self.owner_id)
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }

    /// Any hit destroys the bullet (melee, arrow, etc.).
    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        _source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        // Guard against double-hit
        if self.has_hit.swap(true, Ordering::SeqCst) {
            return false;
        }
        let entity = &self.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.play_sound_fine(
            Sound::EntityShulkerBulletHurt,
            SoundCategory::Hostile,
            &pos,
            1.0,
            1.0,
        );
        world.spawn_particle(
            pos,
            Vector3::new(0.2, 0.2, 0.2),
            0.0,
            crate::entity::combat::get_shulker_bullet_deflect_particle_count(),
            Particle::Crit,
        );
        entity.remove();
        true
    }
    #[allow(clippy::too_many_lines)]
    fn tick(&self, _caller: &dyn EntityBase, _server: &Server) {
        if self.has_hit.load(Ordering::Relaxed) {
            return;
        }

        let entity = &self.entity;
        let world = entity.world.load();

        // Vanilla ShulkerBullet.java:179-181 checkDespawn on peaceful
        if crate::entity::combat::should_shulker_bullet_despawn_in_peaceful(
            world.level_info.load().difficulty,
        ) {
            entity.remove();
            return;
        }

        // Discard bullet after max lifespan if it never hits anything
        let age = self.age.fetch_add(1, Ordering::Relaxed) + 1;
        if age > crate::entity::combat::SHULKER_BULLET_MAX_AGE {
            if !self.has_hit.swap(true, Ordering::SeqCst) {
                let entity = &self.entity;
                let world = entity.world.load();
                let pos = entity.pos.load();
                world.play_sound_fine(
                    Sound::EntityShulkerBulletHit,
                    SoundCategory::Hostile,
                    &pos,
                    1.0,
                    1.0,
                );
                world.spawn_particle(
                    pos,
                    Vector3::new(0.2, 0.2, 0.2),
                    0.0,
                    2,
                    Particle::Explosion,
                );
                entity.remove();
            }
            return;
        }

        let entity = &self.entity;
        let world = entity.world.load();

        let mut target_id = self.target_id.load(Ordering::Relaxed);
        if target_id < 0 {
            if let Some(target_uuid) = *self
                .target_uuid
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
            {
                if let Some(target_ent) = world.get_entity_by_uuid(target_uuid) {
                    target_id = target_ent.get_entity().entity_id;
                    self.target_id.store(target_id, Ordering::Relaxed);
                }
            }
        }
        let target_opt = if target_id >= 0 {
            world.get_entity_by_id(target_id)
        } else {
            None
        };

        // Apply gravity only if target is null, dead, or a spectator.
        let target_alive = target_opt
            .as_ref()
            .is_some_and(|t| t.get_entity().is_alive());

        if !target_alive && target_id >= 0 {
            // Target ID is set but entity was not found (dead/left the world).
            // Only permanently clear the target when we're sure it's gone.
            if let Some(t) = &target_opt {
                if !t.get_entity().is_alive() {
                    self.target_id.store(-1, Ordering::Relaxed);
                }
            } else {
                // Not found -> clear target
                self.target_id.store(-1, Ordering::Relaxed);
            }
        }

        if target_alive {
            // Accelerate target deltas x 1.025, clamped
            let mut tdx = (self.target_delta_x.load() * 1.025).clamp(-1.0, 1.0);
            let mut tdy = (self.target_delta_y.load() * 1.025).clamp(-1.0, 1.0);
            let mut tdz = (self.target_delta_z.load() * 1.025).clamp(-1.0, 1.0);
            self.target_delta_x.store(tdx);
            self.target_delta_y.store(tdy);
            self.target_delta_z.store(tdz);

            // Clamp initialised sentinel values
            if tdx.abs() < 1e-10 && tdy.abs() < 1e-10 && tdz.abs() < 1e-10 {
                tdx = 0.0;
                tdy = 0.0;
                tdz = 0.0;
            }

            // Lerp actual velocity toward steering delta
            let mut vel = entity.velocity.load();
            vel.x += (tdx - vel.x) * 0.2;
            vel.y += (tdy - vel.y) * 0.2;
            vel.z += (tdz - vel.z) * 0.2;
            entity.velocity.store(vel);
        } else {
            // No live target – apply gravity and drift
            let mut vel = entity.velocity.load();
            vel.y -= 0.04;
            entity.velocity.store(vel);
        }

        let vel = entity.velocity.load();
        let old_pos = entity.pos.load();
        let new_pos = old_pos.add(&vel);
        entity.set_pos(new_pos);

        // Check for block collisions
        let new_bp = entity.block_pos.load();
        let state = world.get_block_state(&new_bp);
        if !state.is_air() && state.is_solid() {
            if !self.has_hit.swap(true, Ordering::SeqCst) {
                let pos = entity.pos.load();
                world.play_sound_fine(
                    Sound::EntityShulkerBulletHit,
                    SoundCategory::Hostile,
                    &pos,
                    1.0,
                    1.0,
                );
                world.spawn_particle(
                    pos,
                    Vector3::new(0.2, 0.2, 0.2),
                    0.0,
                    2,
                    Particle::Explosion,
                );
                entity.remove();
            }
            return;
        }

        // Check for entity collisions
        let bullet_bb = entity.bounding_box.load().expand(0.1, 0.1, 0.1);
        let nearby = world.get_all_at_box(&bullet_bb);
        for hit_entity in nearby {
            let he = hit_entity.get_entity();
            // Skip self
            if he.entity_id == entity.entity_id {
                continue;
            }
            // Never hit the owner shulker
            if he.entity_id == self.owner_id {
                continue;
            }
            // Must be alive
            if !he.is_alive() {
                continue;
            }
            // Must be a living entity
            let Some(living) = hit_entity.get_living_entity() else {
                continue;
            };
            if !living.entity.is_alive() {
                continue;
            }

            if self.has_hit.swap(true, Ordering::SeqCst) {
                break;
            }

            // Deal 4 (MOB_PROJECTILE) damage
            let owner_arc = world.get_entity_by_id(self.owner_id);
            let damaged = hit_entity.damage_with_context(
                hit_entity.as_ref(),
                crate::entity::combat::get_shulker_bullet_damage(),
                DamageType::MOB_PROJECTILE,
                None,
                owner_arc.as_deref(),
                None,
            );

            if damaged {
                let effect = Effect {
                    effect_type: &StatusEffect::LEVITATION,
                    duration: crate::entity::combat::get_shulker_bullet_levitation_duration(),
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                };
                if let Some(player) = hit_entity.get_player() {
                    player.add_effect(effect);
                } else if let Some(living) = hit_entity.get_living_entity() {
                    living.add_effect(effect);
                }
            }

            let pos = entity.pos.load();
            world.play_sound_fine(
                Sound::EntityShulkerBulletHit,
                SoundCategory::Hostile,
                &pos,
                1.0,
                1.0,
            );
            world.spawn_particle(
                pos,
                Vector3::new(0.2, 0.2, 0.2),
                0.0,
                2,
                Particle::Explosion,
            );
            entity.remove();
            break;
        }

        if !target_alive || self.has_hit.load(Ordering::Relaxed) {
            return;
        }

        let target_pos = target_opt.as_ref().map(|t| t.get_entity().pos.load());

        let raw_dir = self.current_dir.load(Ordering::Relaxed);
        let avoid_axis = dir_axis(raw_dir);

        // Decrement flight-step counter; re-select direction when it hits 0
        let steps = self.flight_steps.fetch_sub(1, Ordering::Relaxed) - 1;
        if steps <= 0 {
            self.select_next_dir(avoid_axis, target_pos);
        }

        // Check the block immediately ahead.
        // If it is solid we must re-select; if we've aligned axes with the target we also re-select.
        let dir = self.current_dir.load(Ordering::Relaxed);
        if dir != DIR_NONE {
            let cur_bp = entity.block_pos.load();
            if !is_empty_block(&world, &cur_bp, dir) {
                // Solid obstacle -> navigate around it
                self.select_next_dir(dir_axis(dir), target_pos);
            } else if let Some(tp) = target_pos {
                let axis = dir_axis(dir);
                let tbp = BlockPos::new(
                    tp.x.floor() as i32,
                    tp.y.floor() as i32,
                    tp.z.floor() as i32,
                );
                let cur_bp2 = entity.block_pos.load();
                let reached = match axis {
                    0 => cur_bp2.0.x == tbp.0.x,
                    1 => cur_bp2.0.y == tbp.0.y,
                    _ => cur_bp2.0.z == tbp.0.z,
                };
                if reached {
                    // Aligned on this axis -> switch to next best axis
                    self.select_next_dir(axis, Some(tp));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::entity::combat::*;

    #[test]
    fn test_shulker_bullet_nbt_persistence_parity() {
        use pumpkin_nbt::compound::NbtCompound;
        use uuid::Uuid;

        // Verify NBT serialization and deserialization parity (ShulkerBullet.java:67-89)
        let mut nbt = NbtCompound::new();
        let target_uuid = Uuid::new_v4();
        let owner_uuid = Uuid::new_v4();

        nbt.put_uuid("Target", target_uuid);
        nbt.put_uuid("Owner", owner_uuid);
        nbt.put_byte("Dir", 3); // South
        nbt.put_int("Steps", 25);
        nbt.put_double("TXD", 0.123);
        nbt.put_double("TYD", -0.456);
        nbt.put_double("TZD", 0.789);

        // Verify tag reads
        assert_eq!(nbt.get_uuid("Target"), Some(target_uuid));
        assert_eq!(nbt.get_uuid("Owner"), Some(owner_uuid));
        assert_eq!(nbt.get_byte("Dir"), Some(3));
        assert_eq!(nbt.get_int("Steps"), Some(25));
        assert_eq!(nbt.get_double("TXD"), Some(0.123));
        assert_eq!(nbt.get_double("TYD"), Some(-0.456));
        assert_eq!(nbt.get_double("TZD"), Some(0.789));
    }

    #[test]
    fn test_shulker_bullet_damage_and_levitation_parity() {
        assert_eq!(get_shulker_bullet_damage(), 4.0);
        assert_eq!(get_shulker_bullet_levitation_duration(), 200);
        assert!(should_shulker_bullet_despawn_in_peaceful(pumpkin_util::Difficulty::Peaceful));
        assert!(!should_shulker_bullet_despawn_in_peaceful(pumpkin_util::Difficulty::Easy));
        assert_eq!(get_shulker_bullet_deflect_particle_count(), 15);
    }
}
