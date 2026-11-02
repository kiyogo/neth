use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityType;
use pumpkin_data::particle::Particle;
use pumpkin_data::world::WorldEvent;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{Entity, EntityBase};
use crate::entity::mob::spawn::finalize_spawn;
use crate::world::World;

/// Spawner Minecart implementation.
///
/// Matches vanilla `MinecartSpawner.java` and `BaseSpawner.java`:
/// - Delay ranges: default delay 20, min 200, max 800 ticks.
/// - Required player range: 16 blocks.
/// - Spawns up to `spawn_count` (4) entities within `spawn_range` (4).
/// - Drops standard `Items.MINECART` on destruction (`MinecartSpawner.java:32-34`).
pub struct SpawnerMinecart {
    pub delay: AtomicI32,
    pub min_delay: AtomicI32,
    pub max_delay: AtomicI32,
    pub spawn_count: AtomicI32,
    pub spawn_range: AtomicI32,
    pub max_nearby_entities: AtomicI32,
    pub required_player_range: AtomicI32,
    pub entity_type: AtomicCell<Option<&'static EntityType>>,
}

impl Default for SpawnerMinecart {
    fn default() -> Self {
        Self::new()
    }
}

impl SpawnerMinecart {
    pub const DEFAULT_DELAY: i32 = crate::entity::combat::SPAWNER_MINECART_DEFAULT_DELAY;
    pub const DEFAULT_MIN_SPAWN_DELAY: i32 =
        crate::entity::combat::SPAWNER_MINECART_DEFAULT_MIN_SPAWN_DELAY;
    pub const DEFAULT_MAX_SPAWN_DELAY: i32 =
        crate::entity::combat::SPAWNER_MINECART_DEFAULT_MAX_SPAWN_DELAY;
    pub const DEFAULT_SPAWN_COUNT: i32 =
        crate::entity::combat::SPAWNER_MINECART_DEFAULT_SPAWN_COUNT;
    pub const DEFAULT_SPAWN_RANGE: i32 =
        crate::entity::combat::SPAWNER_MINECART_DEFAULT_SPAWN_RANGE;
    pub const DEFAULT_MAX_NEARBY_ENTITIES: i32 =
        crate::entity::combat::SPAWNER_MINECART_DEFAULT_MAX_NEARBY_ENTITIES;
    pub const DEFAULT_REQUIRED_PLAYER_RANGE: i32 =
        crate::entity::combat::SPAWNER_MINECART_DEFAULT_REQUIRED_PLAYER_RANGE;

    #[must_use]
    pub fn new() -> Self {
        Self {
            delay: AtomicI32::new(Self::DEFAULT_DELAY),
            min_delay: AtomicI32::new(Self::DEFAULT_MIN_SPAWN_DELAY),
            max_delay: AtomicI32::new(Self::DEFAULT_MAX_SPAWN_DELAY),
            spawn_count: AtomicI32::new(Self::DEFAULT_SPAWN_COUNT),
            spawn_range: AtomicI32::new(Self::DEFAULT_SPAWN_RANGE),
            max_nearby_entities: AtomicI32::new(Self::DEFAULT_MAX_NEARBY_ENTITIES),
            required_player_range: AtomicI32::new(Self::DEFAULT_REQUIRED_PLAYER_RANGE),
            entity_type: AtomicCell::new(None),
        }
    }

