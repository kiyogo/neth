use crate::block::PathComputationType;
use crate::entity::EntityBase;
use crate::entity::mob::spawn::finalize_spawn;
use crate::entity::r#type::{check_spawn_rules, from_type};
use crate::world::World;
use arc_swap::ArcSwap;
use crossbeam::atomic::AtomicCell;
use dashmap::DashMap;
use pumpkin_data::biome::Spawner;
use pumpkin_data::chunk::Biome;
use pumpkin_data::dimension::Dimension;
use pumpkin_data::entity::{EntityType, MobCategory, SpawnLocation};
use pumpkin_data::tag::Block::MINECRAFT_PREVENT_MOB_SPAWNING_INSIDE;
use pumpkin_data::tag::Fluid::{MINECRAFT_LAVA, MINECRAFT_WATER};
use pumpkin_data::tag::Taggable;
use pumpkin_data::tag::WorldgenBiome::MINECRAFT_REDUCE_WATER_AMBIENT_SPAWNS;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::{Block, BlockDirection, BlockState, SpawnFloorPredicate};
use pumpkin_util::GameMode;
use pumpkin_util::math::get_section_cord;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::random::xoroshiro128::Xoroshiro;
use pumpkin_util::random::{RandomImpl, get_seed};
use pumpkin_world::chunk::{ChunkData, ChunkHeightmapType};
use pumpkin_world::generation::proto_chunk::GenerationCache;
use rand::seq::IndexedRandom;
use rand::{RngExt, rng};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
use uuid::Uuid;

const MAGIC_NUMBER: i32 = 17 * 17;

pub struct MobCounts([AtomicI32; 8]);

impl Default for MobCounts {
    fn default() -> Self {
        Self(std::array::from_fn(|_| AtomicI32::new(0)))
    }
}

impl fmt::Debug for MobCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|a| a.load(Relaxed)))
            .finish()
    }
}

impl Clone for MobCounts {
    fn clone(&self) -> Self {
        Self(std::array::from_fn(|i| {
            AtomicI32::new(self.0[i].load(Relaxed))
        }))
    }
}

impl MobCounts {
    #[inline]
    pub fn add(&self, category: &'static MobCategory) {
        self.0[category.id].fetch_add(1, Relaxed);
    }

    #[inline]
    pub fn remove(&self, category: &'static MobCategory) {
        self.0[category.id].fetch_sub(1, Relaxed);
    }

    #[inline]
    pub fn can_spawn(&self, category: &'static MobCategory) -> bool {
        self.0[category.id].load(Relaxed) < category.max
    }
}

pub struct LocalMobCapCalculator {
    player_mob_counts: DashMap<i32, MobCounts>,
    players_near_chunk: DashMap<Vector2<i32>, Arc<[i32]>>,
}

impl Clone for LocalMobCapCalculator {
    fn clone(&self) -> Self {
        let player_mob_counts = DashMap::new();
        for r in &self.player_mob_counts {
            player_mob_counts.insert(*r.key(), r.value().clone());
        }
        let players_near_chunk = DashMap::new();
        for r in &self.players_near_chunk {
            players_near_chunk.insert(*r.key(), r.value().clone());
        }
        Self {
            player_mob_counts,
            players_near_chunk,
        }
    }
}

impl Default for LocalMobCapCalculator {
    fn default() -> Self {
        Self {
            player_mob_counts: DashMap::new(),
            players_near_chunk: DashMap::new(),
        }
    }
}

impl fmt::Debug for LocalMobCapCalculator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalMobCapCalculator")
            .field("world", &"<skipped>")
            .finish()
    }
}

impl LocalMobCapCalculator {
    const fn calc_distance(chunk_pos: Vector2<i32>, player_pos: &Vector3<f64>) -> f64 {
        let dx = ((chunk_pos.x << 4) + 8) as f64 - player_pos.x;
        let dy = ((chunk_pos.y << 4) + 8) as f64 - player_pos.z;
        dx * dx + dy * dy
    }

    fn get_players_near(&self, world: &World, chunk_pos: Vector2<i32>) -> Arc<[i32]> {
        if let Some(players) = self.players_near_chunk.get(&chunk_pos) {
            return players.value().clone();
        }

        let mut players = Vec::new();
        for player in world.players.load().iter() {
            if player.gamemode.load() == GameMode::Spectator {
                continue;
            }
            if Self::calc_distance(chunk_pos, &player.position()) < 16384.0 {
                players.push(player.entity_id());
            }
        }
        let players: Arc<[i32]> = players.into();
        self.players_near_chunk.insert(chunk_pos, players.clone());
        players
    }

    pub fn add_mob(&self, chunk_pos: Vector2<i32>, world: &World, category: &'static MobCategory) {
        let players = self.get_players_near(world, chunk_pos);
        for &player in players.iter() {
            self.player_mob_counts
                .entry(player)
                .or_default()
                .add(category);
        }
    }

    pub fn remove_mob(
        &self,
        chunk_pos: Vector2<i32>,
        world: &World,
        category: &'static MobCategory,
    ) {
        let players = self.get_players_near(world, chunk_pos);
        for &player in players.iter() {
            if let Some(count) = self.player_mob_counts.get(&player) {
                count.remove(category);
            }
        }
    }

