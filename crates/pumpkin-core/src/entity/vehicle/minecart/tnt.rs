use std::sync::atomic::{AtomicI32, Ordering};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityStatus;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::Entity;

pub(super) struct TntMinecart {
    fuse: AtomicI32,
    explosion_power: AtomicCell<f32>,
    explosion_speed_factor: AtomicCell<f32>,
    ignition_source_id: AtomicCell<Option<i32>>,
}

impl TntMinecart {
    pub(super) const fn new() -> Self {
        Self {
            fuse: AtomicI32::new(-1),
            explosion_power: AtomicCell::new(crate::entity::combat::MINECART_TNT_DEFAULT_POWER),
            explosion_speed_factor: AtomicCell::new(crate::entity::combat::MINECART_TNT_DEFAULT_SPEED_FACTOR),
            ignition_source_id: AtomicCell::new(None),
        }
    }

    #[must_use]
    pub(super) fn get_fuse(&self) -> i32 {
        self.fuse.load(Ordering::Relaxed)
    }

    #[must_use]
    pub(super) fn is_primed(&self) -> bool {
        self.fuse.load(Ordering::Relaxed) > -1
    }

    pub(super) fn prime(&self, entity: &Entity, fuse: i32) {
        self.prime_with_fuse_and_source(entity, fuse, None);
    }

    pub(super) fn prime_with_fuse_and_source(
        &self,
        entity: &Entity,
        fuse: i32,
        source_id: Option<i32>,
    ) {
        if self.fuse.load(Ordering::Relaxed) >= 0
            || !entity
                .world
                .load()
                .level_info
                .load()
                .game_rules
                .tnt_explodes
        {
            return;
        }

        self.fuse.store(fuse, Ordering::Relaxed);
        if let Some(src_id) = source_id {
            if self.ignition_source_id.load().is_none() {
                self.ignition_source_id.store(Some(src_id));
            }
        }

        let world = entity.world.load();
        world.send_entity_status(
            entity,
            EntityStatus::TntPrime,
            Some(ActorEventID::PrimeTNTCart),
        );
        world.play_sound(
            Sound::EntityTntPrimed,
            SoundCategory::Blocks,
            &entity.pos.load(),
        );
    }

    pub(super) fn cause_fall_damage(&self, entity: &Entity, fall_distance: f64) {
        if fall_distance >= crate::entity::combat::MINECART_TNT_FALL_EXPLOSION_MIN_DISTANCE {
            let speed_sq = crate::entity::combat::calculate_tnt_minecart_fall_speed_squared(fall_distance);
            self.explode(entity, speed_sq);
        }
    }

    pub(super) fn tick(&self, entity: &Entity) -> bool {
        let fuse = self.fuse.load(Ordering::Relaxed);
        if fuse > 0 {
            self.fuse.store(fuse - 1, Ordering::Relaxed);
            let mut smoke_pos = entity.pos.load();
            smoke_pos.y += 0.5;
            entity.world.load().spawn_particle(
                smoke_pos,
                Vector3::new(0.0, 0.0, 0.0),
                0.0,
                1,
                Particle::Smoke,
            );
        } else if fuse == 0 {
            let velocity = entity.velocity.load();
            self.explode(
                entity,
                velocity.x.mul_add(velocity.x, velocity.z * velocity.z),
            );
            return true;
        }
        false
    }

    pub(super) fn explode(&self, entity: &Entity, horizontal_speed_squared: f64) {
        self.explode_with_source(entity, horizontal_speed_squared, None);
    }

