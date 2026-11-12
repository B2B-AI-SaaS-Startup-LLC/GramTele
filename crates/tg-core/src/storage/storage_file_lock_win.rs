//! Storage's duplicate Windows implementation is replaced by the matching base port.
//! LockFile/Restart Manager cannot be executed in the Linux test environment.
pub use tg_base::file_lock::FileLock;
