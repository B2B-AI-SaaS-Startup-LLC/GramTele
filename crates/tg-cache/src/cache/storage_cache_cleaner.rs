use super::{storage_cache_database_object::DatabaseObject, storage_cache_types::*};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tg_base::binary_guard::BinaryGuard;
use tg_runtime::{ObjectOnQueue, WeakOnQueue};

pub(super) fn directories(base: &Path) -> Vec<String> {
    let mut result: Vec<_> = fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            #[cfg(windows)]
            let hidden = {
                use std::os::windows::fs::MetadataExt;
                e.metadata().is_ok_and(|m| m.file_attributes() & 0x2 != 0)
            };
            #[cfg(not(windows))]
            let hidden = name.starts_with('.');
            (!hidden && e.path().is_dir()).then_some(name)
        })
        .collect();
    // QDir defaults to Name | IgnoreCase.
    result.sort_by_cached_key(|s| s.to_lowercase().encode_utf16().collect::<Vec<_>>());
    result
}
pub(super) struct CleanerObject {
    weak: WeakOnQueue<Self>,
    database: WeakOnQueue<DatabaseObject>,
    base: PathBuf,
    guard: BinaryGuard,
    queue: Vec<String>,
    error: Option<PathBuf>,
}
pub(super) fn start(
    base: PathBuf,
    guard: BinaryGuard,
    database: WeakOnQueue<DatabaseObject>,
) -> ObjectOnQueue<CleanerObject> {
    ObjectOnQueue::new_with_weak(move |weak| {
        let mut result = CleanerObject {
            weak,
            database,
            base,
            guard,
            queue: Vec::new(),
            error: None,
        };
        result.start();
        result
    })
}
impl CleanerObject {
    fn start(&mut self) {
        self.queue = directories(&self.base);
        if let Some(v) = read_version_value(&self.base) {
            self.queue.retain(|s| *s != v.to_string());
            self.schedule_next();
        } else {
            self.error = Some(version_file_path(&self.base));
            self.done();
        }
    }
    fn schedule_next(&mut self) {
        if self.queue.is_empty() {
            self.done();
        } else {
            self.weak.with(|s| {
                if s.guard.alive() {
                    s.clean_next();
                }
            });
        }
    }
    fn clean_next(&mut self) {
        let name = self.queue.pop().expect("cleaner queue");
        let path = self.base.join(name);
        if !remove_recursively(&path) {
            self.error = Some(path);
        }
        self.schedule_next();
    }
    fn done(&mut self) {
        let error = self
            .error
            .take()
            .map_or_else(Error::no_error, |p| Error::new(ErrorType::Io, p));
        self.database.with(move |d| d.cleaner_done(error));
    }
}

// QFile/QDir's recursive removal is not std::fs::remove_dir_all: the root
// may itself be a directory symlink (followed), while nested symlinks are
// removed as links. Keep that original behavior, including a failed rmdir
// of the root symlink on POSIX. Only synthetic paths exercise this in tests.
fn remove_recursively(path: &Path) -> bool {
    if !path.is_dir() {
        return true;
    }
    let mut success = true;
    let mut pending = vec![(path.to_owned(), false)];
    while let Some((path, seen)) = pending.pop() {
        if seen {
            if fs::remove_dir(&path).is_err() {
                success = false;
            }
            continue;
        }
        pending.push((path.clone(), true));
        for entry in fs::read_dir(&path).into_iter().flatten().flatten() {
            let path = entry.path();
            let kind = entry.file_type().ok();
            if kind.is_some_and(|k| k.is_dir() && !k.is_symlink()) {
                pending.push((path, false));
                continue;
            }
            let remove = || {
                let result = fs::remove_file(&path);
                #[cfg(windows)]
                let result =
                    if result.is_err() && kind.is_some_and(|k| k.is_symlink()) && path.is_dir() {
                        fs::remove_dir(&path)
                    } else {
                        result
                    };
                result
            };
            let mut removed = remove().is_ok();
            if !removed {
                if let Ok(metadata) = fs::metadata(&path) {
                    let mut permissions = metadata.permissions();
                    if permissions.readonly() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            permissions.set_mode(permissions.mode() | 0o200);
                        }
                        #[cfg(not(unix))]
                        permissions.set_readonly(false);
                        removed =
                            fs::set_permissions(&path, permissions).is_ok() && remove().is_ok();
                    }
                }
            }
            success &= removed;
        }
    }
    success
}