    pub fn can_spawn(
        &self,
        category: &'static MobCategory,
        world: &World,
        chunk_pos: Vector2<i32>,
    ) -> bool {
        let players = self.get_players_near(world, chunk_pos);
        for &player in players.iter() {
            if let Some(count) = self.player_mob_counts.get(&player) {
                if count.can_spawn(category) {
                    return true;
                }
            } else {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
struct PointCharge(BlockPos, f64);

impl PointCharge {
    /// Change in spawn potential this charge adds at `pos`.
    ///
    /// Widen before subtracting, like vanilla's `Vec3i.distSqr` does with `i2d`;
    /// multiplying in `i32` overflows past ~46341 blocks before any cast runs.
    fn get_potential_change(&self, pos: &BlockPos) -> f64 {
        let dx = f64::from(self.0.0.x) - f64::from(pos.0.x);
        let dy = f64::from(self.0.0.y) - f64::from(pos.0.y);
        let dz = f64::from(self.0.0.z) - f64::from(pos.0.z);
        let dist_sq = dx * dx + dy * dy + dz * dz;
        if dist_sq == 0.0 {
            f64::INFINITY
        } else {
            self.1 / dist_sq.sqrt()
        }
    }
}

#[derive(Default, Debug)]
struct PotentialCalculator(std::sync::Mutex<Vec<PointCharge>>);

impl Clone for PotentialCalculator {
    fn clone(&self) -> Self {
        Self(std::sync::Mutex::new(
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        ))
    }
}

impl PotentialCalculator {
    pub fn add_charge(&self, pos: &BlockPos, charge: f64) {
        if charge != 0.0 {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(PointCharge(*pos, charge));
        }
    }

    pub fn remove_charge(&self, pos: &BlockPos, charge: f64) {
        if charge != 0.0 {
            let mut charges = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(idx) = charges.iter().position(|c| c.0 == *pos && c.1 == charge) {
                charges.swap_remove(idx);
            }
        }
    }

    pub fn get_potential_energy_change(&self, pos: &BlockPos, charge: f64) -> f64 {
        if charge == 0.0 {
            return 0.0;
        }
        let mut sum: f64 = 0.0;
        let charges = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for i in charges.iter() {
            sum += i.get_potential_change(pos);
        }
        sum * charge
    }
}

#[derive(Clone)]
pub struct SpawnState {
    spawnable_chunk_count: i32,
    pub mob_category_counts: Arc<MobCounts>,
    spawn_potential: Arc<PotentialCalculator>,
    local_mob_cap_calculator: Arc<LocalMobCapCalculator>,
    last_checked: Arc<AtomicCell<Option<(BlockPos, &'static EntityType, f64)>>>,
}

impl fmt::Debug for SpawnState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpawnState")
            .field("spawnable_chunk_count", &self.spawnable_chunk_count)
            .field("mob_category_counts", &self.mob_category_counts)
            .field("spawn_potential", &self.spawn_potential)
            .field("local_mob_cap_calculator", &self.local_mob_cap_calculator)
            .field("last_checked", &self.last_checked)
            .finish()
    }
}

impl SpawnState {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            spawnable_chunk_count: 0,
            mob_category_counts: Arc::new(MobCounts::default()),
            spawn_potential: Arc::new(PotentialCalculator::default()),
            local_mob_cap_calculator: Arc::new(LocalMobCapCalculator::default()),
            last_checked: Arc::new(AtomicCell::new(None)),
        }
    }

    pub const fn set_spawnable_chunk_count(&mut self, count: i32) {
        self.spawnable_chunk_count = count;
    }

    pub fn add_entity(&self, world: &World, entity: &dyn EntityBase) {
        if let Some(mob) = entity.get_mob()
            && (mob.get_mob_entity().persistence_required.load(Relaxed)
                || mob.requires_custom_persistence())
        {
            return;
        }
        let base_entity = entity.get_entity();
        let entity_type = base_entity.entity_type;
        if entity_type.category == &MobCategory::MISC {
            return;
        }
        let entity_pos = base_entity.block_pos.load();
        let biome = base_entity.current_biome.load();
        if let Some(cost) = biome.spawn_costs.get(entity_type.resource_name) {
            self.spawn_potential.add_charge(&entity_pos, cost.charge);
        }
        if entity.get_mob().is_some() || entity_type.mob {
            self.local_mob_cap_calculator.add_mob(
                base_entity.chunk_pos.load(),
                world,
                entity_type.category,
            );
        }
        self.mob_category_counts.add(entity_type.category);
    }

    pub fn remove_entity(&self, world: &World, entity: &dyn EntityBase) {
        if let Some(mob) = entity.get_mob()
            && (mob.get_mob_entity().persistence_required.load(Relaxed)
                || mob.requires_custom_persistence())
        {
            return;
        }
        let base_entity = entity.get_entity();
        let entity_type = base_entity.entity_type;
        if entity_type.category == &MobCategory::MISC {
            return;
        }
        let entity_pos = base_entity.block_pos.load();
        let biome = base_entity.current_biome.load();
        if let Some(cost) = biome.spawn_costs.get(entity_type.resource_name) {
            self.spawn_potential.remove_charge(&entity_pos, cost.charge);
        }
        if entity.get_mob().is_some() || entity_type.mob {
            self.local_mob_cap_calculator.remove_mob(
                base_entity.chunk_pos.load(),
                world,
                entity_type.category,
            );
        }
        self.mob_category_counts.remove(entity_type.category);
    }

    pub fn new(
        chunk_count: i32,
        entities: &ArcSwap<Vec<Arc<dyn EntityBase>>>,
        world: &World,
    ) -> Self {
        let potential = PotentialCalculator::default();
        let local_mob_cap = LocalMobCapCalculator::default();
        let counter = MobCounts::default();
        let active_chunks = world
            .active_chunks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for entity in entities.load().iter() {
            if let Some(mob) = entity.get_mob()
                && (mob.get_mob_entity().persistence_required.load(Relaxed)
                    || mob.requires_custom_persistence())
            {
                continue;
            }
            let base_entity = entity.get_entity();
            let entity_type = base_entity.entity_type;
            if entity_type.category == &MobCategory::MISC {
                continue;
            }
            let chunk_pos = base_entity.chunk_pos.load();
            if !active_chunks.contains(&chunk_pos) {
                continue;
            }
            let entity_pos = base_entity.block_pos.load();
            let biome = base_entity.current_biome.load();
            if let Some(cost) = biome.spawn_costs.get(entity_type.resource_name) {
                potential.add_charge(&entity_pos, cost.charge);
            }
            if entity.get_mob().is_some() || entity_type.mob {
                local_mob_cap.add_mob(chunk_pos, world, entity_type.category);
            }
            counter.add(entity_type.category);
        }
        Self {
            spawnable_chunk_count: chunk_count,
            mob_category_counts: Arc::new(counter),
            spawn_potential: Arc::new(potential),
            local_mob_cap_calculator: Arc::new(local_mob_cap),
            last_checked: Arc::new(AtomicCell::new(None)),
        }
    }

    #[inline]
    pub fn can_spawn_for_category_global(&self, category: &'static MobCategory) -> bool {
        self.mob_category_counts.0[category.id].load(Relaxed)
            < category.max * self.spawnable_chunk_count / MAGIC_NUMBER
    }

    pub fn can_spawn_for_category_local(
        &self,
        world: &Arc<World>,
        category: &'static MobCategory,
        chunk_pos: Vector2<i32>,
    ) -> bool {
        self.local_mob_cap_calculator
            .can_spawn(category, world, chunk_pos)
    }

    pub fn can_spawn(
        &self,
        entity_type: &'static EntityType,
        pos: &BlockPos,
        world: &Arc<World>,
    ) -> bool {
        let biome = world.level.get_rough_biome(pos);
        biome
            .spawn_costs
            .get(entity_type.resource_name)
            .map_or_else(
                || {
                    self.last_checked.store(Some((*pos, entity_type, 0.0)));
                    true
                },
                |cost| {
                    self.last_checked
                        .store(Some((*pos, entity_type, cost.charge)));
                    self.spawn_potential
                        .get_potential_energy_change(pos, cost.charge)
                        <= cost.energy_budget
                },
            )
    }

    pub fn after_spawn(
        &self,
        entity_type: &'static EntityType,
        pos: &BlockPos,
        world: &Arc<World>,
    ) {
        let charge = if let Some((l_pos, l_type, l_charge)) = self.last_checked.load()
            && l_pos.eq(pos)
            && l_type == entity_type
        {
            Some(l_charge)
        } else {
            None
        };

        let charge = charge.unwrap_or_else(|| {
            let biome = world.level.get_rough_biome(pos);
            biome
                .spawn_costs
                .get(entity_type.resource_name)
                .map_or(0.0, |cost| cost.charge)
        });

        self.spawn_potential.add_charge(pos, charge);
        self.mob_category_counts.add(entity_type.category);
        self.local_mob_cap_calculator.add_mob(
            Vector2::<i32>::new(get_section_cord(pos.0.x), get_section_cord(pos.0.z)),
            world,
            entity_type.category,
        );
    }
}

#[must_use]
pub fn get_filtered_spawning_categories(
    state: &SpawnState,
    spawn_friendlies: bool,
    spawn_enemies: bool,
    spawn_passives: bool,
) -> Vec<&'static MobCategory> {
    let mut ret = Vec::with_capacity(MobCategory::SPAWNING_CATEGORIES.len());
    for category in MobCategory::SPAWNING_CATEGORIES {
        let is_type_allowed = if category.is_friendly {
            spawn_friendlies
        } else {
            spawn_enemies
        };

        if !is_type_allowed {
            continue;
        }

        if category.is_persistent && !spawn_passives {
            continue;
        }

        if state.can_spawn_for_category_global(category) {
            ret.push(category);
        }
    }
    ret
}

