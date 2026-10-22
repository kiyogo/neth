# Java vs Rust Persistence Parity Matrix & Specification

## 1. Scope & References
- **Target Java Version:** Minecraft Java Edition 1.21.9 / Snapshot 26.3 (Protocol Version 777, Data Version 5023)
- **Reference Codebase (`jvmc`):** `E:\DATA\source\svmc\ServerMiecraft\jvmc`
- **Target Codebase (`Neth`):** `E:\DATA\source\svmc\ServerMiecraft\Neth`

---

## 2. Core Class & Module Mapping

| Minecraft Java Class (`jvmc`) | Rust Struct / Module (`Neth`) | Parity Status | Parity Notes |
|---|---|---|---|
| `net.minecraft.util.DirectoryLock` | `pumpkin_world::session_lock::DirectoryLock` | **Implemented** | Snowman UTF-8 `\u2603` bytes + OS-level exclusive file lock (`LockFileEx` / `flock`). |
| `net.minecraft.util.Util.safeReplaceFile` | `pumpkin_world::fs_util::safe_replace_file` | **Implemented** | 10-retry loop, atomic backup rename, target delete, replace rename, rollback on failure. |
| `net.minecraft.world.level.storage.LevelStorageSource` | `pumpkin_world::world_info::anvil::AnvilLevelInfo` | **Parity Achieved** | `level.dat` GZip NBT parsing, fallback to `level.dat_old`, corrupted backup, auxiliary data files. |
| `net.minecraft.world.level.storage.PlayerDataStorage` | `pumpkin_world::data::player_data::PlayerDataStorage` | **Parity Achieved** | `<UUID>.dat` saving, `<UUID>.dat_old` atomic rotation, corrupted fallback, forensic backup. |
| `net.minecraft.world.level.chunk.storage.RegionFile` | `pumpkin_world::chunk::format::anvil::AnvilRegion` | **Full Parity** | 8KB header, 1024 locations/timestamps, 4KB sectors, bitmap allocator, `.mcc` oversized external chunks. |
| `net.minecraft.world.level.chunk.storage.RegionFileVersion` | `pumpkin_world::chunk::format::anvil::Compression` | **Full Parity** | GZip (1), ZLib (2), None (3), LZ4 (4), Custom (127). |
| `net.minecraft.nbt.NbtIo` | `pumpkin_nbt::nbt_compress` & `pumpkin_nbt::deserializer` | **Full Parity** | Big-endian NBT tags, CESU-8 strings, depth limit 512, byte/int/long arrays. |

---

## 3. Directory Lock (`session.lock`) Parity

### Java Specification (`DirectoryLock.java`)
1. Path: `<world_dir>/session.lock`
2. Payload: UTF-8 encoding of snowman character `"☃"` (`\u2603`), consisting of 3 bytes: `0xE2, 0x98, 0x83`.
3. Flush: `FileChannel.force(true)` immediately after writing.
4. Concurrency: `FileChannel.tryLock()` acquires an exclusive file lock.
5. Error handling: Throws `DirectoryLock.LockException.alreadyLocked` if lock fails or returns `null`.
6. Detection: `DirectoryLock.isLocked(path)` tests `tryLock()`.

### Rust Implementation (`session_lock.rs`)
1. Creates or opens `session.lock` with read/write access.
2. Writes `&[0xE2, 0x98, 0x83]`.
3. Calls `sync_all()`.
4. Acquires non-blocking exclusive OS lock:
   - **Windows:** `LockFileEx(handle, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY, 0, u32::MAX, u32::MAX, &mut overlapped)`.
   - **Unix:** `libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB)`.
5. Error returns `DirectoryLockError::AlreadyLocked(path)`.
6. `is_locked(&dir)` probes lock acquisition without retaining the handle.
7. `Drop` implementation safely unlocks and closes the file descriptor.

---

## 4. Atomic File Replacement (`safeReplaceFile`) Parity

