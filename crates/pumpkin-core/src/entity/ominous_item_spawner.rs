use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::world::WorldEvent;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::item::ItemEntity;
use crate::entity::living::LivingEntity;
use crate::entity::projectile::ThrownItemEntity;
use crate::entity::projectile::arrow::{ArrowEntity, ArrowPickup};
use crate::entity::projectile::lingering_potion::LingeringPotionEntity;
use crate::entity::projectile::small_fireball::SmallFireballEntity;
use crate::entity::projectile::splash_potion::SplashPotionEntity;
use crate::entity::projectile::wind_charge::{WIND_CHARGE_GRAVITY, WindChargeEntity};
use crate::entity::{Entity, EntityBase};
use crate::server::Server;

pub const DEFAULT_MIN_SPAWN_TICKS: i64 = 60;
pub const DEFAULT_MAX_SPAWN_TICKS: i64 = 120;
pub const TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND: i64 = 36;
pub const TICKS_BETWEEN_ITEM_SPAWNERS: i64 = 160;
pub const CEILING_OFFSET_MIN: f64 = 2.0;
pub const CEILING_OFFSET_MAX_EXTRA: u32 = 4;
pub const DEFAULT_LOOT_TABLE: &str = "minecraft:spawners/trial_chamber/items_to_drop_when_ominous";
pub const SOUND_SPAWN_BEGIN: Sound = Sound::BlockTrialSpawnerSpawnItemBegin;
pub const SOUND_ABOUT_TO_SPAWN: Sound = Sound::BlockTrialSpawnerAboutToSpawnItem;
pub const EVENT_SPAWN_ITEM: i32 = 3021;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPool {
    Mobs,
    Players,
}

pub struct OminousItemSpawnerEntity {
    pub entity: Entity,
    item_stack: Mutex<ItemStack>,
    spawn_item_after_ticks: AtomicI64,
    ticks: AtomicI64,
}

impl OminousItemSpawnerEntity {
    pub fn new(entity: Entity) -> Self {
        let delay = rand::random_range(DEFAULT_MIN_SPAWN_TICKS..=DEFAULT_MAX_SPAWN_TICKS);
        Self::new_with_delay(entity, ItemStack::new(0, &pumpkin_data::item::Item::AIR), delay)
    }

    pub fn new_with_delay(entity: Entity, item: ItemStack, delay: i64) -> Self {
        Self {
            entity,
            item_stack: Mutex::new(item),
            spawn_item_after_ticks: AtomicI64::new(delay),
            ticks: AtomicI64::new(0),
        }
    }