pub fn spawn_for_chunk(
    world: &Arc<World>,
    chunk_pos: Vector2<i32>,
    chunk: &Arc<ChunkData>,
    spawn_state: &SpawnState,
    spawn_list: &Vec<&'static MobCategory>,
    is_thundering: bool,
) -> Vec<Arc<dyn EntityBase>> {
    let mut entities = Vec::new();
    for category in spawn_list {
        if spawn_state.can_spawn_for_category_global(category)
            && spawn_state.can_spawn_for_category_local(world, category, chunk_pos)
        {
            let random_pos = get_random_pos_within(world.min_y, &chunk_pos, chunk);
            if random_pos.0.y > world.min_y {
                entities.extend(spawn_category_for_position(
                    category,
                    world,
                    random_pos,
                    &chunk_pos,
                    spawn_state,
                    is_thundering,
                ));
            }
        }
    }
    entities
}

pub fn get_random_pos_within(
    min_y: i32,
    chunk_pos: &Vector2<i32>,
    chunk: &Arc<ChunkData>,
) -> BlockPos {
    let mut rng = Xoroshiro::from_seed(get_seed());

    let x = (chunk_pos.x << 4) + rng.next_bounded_i32(16);
    let z = (chunk_pos.y << 4) + rng.next_bounded_i32(16);
    let mut heightmaps_guard = chunk
        .heightmap
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if heightmaps_guard.world_surface.is_none() {
        *heightmaps_guard = chunk.calculate_heightmap();
    }
    let mut temp_y = heightmaps_guard
        .get(ChunkHeightmapType::WorldSurface, x, z, chunk.section.min_y)
        + 1;
    if temp_y <= min_y {
        temp_y = 64.max(min_y + 16);
    }
    let y = rng.next_inbetween_i32(min_y, temp_y);
    BlockPos::new(x, y, z)
}

fn is_spawn_rules_ok_cache(
    cache: &dyn GenerationCache,
    entity_type: &'static EntityType,
    pos: &BlockPos,
) -> bool {
    let below_pos = pos.down().0;
    let below_state = GenerationCache::get_block_state(cache, &below_pos).to_state();
    let below_block = Block::from_state_id(below_state.id);

    let id = entity_type.id;
    if id == EntityType::ARMADILLO.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_ARMADILLO_SPAWNABLE_ON)
    } else if id == EntityType::CAMEL.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_CAMELS_SPAWNABLE_ON)
    } else if id == EntityType::FROG.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_FROGS_SPAWNABLE_ON)
    } else if id == EntityType::GOAT.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_GOATS_SPAWNABLE_ON)
    } else if id == EntityType::MOOSHROOM.id {
        below_block.id == Block::MYCELIUM.id
    } else if id == EntityType::RABBIT.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_RABBITS_SPAWNABLE_ON)
    } else if id == EntityType::WOLF.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_WOLVES_SPAWNABLE_ON)
    } else if id == EntityType::FOX.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_FOXES_SPAWNABLE_ON)
    } else if id == EntityType::POLAR_BEAR.id {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_POLAR_BEARS_SPAWNABLE_ON_ALTERNATE)
            || below_block.id == Block::ICE.id
            || below_block.id == Block::BLUE_ICE.id
            || below_block.id == Block::PACKED_ICE.id
            || below_block.id == Block::FROSTED_ICE.id
    } else {
        below_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_ANIMALS_SPAWNABLE_ON)
    }
}

