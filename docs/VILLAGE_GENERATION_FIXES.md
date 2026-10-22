# Village Generation & Entity Spawning Fixes (Parity Report)

**Target Version**: Minecraft Java 1.26.3  
**Reference**: `E:\DATA\source\svmc\ServerMiecraft\jvmc` (READ-ONLY)  
**Implementation**: `E:\DATA\source\svmc\ServerMiecraft\Pumpkin`

---

## 1. Problem Statement & Root Cause Analysis

Prior to these fixes, villages generated in the Rust server engine exhibited several critical defects:
1. **Empty Villages (Zero Villagers & Iron Golems)**: Villages generated with houses, paths, and farms, but completely lacked any villagers, iron golems, or village animals.
2. **Entity Coordinate Misplacement & Clipping**: When entities were evaluated, coordinate math applied bounding-box offsets twice, causing them to fall outside the generating chunk's `chunk_box` and be silently discarded by `if !chunk_box.contains_pos(&world_block_pos)`.
3. **Road Disconnections & House Elevation Inconsistencies**: Roads and houses in complex terrain sometimes appeared disconnected or floating if height sampling diverged from Vanilla surface estimators.

---

## 2. Technical Deep Dive: Village Entities in Minecraft 1.26.3

### 2.1. Vanilla Entity Template Architecture
In Minecraft Java 1.26.3, villagers are **not hardcoded** into village house `.nbt` templates. Instead:
- Village houses contain Jigsaw connection blocks targeting the `minecraft:village/<biome>/villagers` template pool.
- The pool contains 1x1x1 jigsaw pieces:
  - `unemployed.nbt`
  - `nitwit.nbt`
  - `baby.nbt`
- Each of these small template files contains an entity in its `entities` NBT compound tag.
- Similarly, meeting points contain jigsaw blocks connecting to `minecraft:village/common/iron_golem.nbt` and `village/common/animals/*` (cats, horses, cows, pigs, sheep).

### 2.2. The Coordinate Transformation Bug in Pumpkin

#### Before the Fix:
In `crates/pumpkin-world/src/generation/structure/structures/jigsaw.rs`:
```rust
let corner = self.rotation.rotate_offset(
    template.size.x.saturating_sub(1),
    template.size.z.saturating_sub(1),
);
let placement_origin = pumpkin_util::math::vector3::Vector3::new(
    origin.x + corner.0.min(0),
    origin.y,
    origin.z + corner.1.min(0),
);
...
crate::generation::structure::template::place_template_entities(
    chunk,
    &template,
    placement_origin, // BUG 1: passed placement_origin which is min-corner
    self.rotation,
    chunk_box,
);
```
And inside `place_template_entities` (`template/mod.rs`):
```rust
let block_pos = rotation.transform_pos(entity.block_pos, template.size); // BUG 2: transform_pos assumes rotation inside [0..size]
let world_block_pos = Vector3::new(
    origin.x + block_pos.x,
    origin.y + block_pos.y,
    origin.z + block_pos.z,
);
if !chunk_box.contains_pos(&world_block_pos) {
    continue; // BUG 3: Due to double-offsetting, pos fell outside chunk_box and was discarded!
}
```

#### Vanilla Java Mathematical Reality (`StructureTemplate.java:483-513`):
In Vanilla Java:
```java
BlockPos blockPos = transform(entityInfo.blockPos, mirror, rotation, pivot).offset(position);
Vec3 relativePos = transform(entityInfo.pos, mirror, rotation, pivot);
Vec3 pos = relativePos.add(position.getX(), position.getY(), position.getZ());
```
Where `position` is the piece origin (`self.pos.0`), `pivot` is `(0, 0, 0)`, and `mirror` is `Mirror.NONE`.
The exact transformation for `BlockPos` and `Vec3` rotates around `pivot = (0, 0, 0)`:
- `Clockwise90`:
  - `BlockPos`: `(-z, y, x)`
  - `Vec3`: `(1.0 - z, y, x)`
- `Rotate180`:
  - `BlockPos`: `(-x, y, -z)`
  - `Vec3`: `(1.0 - x, y, 1.0 - z)`
