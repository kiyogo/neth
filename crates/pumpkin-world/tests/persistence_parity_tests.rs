#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::world_seed::Seed;
use pumpkin_world::{
    data::player_data::PlayerDataStorage,
    fs_util::safe_replace_file,
    session_lock::{DUMMY_SNOWMAN, DirectoryLock, DirectoryLockError, LOCK_FILE},
    world_info::{
        LevelData, WorldInfoReader, WorldInfoWriter,
        anvil::{AnvilLevelInfo, LEVEL_DAT_BACKUP_FILE_NAME, LEVEL_DAT_FILE_NAME},
    },
};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn test_session_lock_vanilla_parity() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path();

    // 1. Initial state: not locked
    assert!(!DirectoryLock::is_locked(path).unwrap());

    // 2. Acquire lock
    let lock1 = DirectoryLock::create(path).expect("Failed to acquire initial DirectoryLock");
    assert!(lock1.is_valid());

    // 3. Probing reports locked
    assert!(DirectoryLock::is_locked(path).unwrap());

    // 4. Concurrent acquisition must fail with AlreadyLocked
    let lock2_err = DirectoryLock::create(path).expect_err("Concurrent lock must fail");
    assert!(matches!(lock2_err, DirectoryLockError::AlreadyLocked(_)));

    // 5. Release lock
    drop(lock1);

    // 6. Snowman payload validation after release (UTF-8 \u2603)
    let lock_file_path = path.join(LOCK_FILE);
    assert!(lock_file_path.exists());
    let bytes = fs::read(&lock_file_path).unwrap();
    assert_eq!(bytes, DUMMY_SNOWMAN);

    // 7. Should now be unlocked
    assert!(!DirectoryLock::is_locked(path).unwrap());

    // 8. Re-acquisition succeeds
    let lock3 = DirectoryLock::create(path).expect("Should re-acquire after drop");
    assert!(lock3.is_valid());
}

#[test]
fn test_safe_replace_file_atomicity_and_backup() {
    let temp_dir = TempDir::new().unwrap();
    let target = temp_dir.path().join("data.dat");
    let new_file = temp_dir.path().join("data.dat_new");
    let backup = temp_dir.path().join("data.dat_old");

    // Initial creation: target does not exist
    fs::write(&new_file, b"payload_v1").unwrap();
    assert!(safe_replace_file(&target, &new_file, &backup).is_ok());
    assert_eq!(fs::read(&target).unwrap(), b"payload_v1");
    assert!(!new_file.exists());
    assert!(!backup.exists());

    // Second update: target exists, should be backed up to backup path
    fs::write(&new_file, b"payload_v2").unwrap();
    assert!(safe_replace_file(&target, &new_file, &backup).is_ok());
    assert_eq!(fs::read(&target).unwrap(), b"payload_v2");
    assert_eq!(fs::read(&backup).unwrap(), b"payload_v1");
    assert!(!new_file.exists());
}

#[test]
fn test_level_dat_fallback_and_corrupted_backup() {
    let temp_dir = TempDir::new().unwrap();
    let mut level = LevelData::default(Seed(777));
    level.level_name = "Vanilla Parity World V1".to_string();

    // Write version 1
    AnvilLevelInfo
        .write_world_info(&level, temp_dir.path())
        .unwrap();

    // Write version 2 (moves V1 to level.dat_old)
    level.level_name = "Vanilla Parity World V2".to_string();
    AnvilLevelInfo
        .write_world_info(&level, temp_dir.path())
        .unwrap();

    let dat_path = temp_dir.path().join(LEVEL_DAT_FILE_NAME);
    let old_path = temp_dir.path().join(LEVEL_DAT_BACKUP_FILE_NAME);
    assert!(dat_path.exists());
    assert!(old_path.exists());

    // Corrupt level.dat
    fs::write(&dat_path, b"random garbage bytes that cannot be decoded").unwrap();

    // read_world_info should fall back to level.dat_old (V1)
    let reloaded = AnvilLevelInfo
        .read_world_info(temp_dir.path())
        .expect("Should recover from fallback");
    assert_eq!(reloaded.level_name, "Vanilla Parity World V1");

    // Check that forensic corrupted backup was created
    let backups: Vec<_> = fs::read_dir(temp_dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains("level.dat_corrupted_"))
        .collect();
    assert!(!backups.is_empty(), "Expected corrupted level.dat backup");
}

#[test]
fn test_player_data_rotation_and_corrupted_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let storage = PlayerDataStorage::new(temp_dir.path(), true);
    let uuid = Uuid::new_v4();

    // 1. Initial save: v1
    let mut v1 = NbtCompound::new();
    v1.put_string("Inventory", "Diamond Sword".to_string());
    storage.save_player_data(&uuid, v1).unwrap();

    let dat_path = storage.get_player_data_path(&uuid);
    let old_path = storage.get_old_player_data_path(&uuid);
    assert!(dat_path.exists());
    assert!(!old_path.exists());

    // 2. Second save: v2 -> rotates v1 into .dat_old
    let mut v2 = NbtCompound::new();
    v2.put_string("Inventory", "Netherite Sword".to_string());
    storage.save_player_data(&uuid, v2).unwrap();

    assert!(dat_path.exists());
    assert!(old_path.exists());

    let (found, loaded) = storage.load_player_data(&uuid).unwrap();
    assert!(found);
    assert_eq!(loaded.get_string("Inventory").unwrap(), "Netherite Sword");

    // 3. Corrupt primary .dat
    fs::write(&dat_path, b"corrupted inventory payload").unwrap();

    // 4. Load player data -> should recover v1 from .dat_old
    let (found_recovered, recovered) = storage
        .load_player_data(&uuid)
        .expect("Should recover from .dat_old");
    assert!(found_recovered);
    assert_eq!(recovered.get_string("Inventory").unwrap(), "Diamond Sword");

    // 5. Verify forensic backup was created
    let corrupted_backups: Vec<_> = fs::read_dir(temp_dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains("_corrupted_"))
        .collect();
    assert!(
        !corrupted_backups.is_empty(),
        "Expected player data corrupted backup"
    );
}