    pub fn item(&self) -> ItemStack {
        self.item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_item(&self, item: ItemStack) {
        *self
            .item_stack
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = item;
    }

    pub fn spawn_item_after_ticks(&self) -> i64 {
        self.spawn_item_after_ticks.load(Ordering::Relaxed)
    }

    pub fn set_spawn_item_after_ticks(&self, ticks: i64) {
        self.spawn_item_after_ticks.store(ticks, Ordering::Relaxed);
    }

    /// Checks if current tick matches warning sound threshold (tick == spawn_delay - 36)
    #[must_use]
    pub const fn should_play_warning_sound(current_tick: i64, spawn_delay: i64) -> bool {
        current_tick == spawn_delay - TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND
    }

    /// Checks if spawner has reached or passed spawn delay
    #[must_use]
    pub const fn should_spawn_item(current_tick: i64, spawn_delay: i64) -> bool {
        current_tick >= spawn_delay
    }

    /// Validates randomized spawn delay against vanilla range [60..120]
    #[must_use]
    pub const fn is_delay_within_range(delay: i64) -> bool {
        delay >= DEFAULT_MIN_SPAWN_TICKS && delay <= DEFAULT_MAX_SPAWN_TICKS
    }

    /// Vanilla TrialSpawnerState.java:200: calculatePositionAbove
    /// offset = bbHeight + 2.0F + random.nextInt(4)
    #[must_use]
    pub fn calculate_spawn_height(bb_height: f64, random_offset: u32) -> f64 {
        bb_height + CEILING_OFFSET_MIN + (random_offset.min(3) as f64)
    }

    /// Vanilla TrialSpawnerState.java:168:
    /// pitch = (random.nextFloat() - random.nextFloat()) * 0.2F + 1.0F
    #[must_use]
    pub fn calculate_spawn_pitch(rand1: f32, rand2: f32) -> f32 {
        (rand1 - rand2) * 0.2 + 1.0
    }

    /// Vanilla TrialSpawnerStateData.java:300: lowResolutionPosition
    #[must_use]
    pub fn calculate_low_resolution_position_seed(x: i32, y: i32, z: i32, world_seed: i64) -> i64 {
        let rx = (x as f32 / 30.0).floor() as i32;
        let ry = (y as f32 / 20.0).floor() as i32;
        let rz = (z as f32 / 30.0).floor() as i32;
        let low_res_pos = pumpkin_util::math::position::BlockPos::new(rx, ry, rz);
        world_seed.wrapping_add(low_res_pos.as_long())
    }

    /// Vanilla TrialSpawnerState.java:215: selectEntityToSpawnItemAbove
    /// 50% chance mobs, 50% chance players if both exist
    #[must_use]
    pub const fn select_target_pool(has_mobs: bool, has_players: bool, coin_flip: bool) -> Option<TargetPool> {
        if has_mobs && has_players {
            if coin_flip {
                Some(TargetPool::Mobs)
            } else {
                Some(TargetPool::Players)
            }
        } else if has_mobs {
            Some(TargetPool::Mobs)
        } else if has_players {
            Some(TargetPool::Players)
        } else {
            None
        }
    }

    /// Vanilla TrialSpawnerState.java:204: checks if collision shape is empty
    #[must_use]
    pub const fn is_valid_ceiling_drop_position(has_block_collision: bool) -> bool {
        !has_block_collision
    }
}

impl EntityBase for OminousItemSpawnerEntity {
    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn tick(&self, _caller: &dyn EntityBase, _server: &Server) {
        let current_tick = self.ticks.fetch_add(1, Ordering::Relaxed) + 1;
        let spawn_delay = self.spawn_item_after_ticks.load(Ordering::Relaxed);
        let world = self.entity.world.load();

        if current_tick == spawn_delay - TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND {
            let pos = self.entity.pos.load();
            world.play_sound(
                Sound::BlockTrialSpawnerAboutToSpawnItem,
                SoundCategory::Neutral,
                &pos,
            );
        }

        if current_tick >= spawn_delay {
            let item = self
                .item_stack
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();

            if !item.is_empty() {
                let pos = self.entity.pos.load();
                let item_id = item.get_item().id;

                if item_id == Item::LINGERING_POTION.id {
                    let ent = Entity::new(world.clone(), pos, &EntityType::LINGERING_POTION);
                    let potion = LingeringPotionEntity::new(ent);
                    potion.set_item_stack(item);
                    potion.thrown.entity.set_velocity(Vector3::new(0.0, -0.2, 0.0));
                    world.spawn_entity(Arc::new(potion));
                } else if item_id == Item::SPLASH_POTION.id {
                    let ent = Entity::new(world.clone(), pos, &EntityType::SPLASH_POTION);
                    let potion = SplashPotionEntity::new(ent);
                    potion.set_item_stack(item);
                    potion.thrown.entity.set_velocity(Vector3::new(0.0, -0.2, 0.0));
                    world.spawn_entity(Arc::new(potion));
                } else if item_id == Item::WIND_CHARGE.id {
                    let ent = Entity::new(world.clone(), pos, &EntityType::WIND_CHARGE);
                    let thrown = ThrownItemEntity {
                        entity: ent,
                        owner_id: None,
                        collides_with_projectiles: false,
                        has_hit: AtomicBool::new(false),
                        has_left_owner: AtomicBool::new(true),
                        gravity: WIND_CHARGE_GRAVITY,
                    };
                    let wc = WindChargeEntity::new_normal(thrown);
                    wc.get_entity().set_velocity(Vector3::new(0.0, -0.4, 0.0));
                    world.spawn_entity(Arc::new(wc));
                } else if item_id == Item::ARROW.id || item_id == Item::TIPPED_ARROW.id {
                    let ent = Entity::new(world.clone(), pos, &EntityType::ARROW);
                    let arrow = ArrowEntity::new_with_item(
                        ent,
                        None,
                        &item,
                        ArrowPickup::Disallowed,
                    );
                    arrow.entity.set_velocity(Vector3::new(0.0, -0.5, 0.0));
                    world.spawn_entity(Arc::new(arrow));
                } else if item_id == Item::FIRE_CHARGE.id {
                    let ent = Entity::new(world.clone(), pos, &EntityType::SMALL_FIREBALL);
                    let fireball = SmallFireballEntity::new(ent);
                    fireball.thrown.entity.set_velocity(Vector3::new(0.0, -0.3, 0.0));
                    world.spawn_entity(Arc::new(fireball));
                } else {
                    let item_entity = Entity::new(world.clone(), pos, &EntityType::ITEM);
                    let spawned = Arc::new(ItemEntity::new_with_velocity(
                        item_entity,
                        item,
                        Vector3::new(0.0, 0.0, 0.0),
                        10,
                    ));
                    world.spawn_entity(spawned);
                }

                world.sync_world_event(
                    WorldEvent::ParticlesTrialSpawnerSpawnItem,
                    self.entity.block_pos.load(),
                    1,
                );
            }

            self.entity.remove();
        }
    }

    fn damage(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
    ) -> bool {
        // Matches vanilla hurtServer -> false
        false
    }

