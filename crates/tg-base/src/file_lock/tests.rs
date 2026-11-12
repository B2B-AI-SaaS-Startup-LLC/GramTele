//! Tests for the POSIX implementation: a real second process holds the lock.

use std::fs::{File, OpenOptions};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::file_lock_posix::{acquire, Acquired};
use super::FileLock;

const HOLDER_ENV: &str = "TG_BASE_FILE_LOCK_HOLDER";

fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-base-file-lock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn write_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    options
}

/// Runs only inside the child process started by `other_process_is_killed`:
/// takes the lock, signals readiness and waits to be killed.
#[test]
fn holder_process() {
    let Ok(path) = std::env::var(HOLDER_ENV) else {
        return;
    };
    let path = PathBuf::from(path);
    let mut file = None;
    let mut lock = FileLock::new();
    assert!(lock.lock(&mut file, &path, &write_options()));
    File::create(path.with_extension("ready")).unwrap();
    std::thread::sleep(Duration::from_secs(60));
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.exists() {
        assert!(Instant::now() < deadline, "holder process did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn other_process_is_killed() {
    let path = temp_path("killed.lock");
    let _ = std::fs::remove_file(path.with_extension("ready"));
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "file_lock::tests::holder_process",
            "--test-threads=1",
        ])
        .env(HOLDER_ENV, &path)
        .spawn()
        .unwrap();
    wait_for(&path.with_extension("ready"));

    // The lock is visible from this process and names the holder.
    let probe = write_options().open(&path).unwrap();
    match acquire(&probe) {
        Acquired::LockingPid(pid) => assert_eq!(pid as u32, child.id()),
        Acquired::Locked => panic!("the child's lock was not seen"),
    }
    drop(probe);

    // KillProcess waits for ESRCH. Reap concurrently: otherwise the child
    // remains a zombie and kill(pid, ...) succeeds forever, as in C++.
    let reaper = std::thread::spawn(move || child.wait().unwrap());

    // As in the original, taking the lock terminates the holder.
    let mut file = None;
    let mut lock = FileLock::new();
    assert!(lock.lock(&mut file, &path, &write_options()));
    assert!(lock.locked());
    assert!(file.is_some());
    let status = reaper.join().unwrap();
    assert_eq!(status.signal(), Some(nix::libc::SIGTERM));

    lock.unlock();
    assert!(!lock.locked());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn same_process_locks_do_not_conflict() {
    // POSIX record locks belong to the process, so two independent
    // descriptors in one process both succeed, as in the original.
    let path = temp_path("same.lock");
    let mut first_file = None;
    let mut second_file = None;
    let mut first = FileLock::new();
    let mut second = FileLock::new();
    assert!(first.lock(&mut first_file, &path, &write_options()));
    assert!(second.lock(&mut second_file, &path, &write_options()));
    assert!(first.locked() && second.locked());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn open_failure() {
    let path = temp_path("missing-dir").join("x.lock");
    let mut file = None;
    let mut lock = FileLock::new();
    let mut options = OpenOptions::new();
    options.read(true);
    assert!(!lock.lock(&mut file, &path, &options));
    assert!(!lock.locked());
    assert!(file.is_none());
}
