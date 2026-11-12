//! Port of `lib_base/base/file_lock_posix.cpp`.
//!
//! Uses POSIX record locks (`fcntl(F_SETLK)` on bytes 0..4), the same kind
//! of lock the original takes, so the port and the original exclude each
//! other. Record locks belong to the process: a second lock from the same
//! process on the same file succeeds, and closing *any* descriptor of the
//! file in the process releases them.

use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, AsRawFd};
use std::path::Path;
use std::time::Duration;

use nix::errno::Errno;
use nix::fcntl::{fcntl, FcntlArg};
use nix::libc;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

use super::FileLock;

/// `KillProcess(pid)`: `SIGTERM` every 10 ms, `SIGKILL` from the 50th
/// attempt on, until the process is gone (`ESRCH`). Any other error
/// (for example `EPERM`) gives up.
fn kill_process(pid: libc::pid_t) -> bool {
    let pid = Pid::from_raw(pid);
    let mut signal = Signal::SIGTERM;
    let mut attempts = 0;
    loop {
        if let Err(error) = kill(pid, signal) {
            return error == Errno::ESRCH;
        }
        std::thread::sleep(Duration::from_millis(10));
        attempts += 1;
        if attempts == 50 {
            signal = Signal::SIGKILL;
        }
    }
}

fn range(kind: libc::c_short) -> libc::flock {
    libc::flock {
        l_type: kind,
        l_whence: libc::SEEK_SET as libc::c_short,
        l_start: FileLock::LOCK_OFFSET as libc::off_t,
        l_len: FileLock::LOCK_LIMIT as libc::off_t,
        l_pid: 0,
    }
}

pub(super) enum Acquired {
    /// `Descriptor { descriptor }`.
    Locked,
    /// `LockingPid { pid }`, 0 when `F_GETLK` failed.
    LockingPid(libc::pid_t),
}

/// `FileLock::Lock::Acquire`.
pub(super) fn acquire(file: &File) -> Acquired {
    loop {
        if fcntl(
            file.as_fd(),
            FcntlArg::F_SETLK(&range(libc::F_WRLCK as libc::c_short)),
        )
        .is_ok()
        {
            return Acquired::Locked;
        }
        let mut lock = range(libc::F_WRLCK as libc::c_short);
        if fcntl(file.as_fd(), FcntlArg::F_GETLK(&mut lock)).is_err() {
            return Acquired::LockingPid(0);
        } else if lock.l_type != libc::F_UNLCK as libc::c_short {
            return Acquired::LockingPid(lock.l_pid);
        }
        // The holder went away between the two calls: try again.
    }
}

/// `FileLock::Lock`: holds a duplicate of the locked descriptor and releases
/// the range on drop.
#[derive(Debug)]
pub(super) struct Lock {
    file: File,
}

impl Lock {
    #[allow(clippy::self_named_constructors)] // Mirrors FileLock::Lock.
    pub(super) fn lock(
        file: &mut Option<File>,
        path: &Path,
        options: &OpenOptions,
    ) -> Option<Self> {
        let Ok(opened) = options.open(path) else {
            return None;
        };
        let opened = file.insert(opened);
        // Original Acquire treats descriptor 0 as invalid, even though it
        // is a valid POSIX descriptor. Preserve that edge case.
        if opened.as_raw_fd() == 0 {
            return None;
        }
        loop {
            match acquire(opened) {
                Acquired::Locked => {
                    return opened.try_clone().ok().map(|file| Self { file });
                }
                Acquired::LockingPid(pid) => {
                    if pid <= 0 || !kill_process(pid) {
                        return None;
                    }
                }
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fcntl(
            self.file.as_fd(),
            FcntlArg::F_SETLK(&range(libc::F_UNLCK as libc::c_short)),
        );
    }
}
