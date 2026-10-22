# WorldGen & Structure Differential Testing Suite

**Target**: Minecraft Java 1.26.3 Parity  
**Test Harness**: Rust `cargo test` suite in `crates/pumpkin-world`  
**Reference Codebase**: `E:\DATA\source\svmc\ServerMiecraft\jvmc` (READ-ONLY)

---

## 1. Test Architecture Overview

To guarantee that the Rust engine's world generation, jigsaw algorithms, and entity placement match Vanilla Minecraft Java 1.26.3, a comprehensive multi-layered differential test suite is implemented in `pumpkin-world`.

```
                    ┌───────────────────────────────────┐
                    │    Vanilla Java 1.26.3 (jvmc)     │
                    └─────────────────┬─────────────────┘
                                      │ (Reference Math & Specs)
                                      ▼
                    ┌───────────────────────────────────┐
                    │     Pumpkin World Engine (Rust)   │
                    └─────────────────┬─────────────────┘
                                      │
         ┌────────────────────────────┼────────────────────────────┐
         ▼                            ▼                            ▼
┌──────────────────┐        ┌──────────────────┐        ┌──────────────────┐
│  Jigsaw Expansion│        │  Street Gravity  │        │  Entity Spawning │
│  & Collision     │        │  Projection      │        │  & Persistence   │
│  (1024 cases)    │        │  (1024 cases)    │        │  (Unit tests)    │
└──────────────────┘        └──────────────────┘        └──────────────────┘
```

---

## 2. Key Test Suites & Execution Commands

### 2.1. Village Jigsaw Graph & Entity Spawning Test
- **Test Target**: `generation::structure::structures::jigsaw::tests::test_village_plains_templates_and_pieces`
- **Location**: `crates/pumpkin-world/src/generation/structure/structures/jigsaw.rs:1367-1462`
- **What it Validates**:
  1. Instantiates a Plains Village jigsaw graph at seed `SEED = 123456789`.
  2. Expands all template pools up to max depth.
  3. Asserts that villager templates (`unemployed.nbt`, etc.) and iron golem templates are generated into the piece graph.
  4. Generates pieces into proto-chunks covering the village bounding box.
  5. Asserts that `chunk.take_pending_structure_entities()` produces valid `minecraft:villager` and `minecraft:iron_golem` entities with valid `Pos` tags.
- **Run Command**:
  ```powershell
  cargo test --package pumpkin-world --lib test_village_plains
  ```
- **Result**: **PASSED (1.80s)**

---

### 2.2. Village Street Projection Differential Suite (1024 Cases)
- **Test Target**: `generation::structure::template::placement_tests::village_path_projection_1024_cases`
- **Location**: `crates/pumpkin-world/src/generation/structure/template/placement_tests.rs:440-680`
- **What it Validates**:
  1. Tests all 5 village biome street pools (`plains`, `desert`, `savanna`, `snowy`, `taiga`).
  2. Evaluates every piece across all 4 rotations (`None`, `Clockwise90`, `Rotate180`, `CounterClockwise90`).
  3. Tests across synthetic hilly, sloped, obstructed, and stepped terrain surfaces.
  4. Validates that every path block matches the expected gravity-projected surface height without gaps or cutoffs.
- **Run Command**:
  ```powershell
  cargo test --package pumpkin-world --lib village_path_projection
  ```
- **Result**: **PASSED (23.56s)**

---

### 2.3. Terrain-Following Surface Test
- **Test Target**: `generation::proto_chunk::village_path_tests::village_paths_follow_generated_surface`
- **Location**: `crates/pumpkin-world/src/generation/village_path_tests.rs`
- **What it Validates**:
  1. Verifies that village streets placed into real proto-chunks follow `flat_surface_wg_height_map` exactly.
  2. Ensures that obstacles like grass or flowers above the solid terrain do not push path blocks upwards.
- **Run Command**:
  ```powershell
  cargo test --package pumpkin-world --lib village_path_tests
  ```
- **Result**: **PASSED (20.06s)**

---

### 2.4. Template Placement & Processing Suite (9 Tests)
- **Test Target**: `generation::structure::template::placement_tests`
- **Location**: `crates/pumpkin-world/src/generation/structure/template/placement_tests.rs`
- **Sub-Tests**:
  1. `projected_blocks_are_clipped_after_processing`: Verifies chunk boundary clipping.
  2. `capped_gravity_counts_position_changes`: Verifies gravity limits.
  3. `projected_jigsaws_respect_keep_jigsaws_and_legacy_air`: Verifies jigsaw final-state replacement.
  4. `gravity_obeys_heightmap_selection_and_empty_columns`: Verifies heightmap selection.
  5. `terrain_heightmaps_freeze_after_carving_and_survive_cleared_columns`: Verifies carving snapshot preservation.
  6. `processors_observe_positions_in_order_and_waterlogging_uses_the_final_state`: Verifies waterlogging logic.
  7. `gravity_uses_a_stable_surface_and_preserves_local_heights`: Verifies relative elevation stability.
  8. `village_paths_ignore_blocks_above_finished_terrain`: Verifies non-solid block bypass.
  9. `village_path_projection_1024_cases`: Verifies 1024-case matrix.
- **Run Command**:
  ```powershell
  cargo test --package pumpkin-world --lib placement_tests
  ```
- **Result**: **9 PASSED; 0 FAILED (23.52s)**

---

## 3. Summary of Test Results

| Test Target | Category | Cases Checked | Outcome |
| :--- | :--- | :--- | :--- |
| `test_village_plains_templates_and_pieces` | Jigsaw & Mob Spawning | 1 Full Village Graph | ✅ **PASSED** |
| `village_path_projection_1024_cases` | Road Height Parity | 1024 Terrain Variations | ✅ **PASSED** |
| `village_paths_follow_generated_surface` | Surface Conformance | Multi-chunk Surface | ✅ **PASSED** |
| `placement_tests` Suite | Template & Processors | 9 Sub-tests | ✅ **PASSED** |
| `cargo check --bin pumpkin` | Binary Compilation | All Crates & Dependencies | ✅ **PASSED** |
