use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const LOCK_FILE: &str = "session.lock";
pub const DUMMY_SNOWMAN: &[u8] = "☃".as_bytes(); // UTF-8: [0xE2, 0x98, 0x83]

#[derive(Debug, thiserror::Error)]
pub enum DirectoryLockError {
    #[error("{}: already locked (possibly by other Minecraft instance?)", .0.display())]
    AlreadyLocked(PathBuf),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Manages directory-level mutual exclusion using `session.lock` to prevent concurrent
/// servers from corrupting world data, mirroring Vanilla Minecraft's `DirectoryLock.java`.
#[derive(Debug)]
pub struct DirectoryLock {
    file: File,
    lock_path: PathBuf,
    locked: bool,
}

impl DirectoryLock {
    /// Acquires an exclusive lock on `session.lock` within `dir`.
    ///
    /// Writes the snowman UTF-8 byte sequence (`\u2603`) to `session.lock`, flushes to disk,
    /// and locks the file. Fails with `DirectoryLockError::AlreadyLocked` if another instance
    /// already holds the lock.
    pub fn create(dir: &Path) -> Result<Self, DirectoryLockError> {
        std::fs::create_dir_all(dir)?;
        let lock_path = dir.join(LOCK_FILE);

        let mut file = match std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
        {
            Ok(file) => file,
            Err(err)
                if err.raw_os_error() == Some(33)
                    || err.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                return Err(DirectoryLockError::AlreadyLocked(lock_path));
            }
            Err(err) => return Err(DirectoryLockError::Io(err)),
        };

        if let Err(err) = file.write_all(DUMMY_SNOWMAN) {
            if err.raw_os_error() == Some(33)
                || err.kind() == std::io::ErrorKind::PermissionDenied
            {
                return Err(DirectoryLockError::AlreadyLocked(lock_path));
            }
            return Err(DirectoryLockError::Io(err));
        }
        let _ = file.sync_all();

        let acquired = match os::try_lock_file(&file) {
            Ok(acq) => acq,
            Err(err)
                if err.raw_os_error() == Some(33)
                    || err.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                false
            }
            Err(err) => return Err(DirectoryLockError::Io(err)),
        };
        if !acquired {
            return Err(DirectoryLockError::AlreadyLocked(lock_path));
        }

        Ok(Self {
            file,
            lock_path,
            locked: true,
        })
    }

    /// Checks whether `session.lock` inside `dir` is currently locked by any process.
    pub fn is_locked(dir: &Path) -> std::io::Result<bool> {
        let lock_path = dir.join(LOCK_FILE);
        if !lock_path.exists() {
            return Ok(false);
        }

        match std::fs::OpenOptions::new().write(true).open(&lock_path) {
            Ok(file) => match os::try_lock_file(&file) {
                Ok(true) => {
                    let _ = os::unlock_file(&file);
                    Ok(false)
                }
                Ok(false) => Ok(true),
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::PermissionDenied {
                        Ok(true)
                    } else {
                        Err(err)
                    }
                }
            },
            Err(err) => {
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    Ok(true)
                } else if err.kind() == std::io::ErrorKind::NotFound {
                    Ok(false)
                } else {
                    Err(err)
                }
            }
        }
    }

    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.locked
    }

    #[must_use]
    pub const fn lock_path(&self) -> &PathBuf {
        &self.lock_path
    }

    /// Releases the held lock and closes the file descriptor.
    pub fn close(&mut self) -> std::io::Result<()> {
        if self.locked {
            self.locked = false;
            os::unlock_file(&self.file)?;
        }
        Ok(())
    }
}

impl Drop for DirectoryLock {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(windows)]
mod os {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle;

    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x0000_0001;
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x0000_0002;

    #[repr(C)]
    struct OVERLAPPED {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        h_event: *mut std::ffi::c_void,
    }

    unsafe extern "system" {
        fn LockFileEx(
            h_file: *mut std::ffi::c_void,
            dw_flags: u32,
            dw_reserved: u32,
            n_number_of_bytes_to_lock_low: u32,
            n_number_of_bytes_to_lock_high: u32,
            lp_overlapped: *mut OVERLAPPED,
        ) -> i32;

        fn UnlockFileEx(
            h_file: *mut std::ffi::c_void,
            dw_reserved: u32,
            n_number_of_bytes_to_unlock_low: u32,
            n_number_of_bytes_to_unlock_high: u32,
            lp_overlapped: *mut OVERLAPPED,
        ) -> i32;
    }

    pub fn try_lock_file(file: &File) -> Result<bool, std::io::Error> {
        let handle = file.as_raw_handle() as *mut std::ffi::c_void;
        let mut overlapped = OVERLAPPED {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            h_event: std::ptr::null_mut(),
        };
        let flags = LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY;
        // SAFETY: `handle` is a valid open file handle, `overlapped` is a valid stack-allocated OVERLAPPED struct.
        let ret = unsafe { LockFileEx(handle, flags, 0, 1, 0, &mut overlapped) };
        if ret != 0 {
            Ok(true)
        } else {
            let err = std::io::Error::last_os_error();
            // ERROR_LOCK_VIOLATION = 33
            if err.raw_os_error() == Some(33) {
                Ok(false)
            } else {
                Err(err)
            }
        }
    }

    pub fn unlock_file(file: &File) -> Result<(), std::io::Error> {
        let handle = file.as_raw_handle() as *mut std::ffi::c_void;
        let mut overlapped = OVERLAPPED {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            h_event: std::ptr::null_mut(),
        };
        // SAFETY: `handle` is a valid open file handle that was locked by LockFileEx.
        let ret = unsafe { UnlockFileEx(handle, 0, 1, 0, &mut overlapped) };
        if ret != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

#[cfg(unix)]
mod os {
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    pub fn try_lock_file(file: &File) -> Result<bool, std::io::Error> {
        let fd = file.as_raw_fd();
        // SAFETY: `fd` is a valid open file descriptor.
        let ret = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if ret == 0 {
            Ok(true)
        } else {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EWOULDBLOCK)
                || err.raw_os_error() == Some(libc::EAGAIN)
            {
                Ok(false)
            } else {
                Err(err)
            }
        }
    }

    pub fn unlock_file(file: &File) -> Result<(), std::io::Error> {
        let fd = file.as_raw_fd();
        // SAFETY: `fd` is a valid open file descriptor.
        let ret = unsafe { libc::flock(fd, libc::LOCK_UN) };
        if ret == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod os {
    use std::fs::File;

    pub fn try_lock_file(_file: &File) -> Result<bool, std::io::Error> {
        Ok(true)
    }

    pub fn unlock_file(_file: &File) -> Result<(), std::io::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_lock_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path();

        assert!(!DirectoryLock::is_locked(path).unwrap());

        let lock1 = DirectoryLock::create(path).unwrap();
        assert!(lock1.is_valid());

        // Second lock must fail while first is held
        let lock2 = DirectoryLock::create(path);
        assert!(matches!(lock2, Err(DirectoryLockError::AlreadyLocked(_))));

        // Drop first lock
        drop(lock1);

        // Check snowman content after lock is released
        let contents = std::fs::read(path.join(LOCK_FILE)).unwrap();
        assert_eq!(contents, DUMMY_SNOWMAN);

        // Now should be unlocked and second lock succeeds
        let lock3 = DirectoryLock::create(path).unwrap();
        assert!(lock3.is_valid());
    }
}
