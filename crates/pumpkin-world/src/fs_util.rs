use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, warn};

/// Safely replaces `target_path` with `new_path`, preserving `backup_path`, matching
/// Vanilla Minecraft's `Util.java:safeReplaceFile`.
///
/// # Errors
/// Returns an I/O error if the replacement could not be completed after retries.
pub fn safe_replace_file(
    target_path: &Path,
    new_path: &Path,
    backup_path: &Path,
) -> std::io::Result<()> {
    if safe_replace_or_move_file(target_path, new_path, backup_path, false) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "safe_replace_file failed for {}",
            target_path.display()
        )))
    }
}

/// Robust file replacement and rotation mirroring Vanilla Minecraft's
/// `Util.java:safeReplaceOrMoveFile`.
///
/// Steps executed with up to 10 retries:
/// 1. If `target_path` exists, atomically move it to `backup_path` (removing any stale backup first).
/// 2. Verify `target_path` is deleted / free.
/// 3. Move `new_path` into `target_path`.
/// 4. If step 3 fails and `!no_rollback`, restore `target_path` from `backup_path`.
pub fn safe_replace_or_move_file(
    target_path: &Path,
    new_path: &Path,
    backup_path: &Path,
    no_rollback: bool,
) -> bool {
    // 1. Create backup if target exists
    if target_path.exists() {
        let backup_ok = run_with_retries(
            10,
            &format!("create backup {}", backup_path.display()),
            || {
                let _ = std::fs::remove_file(backup_path);
                if std::fs::rename(target_path, backup_path).is_err() {
                    return false;
                }
                backup_path.is_file()
            },
        );
        if !backup_ok {
            return false;
        }
    }

    // 2. Ensure target path is deleted
    let remove_ok = run_with_retries(
        10,
        &format!("remove old {}", target_path.display()),
        || {
            if target_path.exists() {
                let _ = std::fs::remove_file(target_path);
            }
            !target_path.exists()
        },
    );
    if !remove_ok {
        return false;
    }

    // 3. Replace target with new_path
    let replace_ok = run_with_retries(
        10,
        &format!(
            "replace {} with {}",
            target_path.display(),
            new_path.display()
        ),
        || {
            if std::fs::rename(new_path, target_path).is_err() {
                return false;
            }
            target_path.is_file()
        },
    );

    if !replace_ok {
        if !no_rollback && backup_path.is_file() {
            let _ = run_with_retries(
                10,
                &format!(
                    "restore {} from {}",
                    target_path.display(),
                    backup_path.display()
                ),
                || {
                    let _ = std::fs::rename(backup_path, target_path);
                    target_path.is_file()
                },
            );
        }
        return false;
    }

    true
}

/// Backs up a corrupted data file to `<stem>_corrupted_<timestamp><suffix>`, mirroring
/// Vanilla Minecraft's `LevelStorageSource:corruptedDataFile` and `PlayerDataStorage:backup`.
pub fn backup_corrupted_file(
    real_path: &Path,
    suffix: &str,
) -> std::io::Result<Option<PathBuf>> {
    if !real_path.is_file() {
        return Ok(None);
    }
    let parent = real_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = real_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let backup_name = if suffix.is_empty() {
        format!("{file_name}_corrupted_{now}")
    } else {
        let stem = real_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_name);
        format!("{stem}_corrupted_{now}{suffix}")
    };
    let backup_path = parent.join(backup_name);

    std::fs::copy(real_path, &backup_path)?;
    warn!(
        "Corrupted file {} has been backed up to {}",
        real_path.display(),
        backup_path.display()
    );
    Ok(Some(backup_path))
}

fn run_with_retries<F>(retries: usize, description: &str, mut op: F) -> bool
where
    F: FnMut() -> bool,
{
    for retry in 0..retries {
        if op() {
            return true;
        }
        debug!(
            "Failed to {description}, retrying {}/{retries}",
            retry + 1
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    error!("Failed to {description}, aborting, progress might be lost");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_replace_file_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target = temp_dir.path().join("target.dat");
        let new_file = temp_dir.path().join("target.dat_new");
        let backup = temp_dir.path().join("target.dat_old");

        // First creation (target does not exist yet)
        std::fs::write(&new_file, b"version 1").unwrap();
        assert!(safe_replace_file(&target, &new_file, &backup).is_ok());
        assert_eq!(std::fs::read(&target).unwrap(), b"version 1");
        assert!(!new_file.exists());
        assert!(!backup.exists());

        // Second update (target exists, should be backed up)
        std::fs::write(&new_file, b"version 2").unwrap();
        assert!(safe_replace_file(&target, &new_file, &backup).is_ok());
        assert_eq!(std::fs::read(&target).unwrap(), b"version 2");
        assert_eq!(std::fs::read(&backup).unwrap(), b"version 1");
        assert!(!new_file.exists());
    }

    #[test]
    fn test_backup_corrupted_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("player.dat");
        std::fs::write(&path, b"corrupted bytes").unwrap();

        let backed_up = backup_corrupted_file(&path, ".dat").unwrap();
        assert!(backed_up.is_some());
        let backup_path = backed_up.unwrap();
        assert!(backup_path.is_file());
        assert_eq!(std::fs::read(&backup_path).unwrap(), b"corrupted bytes");
    }
}
