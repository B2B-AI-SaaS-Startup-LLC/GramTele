//! `Storage::Databases`. One active move-only pointer per *literal* path;
//! reacquisition cancels the pending close/cleaner/main-queue erasure.
use crate::cache::{Database, Environment, Settings};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    ops::Deref,
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
};
use tg_base::binary_guard::BinaryGuard;
struct Kept {
    database: Arc<Database>,
    destroying: BinaryGuard,
}
struct Owner {
    map: Mutex<BTreeMap<OsString, Kept>>,
    env: Environment,
}
pub struct Databases {
    owner: Arc<Owner>,
}
pub struct DatabasePointer {
    value: Option<Arc<Database>>,
    owner: Weak<Owner>,
    path: OsString,
}
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
impl Default for Databases {
    fn default() -> Self {
        Self::new()
    }
}
impl Databases {
    pub fn new() -> Self {
        Self::with_environment(Environment::default())
    }
    pub fn with_environment(env: Environment) -> Self {
        Self {
            owner: Arc::new(Owner {
                map: Mutex::new(BTreeMap::new()),
                env,
            }),
        }
    }
    pub fn get(&self, path: impl Into<PathBuf>, settings: Settings) -> DatabasePointer {
        // Path equality folds '.' and trailing separators. QString keys in
        // the original do not, so use the original path's unmodified spelling.
        let path = path.into().into_os_string();
        let mut map = lock(&self.owner.map);
        let value = if let Some(kept) = map.get_mut(&path) {
            assert!(
                kept.destroying.alive(),
                "Databases::get: second active pointer"
            );
            kept.destroying.reset();
            kept.database.reconfigure(settings);
            kept.database.clone()
        } else {
            let database = Arc::new(Database::with_environment(
                &path,
                settings,
                self.owner.env.clone(),
            ));
            map.insert(
                path.clone(),
                Kept {
                    database: database.clone(),
                    destroying: BinaryGuard::new(),
                },
            );
            database
        };
        DatabasePointer {
            value: Some(value),
            owner: Arc::downgrade(&self.owner),
            path,
        }
    }
}
impl DatabasePointer {
    pub fn get(&self) -> Option<&Database> {
        self.value.as_deref()
    }
    pub fn is_valid(&self) -> bool {
        self.value.is_some()
    }
    pub fn destroy(&mut self) {
        let Some(value) = self.value.take() else {
            return;
        };
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut map = lock(&owner.map);
        let Some(kept) = map.get_mut(&self.path) else {
            return;
        };
        assert!(Arc::ptr_eq(&value, &kept.database));
        assert!(!kept.destroying.alive());
        value.close(None);
        let guard = kept.destroying.make_guard();
        let weak = Arc::downgrade(&owner);
        let path = self.path.clone();
        let main = owner.env.main.clone();
        value.wait_for_cleaner(Some(Box::new(move || {
            main.post(move || {
                if guard.alive() {
                    if let Some(owner) = weak.upgrade() {
                        lock(&owner.map).remove(&path);
                    }
                }
            });
        })));
    }
}
impl Deref for DatabasePointer {
    type Target = Database;
    fn deref(&self) -> &Self::Target {
        self.get().expect("empty DatabasePointer")
    }
}
impl Drop for DatabasePointer {
    fn drop(&mut self) {
        self.destroy();
    }
}