    /// Vanilla `MinecartSpawner.tick()` -> `ticker.run()` -> `spawner.serverTick(serverLevel, pos)`.
    pub fn tick(&self, entity: &Entity) {
        let world = entity.world.load();
        let center = entity.pos.load();
        let req_player_range = f64::from(self.required_player_range.load(Ordering::Relaxed));
        let max_dist_sq = req_player_range * req_player_range;

        let player_nearby = world.players.load().iter().any(|p| {
            p.get_entity().pos.load().squared_distance_to_vec(&center) <= max_dist_sq
        });

        if !player_nearby {
            return;
        }

        // Vanilla `BaseSpawner.clientTick`: smoke and flame particles when player is nearby
        if rand::random_range(0..4) == 0 {
            let p_pos = Vector3::new(
                center.x + (rand::random::<f64>() - 0.5),
                center.y + (rand::random::<f64>() * 0.5) + 0.3,
                center.z + (rand::random::<f64>() - 0.5),
            );
            world.spawn_particle(
                p_pos,
                Vector3::new(0.0, 0.0, 0.0),
                0.0,
                1,
                Particle::Smoke,
            );
            world.spawn_particle(
                p_pos,
                Vector3::new(0.0, 0.0, 0.0),
                0.0,
                1,
                Particle::Flame,
            );
        }

        let current_delay = self.delay.load(Ordering::Relaxed);
        if current_delay < 0 {
            self.reset_delay(&world, entity);
            return;
        }
        if current_delay > 0 {
            self.delay.fetch_sub(1, Ordering::Relaxed);
            return;
        }

        let Some(entity_type) = self.entity_type.load() else {
            self.reset_delay(&world, entity);
            return;
        };

        let spawn_range = self.spawn_range.load(Ordering::Relaxed);
        let search_horiz = f64::from(spawn_range * 2);
        let search_vert = 4.0;
        let nearby_count = world
            .entities
            .load()
            .iter()
            .filter(|e| {
                let ent = e.get_entity();
                if ent.entity_type.id != entity_type.id {
                    return false;
                }
                let pos = ent.pos.load();
                (pos.x - center.x).abs() <= search_horiz
                    && (pos.z - center.z).abs() <= search_horiz
                    && (pos.y - center.y).abs() <= search_vert
            })
            .count();

        if nearby_count as i32 >= self.max_nearby_entities.load(Ordering::Relaxed) {
            self.reset_delay(&world, entity);
            return;
        }

        let spawn_count = self.spawn_count.load(Ordering::Relaxed);
        let mut spawned_any = false;
        for _ in 0..spawn_count {
            let spawn_pos = Vector3::new(
                (rand::random::<f64>() - rand::random::<f64>()).mul_add(f64::from(spawn_range), center.x) + 0.5,
                (center.y.floor() as i32 + rand::random_range(0..3) - 1) as f64,
                (rand::random::<f64>() - rand::random::<f64>()).mul_add(f64::from(spawn_range), center.z) + 0.5,
            );
            if !world.is_space_empty(entity_type.get_spawn_bounding_box(
                spawn_pos.x,
                spawn_pos.y,
                spawn_pos.z,
            )) {
                continue;
            }
            let spawned_entity = crate::entity::r#type::from_type(
                entity_type,
                spawn_pos,
                &world,
                uuid::Uuid::new_v4(),
            );
            let yaw = rand::random::<f32>() * 360.0;
            spawned_entity.get_entity().set_rotation(yaw, 0.0);

            let mut event =
                crate::plugin::api::events::entity::spawner_spawn::SpawnerSpawnEvent::new(
                    spawned_entity.get_entity().entity_id,
                    entity.block_pos.load(),
                );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                continue;
            }