    fn get_gravity(&self) -> f64 {
        0.0
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::entities::trial_spawner::TrialSpawnerConfig;

    #[test]
    fn test_ominous_item_spawner_timing_ceiling_distribution_and_parity() {
        // 1. Constants parity (OminousItemSpawner.java:24-30, TrialSpawnerStateData.java:299-302)
        assert_eq!(DEFAULT_MIN_SPAWN_TICKS, 60);
        assert_eq!(DEFAULT_MAX_SPAWN_TICKS, 120);
        assert_eq!(TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND, 36);
        assert_eq!(TICKS_BETWEEN_ITEM_SPAWNERS, 160);
        assert_eq!(CEILING_OFFSET_MIN, 2.0);
        assert_eq!(CEILING_OFFSET_MAX_EXTRA, 4);
        assert_eq!(DEFAULT_LOOT_TABLE, "minecraft:spawners/trial_chamber/items_to_drop_when_ominous");
        assert_eq!(EVENT_SPAWN_ITEM, 3021);
        assert_eq!(SOUND_SPAWN_BEGIN, Sound::BlockTrialSpawnerSpawnItemBegin);
        assert_eq!(SOUND_ABOUT_TO_SPAWN, Sound::BlockTrialSpawnerAboutToSpawnItem);

        // 2. TrialSpawnerConfig item spawner interval parity (TrialSpawnerConfig.java)
        assert_eq!(TrialSpawnerConfig::TICKS_BETWEEN_ITEM_SPAWNERS, 160);
        let config = TrialSpawnerConfig::default();
        assert_eq!(config.ticks_between_item_spawners(), 160);

        // 3. Timing checks
        assert!(OminousItemSpawnerEntity::should_play_warning_sound(84, 120));
        assert!(!OminousItemSpawnerEntity::should_play_warning_sound(83, 120));
        assert!(!OminousItemSpawnerEntity::should_play_warning_sound(85, 120));
        assert!(OminousItemSpawnerEntity::should_play_warning_sound(24, 60));

        assert!(!OminousItemSpawnerEntity::should_spawn_item(59, 60));
        assert!(OminousItemSpawnerEntity::should_spawn_item(60, 60));
        assert!(OminousItemSpawnerEntity::should_spawn_item(61, 60));

        assert!(OminousItemSpawnerEntity::is_delay_within_range(60));
        assert!(OminousItemSpawnerEntity::is_delay_within_range(90));
        assert!(OminousItemSpawnerEntity::is_delay_within_range(120));
        assert!(!OminousItemSpawnerEntity::is_delay_within_range(59));
        assert!(!OminousItemSpawnerEntity::is_delay_within_range(121));

        // 4. Ceiling offset & drop calculation (TrialSpawnerState.java:200-205)
        let player_height = 1.8;
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_height(player_height, 0), 3.8);
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_height(player_height, 1), 4.8);
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_height(player_height, 2), 5.8);
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_height(player_height, 3), 6.8);
        // Clamped to max extra 3 (random 0..4 in java nextInt(4))
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_height(player_height, 5), 6.8);

        // Ceiling collision validation
        assert!(OminousItemSpawnerEntity::is_valid_ceiling_drop_position(false));
        assert!(!OminousItemSpawnerEntity::is_valid_ceiling_drop_position(true));

        // 5. Sound pitch calculation (TrialSpawnerState.java:168: 1.0 + (rand1 - rand2) * 0.2)
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_pitch(0.0, 0.0), 1.0);
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_pitch(1.0, 0.0), 1.2);
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_pitch(0.0, 1.0), 0.8);
        assert_eq!(OminousItemSpawnerEntity::calculate_spawn_pitch(0.5, 0.5), 1.0);

        // 6. Target pool selection (TrialSpawnerState.java:215)
        assert_eq!(OminousItemSpawnerEntity::select_target_pool(true, true, true), Some(TargetPool::Mobs));
        assert_eq!(OminousItemSpawnerEntity::select_target_pool(true, true, false), Some(TargetPool::Players));
        assert_eq!(OminousItemSpawnerEntity::select_target_pool(true, false, false), Some(TargetPool::Mobs));
        assert_eq!(OminousItemSpawnerEntity::select_target_pool(false, true, true), Some(TargetPool::Players));
        assert_eq!(OminousItemSpawnerEntity::select_target_pool(false, false, true), None);

        // 7. Low resolution position seed consistency
        let seed1 = OminousItemSpawnerEntity::calculate_low_resolution_position_seed(10, 20, 10, 12345);
        let seed2 = OminousItemSpawnerEntity::calculate_low_resolution_position_seed(15, 25, 15, 12345);
        let seed3 = OminousItemSpawnerEntity::calculate_low_resolution_position_seed(100, 20, 10, 12345);
        // Same 30x20x30 region produces same seed
        assert_eq!(seed1, seed2);
        // Different region produces different seed
        assert_ne!(seed1, seed3);
    }
}