/// Spawns the initial passive creatures for a newly generated chunk, like vanilla
/// `NaturalSpawner::spawnMobsForChunkGeneration`.
pub fn spawn_mobs_for_chunk_generation(
    world: &Arc<World>,
    cache: &mut dyn GenerationCache,
    biome: &'static Biome,
    chunk_x: i32,
    chunk_z: i32,
) {
    let mob_settings = &biome.spawners;
    let creatures = &mob_settings.creature;

    if creatures.is_empty() || !world.level_info.load().game_rules.spawn_mobs {
        return;
    }

    let xo = chunk_x << 4;
    let zo = chunk_z << 4;

    while rand::random::<f32>() < biome.creature_spawn_probability {
        let Some(spawner_data) = choose_spawner(creatures, &mut rand::rng()) else {
            continue;
        };

        let count = spawner_data.min_count
            + rand::random_range(0..=(spawner_data.max_count - spawner_data.min_count).max(0));
        let name = spawner_data
            .r#type
            .strip_prefix("minecraft:")
            .unwrap_or(spawner_data.r#type);
        let Some(entity_type) = EntityType::from_name(name) else {
            return;
        };

        let mut x = xo + rand::random_range(0..16);
        let mut z = zo + rand::random_range(0..16);
        let start_x = x;
        let start_z = z;

        let mut group_data = None;
        for _ in 0..count {
            let mut success = false;

            for _ in 0..4 {
                if success {
                    break;
                }

                let pos = get_top_non_colliding_pos(world, cache, entity_type, x, z);

                if entity_type.summonable && is_spawn_position_ok_cache(cache, &pos, entity_type) {
                    let width = f64::from(entity_type.dimension[0]);
                    let fx =
                        (f64::from(x)).clamp(f64::from(xo) + width, f64::from(xo) + 16.0 - width);
                    let fz =
                        (f64::from(z)).clamp(f64::from(zo) + width, f64::from(zo) + 16.0 - width);
                    let spawn_pos_f64 = Vector3::new(fx, f64::from(pos.0.y), fz);
                    let check_pos = BlockPos::new(fx.floor() as i32, pos.0.y, fz.floor() as i32);

                    if is_spawn_rules_ok_cache(cache, entity_type, &check_pos) {
                        let entity = from_type(entity_type, spawn_pos_f64, world, Uuid::new_v4());
                        entity
                            .get_entity()
                            .set_rotation(rand::random::<f32>() * 360.0, 0.0);
                        group_data = finalize_spawn(&entity, world, group_data);
                        world.spawn_entity_non_save(entity);
                        success = true;
                    }
                }

                x += rand::random_range(0..5) - rand::random_range(0..5);
                z += rand::random_range(0..5) - rand::random_range(0..5);
                while x < xo || x >= xo + 16 || z < zo || z >= zo + 16 {
                    x = start_x + rand::random_range(0..5) - rand::random_range(0..5);
                    z = start_z + rand::random_range(0..5) - rand::random_range(0..5);
                }
            }
        }
    }
}

pub fn get_top_non_colliding_pos(
    world: &World,
    cache: &dyn GenerationCache,
    entity_type: &'static EntityType,
    x: i32,
    z: i32,
) -> BlockPos {
    let mut y = cache.get_top_y(&entity_type.spawn_restriction.heightmap, x, z);
    let mut pos_vec = Vector3::new(x, y, z);
    let min_y = world.min_y;

    if world.dimension.has_ceiling {
        loop {
            y -= 1;
            pos_vec.y = y;
            if GenerationCache::get_block_state(cache, &pos_vec)
                .to_state()
                .is_air()
                || y <= min_y
            {
                break;
            }
        }

        loop {
            y -= 1;
            pos_vec.y = y;
            if !GenerationCache::get_block_state(cache, &pos_vec)
                .to_state()
                .is_air()
                || y <= min_y
            {
                break;
            }
        }
    }

    let pos = BlockPos::new(x, y, z);

    adjust_spawn_position_cache(cache, pos, entity_type)
}

pub fn adjust_spawn_position_cache(
    cache: &dyn GenerationCache,
    pos: BlockPos,
    entity_type: &'static EntityType,
) -> BlockPos {
    if matches!(
        entity_type.spawn_restriction.location,
        SpawnLocation::OnGround
    ) {
        let below = pos.down();
        let state = GenerationCache::get_block_state(cache, &below.0).to_state();

        if !state.is_full_cube() && !state.is_liquid() {
            return below;
        }
    }
    pos
}

pub fn adjust_spawn_position(
    world: &World,
    pos: BlockPos,
    entity_type: &'static EntityType,
) -> BlockPos {
    if matches!(
        entity_type.spawn_restriction.location,
        SpawnLocation::OnGround
    ) {
        let below = pos.down();
        let (block, state) = world.get_block_and_state(&below);
        if world
            .block_registry
            .is_pathfindable(block, state, PathComputationType::Land)
        {
            return below;
        }
    }
    pos
}

#[allow(clippy::too_many_lines)]
pub fn spawn_category_for_position(
    category: &'static MobCategory,
    world: &Arc<World>,
    pos: BlockPos,
    chunk_pos: &Vector2<i32>,
    spawn_state: &SpawnState,
    is_thundering: bool,
) -> Vec<Arc<dyn EntityBase>> {
    let mut batch_buffer = Vec::new();
    let y_start = pos.0.y;
    let state = world.get_block_state(&pos);
    if state.is_solid_block() || state.is_full_cube() {
        return batch_buffer;
    }

    let mut cluster_size = 0;

    for _ in 0..3 {
        let mut x = pos.0.x;
        let mut z = pos.0.z;
        let mut current_spawner: Option<&'static Spawner> = None;
        let mut max = (rng().random::<f32>() * 4.0).ceil() as i32;
        let mut group_size = 0;
        let mut group_data = None;
        let mut ll = 0;

        while ll < max {
            x += rng().random_range(0..6) - rng().random_range(0..6);
            z += rng().random_range(0..6) - rng().random_range(0..6);
            let check_pos = BlockPos::new(x, y_start, z);
            let xx = f64::from(x) + 0.5;
            let zz = f64::from(z) + 0.5;

            let nearest_player_distance_sq = {
                let players = world.players.load();
                let mut min_dst_sq = f64::MAX;
                let mut found = false;
                for player in players.iter() {
                    if player.gamemode.load() != GameMode::Spectator {
                        let ppos = player.position();
                        let dx = xx - ppos.x;
                        let dy = f64::from(y_start) - ppos.y;
                        let dz = zz - ppos.z;
                        let dist_sq = dx * dx + dy * dy + dz * dz;
                        if dist_sq < min_dst_sq {
                            min_dst_sq = dist_sq;
                            found = true;
                        }
                    }
                }
                found.then_some(min_dst_sq)
            };

            if let Some(player_distance_sq) = nearest_player_distance_sq
                && is_right_distance_to_player_and_spawn_point(
                    world,
                    &check_pos,
                    chunk_pos,
                    player_distance_sq,
                )
            {
                if current_spawner.is_none() {
                    let Some(spawner) = get_random_spawn_mob_at(world, category, &check_pos) else {
                        break;
                    };
                    current_spawner = Some(spawner);
                    max = spawner.min_count
                        + rng().random_range(0..=(spawner.max_count - spawner.min_count).max(0));
                }

                let Some(spawner) = current_spawner else {
                    break;
                };
                let name = spawner
                    .r#type
                    .strip_prefix("minecraft:")
                    .unwrap_or(spawner.r#type);
                let Some(entity_type) = EntityType::from_name(name) else {
                    break;
                };

                let check_pos = adjust_spawn_position(world, check_pos, entity_type);

                if !spawn_state.can_spawn_for_category_global(category)
                    || !spawn_state.can_spawn_for_category_local(world, category, *chunk_pos)
                {
                    return batch_buffer;
                }

                if is_valid_spawn_position_for_type(
                    world,
                    &check_pos,
                    category,
                    entity_type,
                    spawner.r#type,
                    player_distance_sq,
                    is_thundering,
                ) && spawn_state.can_spawn(entity_type, &check_pos, world)
                {
                    let spawn_pos_f64 = Vector3::new(xx, f64::from(check_pos.0.y), zz);
                    let entity = from_type(entity_type, spawn_pos_f64, world, Uuid::new_v4());
                    entity
                        .get_entity()
                        .set_rotation(rng().random::<f32>() * 360.0, 0.0);

                    let is_valid_for_mob = entity.get_mob().is_none_or(|mob| {
                        let despawn_dist = f64::from(entity_type.category.despawn_distance);
                        !(player_distance_sq > despawn_dist * despawn_dist
                            && mob.remove_when_far_away(player_distance_sq))
                    });

                    if is_valid_for_mob {
                        group_data = finalize_spawn(&entity, world, group_data);
                        cluster_size += 1;
                        group_size += 1;
                        batch_buffer.push(entity);
                        spawn_state.after_spawn(entity_type, &check_pos, world);
                        let max_cluster_size = if entity_type.limit_per_chunk > 0 {
                            entity_type.limit_per_chunk
                        } else {
                            4
                        };
                        if cluster_size >= max_cluster_size {
                            return batch_buffer;
                        }

                        if entity_type.resource_name == "tropical_fish" && group_size >= 8 {
                            break;
                        }
                    }
                }
            }

            ll += 1;
        }
    }
    batch_buffer
}

#[must_use]
pub fn get_nearest_player(pos: &Vector3<f64>, player_positions: &[Vector3<f64>]) -> f64 {
    let mut min_dst_sq = f64::MAX;

    for player_pos in player_positions {
        let cur_dst_sq = player_pos.squared_distance_to_vec(pos);
        if cur_dst_sq < min_dst_sq {
            min_dst_sq = cur_dst_sq;
        }
    }
    min_dst_sq
}

#[must_use]
pub fn is_right_distance_to_player_and_spawn_point(
    world: &World,
    pos: &BlockPos,
    chunk_pos: &Vector2<i32>,
    nearest_player_distance_sq: f64,
) -> bool {
    if nearest_player_distance_sq <= 576.0 {
        return false;
    }

    if world.dimension == Dimension::OVERWORLD {
        let level_info = world.level_info.load();
        let dx = f64::from(level_info.spawn_x - pos.0.x);
        let dy = (f64::from(level_info.spawn_y) + 0.5) - f64::from(pos.0.y);
        let dz = f64::from(level_info.spawn_z - pos.0.z);
        if dx * dx + dy * dy + dz * dz < 576.0 {
            return false;
        }
    }

    let chunk_x = get_section_cord(pos.0.x);
    let chunk_z = get_section_cord(pos.0.z);
    let target_chunk = Vector2::new(chunk_x, chunk_z);
    target_chunk == *chunk_pos || world.can_spawn_entities_in_chunk(target_chunk)
}

pub const FORTRESS_ENEMIES: &[Spawner] = &[
    Spawner {
        r#type: "minecraft:blaze",
        min_count: 2,
        max_count: 3,
        weight: 10,
    },
    Spawner {
        r#type: "minecraft:wither_skeleton",
        min_count: 5,
        max_count: 5,
        weight: 8,
    },
    Spawner {
        r#type: "minecraft:zombified_piglin",
        min_count: 4,
        max_count: 4,
        weight: 5,
    },
    Spawner {
        r#type: "minecraft:magma_cube",
        min_count: 4,
        max_count: 4,
        weight: 3,
    },
    Spawner {
        r#type: "minecraft:skeleton",
        min_count: 5,
        max_count: 5,
        weight: 2,
    },
];

/// Picks a spawner in proportion to its weight, like vanilla `WeightedList::getRandom`.
/// Returns `None` for an empty list or when every weight is zero.
pub fn choose_spawner<'a, R: rand::Rng + ?Sized>(
    spawners: &'a [Spawner],
    rng: &mut R,
) -> Option<&'a Spawner> {
    spawners.choose_weighted(rng, |s| s.weight).ok()
}

