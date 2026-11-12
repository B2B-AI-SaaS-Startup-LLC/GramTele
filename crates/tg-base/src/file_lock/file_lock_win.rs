//! Port of `lib_base/base/file_lock_win.cpp`.
//!
//! `LockFile` on bytes 0..4 through the `file-guard` crate (which calls
//! `LockFileEx` with `LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY`,
//! the same semantics as `LockFile`). Not compiled or tested on Windows in
//! this port yet.

use std::fs::{File, OpenOptions};
use std::path::Path;

use file_guard::{FileGuard, Lock as GuardLock};

use super::FileLock;

/// `FileLock::Lock`: the guard unlocks the range on drop.
#[derive(Debug)]
pub(super) struct Lock {
    _guard: FileGuard<Box<File>>,
}

/// `Platform::CloseProcesses`, through safe Restart Manager bindings.
/// Leaves applications stopped, as in the original.
fn close_processes(path: &Path) -> bool {
    use restart_manager::{RestartSession, ShutdownOptions};
    let Ok(mut session) = RestartSession::new() else {
        return false;
    };
    if session.register_files([path]).is_err() {
        return false;
    }
    let Ok(affected) = session.affected_applications() else {
        return false;
    };
    if affected.is_empty() {
        return true;
    }
    let pending = session.shutdown_with_options(ShutdownOptions::new().with_force_if_hung(true));
    let success = pending.shutdown_outcome().is_success();
    let _ = pending.leave_stopped();
    success
}

impl Lock {
    #[allow(clippy::self_named_constructors)]
    pub(super) fn lock(
        file: &mut Option<File>,
        path: &Path,
        options: &OpenOptions,
    ) -> Option<Self> {
        loop {
            let Ok(opened) = options.open(path) else {
                return None;
            };
            let opened = file.insert(opened);
            if let Ok(clone) = opened.try_clone() {
                if let Ok(guard) = file_guard::try_lock(
                    Box::new(clone),
                    GuardLock::Exclusive,
                    FileLock::LOCK_OFFSET as usize,
                    FileLock::LOCK_LIMIT as usize,
                ) {
                    return Some(Self { _guard: guard });
                }
            }
            *file = None;
            if !close_processes(path) {
                return None;
            }
        }
    }
}
