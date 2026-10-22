# Technical Audit: Minecraft Vanilla Mob Spawning Parity for Neth (Rust)

**Reference Target:** Minecraft Java Edition (decompiled at `E:\DATA\source\svmc\ServerMiecraft\jvmc`)  
**Implementation:** Neth Server (`E:\DATA\source\svmc\ServerMiecraft\Neth`)  
**Target Protocol/Version:** Minecraft 1.21.9 / 26.3 (`SharedConstants.RELEASE_NETWORK_PROTOCOL_VERSION = 777`)  
**Audit Scope:** Natural Mob Spawning, Chunk Generation Spawning, Global & Local Mob Caps, Spawn Predicates, Light Engine Integration, Biome & Dimension Rules.

---

## 1. Executive Summary

Mob spawning in Minecraft Java Edition is governed by a tightly coupled interaction between world chunk ticking, player distance tracking, global and local density limits (mob caps), light level tests, spawn floor collision predicates, and mob-specific rules.

An in-depth audit of `crates/pumpkin-core/src/world/natural_spawner.rs`, `crates/pumpkin-core/src/world/mod.rs`, `crates/pumpkin-core/src/entity/mob/`, and `crates/pumpkin-core/src/entity/type.rs` against Java reference sources (`NaturalSpawner.java`, `LocalMobCapCalculator.java`, `ServerChunkCache.java`, `ChunkMap.java`, `SpawnPlacements.java`, `SpawnPlacementTypes.java`, `Monster.java`, `Slime.java`, etc.) was conducted.

The audit confirmed high foundational parity in the core mathematical formulas (such as the $17 \times 17$ chunk density box, $128^2 = 16384.0$ player radius, and charge-based spawn potential), but identified 5 critical logic discrepancies and edge cases that deviate from vanilla behavior.

---

## 2. Architecture & Control Flow Comparison

| Component | Java Vanilla (`jvmc`) | Neth Rust (`crates/pumpkin-core`) | Status / Parity Evaluation |
| :--- | :--- | :--- | :--- |
| **Tick Driver** | `ServerChunkCache::tickChunks` runs every world tick. | `World::tick` calls `update_active_chunks` and `tick_environment`. | **PARITY CONFIRMED** |
| **Local Mob Cap Lifecycle** | Re-instantiated freshly every tick (`new LocalMobCapCalculator(this.chunkMap)`). | Fresh `LocalMobCapCalculator::default()` instantiated in `SpawnState::new` inside `update_active_chunks`. | **PARITY CONFIRMED** |
| **Spawnable Chunks** | Chunks within Chebyshev distance $\le 8$ ($17 \times 17$ window) around non-spectator players. | `dx in -8..=8`, `dz in -8..=8` surrounding non-spectator players, deduplicated via `FxHashSet`. | **PARITY CONFIRMED** |
| **Global Mob Cap** | `category.getMaxInstancesPerChunk() * spawnableChunkCount / 289`. | `category.max * self.spawnable_chunk_count / MAGIC_NUMBER` where `MAGIC_NUMBER = 17 * 17 = 289`. | **PARITY CONFIRMED** |
| **Local Mob Cap Radius** | Euclidean distance squared to chunk center $< 128^2 = 16384.0$. | `calc_distance(chunk_pos, player_pos) < 16384.0` using center coordinates `(cx << 4) + 8`. | **PARITY CONFIRMED** |
| **Multi-player Local Cap** | If *any* player in range has count `< category.max`, allow spawn. If no players in range, deny. | Iterates `get_players_near`; returns `true` if any player is under cap; returns `false` if empty. | **PARITY CONFIRMED** |
| **Creature Spawning Period** | `level.getGameTime() % 400L == 0L` (every 20 seconds). | `self.get_world_age() % 400 == 0` for `category.is_persistent`. | **PARITY CONFIRMED** |
| **Chunk Eligibility Gate** | `ServerChunkCache.java:412`: `this.level.canSpawnEntitiesInChunk(chunkPos)`. | Marked with `// TODO this.level.canSpawnEntitiesInChunk(chunkPos)` in `world/mod.rs:2994`. | **GAP IDENTIFIED (Fixed below)** |
| **Player Distance Exclusion** | Distance squared to nearest player $\le 576.0$ ($24^2$) denies spawn. | `nearest_player_distance_sq <= 576.0` rejects attempt. | **PARITY CONFIRMED** |
| **World Spawn Exclusion** | Rejects spawn within 24 blocks of world spawn in Overworld. | Rejects spawn within Euclidean distance $< 24.0$ of spawn in Overworld. | **PARITY CONFIRMED** |