#[must_use]
pub fn choose_weighted_spawner(spawners: &[Spawner]) -> Option<&Spawner> {
    choose_spawner(spawners, &mut rng())
}

#[must_use]
pub fn can_spawn_mob_at(
    world: &World,
    category: &'static MobCategory,
    spawner_type: &'static str,
    pos: &BlockPos,
) -> bool {
    let target = spawner_type
        .strip_prefix("minecraft:")
        .unwrap_or(spawner_type);

    if world.dimension == Dimension::THE_NETHER && category == &MobCategory::MONSTER {
        let below = pos.down();
        let below_block = world.get_block(&below);
        if below_block == &Block::NETHER_BRICKS || below_block == &Block::RED_NETHER_BRICKS {
            return FORTRESS_ENEMIES.iter().any(|s| {
                let name = s.r#type.strip_prefix("minecraft:").unwrap_or(s.r#type);
                name == target
            });
        }
    }

    let biome = world.level.get_rough_biome(pos);
    let spawners = match category.id {
        id if id == MobCategory::MONSTER.id => biome.spawners.monster,
        id if id == MobCategory::CREATURE.id => biome.spawners.creature,
        id if id == MobCategory::AMBIENT.id => biome.spawners.ambient,
        id if id == MobCategory::AXOLOTLS.id => biome.spawners.axolotls,
        id if id == MobCategory::UNDERGROUND_WATER_CREATURE.id => {
            biome.spawners.underground_water_creature
        }
        id if id == MobCategory::WATER_CREATURE.id => biome.spawners.water_creature,
        id if id == MobCategory::WATER_AMBIENT.id => biome.spawners.water_ambient,
        id if id == MobCategory::MISC.id => biome.spawners.misc,
        _ => biome.spawners.misc,
    };
    spawners.iter().any(|s| {
        let name = s.r#type.strip_prefix("minecraft:").unwrap_or(s.r#type);
        name == target
    })
}

/// Picks a weighted random spawner for `category` from the biome at `block_pos`, like
/// vanilla `NaturalSpawner::getRandomSpawnMobAt`.
#[must_use]
pub fn get_random_spawn_mob_at(
    world: &Arc<World>,
    category: &'static MobCategory,
    block_pos: &BlockPos,
) -> Option<&'static Spawner> {
    if world.dimension == Dimension::THE_NETHER && category == &MobCategory::MONSTER {
        let below = block_pos.down();
        let below_block = world.get_block(&below);
        if below_block == &Block::NETHER_BRICKS || below_block == &Block::RED_NETHER_BRICKS {
            return choose_spawner(FORTRESS_ENEMIES, &mut rng());
        }
    }

    let biome = world.level.get_rough_biome(block_pos);
    if category == &MobCategory::WATER_AMBIENT
        && biome.has_tag(&MINECRAFT_REDUCE_WATER_AMBIENT_SPAWNS)
        && rng().random::<f32>() < 0.98f32
    {
        None
    } else {
        let spawners = match category.id {
            id if id == MobCategory::MONSTER.id => biome.spawners.monster,
            id if id == MobCategory::CREATURE.id => biome.spawners.creature,
            id if id == MobCategory::AMBIENT.id => biome.spawners.ambient,
            id if id == MobCategory::AXOLOTLS.id => biome.spawners.axolotls,
            id if id == MobCategory::UNDERGROUND_WATER_CREATURE.id => {
                biome.spawners.underground_water_creature
            }
            id if id == MobCategory::WATER_CREATURE.id => biome.spawners.water_creature,
            id if id == MobCategory::WATER_AMBIENT.id => biome.spawners.water_ambient,
            id if id == MobCategory::MISC.id => biome.spawners.misc,
            _ => biome.spawners.misc,
        };
        choose_spawner(spawners, &mut rng())
    }
}


#[must_use]
pub fn is_valid_spawn_position_for_type(
    world: &Arc<World>,
    block_pos: &BlockPos,
    category: &'static MobCategory,
    entity_type: &'static EntityType,
    spawner_type: &'static str,
    distance: f64,
    is_thundering: bool,
) -> bool {
    if category == &MobCategory::MISC || entity_type.category == &MobCategory::MISC {
        return false;
    }
    if !entity_type.can_spawn_far_from_player
        && distance
            > f64::from(entity_type.category.despawn_distance)
                * f64::from(entity_type.category.despawn_distance)
    {
        return false;
    }
    if !entity_type.summonable || !can_spawn_mob_at(world, category, spawner_type, block_pos) {
        return false;
    }
    if !is_spawn_position_ok(world, block_pos, entity_type) {
        return false;
    }
    if !check_spawn_rules(entity_type, world, block_pos, is_thundering) {
        return false;
    }
    if !world.is_space_empty(entity_type.get_spawn_bounding_box(
        f64::from(block_pos.0.x) + 0.5,
        f64::from(block_pos.0.y),
        f64::from(block_pos.0.z) + 0.5,
    )) {
        return false;
    }
    true
}

#[must_use]
pub fn is_spawn_position_ok(
    world: &Arc<World>,
    block_pos: &BlockPos,
    entity_type: &'static EntityType,
) -> bool {
    let within_bounds = world
        .worldborder
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_block(block_pos.0.x, block_pos.0.z);

    match entity_type.spawn_restriction.location {
        SpawnLocation::InLava => {
            within_bounds && world.get_fluid(block_pos).has_tag(&MINECRAFT_LAVA)
        }
        SpawnLocation::InWater => {
            if !within_bounds {
                return false;
            }
            let above_state = world.get_block_state(&block_pos.up());
            world.get_fluid(block_pos).has_tag(&MINECRAFT_WATER)
                && !above_state.is_solid_block()
                && !above_state.is_full_cube()
        }
        SpawnLocation::OnGround => {
            if !within_bounds {
                return false;
            }
            if world.get_fluid(block_pos).id != Fluid::EMPTY.id
                || world.get_fluid(&block_pos.up()).id != Fluid::EMPTY.id
            {
                return false;
            }
            let down = world.get_block_state(&block_pos.down());
            let up = world.get_block_state(&block_pos.up());
            let cur = world.get_block_state(block_pos);
            let is_valid_spawn_below = is_valid_spawn_floor(down, entity_type);

            if is_valid_spawn_below {
                is_valid_empty_spawn_block(cur, entity_type)
                    && is_valid_empty_spawn_block(up, entity_type)
            } else {
                false
            }
        }
        SpawnLocation::Unrestricted => true,
    }
}

#[must_use]
pub fn is_spawn_position_ok_cache(
    cache: &dyn GenerationCache,
    block_pos: &BlockPos,
    entity_type: &'static EntityType,
) -> bool {
    let pos_vec = block_pos.0;
    let state = GenerationCache::get_block_state(cache, &pos_vec).to_state();

    match entity_type.spawn_restriction.location {
        SpawnLocation::InLava => {
            state.is_liquid() && Block::from_state_id(state.id).has_tag(&MINECRAFT_LAVA)
        }
        SpawnLocation::InWater => {
            let above_pos = block_pos.up().0;
            let above_state = GenerationCache::get_block_state(cache, &above_pos).to_state();

            state.is_liquid()
                && Block::from_state_id(state.id).has_tag(&MINECRAFT_WATER)
                && !above_state.is_solid_block()
                && !above_state.is_full_cube()
        }
        SpawnLocation::OnGround => {
            let down_pos = block_pos.down().0;
            let up_pos = block_pos.up().0;

            let down = GenerationCache::get_block_state(cache, &down_pos).to_state();
            let up = GenerationCache::get_block_state(cache, &up_pos).to_state();

            let is_valid_spawn_below = is_valid_spawn_floor(down, entity_type);

            if is_valid_spawn_below {
                is_valid_empty_spawn_block(state, entity_type)
                    && is_valid_empty_spawn_block(up, entity_type)
            } else {
                false
            }
        }
        SpawnLocation::Unrestricted => true,
    }
}

