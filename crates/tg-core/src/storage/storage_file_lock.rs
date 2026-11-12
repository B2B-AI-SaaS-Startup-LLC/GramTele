//! Same reopen, four-byte range, owner shutdown and RAII unlock as base::FileLock.
#[cfg(unix)]
pub use super::storage_file_lock_posix::FileLock;
#[cfg(windows)]
pub use super::storage_file_lock_win::FileLock;