---

## 3. Discrepancies and Gaps Identified

### Gap 1: Chunk Spawning Eligibility Check (`canSpawnEntitiesInChunk`)
- **Vanilla Java (`ServerChunkCache.java:412` & `ServerLevel.java:1950`):**
  A chunk is only ticked for mob spawning if `level.canSpawnEntitiesInChunk(chunkPos)` is true:
  ```java
  public boolean canSpawnEntitiesInChunk(final ChunkPos pos) {
     return this.entityManager.canPositionTick(pos) && this.getWorldBorder().isWithinBounds(pos);
  }
  ```
- **Neth Discrepancy:**
  In `crates/pumpkin-core/src/world/mod.rs:2994`, this check was left as a comment:
  `// TODO this.level.canSpawnEntitiesInChunk(chunkPos)`.
  This allowed chunks outside the world border or in non-ticking status to attempt entity generation.
- **Resolution:**
  Implement `World::can_spawn_entities_in_chunk(&self, chunk_pos: Vector2<i32>) -> bool` checking active chunk status and border inclusion (`contains_block` at both `(cx << 4, cz << 4)` and `((cx << 4) + 15, (cz << 4) + 15)`), and gate `spawn_for_chunk` in `tick_spawning_chunk`.

---

### Gap 2: Signal Sources in Empty Spawn Block Check
- **Vanilla Java (`NaturalSpawner.java:344`):**
  ```java
  public static boolean isValidEmptySpawnBlock(
     final BlockGetter level, final BlockPos pos, final BlockState blockState, final FluidState fluidState, final EntityType<?> type
  ) {
     if (blockState.isCollisionShapeFullBlock(level, pos)) {
        return false;
     } else if (blockState.isSignalSource()) {
        return false;
     } else if (!fluidState.isEmpty()) {
        return false;
     } else {
        return blockState.is(BlockTags.PREVENT_MOB_SPAWNING_INSIDE) ? false : !type.isBlockDangerous(blockState);
     }
  }
  ```
- **Neth Discrepancy:**
  `is_valid_empty_spawn_block` did not check `is_signal_source`. As a result, mobs could spawn inside redstone wire, redstone torches, pressure plates, buttons, levers, repeaters, comparators, daylight detectors, and observer blocks.
- **Resolution:**
  Introduce `is_signal_source(block: &Block)` recognizing all vanilla redstone signal sources and gate `is_valid_empty_spawn_block` accordingly.

---

### Gap 3: Waterlogged Block Detection in Ground Spawns
- **Vanilla Java (`NaturalSpawner.java:346`):**
  Ground spawn blocks must have `fluidState.isEmpty()`. A waterlogged block (such as a waterlogged slab, stair, chest, or fence) has an empty or partial block collision shape, but `!fluidState.isEmpty()`, which immediately rejects ground mob spawning.
- **Neth Discrepancy:**
  Neth checked `state.is_liquid()`. For waterlogged non-liquid blocks, `state.is_liquid()` returns `false`, erroneously allowing ground mobs to spawn inside waterlogged blocks.
- **Resolution:**
  Ensure ground spawning checks `world.get_fluid(pos).id == Fluid::EMPTY.id` and `world.get_fluid(above).id == Fluid::EMPTY.id`.

---