- `CounterClockwise90`:
  - `BlockPos`: `(z, y, -x)`
  - `Vec3`: `(z, y, 1.0 - x)`

#### The Solution Implemented:
1. Updated `place_template_entities` in `crates/pumpkin-world/src/generation/structure/template/mod.rs`:
   - Calls `StructureTemplate::transform_block_pos(entity.block_pos, Mirror::None, rotation, Vector3::new(0, 0, 0)) + origin` for block containment checks.
   - Calls `StructureTemplate::transform_vec3(entity.pos, Mirror::None, rotation, Vector3::new(0, 0, 0))` to obtain accurate fractional double world coordinates.
   - Updates `Pos` tag with `[world_pos.x, world_pos.y, world_pos.z]`.
   - Updates `block_pos` tag with integer coordinates if present.
   - Correctly rotates yaw by rotation angle `(0, 90, 180, 270)`.
2. In `jigsaw.rs:880`:
   - Passes `origin` (`self.pos.0`) directly into `place_template_entities`.

---

## 3. Terrain Adaptation & Village Foundation Parity

### 3.1. Beardifier (`BEARD_THIN`) Integration
- Village structures have `terrain_adaptation: BEARD_THIN`.
- For each rigid village piece (houses, towers), a `BeardifierStructure` record is registered with:
  - `bounding_box`: The piece's bounding box.
  - `ground_level_delta`: Delta between box bottom and floor level.
  - `terrain_adaptation`: `TerrainAdaptation::BeardThin`.
- During the `Noise` generation stage (`proto_chunk.rs:step_to_noise`), `Beardifier` computes:
  ```rust
  let contrib = get_beard_contribution(dx, dy, dz, dy_to_ground) * 0.8;
  ```
  Where `get_beard_contribution` samples the 24x24x24 precomputed exponential falloff kernel (`BEARD_KERNEL`). This adds density beneath house perimeters so structures have a natural earthy/stony base and do not float over hollow gaps.

### 3.2. Street Projection & Heightmap Conformance
- Village roads use `JigsawProjection::TerrainMatching`.
- In `jigsaw_placement.rs`:
  ```rust
  let target_box_y = if source_rigid && target_rigid {
      source_box_y + delta_y
  } else {
      if source_jigsaw_base_height == i32::MIN {
          source_jigsaw_base_height = height_sampler.estimate_height(source_jigsaw_pos.0.x, source_jigsaw_pos.0.z);
      }
      source_jigsaw_base_height - target_jigsaw_local_y
  };
  ```
- Each path block is gravity-projected onto the finished terrain surface (`HeightmapType::WorldSurfaceWg`), properly adapting to hills and dips.
- Verified across 1024 simulated terrain cases in unit test `village_path_projection_1024_cases`.

---

## 4. Entity Lifecycle & Persistence

1. **Staged Generation**:
   - Entities generated during structure placement are accumulated in `ProtoChunk::pending_structure_entities`.
   - When the chunk advances to `StagedChunkEnum::Spawn` (`proto_chunk.rs:1188`), `block_registry.spawn_structure_entities(entities)` executes.
2. **Biome-Adaptive Villager Types**:
   - In `pumpkin-core/src/world/mod.rs` & `villager/mod.rs:2819`, `finalize_spawn` samples the biome at the entity's position:
     ```rust
     let biome = world.level.get_rough_biome(&pos);
     let villager_type = VillagerType::from_biome_name(biome.registry_id);
     data.r#type = VarInt(villager_type as i32);
     self.set_villager_data(data);
     ```
   - Desert villages spawn Desert Villagers, Savanna villages spawn Savanna Villagers, Taiga villages spawn Taiga Villagers, and Plains villages spawn Plains Villagers.
3. **Persistence Guarantee**:
   - In `spawn_structure_entities`:
     ```rust
     if let Some(mob) = entity.get_mob() {
         mob.get_mob_entity().persistence_required.store(true, Ordering::Relaxed);
     }
     ```
   - Structure-spawned mobs will not despawn when players leave the chunk or during server restarts.
