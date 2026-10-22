# Minecraft 1.26.3 Structure Parity Matrix: Vanilla Java (`jvmc`) vs Pumpkin (Rust)

**Date**: October 2026  
**Minecraft Version**: 1.26.3 | **Protocol**: 777 | **World Version**: 5023  
**Reference Source**: `E:\DATA\source\svmc\ServerMiecraft\jvmc` (READ-ONLY)  
**Target Engine**: `E:\DATA\source\svmc\ServerMiecraft\Pumpkin`

---

## 1. Structure Parity Overview

This matrix catalogs all Vanilla Minecraft structures, tracking their generator engine, terrain adaptation mode, placement type, template processors, embedded entities, and parity status between Java reference source and Rust implementation.

| Status Indicator | Definition |
| :--- | :--- |
| 🟢 **Full Parity (100%)** | Algorithm, template placement, coordinate transform, and entities match Vanilla Java exactly. |
| 🟡 **High Parity (>90%)** | Structural generation matches; specific minor decor or processor behavior under ongoing verification. |
| 🔵 **Procedural Parity** | Code-driven procedural generation (Mineshafts, Monuments, Strongholds) verified with Vanilla parity tests. |

---

## 2. Comprehensive Structure Matrix

| Structure ID | Generator Engine | Java Class (`jvmc`) | Rust Module (`Pumpkin`) | Terrain Adaptation | Spacing / Separation | Embedded Entities | Parity Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `minecraft:village_plains` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BEARD_THIN` | 34 / 8 (Salt: 10387312) | Villagers (Plains), Iron Golem, Cats, Horses, Sheep, Pigs, Cows | 🟢 Full Parity |
| `minecraft:village_desert` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BEARD_THIN` | 34 / 8 (Salt: 10387312) | Villagers (Desert), Camels, Cats | 🟢 Full Parity |
| `minecraft:village_savanna` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BEARD_THIN` | 34 / 8 (Salt: 10387312) | Villagers (Savanna), Iron Golem, Animals | 🟢 Full Parity |
| `minecraft:village_snowy` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BEARD_THIN` | 34 / 8 (Salt: 10387312) | Villagers (Snowy), Iron Golem, Animals | 🟢 Full Parity |
| `minecraft:village_taiga` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BEARD_THIN` | 34 / 8 (Salt: 10387312) | Villagers (Taiga), Iron Golem, Animals | 🟢 Full Parity |
| `minecraft:pillager_outpost` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BEARD_THIN` | 80 / 24 (Salt: 165745296) | Pillagers, Iron Golem (Cage), Allays (Cage) | 🟢 Full Parity |
| `minecraft:ancient_city` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BEARD_BOX` | 24 / 8 (Salt: 20083232) | Wardens (via Sculk Shriekers) | 🟢 Full Parity |
| `minecraft:bastion_remnant` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `NONE` | 27 / 4 (Salt: 30084232) | Piglins, Piglin Brutes, Hoglins | 🟢 Full Parity |
| `minecraft:trail_ruins` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `BURY` | 34 / 8 (Salt: 83469867) | Suspicious Gravel/Sand Block Entities | 🟢 Full Parity |
| `minecraft:trial_chambers` | Jigsaw | `JigsawStructure.java` | `structures/jigsaw.rs` | `ENCAPSULATE` | 34 / 12 (Salt: 94251324) | Trial Spawners, Breezes, Vaults | 🟢 Full Parity |
| `minecraft:mansion` | Template | `WoodlandMansionStructure.java` | `structures/mansion/` | `NONE` | 80 / 20 (Salt: 10387319) | Evokers, Vindicators, Allays | 🟢 Full Parity |
| `minecraft:igloo` | Template | `IglooStructure.java` | `structures/igloo.rs` | `NONE` | 32 / 8 (Salt: 14357618) | Villager & Zombie Villager (Basement) | 🟢 Full Parity |
| `minecraft:desert_pyramid` | Template | `DesertPyramidStructure.java` | `structures/desert_pyramid.rs` | `NONE` | 32 / 8 (Salt: 14357617) | Suspicious Sand Entities | 🟢 Full Parity |
| `minecraft:jungle_pyramid` | Template | `JungleTempleStructure.java` | `structures/jungle_pyramid.rs` | `NONE` | 32 / 8 (Salt: 14357619) | Dispensers & Arrow Traps | 🟢 Full Parity |
| `minecraft:swamp_hut` | Template | `SwampHutStructure.java` | `structures/swamp_hut.rs` | `NONE` | 32 / 8 (Salt: 14357620) | Witch, Cat | 🟢 Full Parity |
| `minecraft:ocean_monument` | Procedural | `OceanMonumentStructure.java` | `structures/ocean_monument/` | `NONE` | 32 / 5 (Salt: 10387313) | Elder Guardians (3), Guardians | 🔵 Procedural Parity |
| `minecraft:mineshaft` | Procedural | `MineshaftStructure.java` | `structures/mineshaft.rs` | `NONE` | 0.004 Frequency | Cave Spiders (Spawner), Minecarts with Chests | 🔵 Procedural Parity |
| `minecraft:stronghold` | Procedural | `StrongholdStructure.java` | `structures/stronghold/` | `NONE` | Concentric Rings (128 total) | Silverfish (Spawner), End Portal | 🔵 Procedural Parity |
| `minecraft:end_city` | Jigsaw/Procedural | `EndCityStructure.java` | `structures/end_city/` | `NONE` | 20 / 11 (Salt: 10387313) | Shulkers, Elytra (Item Frame) | 🟢 Full Parity |
| `minecraft:nether_fortress` | Procedural | `NetherFortressStructure.java` | `structures/nether_fortress.rs` | `NONE` | 27 / 4 (Salt: 30084232) | Blazes (Spawner), Wither Skeletons | 🔵 Procedural Parity |
| `minecraft:ocean_ruin` | Template | `OceanRuinStructure.java` | `structures/ocean_ruin.rs` | `NONE` | 20 / 8 (Salt: 14357621) | Drowned | 🟢 Full Parity |
| `minecraft:shipwreck` | Template | `ShipwreckStructure.java` | `structures/shipwreck.rs` | `NONE` | 24 / 4 (Salt: 165745295) | Loot Chests | 🟢 Full Parity |
| `minecraft:buried_treasure` | Procedural | `BuriedTreasureStructure.java` | `structures/buried_treasure.rs` | `NONE` | 0.01 Frequency | Buried Chest | 🟢 Full Parity |

---

## 3. Detailed Component Breakdown

### 3.1. Terrain Adaptation Categories
1. **`BEARD_THIN`**:
   - Applied to: Villages, Pillager Outposts.
   - Behavior: Adds noise density below structure foundations (`get_beard_contribution * 0.8`), ensuring houses and platforms have natural ground underneath without carving flat cliffs.
2. **`BEARD_BOX`**:
   - Applied to: Ancient Cities.
   - Behavior: Adds density both underneath and around walls to hollow out underground caverns cleanly.
3. **`ENCAPSULATE`**:
   - Applied to: Trial Chambers.
   - Behavior: Encapsulates chambers in terrain with `get_bury_contribution / 2.0 * 0.8`.
4. **`BURY`**:
   - Applied to: Trail Ruins.
   - Behavior: Buries structure entirely within surface gravel/mud layers.

### 3.2. Jigsaw Structure Processing Rules
- **Connector Matching**: Handled via `name` and `target` pool keys matching, with joint type orientation constraint.
- **Selection Priority**: Jigsaw blocks with higher `selection_priority` are expanded first.
- **Placement Priority**: High placement priority pieces claim space before lower ones.
- **Pool Fallback**: Fallback pools are evaluated when pieces from the primary target pool fail collision bounds checks.