/// Vanilla `blockState.isSignalSource()`:
/// Checks if the block emits or conducts redstone signals, which prevents mobs from spawning inside it.
#[must_use]
pub fn is_signal_source(block: &Block) -> bool {
    block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_BUTTONS)
        || block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_PRESSURE_PLATES)
        || block.id == Block::REDSTONE_WIRE.id
        || block.id == Block::REDSTONE_TORCH.id
        || block.id == Block::REDSTONE_WALL_TORCH.id
        || block.id == Block::REPEATER.id
        || block.id == Block::COMPARATOR.id
        || block.id == Block::LEVER.id
        || block.id == Block::DAYLIGHT_DETECTOR.id
        || block.id == Block::OBSERVER.id
        || block.id == Block::REDSTONE_BLOCK.id
        || block.id == Block::TARGET.id
        || block.id == Block::SCULK_SENSOR.id
        || block.id == Block::CALIBRATED_SCULK_SENSOR.id
        || block.id == Block::TRIPWIRE_HOOK.id
        || block.id == Block::LIGHTNING_ROD.id
        || block.id == Block::LECTERN.id
        || block.id == Block::JUKEBOX.id
        || block.id == Block::TRAPPED_CHEST.id
        || block.id == Block::CRAFTER.id
        || block.id == Block::COPPER_BULB.id
        || block.id == Block::EXPOSED_COPPER_BULB.id
        || block.id == Block::WEATHERED_COPPER_BULB.id
        || block.id == Block::OXIDIZED_COPPER_BULB.id
        || block.id == Block::WAXED_COPPER_BULB.id
        || block.id == Block::WAXED_EXPOSED_COPPER_BULB.id
        || block.id == Block::WAXED_WEATHERED_COPPER_BULB.id
        || block.id == Block::WAXED_OXIDIZED_COPPER_BULB.id
        || block.id == Block::DETECTOR_RAIL.id
        || block.id == Block::ACTIVATOR_RAIL.id
}

#[must_use]
pub fn is_valid_empty_spawn_block(
    state: &'static BlockState,
    entity_type: &'static EntityType,
) -> bool {
    if state.is_full_cube() || state.is_solid_block() {
        return false;
    }
    let block = Block::from_state_id(state.id);
    if is_signal_source(block) {
        return false;
    }
    if state.is_liquid() {
        return false;
    }
    if block.has_tag(&MINECRAFT_PREVENT_MOB_SPAWNING_INSIDE) {
        return false;
    }
    !is_block_dangerous(entity_type, state)
}

#[must_use]
fn is_valid_spawn_floor(state: &'static BlockState, entity_type: &'static EntityType) -> bool {
    match Block::from_state_id(state.id).spawn_floor_predicate() {
        SpawnFloorPredicate::Default => {
            state.is_side_solid(BlockDirection::Up) && state.luminance < 14
        }
        SpawnFloorPredicate::Never => false,
        SpawnFloorPredicate::Always => true,
        SpawnFloorPredicate::OcelotOrParrot => {
            entity_type == &EntityType::OCELOT || entity_type == &EntityType::PARROT
        }
        SpawnFloorPredicate::PolarBear => entity_type == &EntityType::POLAR_BEAR,
        SpawnFloorPredicate::FireImmune => entity_type.fire_immune,
    }
}

fn is_block_dangerous(entity_type: &'static EntityType, state: &'static BlockState) -> bool {
    let block = Block::from_state_id(state.id);
    if !entity_type.fire_immune && is_burning_block(block) {
        return true;
    }
    if block.id == Block::WITHER_ROSE.id {
        return entity_type != &EntityType::WITHER
            && entity_type != &EntityType::WITHER_SKELETON;
    }
    if block.id == Block::SWEET_BERRY_BUSH.id {
        return entity_type != &EntityType::FOX;
    }
    if block.id == Block::CACTUS.id {
        return entity_type != &EntityType::CAMEL;
    }
    if block.id == Block::POWDER_SNOW.id {
        return entity_type != &EntityType::POLAR_BEAR
            && entity_type != &EntityType::STRAY
            && entity_type != &EntityType::SNOW_GOLEM;
    }
    false
}

