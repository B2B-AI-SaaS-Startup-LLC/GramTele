//! Port of `lib_base/base/file_lock.h` (with `file_lock_posix.cpp` and
//! `file_lock_win.cpp` as the platform submodules).
//!
//! Inter-process exclusive lock of the first [`FileLock::SKIP_BYTES`] bytes of
//! a file. Readers of locked files skip those bytes (see
//! `storage_encrypted_file.cpp`).
//!
//! The C++ `lock(QFile &file, OpenMode mode)` closes the `QFile` and reopens
//! it with `mode`. In Rust the file is an `Option<File>` owned by the caller
//! plus the path and `OpenOptions` to reopen it with; the lock keeps its own
//! duplicate of the handle to release the range later.

use std::fs::{File, OpenOptions};
use std::path::Path;

#[cfg(unix)]
mod file_lock_posix;
#[cfg(windows)]
mod file_lock_win;

#[cfg(unix)]
use file_lock_posix::Lock;
#[cfg(windows)]
use file_lock_win::Lock;

/// `base::FileLock`.
#[derive(Debug, Default)]
pub struct FileLock {
    lock: Option<Lock>,
}

impl FileLock {
    /// `FileLock::kSkipBytes`.
    pub const SKIP_BYTES: usize = 4;

    /// `kLockOffset`.
    const LOCK_OFFSET: u64 = 0;
    /// `kLockLimit`.
    const LOCK_LIMIT: u64 = Self::SKIP_BYTES as u64;

    pub fn new() -> Self {
        Self::default()
    }

    /// `FileLock::lock(file, mode)`: drops any held lock, closes `file`,
    /// reopens `path` with `options` into `file` and takes the lock.
    ///
    /// If another process holds the lock, the original asks it to quit: on
    /// POSIX it sends `SIGTERM` (and `SIGKILL` after 50 attempts 10 ms apart)
    /// to the process reported by `F_GETLK`; on Windows it uses the Restart
    /// Manager. Returns `false` (leaving `file` open if it could be opened)
    /// when the lock cannot be taken.
    pub fn lock(&mut self, file: &mut Option<File>, path: &Path, options: &OpenOptions) -> bool {
        assert!(
            self.lock.is_none() || file.is_some(),
            "FileLock::lock: locked without an open file"
        );
        self.unlock();
        *file = None;
        match Lock::lock(file, path, options) {
            Some(lock) => {
                self.lock = Some(lock);
                true
            }
            None => false,
        }
    }

    /// `FileLock::locked()`.
    #[must_use]
    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }

    /// `FileLock::unlock()`.
    pub fn unlock(&mut self) {
        self.lock = None;
    }
}

#[cfg(all(test, unix))]
mod tests;
