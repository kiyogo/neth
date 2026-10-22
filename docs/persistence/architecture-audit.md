# Neth Persistence Subsystem: Architecture & Vulnerability Audit

## 1. Executive Summary & Scope

This document provides a comprehensive architectural audit of the persistence (world storage, chunk I/O, player data, and metadata) subsystem of the **Neth** Rust Minecraft server (`E:\DATA\source\svmc\ServerMiecraft\Neth`), evaluated against the reference vanilla Minecraft Java Edition implementation (`jvmc` / Minecraft 1.21.9 / 26.3, protocol 777) located at `E:\DATA\source\svmc\ServerMiecraft\jvmc`.

The persistence layer is responsible for the durability, crash resilience, concurrency safety, and binary compatibility of all on-disk Minecraft data. World data loss or file corruption can permanently ruin player progress and world state. Therefore, strict adherence to Vanilla Java specifications and defensive storage engineering is mandatory.

---

## 2. Architecture Overview & Components

### 2.1. Level Directory & Dimension Structure
Vanilla Minecraft (since modern data packs and finalized in 26.2/26.3) uses the canonical directory structure:
```
<world_root>/
├── session.lock                     # Snowman UTF-8 + exclusive file lock
├── level.dat                        # Primary GZip NBT level metadata
├── level.dat_old                    # Atomic backup of level metadata
├── data/
│   └── minecraft/
│       ├── game_rules.dat
│       ├── world_gen_settings.dat
│       ├── world_clocks.dat
│       └── weather.dat
├── playerdata/
│   ├── <UUID>.dat                   # GZip NBT player data
│   ├── <UUID>.dat_old               # Atomic fallback backup
│   └── <UUID>_corrupted_<timestamp>.dat # Forensic backup upon corruption
├── region/                          # Overworld chunk files
│   ├── r.<X>.<Z>.mca
│   └── c.<X>.<Z>.mcc                # External oversized chunks (>1MB)
├── entities/                        # Overworld entities
│   └── r.<X>.<Z>.mca
├── poi/                             # Points of interest
├── dimensions/                      # Custom and canonical dimensions
│   └── minecraft/
│       ├── the_nether/
│       │   ├── region/
│       │   └── entities/
│       └── the_end/
│           ├── region/
│           └── entities/
└── DIM-1/, DIM1/                    # Legacy fallback folders
```

In Neth:
- `LevelFolder` (`crates/pumpkin-world/src/level.rs:144`) tracks `root_folder`, `dim_folder`, `region_folder`, `entities_folder`, and `poi_folder`.
- Fallbacks are maintained for canonical 26.2 layout (`dimensions/minecraft/<name>`) and legacy folders (`DIM-1` and `DIM1`).

---

### 2.2. Anvil Region File System (`.mca`)
The Anvil region storage format partitions worlds into 32×32 chunks (1,024 chunks per region):
- **Header Layout (First 8,192 bytes / 2 sectors)**:
  - **Bytes 0..4,095 (Locations Table)**: 1,024 4-byte entries in big-endian format.
    - Bits 8..31 (3 bytes): Sector offset from beginning of file.
    - Bits 0..7 (1 byte): Sector count allocated to this chunk (1..255).
    - An entry of `0` denotes an ungenerated / unallocated chunk.
  - **Bytes 4,096..8,191 (Timestamps Table)**: 1,024 4-byte big-endian integers containing epoch seconds of the last modification.
- **Sector Allocation**:
  - Sector size is strictly 4,096 bytes (`SECTOR_BYTES = 4096`).
  - Sectors 0 and 1 are reserved exclusively for the header.
  - Neth implements `SectorBitmap` and a best-fit `FreeRuns` allocator (`crates/pumpkin-world/src/chunk/format/anvil.rs`), allocating lowest matching contiguous runs and truncating unused tail sectors upon flush.

---

### 2.3. Oversized Chunks (`.mcc`)
Because the sector count in the Anvil location table is limited to a single byte ($2^8 - 1 = 255$ sectors $\approx 1,044,480$ bytes):
- Any chunk whose compressed size + 5-byte header requires $\ge 256$ sectors ($> 1\text{ MiB}$) cannot be indexed within the `.mca` sector table.
- **Vanilla Java Parity**:
  - Minecraft writes an **external stub** in the `.mca` file: 1 sector allocated, declared length = 1, version byte = `compression_id | 128` (`EXTERNAL_STREAM_FLAG = 0x80`).
  - The payload is stored in a separate file: `c.<X>.<Z>.mcc` in the region directory.
  - When loading, if `version & 128 != 0`, the stream is loaded from `c.<X>.<Z>.mcc`.
  - When non-oversized data is written or chunk is cleared, the `.mcc` file is deleted.
- **Neth Implementation**:
  - Implemented in `crates/pumpkin-world/src/chunk/format/anvil.rs` lines 470–489 and 713–738.

---

### 2.4. Compression Algorithms
Vanilla Minecraft supports:
1. `GZip` (`ID = 1`)
2. `ZLib` (`ID = 2`, default for `.mca`)
3. `Uncompressed` (`ID = 3`, used for debugging/network)
4. `LZ4` (`ID = 4`, introduced in 24w04a)
5. `Custom` (`ID = 127`, introduced in 24w05a)

