use pumpkin_nbt::compound::NbtCompound;
use std::fs::{File, create_dir_all};
use std::io;
use std::path::PathBuf;
use tracing::{debug, error};
use uuid::Uuid;

/// Manages the storage and retrieval of player data from disk and memory cache.
///
/// This struct provides functions to load and save player data to/from NBT files,
/// with a memory cache to handle player disconnections temporarily.
pub struct PlayerDataStorage {
    /// Path to the directory where player data is stored
    data_path: PathBuf,
    /// Whether player data saving is enabled
    save_enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerDataError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("NBT error: {0}")]
    Nbt(String),
}

impl PlayerDataStorage {
    /// Creates a new `PlayerDataStorage` with the specified data path and cache expiration time.
    pub fn new(data_path: impl Into<PathBuf>, enabled: bool) -> Self {
        let path = data_path.into();
        if !path.exists()
            && let Err(e) = create_dir_all(&path)
        {
            error!(
                "Failed to create player data directory at {}: {e}",
                path.display()
            );
        }

        Self {
            data_path: path,
            save_enabled: enabled,
        }
    }

    #[must_use]
    pub const fn get_data_path(&self) -> &PathBuf {
        &self.data_path
    }

    #[must_use]
    pub const fn is_save_enabled(&self) -> bool {
        self.save_enabled
    }

    pub const fn set_save_enabled(&mut self, enabled: bool) {
        self.save_enabled = enabled;
    }

    /// Returns the path for a player's data file based on their UUID.
    #[must_use]
    pub fn get_player_data_path(&self, uuid: &Uuid) -> PathBuf {
        self.get_data_path().join(format!("{uuid}.dat"))
    }

    /// Returns the path for a player's backup data file (.dat_old) based on their UUID.
    #[must_use]
    pub fn get_old_player_data_path(&self, uuid: &Uuid) -> PathBuf {
        self.get_data_path().join(format!("{uuid}.dat_old"))
    }

    fn read_data_file(path: &std::path::Path) -> Result<NbtCompound, PlayerDataError> {
        let file = File::open(path)?;
        pumpkin_nbt::nbt_compress::read_gzip_compound_tag(file)
            .map_err(|e| PlayerDataError::Nbt(e.to_string()))
    }

    /// Loads player data from NBT file or fallback to .dat_old.
    ///
    /// This function mirrors Vanilla Minecraft's `PlayerDataStorage.java:load`:
    /// 1. Tries loading `<UUID>.dat`.
    /// 2. If reading `<UUID>.dat` fails due to corruption, backs it up to
    ///    `<UUID>_corrupted_<timestamp>.dat` and falls back to `<UUID>.dat_old`.
    /// 3. If neither exists, returns default empty compound.
    ///
    /// # Arguments
    /// * `uuid` - The UUID of the player to load data for.
    ///
    /// # Returns
    /// A Result containing `(exists, data)` or an error.
    pub fn load_player_data(&self, uuid: &Uuid) -> Result<(bool, NbtCompound), PlayerDataError> {
        // If player data saving is disabled, return empty data
        if !self.is_save_enabled() {
            return Ok((false, NbtCompound::new()));
        }

        let path = self.get_player_data_path(uuid);
        let path_old = self.get_old_player_data_path(uuid);

        if path.exists() {
            match Self::read_data_file(&path) {
                Ok(nbt) => {
                    debug!("Loaded player data for {uuid} from disk");
                    return Ok((true, nbt));
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to read player data for {uuid} from {}: {e}. Backing up and attempting fallback to {}",
                        path.display(),
                        path_old.display()
                    );
                    let _ = crate::fs_util::backup_corrupted_file(&path, ".dat");
                }
            }
        }