fn is_burning_block(block: &Block) -> bool {
    block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_FIRE)
        || block.id == Block::LAVA.id
        || block.id == Block::MAGMA_BLOCK.id
        || block.id == Block::LAVA_CAULDRON.id
        || block.id == Block::CAMPFIRE.id
        || block.id == Block::SOUL_CAMPFIRE.id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potential_change_is_finite_beyond_i32_range() {
        let origin = BlockPos::new(0, 64, 0);
        for (far, distance) in [
            (BlockPos::new(46_342, 64, 0), 46_342.0),
            (
                BlockPos::new(40_000, 64, 40_000),
                40_000f64.hypot(40_000f64),
            ),
        ] {
            let change = PointCharge(far, 1.0).get_potential_change(&origin);

            assert_eq!(change, 1.0 / distance, "wrong potential for {far:?}");
        }
    }

    #[test]
    fn vanilla_spawn_floor_predicates_are_preserved() {
        for (block, entity_type, expected) in [
            (&Block::BEDROCK, &EntityType::CREEPER, false),
            (&Block::STONE, &EntityType::CREEPER, true),
            (&Block::GLASS, &EntityType::CREEPER, false),
            (&Block::OAK_LEAVES, &EntityType::OCELOT, true),
            (&Block::OAK_LEAVES, &EntityType::CREEPER, false),
            (&Block::ICE, &EntityType::POLAR_BEAR, true),
            (&Block::ICE, &EntityType::CREEPER, false),
            (&Block::MAGMA_BLOCK, &EntityType::BLAZE, true),
            (&Block::MAGMA_BLOCK, &EntityType::CREEPER, false),
            (&Block::JACK_O_LANTERN, &EntityType::CREEPER, true),
        ] {
            assert_eq!(
                is_valid_spawn_floor(block.default_state, entity_type),
                expected,
                "{}",
                block.name
            );
        }
    }

    #[test]
    fn test_point_charge_distance_does_not_overflow() {
        let charge = PointCharge(BlockPos(pumpkin_util::math::vector3::Vector3::new(0, 64, 0)), 1.0);
        // Distance > 46,341 blocks would overflow i32 squared distance
        let far_pos = BlockPos(pumpkin_util::math::vector3::Vector3::new(50_000, 64, 50_000));
        let change = charge.get_potential_change(&far_pos);
        assert!(!change.is_nan());
        assert!(change > 0.0);
        assert!(change < 1.0);
    }

    #[test]
    fn test_weighted_spawner_selection() {
        let spawners = [
            Spawner {
                r#type: "minecraft:zombie",
                min_count: 4,
                max_count: 4,
                weight: 100,
            },
            Spawner {
                r#type: "minecraft:witch",
                min_count: 1,
                max_count: 1,
                weight: 5,
            },
        ];

        let zombie_weight = spawners[0].weight;
        let witch_weight = spawners[1].weight;
        assert_eq!(zombie_weight, 100);
        assert_eq!(witch_weight, 5);
        assert!(zombie_weight > witch_weight * 10);

        let mut zombie_count = 0;
        let mut witch_count = 0;
        for _ in 0..1000 {
            let chosen = choose_weighted_spawner(&spawners).expect("Must choose a spawner");
            if chosen.r#type == "minecraft:zombie" {
                zombie_count += 1;
            } else if chosen.r#type == "minecraft:witch" {
                witch_count += 1;
            }
        }
        // In 1000 trials with weights 100 vs 5 (~95.2% vs ~4.8%), zombie should heavily dominate
        assert!(zombie_count > 800, "Zombie should spawn in vast majority: {zombie_count}");
        assert!(witch_count > 0, "Witch should occasionally spawn: {witch_count}");
    }

    #[test]
    fn test_fortress_enemies_table_parity() {
        assert_eq!(FORTRESS_ENEMIES.len(), 5);
        let names: Vec<&str> = FORTRESS_ENEMIES.iter().map(|s| s.r#type).collect();
        assert!(names.contains(&"minecraft:blaze"));
        assert!(names.contains(&"minecraft:wither_skeleton"));
        assert!(names.contains(&"minecraft:zombified_piglin"));
        assert!(names.contains(&"minecraft:magma_cube"));
        assert!(names.contains(&"minecraft:skeleton"));

        for spawner in FORTRESS_ENEMIES {
            assert!(spawner.weight > 0);
        }
    }

    #[test]
    fn test_nether_bricks_floor_allows_fortress_spawns() {
        assert!(is_valid_spawn_floor(Block::NETHER_BRICKS.default_state, &EntityType::BLAZE));
        assert!(is_valid_spawn_floor(Block::NETHER_BRICKS.default_state, &EntityType::WITHER_SKELETON));
        assert!(is_valid_spawn_floor(Block::NETHER_BRICKS.default_state, &EntityType::ZOMBIFIED_PIGLIN));
        assert!(is_valid_spawn_floor(Block::NETHER_BRICKS.default_state, &EntityType::MAGMA_CUBE));
        assert!(is_valid_spawn_floor(Block::NETHER_BRICKS.default_state, &EntityType::SKELETON));
    }

    #[test]
    fn vanilla_spawn_weights_are_preserved() {
        // Nether wastes in vanilla: zombified piglin 100, ghast 50, piglin 15,
        // magma cube 2, enderman 1. A uniform pick would make endermen 34x too common.
        let weights: Vec<_> = Biome::NETHER_WASTES
            .spawners
            .monster
            .iter()
            .map(|s| (s.r#type, s.weight))
            .collect();
        assert_eq!(
            weights,
            [
                ("minecraft:ghast", 50),
                ("minecraft:zombified_piglin", 100),
                ("minecraft:magma_cube", 2),
                ("minecraft:enderman", 1),
                ("minecraft:piglin", 15),
            ]
        );
    }

    #[test]
    fn spawners_are_picked_by_weight() {
        use rand::{SeedableRng, rngs::StdRng};

        let spawner = |r#type, weight| Spawner {
            r#type,
            min_count: 1,
            max_count: 1,
            weight,
        };
        let mut rng = StdRng::seed_from_u64(0);

        let spawners = [
            spawner("minecraft:common", 99),
            spawner("minecraft:rare", 1),
        ];
        let rare = (0..10_000)
            .filter(|_| {
                choose_spawner(&spawners, &mut rng).map(|s| s.r#type) == Some("minecraft:rare")
            })
            .count();
        // Expect about 100; a uniform pick would give about 5000.
        assert!(
            (50..=150).contains(&rare),
            "rare spawner picked {rare} times"
        );

        let spawners = [
            spawner("minecraft:never", 0),
            spawner("minecraft:always", 1),
        ];
        for _ in 0..1_000 {
            assert_eq!(
                choose_spawner(&spawners, &mut rng).map(|s| s.r#type),
                Some("minecraft:always")
            );
        }
        assert!(choose_spawner(&spawners[..1], &mut rng).is_none());
        assert!(choose_spawner(&[], &mut rng).is_none());
    }

    #[test]
    fn test_signal_source_parity() {
        assert!(is_signal_source(&Block::REDSTONE_WIRE));
        assert!(is_signal_source(&Block::REDSTONE_TORCH));
        assert!(is_signal_source(&Block::REDSTONE_WALL_TORCH));
        assert!(is_signal_source(&Block::REPEATER));
        assert!(is_signal_source(&Block::COMPARATOR));
        assert!(is_signal_source(&Block::LEVER));
        assert!(is_signal_source(&Block::DAYLIGHT_DETECTOR));
        assert!(is_signal_source(&Block::OBSERVER));
        assert!(is_signal_source(&Block::REDSTONE_BLOCK));
        assert!(is_signal_source(&Block::TARGET));
        assert!(is_signal_source(&Block::SCULK_SENSOR));
        assert!(is_signal_source(&Block::CALIBRATED_SCULK_SENSOR));
        assert!(is_signal_source(&Block::TRIPWIRE_HOOK));
        assert!(is_signal_source(&Block::LIGHTNING_ROD));
        assert!(is_signal_source(&Block::STONE_BUTTON));
        assert!(is_signal_source(&Block::OAK_BUTTON));
        assert!(is_signal_source(&Block::STONE_PRESSURE_PLATE));
        assert!(is_signal_source(&Block::OAK_PRESSURE_PLATE));
        assert!(is_signal_source(&Block::LIGHT_WEIGHTED_PRESSURE_PLATE));
        assert!(is_signal_source(&Block::HEAVY_WEIGHTED_PRESSURE_PLATE));

        assert!(!is_signal_source(&Block::AIR));
        assert!(!is_signal_source(&Block::STONE));
        assert!(!is_signal_source(&Block::GRASS_BLOCK));
        assert!(!is_signal_source(&Block::GLASS));
        assert!(!is_signal_source(&Block::OAK_PLANKS));
    }

    #[test]
    fn test_empty_spawn_block_rejects_signal_sources_and_solid_blocks() {
        assert!(is_valid_empty_spawn_block(Block::AIR.default_state, &EntityType::CREEPER));
        assert!(!is_valid_empty_spawn_block(Block::STONE.default_state, &EntityType::CREEPER));
        assert!(!is_valid_empty_spawn_block(Block::REDSTONE_WIRE.default_state, &EntityType::CREEPER));
        assert!(!is_valid_empty_spawn_block(Block::REDSTONE_TORCH.default_state, &EntityType::CREEPER));
        assert!(!is_valid_empty_spawn_block(Block::OAK_BUTTON.default_state, &EntityType::CREEPER));
        assert!(!is_valid_empty_spawn_block(Block::STONE_PRESSURE_PLATE.default_state, &EntityType::CREEPER));
        assert!(!is_valid_empty_spawn_block(Block::LEVER.default_state, &EntityType::CREEPER));
        assert!(!is_valid_empty_spawn_block(Block::WATER.default_state, &EntityType::CREEPER));
    }

    #[test]
    fn test_block_danger_immunities() {
        // Fox is immune to sweet berry bushes
        assert!(!is_block_dangerous(&EntityType::FOX, Block::SWEET_BERRY_BUSH.default_state));
        assert!(is_block_dangerous(&EntityType::CREEPER, Block::SWEET_BERRY_BUSH.default_state));
        assert!(is_block_dangerous(&EntityType::ZOMBIE, Block::SWEET_BERRY_BUSH.default_state));

        // Polar bear, Stray, Snow Golem are immune to powder snow
        assert!(!is_block_dangerous(&EntityType::POLAR_BEAR, Block::POWDER_SNOW.default_state));
        assert!(!is_block_dangerous(&EntityType::STRAY, Block::POWDER_SNOW.default_state));
        assert!(!is_block_dangerous(&EntityType::SNOW_GOLEM, Block::POWDER_SNOW.default_state));
        assert!(is_block_dangerous(&EntityType::CREEPER, Block::POWDER_SNOW.default_state));

        // Wither skeleton and Wither are immune to wither rose
        assert!(!is_block_dangerous(&EntityType::WITHER_SKELETON, Block::WITHER_ROSE.default_state));
        assert!(!is_block_dangerous(&EntityType::WITHER, Block::WITHER_ROSE.default_state));
        assert!(is_block_dangerous(&EntityType::SKELETON, Block::WITHER_ROSE.default_state));
        assert!(is_block_dangerous(&EntityType::CREEPER, Block::WITHER_ROSE.default_state));

        // Camel is immune to cactus
        assert!(!is_block_dangerous(&EntityType::CAMEL, Block::CACTUS.default_state));
        assert!(is_block_dangerous(&EntityType::CREEPER, Block::CACTUS.default_state));

        // Fire immune entities
        assert!(!is_block_dangerous(&EntityType::BLAZE, Block::LAVA.default_state));
        assert!(!is_block_dangerous(&EntityType::BLAZE, Block::MAGMA_BLOCK.default_state));
        assert!(is_block_dangerous(&EntityType::CREEPER, Block::LAVA.default_state));
        assert!(is_block_dangerous(&EntityType::CREEPER, Block::MAGMA_BLOCK.default_state));
    }

    #[test]
    fn test_local_mob_cap_calculation_parity() {
        let local_calc = LocalMobCapCalculator::default();
        let chunk_pos = Vector2::new(0, 0);

        // Pre-populate players near chunk (player 100 and player 200)
        let player_ids: Arc<[i32]> = Arc::new([100, 200]);
        local_calc.players_near_chunk.insert(chunk_pos, player_ids);

        // Initial state: both players have 0 mobs, can_spawn should be true
        {
            let counts_100 = local_calc.player_mob_counts.entry(100).or_default();
            assert!(counts_100.can_spawn(&MobCategory::MONSTER));
        }

        // Saturate player 100 to monster cap (70)
        {
            let counts_100 = local_calc.player_mob_counts.entry(100).or_default();
            for _ in 0..70 {
                counts_100.add(&MobCategory::MONSTER);
            }
            assert!(!counts_100.can_spawn(&MobCategory::MONSTER));
        }

        // Player 200 is still under cap (0 < 70)
        {
            let counts_200 = local_calc.player_mob_counts.entry(200).or_default();
            assert!(counts_200.can_spawn(&MobCategory::MONSTER));
        }

        // Java rule: if ANY player near chunk is under cap, chunk can spawn
        let mut can_spawn = false;
        for &pid in &[100, 200] {
            if let Some(count) = local_calc.player_mob_counts.get(&pid) {
                if count.can_spawn(&MobCategory::MONSTER) {
                    can_spawn = true;
                    break;
                }
            } else {
                can_spawn = true;
                break;
            }
        }
        assert!(can_spawn, "Chunk must allow spawning when at least one player is under cap");

        // Saturate player 200 as well
        {
            let counts_200 = local_calc.player_mob_counts.entry(200).or_default();
            for _ in 0..70 {
                counts_200.add(&MobCategory::MONSTER);
            }
            assert!(!counts_200.can_spawn(&MobCategory::MONSTER));
        }

        // Now both players are saturated: chunk must deny spawning
        let mut both_saturated_can_spawn = false;
        for &pid in &[100, 200] {
            if let Some(count) = local_calc.player_mob_counts.get(&pid) {
                if count.can_spawn(&MobCategory::MONSTER) {
                    both_saturated_can_spawn = true;
                    break;
                }
            } else {
                both_saturated_can_spawn = true;
                break;
            }
        }
        assert!(!both_saturated_can_spawn, "Chunk must deny spawning when all nearby players are saturated");

        // Decrement player 100: can_spawn becomes true again
        {
            let counts_100 = local_calc.player_mob_counts.entry(100).or_default();
            counts_100.remove(&MobCategory::MONSTER);
            assert!(counts_100.can_spawn(&MobCategory::MONSTER));
        }
    }

    #[test]
    fn test_global_mob_cap_scaling_parity() {
        // 1 player with full 17x17 chunk window (289 spawnable chunks)
        let state_1p = SpawnState {
            spawnable_chunk_count: 289,
            mob_category_counts: Arc::new(MobCounts::default()),
            spawn_potential: Arc::new(PotentialCalculator::default()),
            local_mob_cap_calculator: Arc::new(LocalMobCapCalculator::default()),
            last_checked: Arc::new(AtomicCell::new(None)),
        };

        // Monster cap for 289 chunks = 70 * 289 / 289 = 70
        assert!(state_1p.can_spawn_for_category_global(&MobCategory::MONSTER));
        for _ in 0..69 {
            state_1p.mob_category_counts.add(&MobCategory::MONSTER);
        }
        assert!(state_1p.can_spawn_for_category_global(&MobCategory::MONSTER));
        state_1p.mob_category_counts.add(&MobCategory::MONSTER);
        // At 70 mobs, cap is reached
        assert!(!state_1p.can_spawn_for_category_global(&MobCategory::MONSTER));

        // 2 players far apart (578 spawnable chunks)
        let state_2p = SpawnState {
            spawnable_chunk_count: 578,
            mob_category_counts: Arc::new(MobCounts::default()),
            spawn_potential: Arc::new(PotentialCalculator::default()),
            local_mob_cap_calculator: Arc::new(LocalMobCapCalculator::default()),
            last_checked: Arc::new(AtomicCell::new(None)),
        };
        // Monster cap for 578 chunks = 70 * 578 / 289 = 140
        for _ in 0..139 {
            state_2p.mob_category_counts.add(&MobCategory::MONSTER);
        }
        assert!(state_2p.can_spawn_for_category_global(&MobCategory::MONSTER));
        state_2p.mob_category_counts.add(&MobCategory::MONSTER);
        assert!(!state_2p.can_spawn_for_category_global(&MobCategory::MONSTER));
    }

    #[test]
    fn test_filtered_spawning_categories_filtering() {
        let mut state = SpawnState::empty();
        state.set_spawnable_chunk_count(289);

        // Standard hostile + friendly night spawning (spawn_passives = false)
        let categories = get_filtered_spawning_categories(&state, true, true, false);
        let names: Vec<usize> = categories.iter().map(|c| c.id).collect();
        assert!(names.contains(&MobCategory::MONSTER.id));
        assert!(names.contains(&MobCategory::AMBIENT.id));
        assert!(names.contains(&MobCategory::WATER_CREATURE.id));
        // CREATURE is persistent, so it is filtered out when spawn_passives = false
        assert!(!names.contains(&MobCategory::CREATURE.id));

        // Passive 400-tick spawn cycle (spawn_passives = true)
        let categories_passive = get_filtered_spawning_categories(&state, true, true, true);
        let names_passive: Vec<usize> = categories_passive.iter().map(|c| c.id).collect();
        assert!(names_passive.contains(&MobCategory::CREATURE.id));

        // Peaceful (spawn_enemies = false)
        let categories_peaceful = get_filtered_spawning_categories(&state, true, false, false);
        let names_peaceful: Vec<usize> = categories_peaceful.iter().map(|c| c.id).collect();
        assert!(!names_peaceful.contains(&MobCategory::MONSTER.id));

        // Mob spawning disabled (spawn_friendlies = false, spawn_enemies = false)
        let categories_disabled = get_filtered_spawning_categories(&state, false, false, false);
        assert!(categories_disabled.is_empty());
    }
}