### Gap 4: Block Danger Immunities (`is_block_dangerous`)
- **Vanilla Java (`EntityType.java:338-345`):**
  Each mob checks `state.is(this.immuneTo)`. Specifically:
  - `FOX` is immune to sweet berry bushes (`BlockTags.FOX_IMMUNE_TO`).
  - `POLAR_BEAR`, `STRAY`, `SNOW_GOLEM` are immune to powder snow (`BlockTags.*_IMMUNE_TO`).
  - `WITHER`, `WITHER_SKELETON` are immune to wither roses (`BlockTags.*_IMMUNE_TO`).
  - `CAMEL` is immune to cactus.
- **Neth Discrepancy:**
  Neth's `is_block_dangerous` treated wither roses, sweet berry bushes, and powder snow as universally dangerous to all non-fire-immune mobs, preventing Foxes from spawning in bushes, Strays and Polar Bears from spawning in powder snow, and Wither Skeletons from spawning near wither roses.
- **Resolution:**
  Incorporate exact entity-specific hazard exemptions into `is_block_dangerous`.

---

### Gap 5: Slime Spawning Rate & Solid Floor Check
- **Vanilla Java (`Slime.java:81-86`):**
  ```java
  if (level.getBiome(pos).is(BiomeTags.ALLOWS_SURFACE_SLIME_SPAWNS) && pos.getY() > 50 && pos.getY() < 70) {
     float surfaceSlimeSpawnChance = level.environmentAttributes().getValue(EnvironmentAttributes.SURFACE_SLIME_SPAWN_CHANCE, pos);
     if (random.nextFloat() < surfaceSlimeSpawnChance && level.getMaxLocalRawBrightness(pos) <= random.nextInt(8)) {
        return checkMobSpawnRules(type, level, spawnReason, pos, random);
     }
  }
  ```
- **Neth Discrepancy:**
  In `crates/pumpkin-core/src/entity/mob/slime.rs:82`, an extraneous `&& rand::random_range(0.0..1.0) < 0.5` halved surface swamp slime spawning, and omitted the `check_mob_spawn_rules` solid-floor check.
- **Resolution:**
  Remove the arbitrary 0.5 penalty and enforce `check_mob_spawn_rules(world, pos)` so swamp slime spawning matches vanilla lunar probability and floor requirements.

---

### Gap 6: Water Creature Mob Category Persistence Discrepancy
- **Vanilla Java (`MobCategory.java:12`):**
  ```java
  WATER_CREATURE("water_creature", "WC", 5, true, false, 128),
  ```
  In Minecraft Java Edition, `WATER_CREATURE` has `isFriendly = true` and `isPersistent = false` (squids and dolphins naturally despawn). Only `CREATURE` (farm animals) and `MISC` are marked `isPersistent = true`.
- **Neth Discrepancy:**
  In `crates/pumpkin-data/src/generated/entity_type.rs:127`, `WATER_CREATURE` was configured with `is_persistent: true`. In `get_filtered_spawning_categories`, persistent categories are excluded unless `spawn_passives = true` (which only runs every 400 ticks / 20 seconds). Consequently, water creatures were suppressed during normal world ticks.
- **Resolution:**
  Corrected `WATER_CREATURE.is_persistent` in `crates/pumpkin-data/src/generated/entity_type.rs` to `false`, matching vanilla Java `MobCategory`.

---

## 4. Test Coverage Strategy

To verify 100% parity and prevent regression, an automated test suite must cover:
1. **Local Mob Cap Calculation**: Isolation, saturation, and multi-player proximity handling.
2. **Global Mob Cap Calculation**: Proportional scaling across $17 \times 17$ chunk windows with multiple players.
3. **Empty Spawn Block & Signal Sources**: Buttons, pressure plates, redstone components, and waterlogged blocks rejected.
4. **Mob Danger Immunities**: Foxes in sweet berries, Strays in powder snow, Wither Skeletons in wither roses.
5. **Distance & Spawn Exclusion**: 24-block radius around players and Overworld world spawn.
6. **Fortress Spawning**: Nether bricks floor condition and exact enemy weights.
7. **Slime Spawning Rules**: Lunar phase surface chance and slime chunk Y < 40 mechanics.