### Java Flow (`Util.java:944-963`)
```
safeReplaceFile(targetPath, newPath, backupPath)
│
├── 1. If targetPath exists:
│      runWithRetries(10, "create backup"):
│         - delete backupPath if exists
│         - rename targetPath -> backupPath
│         - verify backupPath exists
│      if failed: return false (abort)
│
├── 2. runWithRetries(10, "remove old"):
│         - delete targetPath if exists
│         - verify targetPath is deleted
│      if failed: return false (abort)
│
├── 3. runWithRetries(10, "replace"):
│         - rename newPath -> targetPath
│         - verify targetPath exists
│
└── 4. If step 3 failed and !noRollback:
          runWithRetries(10, "restore"):
             - rename backupPath -> targetPath
             - verify targetPath exists
          return false
```

### Rust Implementation (`fs_util.rs`)
- Matches the exact 4-step sequence with 10 attempts per operation.
- Provides thread sleep (10ms) between retry attempts to allow Windows file locks (antivirus, search indexers) to clear.
- Full verification after each filesystem mutation.

---

## 5. Metadata & Player Data Parity

### 5.1. `level.dat` Schema & Recovery
- **Root Tag:** Compound tag containing child compound `"Data"`.
- **Key Fields in `"Data"`:**
  - `DataVersion`: integer (e.g. 5023 for 26.3)
  - `version`: integer (e.g. 19133 for Anvil format)
  - `LevelName`: string
  - `generatorName`: string
  - `RandomSeed`: long
  - `WorldGenSettings`: compound (dimensions, seed, structures)
  - `Time`: long, `DayTime`: long
  - `SpawnX`, `SpawnY`, `SpawnZ`: integers
  - `GameType`: integer, `Difficulty`: byte, `DifficultyLocked`: byte
- **Auxiliary Data Files (`data/minecraft/`):**
  - `game_rules.dat`
  - `world_gen_settings.dat`
  - `world_clocks.dat`
  - `weather.dat`
- **Fallback Recovery:**
  - If `level.dat` is corrupt:
    1. Copy `level.dat` to `level.dat_corrupted_<timestamp>`.
    2. Read `level.dat_old`.
    3. If valid, restore `level.dat` from `level.dat_old` using `safe_replace_or_move_file`.

### 5.2. Player Data Schema & Recovery
- **File Paths:**
  - Normal: `playerdata/<UUID>.dat`
  - Backup: `playerdata/<UUID>.dat_old`
  - Corrupt Archive: `playerdata/<UUID>_corrupted_<timestamp>.dat`
- **Save Operation:**
  - Serialize to temp file `<UUID>.dat.tmp.<pid>.<seq>`.
  - Atomically rotate `<UUID>.dat` -> `<UUID>.dat_old` and `<UUID>.dat.tmp.*` -> `<UUID>.dat`.
- **Load Operation:**
  - Attempt reading `<UUID>.dat`.
  - On corruption:
    1. Forensic backup to `<UUID>_corrupted_<timestamp>.dat`.
    2. Read `<UUID>.dat_old`.

---

## 6. Region File & Chunk Parity

### 6.1. Header & Sector Invariants
| Metric | Specification | Verification |
|---|---|---|
| Header Size | 8,192 bytes | `locations` (4096) + `timestamps` (4096) |
| Chunk Offset Entry | Big-Endian 4 bytes | `(sector_offset << 8) \| sector_count` |
| Sector Size | 4,096 bytes | All chunk data padded to multiple of 4,096 |
| Chunk Header | 5 bytes | `length` (4 bytes BE) + `compression_id` (1 byte) |
| External Chunk Threshold | 256 sectors ($> 1\text{ MiB}$) | Writes stub (1 sector) + external file `c.X.Z.mcc` |
| External Chunk Flag | `0x80` (`128`) | Bit 7 of `compression_id` |