            finalize_spawn(&spawned_entity, &world, None);
            world.spawn_entity(spawned_entity);
            world.sync_world_event(WorldEvent::ParticlesMobblockSpawn, entity.block_pos.load(), 0);
            spawned_any = true;
        }

        if spawned_any {
            self.reset_delay(&world, entity);
        }
    }

    /// Vanilla `BaseSpawner.delay(Level, BlockPos)`:
    /// Resets spawn delay and broadcasts event 1.
    pub fn reset_delay(&self, world: &Arc<World>, entity: &Entity) {
        let min = self.min_delay.load(Ordering::Relaxed);
        let max = self.max_delay.load(Ordering::Relaxed);
        let delay = if max <= min {
            min
        } else {
            min + rand::random_range(0..max - min)
        };
        self.delay.store(delay, Ordering::Relaxed);
        world.send_entity_status(entity, pumpkin_data::entity::EntityStatus::Jump, None);
    }

    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_short("Delay", self.delay.load(Ordering::Relaxed) as i16);
        nbt.put_short("MinSpawnDelay", self.min_delay.load(Ordering::Relaxed) as i16);
        nbt.put_short("MaxSpawnDelay", self.max_delay.load(Ordering::Relaxed) as i16);
        nbt.put_short("SpawnCount", self.spawn_count.load(Ordering::Relaxed) as i16);
        nbt.put_short("SpawnRange", self.spawn_range.load(Ordering::Relaxed) as i16);
        nbt.put_short(
            "MaxNearbyEntities",
            self.max_nearby_entities.load(Ordering::Relaxed) as i16,
        );
        nbt.put_short(
            "RequiredPlayerRange",
            self.required_player_range.load(Ordering::Relaxed) as i16,
        );

        if let Some(entity_type) = self.entity_type.load() {
            let mut spawn_entry = NbtCompound::new();
            let mut entity_nbt = NbtCompound::new();
            entity_nbt.put_string("id", format!("minecraft:{}", entity_type.resource_name));
            spawn_entry.put_compound("entity", entity_nbt);
            nbt.put_compound("SpawnData", spawn_entry);
        }
    }

    pub fn read_nbt(&self, nbt: &NbtCompound) {
        if let Some(delay) = nbt.get_short("Delay") {
            self.delay.store(i32::from(delay), Ordering::Relaxed);
        }
        if let Some(min_delay) = nbt.get_short("MinSpawnDelay") {
            self.min_delay.store(i32::from(min_delay), Ordering::Relaxed);
        }
        if let Some(max_delay) = nbt.get_short("MaxSpawnDelay") {
            self.max_delay.store(i32::from(max_delay), Ordering::Relaxed);
        }
        if let Some(spawn_count) = nbt.get_short("SpawnCount") {
            self.spawn_count.store(i32::from(spawn_count), Ordering::Relaxed);
        }
        if let Some(spawn_range) = nbt.get_short("SpawnRange") {
            self.spawn_range.store(i32::from(spawn_range), Ordering::Relaxed);
        }
        if let Some(max_nearby) = nbt.get_short("MaxNearbyEntities") {
            self.max_nearby_entities.store(i32::from(max_nearby), Ordering::Relaxed);
        }
        if let Some(req_range) = nbt.get_short("RequiredPlayerRange") {
            self.required_player_range.store(i32::from(req_range), Ordering::Relaxed);
        }
        if let Some(spawn_data) = nbt.get_compound("SpawnData") {
            let id = spawn_data
                .get_compound("entity")
                .and_then(|e| e.get_string("id"))
                .or_else(|| spawn_data.get_string("id"));
            if let Some(id_str) = id {
                let stripped = id_str.strip_prefix("minecraft:").unwrap_or(id_str);
                if let Some(entity_type) = EntityType::from_name(stripped) {
                    self.entity_type.store(Some(entity_type));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawner_minecart_defaults() {
        let spawner = SpawnerMinecart::new();
        assert_eq!(spawner.delay.load(Ordering::Relaxed), 20);
        assert_eq!(spawner.min_delay.load(Ordering::Relaxed), 200);
        assert_eq!(spawner.max_delay.load(Ordering::Relaxed), 800);
        assert_eq!(spawner.spawn_count.load(Ordering::Relaxed), 4);
        assert_eq!(spawner.spawn_range.load(Ordering::Relaxed), 4);
        assert_eq!(spawner.max_nearby_entities.load(Ordering::Relaxed), 6);
        assert_eq!(spawner.required_player_range.load(Ordering::Relaxed), 16);
        assert!(spawner.entity_type.load().is_none());
    }

    #[test]
    fn test_spawner_minecart_nbt_roundtrip() {
        let spawner = SpawnerMinecart::new();
        spawner.delay.store(50, Ordering::Relaxed);
        spawner.min_delay.store(150, Ordering::Relaxed);
        spawner.max_delay.store(600, Ordering::Relaxed);
        spawner.spawn_count.store(6, Ordering::Relaxed);
        spawner.spawn_range.store(5, Ordering::Relaxed);
        spawner.max_nearby_entities.store(8, Ordering::Relaxed);
        spawner.required_player_range.store(24, Ordering::Relaxed);
        spawner.entity_type.store(Some(&EntityType::ZOMBIE));

        let mut nbt = NbtCompound::new();
        spawner.write_nbt(&mut nbt);

        assert_eq!(nbt.get_short("Delay"), Some(50));
        assert_eq!(nbt.get_short("MinSpawnDelay"), Some(150));
        assert_eq!(nbt.get_short("MaxSpawnDelay"), Some(600));
        assert_eq!(nbt.get_short("SpawnCount"), Some(6));
        assert_eq!(nbt.get_short("SpawnRange"), Some(5));
        assert_eq!(nbt.get_short("MaxNearbyEntities"), Some(8));
        assert_eq!(nbt.get_short("RequiredPlayerRange"), Some(24));

        let loaded = SpawnerMinecart::new();
        loaded.read_nbt(&nbt);

        assert_eq!(loaded.delay.load(Ordering::Relaxed), 50);
        assert_eq!(loaded.min_delay.load(Ordering::Relaxed), 150);
        assert_eq!(loaded.max_delay.load(Ordering::Relaxed), 600);
        assert_eq!(loaded.spawn_count.load(Ordering::Relaxed), 6);
        assert_eq!(loaded.spawn_range.load(Ordering::Relaxed), 5);
        assert_eq!(loaded.max_nearby_entities.load(Ordering::Relaxed), 8);
        assert_eq!(loaded.required_player_range.load(Ordering::Relaxed), 24);
        assert_eq!(loaded.entity_type.load(), Some(&EntityType::ZOMBIE));
    }
}