Neth supports GZip (via `flate2`), ZLib (via `flate2`), and LZ4 (via `lz4-java-wrc`).

---

### 2.5. NBT Serialization Layer (`pumpkin-nbt`)
- Binary NBT reader and writer supporting standard Java Edition formats (Compound, List, Byte, Short, Int, Long, Float, Double, ByteArray, String, IntArray, LongArray).
- Enforces `MAX_NBT_DEPTH = 512` to prevent stack overflow from recursive maliciously crafted compounds.
- Enforces `MAX_ARRAY_LENGTH = 512,000` to prevent memory exhaustion attacks.
- Java CESU-8 string decoding and UTF-8 encoding support.

---

## 3. Vulnerability & Discrepancy Matrix

| Vulnerability / Gap | Vanilla Java Behavior (`jvmc`) | Prior Neth Behavior | Severity | Parity Impact |
|---|---|---|---|---|
| **World Directory Lock (`session.lock`)** | `DirectoryLock.java` creates `session.lock`, writes `☃` (`\u2603`), holds exclusive OS file lock (`FileLock.tryLock()`). Second instance aborts with `alreadyLocked()`. | **Missing**. No `session.lock` created or locked. Multiple instances opening the same world directory simultaneously cause catastrophic sector overwrites and corruption. | **CRITICAL** | Zero parity with Vanilla safety guarantees. |
| **Windows Atomic File Replacement** | `Util.java:safeReplaceFile` implements 10 retries with deleter, renamer to backup, renamer to target, and rollback on failure. | Used direct `std::fs::rename`. On Windows, `rename` fails when the destination exists or is momentarily inspected by antivirus/indexer. | **HIGH** | Crash during write causes permanent data loss or failed save operations. |
| **`level.dat` Corrupted / Missing Fallback** | `LevelStorageSource.java:getUnfixedDataTagWithFallback` falls back to `level.dat_old`, backs up corrupted file to `level.dat_corrupted_<timestamp>`, and restores `level.dat`. | Only opened `level.dat`. If missing or corrupt, boot aborted with `DeserializationError` or `InfoNotFound`. No fallback to `level.dat_old`. | **HIGH** | World becomes unbootable despite healthy `level.dat_old` existing. |
| **PlayerData `.dat_old` & Corrupted Backup** | `PlayerDataStorage.java` maintains `<UUID>.dat_old` on every save, loads `.dat` with fallback to `.dat_old`, and backs up corrupted `.dat` to `<UUID>_corrupted_<timestamp>.dat`. | Saved directly to `.dat` via temporary file without `.dat_old`. Loaded only `.dat`. Corrupted files caused empty fallback or player inventory loss. | **HIGH** | Player inventory and position loss upon dirty crash. |
| **Shutdown Flush Completeness** | Vanilla saves all player data and flushes all regions during server shutdown. | `Server::shutdown` did not explicitly invoke `save_all_players` when called outside `PumpkinServer`. | **MEDIUM** | Risk of stale player data if server shutdown sequence is invoked directly. |

---

## 4. Remediation Architecture

### 4.1. `session_lock.rs` Implementation
- Create `DirectoryLock` in `crates/pumpkin-world/src/session_lock.rs`.
- Target:
  - Write UTF-8 bytes `[0xE2, 0x98, 0x83]` (`\u2603`, snowman) to `<world_dir>/session.lock`.
  - Call `sync_all()`.
  - Apply exclusive, non-blocking OS lock:
    - On Windows: `LockFileEx` with `LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY` across offset 0 to `u64::MAX`.
    - On Unix: `libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB)` or `fcntl`.
  - Implement `is_locked(dir)` and `is_valid(&self)`.
  - On `Drop`, release lock and close file handle.

### 4.2. `fs_util.rs` (Safe Replace & Retry Mechanism)
- Implement `safe_replace_file` and `safe_replace_or_move_file` matching `net.minecraft.util.Util.java`:
  - 10 retries with 10ms exponential/linear backoff.
  - Step 1: If `target` exists, rename `target` -> `backup` (deleting stale `backup` first).
  - Step 2: Delete `target` if still present.
  - Step 3: Rename `new_file` -> `target`.
  - Step 4: If step 3 fails and `!no_rollback`, restore `backup` -> `target`.
- Implement `backup_corrupted_file(path, suffix)` matching `LevelStorageSource` / `PlayerDataStorage`.

### 4.3. World Info & Player Data Integration
- Update `AnvilLevelInfo`:
  - `read_world_info`: Try `level.dat`. If it fails, log warning, back up corrupted `level.dat`, try `level.dat_old`, and restore `level.dat` from `level.dat_old`.
  - `write_world_info`: Write `level.dat_new`, execute `safe_replace_file(&path, &path_new, &path_old)`.
- Update `PlayerDataStorage`:
  - `save_player_data`: Write to `<uuid>.dat.tmp.*`, replace with `safe_replace_file(&dat_path, &tmp_path, &old_path)`.
  - `load_player_data`: Try `<uuid>.dat`. If corrupted, back up to `<uuid>_corrupted_<timestamp>.dat`, then try `<uuid>.dat_old`.