        // Try fallback to .dat_old
        if path_old.exists() {
            match Self::read_data_file(&path_old) {
                Ok(nbt) => {
                    tracing::info!(
                        "Successfully recovered player data for {uuid} from fallback {}",
                        path_old.display()
                    );
                    return Ok((true, nbt));
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to read fallback player data for {uuid} from {}: {e}",
                        path_old.display()
                    );
                }
            }
        }

        if !path.exists() && !path_old.exists() {
            debug!("No player data file found for {uuid}");
            return Ok((false, NbtCompound::new()));
        }

        Err(PlayerDataError::Nbt(format!(
            "Failed to load player data or fallback for {uuid}"
        )))
    }

    /// Saves player data to NBT file, maintaining `<UUID>.dat_old` backup atomically.
    ///
    /// Mirrors Vanilla Minecraft's `PlayerDataStorage.java:save`:
    /// Writes to `<UUID>.dat.tmp.*`, then safely replaces `<UUID>.dat` and `<UUID>.dat_old`.
    ///
    /// # Arguments
    /// * `uuid` - The UUID of the player to save data for.
    /// * `data` - The NBT compound data to save.
    pub fn save_player_data(&self, uuid: &Uuid, data: NbtCompound) -> Result<(), PlayerDataError> {
        // Skip saving if disabled in config
        if !self.is_save_enabled() {
            return Ok(());
        }

        let path = self.get_player_data_path(uuid);
        let path_old = self.get_old_player_data_path(uuid);

        // Ensure parent directory exists
        if let Some(parent) = path.parent()
            && let Err(e) = create_dir_all(parent)
        {
            error!("Failed to create player data directory for {uuid}: {e}");
            return Err(PlayerDataError::Io(e));
        }

        static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp_path = path.with_extension(format!("dat.tmp.{}.{unique}", std::process::id()));

        let write_result: Result<(), PlayerDataError> = (|| {
            let file = File::create(&temp_path)?;
            pumpkin_nbt::nbt_compress::write_gzip_compound_tag(data, &file)
                .map_err(|e| PlayerDataError::Nbt(e.to_string()))?;
            file.sync_all()?;
            drop(file);
            crate::fs_util::safe_replace_file(&path, &temp_path, &path_old)?;
            Ok(())
        })();

        if write_result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }

        write_result.map_err(|e| {
            error!("Failed to save player data for {uuid}: {e}");
            e
        })?;

        debug!("Saved player data for {uuid} to disk");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_storage() -> (tempfile::TempDir, PlayerDataStorage) {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = PlayerDataStorage::new(temp_dir.path(), true);
        (temp_dir, storage)
    }

    fn test_compound(value: &str) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        nbt.put_string("State", value.to_string());
        nbt
    }

    #[test]
    fn save_and_load_round_trip() {
        let (_dir, storage) = temp_storage();
        let uuid = Uuid::new_v4();

        storage
            .save_player_data(&uuid, test_compound("good"))
            .unwrap();

        let (found, loaded) = storage.load_player_data(&uuid).unwrap();
        assert!(found);
        assert_eq!(loaded.get_string("State").unwrap(), "good");
    }

    #[test]
    fn save_maintains_dat_old_and_recovers_on_corruption() {
        let (dir, storage) = temp_storage();
        let uuid = Uuid::new_v4();

        // First save: creates .dat
        storage
            .save_player_data(&uuid, test_compound("v1"))
            .unwrap();
        assert!(storage.get_player_data_path(&uuid).exists());

        // Second save: rotates .dat -> .dat_old, writes new .dat
        storage
            .save_player_data(&uuid, test_compound("v2"))
            .unwrap();
        assert!(storage.get_player_data_path(&uuid).exists());
        assert!(storage.get_old_player_data_path(&uuid).exists());

        let (found, loaded) = storage.load_player_data(&uuid).unwrap();
        assert!(found);
        assert_eq!(loaded.get_string("State").unwrap(), "v2");

        // Corrupt the .dat file
        let path = storage.get_player_data_path(&uuid);
        std::fs::write(&path, b"corrupted bytes").unwrap();

        // Loading should fall back to .dat_old (v1)
        let (found, loaded_fallback) = storage.load_player_data(&uuid).unwrap();
        assert!(found);
        assert_eq!(loaded_fallback.get_string("State").unwrap(), "v1");

        // Verify forensic backup was created
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(entries.iter().any(|e| e.contains("_corrupted_")));
    }

    #[test]
    fn no_temp_files_left_after_save() {
        let (dir, storage) = temp_storage();
        let uuid = Uuid::new_v4();

        storage
            .save_player_data(&uuid, test_compound("good"))
            .unwrap();
        storage
            .save_player_data(&uuid, test_compound("better"))
            .unwrap();

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!entries.iter().any(|e| e.contains(".tmp.")));
        assert!(entries.contains(&format!("{uuid}.dat")));
        assert!(entries.contains(&format!("{uuid}.dat_old")));
    }
}
