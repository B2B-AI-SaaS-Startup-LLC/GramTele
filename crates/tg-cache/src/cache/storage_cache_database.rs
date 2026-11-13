//! Public `Storage::Cache::Database`, serialized by `tg_runtime::ObjectOnQueue`.
use super::{storage_cache_database_object::DatabaseObject, storage_cache_types::*};
use crate::storage_encrypted_file::{RandomSource, SystemRandom};
use crate::storage_encryption::EncryptionKey;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tg_base::concurrent_timer::ConcurrentTimerEnvironment;
use tg_rpl::{consumer::Consumer, lifetime::Lifetime, producer::Producer};
use tg_runtime::{MainQueue, ObjectOnQueue};

pub trait CacheClock: Send + Sync {
    fn unix_time(&self) -> i64;
    fn monotonic_ms(&self) -> i64;
}
struct SystemCacheClock;
impl CacheClock for SystemCacheClock {
    fn unix_time(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64)
    }
    fn monotonic_ms(&self) -> i64 {
        tg_runtime::now()
    }
}
/// Host services, shared by the database and its workers. An explicit timer
/// environment supports virtual time; the default uses the launcher's global one.
#[derive(Clone)]
pub struct Environment {
    pub clock: Arc<dyn CacheClock>,
    pub random: Arc<dyn RandomSource>,
    pub timers: Option<Arc<ConcurrentTimerEnvironment>>,
    pub main: MainQueue,
}
impl Default for Environment {
    fn default() -> Self {
        Self {
            clock: Arc::new(SystemCacheClock),
            random: Arc::new(SystemRandom),
            timers: None,
            main: tg_runtime::main_queue().clone(),
        }
    }
}
pub type Done = Option<Box<dyn FnOnce(Error) + Send>>;
pub type Ready = Option<Box<dyn FnOnce() + Send>>;

pub struct Database {
    pub(crate) wrapped: ObjectOnQueue<DatabaseObject>,
    main: MainQueue,
}
impl Database {
    pub fn new(path: impl AsRef<Path>, settings: Settings) -> Self {
        Self::with_environment(path, settings, Environment::default())
    }
    pub fn with_environment(path: impl AsRef<Path>, settings: Settings, env: Environment) -> Self {
        let path = path.as_ref().to_owned();
        let main = env.main.clone();
        Self {
            wrapped: ObjectOnQueue::new_with_weak(move |weak| {
                DatabaseObject::new(weak, path, settings, env)
            }),
            main,
        }
    }
    pub fn reconfigure(&self, settings: Settings) {
        self.wrapped.with(move |d| d.reconfigure(settings));
    }
    pub fn update_settings(&self, update: SettingsUpdate) {
        self.wrapped.with(move |d| d.update_settings(update));
    }
    pub fn open(&self, key: EncryptionKey, done: Done) {
        self.wrapped.with(move |d| {
            let e = d.open(key);
            if let Some(done) = done {
                done(e);
            }
        });
    }
    pub fn close(&self, done: Ready) {
        self.wrapped.with(move |d| d.close(done));
    }
    pub fn put(&self, key: Key, value: impl Into<TaggedValue>, done: Done) {
        let value = value.into();
        self.wrapped.with(move |d| d.put(key, value, done));
    }
    pub fn get(&self, key: Key, done: impl FnOnce(Vec<u8>) + Send + 'static) {
        self.get_with_tag(key, move |v| done(v.bytes));
    }
    pub fn get_with_tag(&self, key: Key, done: impl FnOnce(TaggedValue) + Send + 'static) {
        self.wrapped.with(move |d| d.get(key, done));
    }
    pub fn get_with_sizes(
        &self,
        key: Key,
        keys: Vec<Key>,
        done: impl FnOnce(Vec<u8>, Vec<usize>) + Send + 'static,
    ) {
        self.wrapped
            .with(move |d| d.get_with_sizes(key, keys, done));
    }
    pub fn remove(&self, key: Key, done: Done) {
        self.wrapped.with(move |d| {
            let e = d.remove(key);
            if let Some(done) = done {
                done(e);
            }
        });
    }
    pub fn put_if_empty(&self, key: Key, value: impl Into<TaggedValue>, done: Done) {
        let value = value.into();
        self.wrapped.with(move |d| d.put_if_empty(key, value, done));
    }
    pub fn copy_if_empty(&self, from: Key, to: Key, done: Done) {
        self.wrapped.with(move |d| d.copy_if_empty(from, to, done));
    }
    pub fn move_if_empty(&self, from: Key, to: Key, done: Done) {
        self.wrapped.with(move |d| {
            let e = d.move_if_empty(from, to);
            if let Some(done) = done {
                done(e);
            }
        });
    }
    pub fn clear(&self, done: Done) {
        self.wrapped.with(move |d| d.clear(done));
    }
    pub fn clear_by_tag(&self, tag: u8, done: Done) {
        self.wrapped.with(move |d| {
            d.clear_by_tag(tag);
            if let Some(done) = done {
                done(Error::no_error());
            }
        });
    }
    pub fn wait_for_cleaner(&self, done: Ready) {
        self.wrapped.with(move |d| d.wait_for_cleaner(done));
    }
    /// A FIFO barrier, as in the original. Does not flush delayed bundles.
    pub fn sync(&self) {
        self.wrapped.with_sync(|_| ()).expect("cache queue stopped");
    }
    pub fn stats_on_main(&self) -> Producer<Stats> {
        let weak = self.wrapped.weak();
        let main = self.main.clone();
        Producer::new(move |consumer| {
            assert!(
                main.is_main_thread(),
                "stats_on_main must be subscribed on the main queue thread"
            );
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            CONSUMERS.with(|c| c.borrow_mut().insert(id, consumer));
            let queue = main.clone();
            weak.with(move |d| {
                d.subscribers.insert(id, queue);
                d.emit_stats(id);
            });
            Lifetime::from_fn(move || {
                CONSUMERS.with(|c| c.borrow_mut().remove(&id));
                weak.with(move |d| {
                    d.subscribers.remove(&id);
                });
            })
        })
    }
}
// Only Send payloads and numeric subscription IDs cross the queue boundary.
// The rpl Consumer always stays on the subscribing (main) thread.
thread_local! {static CONSUMERS:RefCell<BTreeMap<u64,Consumer<Stats>>> = const {RefCell::new(BTreeMap::new())};}
pub(crate) fn deliver(id: u64, value: Option<Stats>) {
    let consumer = CONSUMERS.with(|c| c.borrow().get(&id).cloned());
    if let Some(c) = consumer {
        if let Some(v) = value {
            c.put_next(v);
        } else {
            c.put_done();
        }
    }
}