    pub(super) fn explode_with_source(
        &self,
        entity: &Entity,
        horizontal_speed_squared: f64,
        source_id: Option<i32>,
    ) {
        if let Some(src_id) = source_id {
            if self.ignition_source_id.load().is_none() {
                self.ignition_source_id.store(Some(src_id));
            }
        }

        let world = entity.world.load();
        if !world.level_info.load().game_rules.tnt_explodes {
            if self.is_primed() {
                entity.remove();
            }
            return;
        }

        let rand_factor = rand::rng().random_range(0.0..1.0);
        let power = crate::entity::combat::calculate_tnt_minecart_explosion_power(
            self.explosion_power.load(),
            self.explosion_speed_factor.load(),
            horizontal_speed_squared,
            rand_factor,
        );
        let pos = entity.pos.load();
        let primed = self.is_primed();
        entity.remove();
        if primed {
            world.explode_tnt_minecart(pos, power);
        } else {
            world.explode(pos, power, crate::world::ExplosionInteraction::Tnt);
        }
    }

    pub(super) fn set_fuse(&self, fuse: i32) {
        self.fuse.store(fuse, Ordering::Relaxed);
    }

    pub(super) fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("fuse", self.fuse.load(Ordering::Relaxed));
        let power = self.explosion_power.load();
        if power != crate::entity::combat::MINECART_TNT_DEFAULT_POWER {
            nbt.put_float("explosion_power", power);
        }
        let speed_factor = self.explosion_speed_factor.load();
        if speed_factor != crate::entity::combat::MINECART_TNT_DEFAULT_SPEED_FACTOR {
            nbt.put_float("explosion_speed_factor", speed_factor);
        }
    }

    pub(super) fn read_nbt(&self, nbt: &NbtCompound) {
        self.fuse
            .store(nbt.get_int("fuse").unwrap_or(-1), Ordering::Relaxed);
        self.explosion_power.store(
            nbt.get_float("explosion_power")
                .unwrap_or(crate::entity::combat::MINECART_TNT_DEFAULT_POWER)
                .clamp(0.0, 128.0),
        );
        self.explosion_speed_factor.store(
            nbt.get_float("explosion_speed_factor")
                .unwrap_or(crate::entity::combat::MINECART_TNT_DEFAULT_SPEED_FACTOR)
                .clamp(0.0, 128.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::TntMinecart;
    use pumpkin_nbt::compound::NbtCompound;

    #[test]
    fn tnt_minecart_explosion_bonus_is_speed_capped() {
        assert_eq!(
            crate::entity::combat::calculate_tnt_minecart_explosion_power(4.0, 1.0, 100.0, 1.0),
            11.5
        );
        assert_eq!(
            crate::entity::combat::calculate_tnt_minecart_explosion_power(4.0, 1.0, 100.0, 0.0),
            4.0
        );
    }

    #[test]
    fn tnt_minecart_defaults_and_nbt_roundtrip() {
        let tnt = TntMinecart::new();
        assert_eq!(tnt.get_fuse(), -1);
        assert!(!tnt.is_primed());
        tnt.set_fuse(35);
        assert_eq!(tnt.get_fuse(), 35);
        assert!(tnt.is_primed());
        tnt.set_fuse(-1);

        let mut nbt = NbtCompound::new();
        tnt.write_nbt(&mut nbt);
        assert_eq!(nbt.get_int("fuse"), Some(-1));
        assert!(nbt.get_float("explosion_power").is_none());
        assert!(nbt.get_float("explosion_speed_factor").is_none());

        // Custom NBT
        let mut custom_nbt = NbtCompound::new();
        custom_nbt.put_int("fuse", 50);
        custom_nbt.put_float("explosion_power", 6.5);
        custom_nbt.put_float("explosion_speed_factor", 2.0);

        let tnt_custom = TntMinecart::new();
        tnt_custom.read_nbt(&custom_nbt);
        assert_eq!(tnt_custom.get_fuse(), 50);
        assert!(tnt_custom.is_primed());

        let mut out_nbt = NbtCompound::new();
        tnt_custom.write_nbt(&mut out_nbt);
        assert_eq!(out_nbt.get_int("fuse"), Some(50));
        assert_eq!(out_nbt.get_float("explosion_power"), Some(6.5));
        assert_eq!(out_nbt.get_float("explosion_speed_factor"), Some(2.0));
    }
}
