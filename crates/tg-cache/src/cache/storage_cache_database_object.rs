//! The queue-owned `DatabaseObject`. Paths never involve a session or user tdata.
use super::{
    storage_cache_binlog_reader::BinlogWrapper,
    storage_cache_cleaner,
    storage_cache_compactor::{self, Compactor, CompactorInfo},
    storage_cache_database::{deliver, Done, Environment, Ready},
    storage_cache_types::*,
};
use crate::{
    storage_encrypted_file::{File, Mode, Result as FileResult},
    storage_encryption::EncryptionKey,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tg_base::{
    binary_guard::BinaryGuard,
    concurrent_timer::{ConcurrentTimer, Runner},
};
use tg_runtime::{MainQueue, ObjectOnQueue, WeakOnQueue};

#[derive(Clone, Debug, Default)]
pub(super) struct Entry {
    pub use_time: u64,
    pub size: usize,
    pub checksum: u32,
    pub place: PlaceId,
    pub tag: u8,
}

// Linux libstdc++'s unordered_map traversal decides which equal-age values are
// evicted. Keep its bucket list (identity high^low hash), not Rust's random order.
#[derive(Default)]
struct Map {
    entries: HashMap<Key, Node>,
    first: Option<Key>,
    bucket_first: HashMap<usize, Key>,
    buckets: usize,
}
struct Node {
    entry: Entry,
    prev: Option<Key>,
    next: Option<Key>,
}
impl Map {
    fn bucket(&self, k: Key) -> usize {
        ((k.high ^ k.low) % (self.buckets as u64)) as usize
    }
    fn order(&self) -> Vec<Key> {
        let mut result = Vec::with_capacity(self.entries.len());
        let mut at = self.first;
        while let Some(k) = at {
            result.push(k);
            at = self.entries[&k].next;
        }
        result
    }
    fn insert_link(&mut self, key: Key, entry: Entry) {
        let bucket = self.bucket(key);
        let next = self.bucket_first.get(&bucket).copied().or(self.first);
        let prev = if self.bucket_first.contains_key(&bucket) {
            next.and_then(|k| self.entries[&k].prev)
        } else {
            None
        };
        self.entries.insert(key, Node { entry, prev, next });
        if let Some(k) = prev {
            self.entries.get_mut(&k).unwrap().next = Some(key);
        } else {
            self.first = Some(key);
        }
        if let Some(k) = next {
            self.entries.get_mut(&k).unwrap().prev = Some(key);
        }
        self.bucket_first.insert(bucket, key);
    }
    fn insert(&mut self, key: Key, entry: Entry) {
        if let Some(node) = self.entries.get_mut(&key) {
            node.entry = entry;
            return;
        }
        if self.entries.len() + 1 > self.buckets {
            let target = (self.entries.len() + 1).max(if self.buckets == 0 {
                13
            } else {
                2 * self.buckets
            });
            let buckets = next_bucket_count(target);
            let order = self.order();
            let mut old = std::mem::take(&mut self.entries);
            self.buckets = buckets;
            self.first = None;
            self.bucket_first.clear();
            for k in order {
                self.insert_link(k, old.remove(&k).unwrap().entry);
            }
        }
        self.insert_link(key, entry);
    }
    fn remove(&mut self, key: Key) -> Option<Entry> {
        let node = self.entries.remove(&key)?;
        let bucket = self.bucket(key);
        if let Some(k) = node.prev {
            self.entries.get_mut(&k).unwrap().next = node.next;
        } else {
            self.first = node.next;
        }
        if let Some(k) = node.next {
            self.entries.get_mut(&k).unwrap().prev = node.prev;
        }
        if self.bucket_first.get(&bucket) == Some(&key) {
            if let Some(k) = node.next.filter(|k| self.bucket(*k) == bucket) {
                self.bucket_first.insert(bucket, k);
            } else {
                self.bucket_first.remove(&bucket);
            }
        }
        Some(node.entry)
    }
    fn get(&self, k: Key) -> Option<&Entry> {
        self.entries.get(&k).map(|n| &n.entry)
    }
    fn get_mut(&mut self, k: Key) -> Option<&mut Entry> {
        self.entries.get_mut(&k).map(|n| &mut n.entry)
    }
    fn len(&self) -> usize {
        self.entries.len()
    }
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    fn clear(&mut self) {
        self.entries.clear();
        self.first = None;
        self.bucket_first.clear();
    }
}
fn next_bucket_count(n: usize) -> usize {
    // _Prime_rehash_policy, max_load_factor=1, growth_factor=2. These are
    // the reachable growth points, rather than every prime in the table.
    const GROWTH: &[usize] = &[
        13, 29, 59, 127, 257, 541, 1109, 2357, 5087, 10273, 20753, 42043, 85229, 172933, 351061,
        712697, 1447153, 2938679, 5967347, 12117689, 24607243, 49969847, 101473717, 206062531,
        418451333, 849749479, 1725587117, 3504151727, 8589934583,
    ];
    *GROWTH
        .iter()
        .find(|p| **p >= n)
        .expect("cache hash table size")
}

struct CleanerWrap {
    object: Option<ObjectOnQueue<storage_cache_cleaner::CleanerObject>>,
    guard: BinaryGuard,
    done: Ready,
}
impl Default for CleanerWrap {
    fn default() -> Self {
        Self {
            object: None,
            guard: BinaryGuard::new(),
            done: None,
        }
    }
}
struct CompactorWrap {
    object: Option<ObjectOnQueue<Compactor>>,
    guard: BinaryGuard,
    excess_length: i64,
    next_attempt: i64,
    delay_after_failure: i64,
}
impl Default for CompactorWrap {
    fn default() -> Self {
        Self {
            object: None,
            guard: BinaryGuard::new(),
            excess_length: 0,
            next_attempt: 0,
            delay_after_failure: 10000,
        }
    }
}

pub(crate) struct DatabaseObject {
    weak: WeakOnQueue<Self>,
    base: PathBuf,
    path: PathBuf,
    settings: Settings,
    env: Environment,
    key: EncryptionKey,
    binlog: File,
    map: Map,
    removing: BTreeSet<Key>,
    accessed: BTreeSet<Key>,
    stale: Vec<Key>,
    time: EstimatedTimePoint,
    excess: i64,
    total_size: i64,
    minimal_time: u64,
    minimal_count: usize,
    tagged: BTreeMap<u8, TaggedSummary>,
    pushing_stats: bool,
    clearing_stale: bool,
    bundles_timer: ConcurrentTimer,
    prune_timer: ConcurrentTimer,
    cleaner: CleanerWrap,
    compactor: CompactorWrap,
    pub subscribers: BTreeMap<u64, MainQueue>,
}
impl DatabaseObject {
    pub fn new(
        weak: WeakOnQueue<Self>,
        path: PathBuf,
        settings: Settings,
        env: Environment,
    ) -> Self {
        settings.check();
        let make_timer = |pruning: bool| {
            let r = weak.runner();
            let runner: Runner = Arc::new(move |method| r.run(method));
            let w = weak.clone();
            let callback = move || {
                w.with(move |d| {
                    if pruning {
                        d.prune();
                    } else {
                        d.write_bundles();
                        d.check_compactor();
                    }
                });
            };
            if let Some(timers) = &env.timers {
                ConcurrentTimer::in_environment(timers, runner, Some(Box::new(callback)))
            } else {
                ConcurrentTimer::new(runner, callback)
            }
        };
        let bundles_timer = make_timer(false);
        let prune_timer = make_timer(true);
        Self {
            weak,
            base: compute_base_path(&path),
            path: PathBuf::new(),
            settings,
            binlog: File::with_random(env.random.clone()),
            env,
            key: EncryptionKey::default(),
            map: Map::default(),
            removing: BTreeSet::new(),
            accessed: BTreeSet::new(),
            stale: Vec::new(),
            time: EstimatedTimePoint::default(),
            excess: 0,
            total_size: 0,
            minimal_time: 0,
            minimal_count: 0,
            tagged: BTreeMap::new(),
            pushing_stats: false,
            clearing_stale: false,
            bundles_timer,
            prune_timer,
            cleaner: CleanerWrap::default(),
            compactor: CompactorWrap::default(),
            subscribers: BTreeMap::new(),
        }
    }
    fn now(&self) -> u32 {
        (self.env.clock.unix_time() as i32).max(1) as u32
    }
    fn io(&self, path: impl Into<PathBuf>) -> Error {
        Error::new(ErrorType::Io, path)
    }
    fn version_path(&self) -> PathBuf {
        version_file_path(&self.base)
    }
    fn compute_path(&self, v: i32) -> PathBuf {
        self.base.join(v.to_string())
    }
    fn binlog_path(&self) -> PathBuf {
        self.path.join("binlog")
    }
    fn ready_path(&self) -> PathBuf {
        self.path.join("binlog-ready")
    }
    fn place_path(&self, p: PlaceId) -> PathBuf {
        self.path.join(place_from_id(p))
    }
    fn callback(done: Done, e: Error) {
        if let Some(done) = done {
            done(e);
        }
    }
    pub fn reconfigure(&mut self, settings: Settings) {
        assert!(self.key.empty());
        settings.check();
        self.settings = settings;
    }
    pub fn update_settings(&mut self, u: SettingsUpdate) {
        self.settings.total_size_limit = u.total_size_limit;
        self.settings.total_time_limit = u.total_time_limit;
        self.settings.check();
        self.optimize();
    }
    pub fn open(&mut self, mut key: EncryptionKey) -> Error {
        self.close(None);
        let version = read_version_value(&self.base).unwrap_or_default();
        let error = match self.open_binlog(version, Mode::ReadAppend, &mut key) {
            FileResult::Success => Error::no_error(),
            FileResult::Failed => self.open_new_binlog(&mut key),
            FileResult::LockFailed => Error::new(
                ErrorType::LockFailed,
                self.compute_path(version).join("binlog"),
            ),
            FileResult::WrongKey => {
                if self.settings.clear_on_wrong_key {
                    self.open_new_binlog(&mut key)
                } else {
                    Error::new(
                        ErrorType::WrongKey,
                        self.compute_path(version).join("binlog"),
                    )
                }
            }
        };
        if !error.is_ok() {
            self.close(None);
        }
        error
    }
    fn open_new_binlog(&mut self, key: &mut EncryptionKey) -> Error {
        let v = self.find_available_version();
        if !write_version_value(&self.base, v) {
            return self.io(self.version_path());
        }
        if self.open_binlog(v, Mode::Write, key) != FileResult::Success {
            self.io(self.compute_path(v).join("binlog"))
        } else {
            Error::no_error()
        }
    }
    fn open_binlog(&mut self, v: i32, mode: Mode, key: &mut EncryptionKey) -> FileResult {
        let path = self.compute_path(v);
        let binlog = path.join("binlog");
        let ready = path.join("binlog-ready");
        if ready.exists() && !File::move_file(&ready, &binlog) {
            return FileResult::Failed;
        }
        let result = self.binlog.open(&binlog, mode, key);
        if result != FileResult::Success {
            return result;
        }
        let required = mode == Mode::Read || (mode == Mode::ReadAppend && self.binlog.size() > 0);
        let success = if required {
            if let Some(h) = BinlogWrapper::read_header(&mut self.binlog, &self.settings) {
                self.time = EstimatedTimePoint {
                    relative: u64::from(h.system_time),
                    system: h.system_time,
                };
                true
            } else {
                false
            }
        } else {
            let now = if self.settings.track_estimated_time {
                self.now()
            } else {
                0
            };
            self.time = EstimatedTimePoint {
                relative: u64::from(now),
                system: now,
            };
            self.binlog.write(
                &mut BasicHeader {
                    flags: u32::from(self.settings.track_estimated_time),
                    system_time: now,
                    ..BasicHeader::default()
                }
                .encode(),
            )
        };
        if !success {
            return FileResult::Failed;
        }
        self.path = path;
        self.key = std::mem::take(key);
        self.create_cleaner();
        self.read_binlog();
        FileResult::Success
    }
    fn read_binlog(&mut self) {
        let mut reader = BinlogWrapper::new(&self.binlog, &self.settings, 0);
        // Move File out: record application may update stats, but never touches
        // the file being read. No aliased borrows or structure transmutation.
        let mut file =
            std::mem::replace(&mut self.binlog, File::with_random(self.env.random.clone()));
        while !reader.read_till_end(&mut file, |record| self.process_record(record)) {}
        self.binlog = file;
        if self.settings.track_estimated_time && self.now() < self.time.system {
            self.write_multi_access_block();
        }
        self.optimize();
    }
    fn process_record(&mut self, r: Record) -> bool {
        match r {
            Record::Store(s) => self.process_store(s),
            Record::MultiStore(list) => {
                for s in list {
                    if !self.process_store(s) {
                        return false;
                    }
                }
                true
            }
            Record::MultiRemove(keys) => {
                self.excess += 16;
                for k in keys {
                    self.excess += 16;
                    self.erase(k);
                }
                true
            }
            Record::MultiAccess(t, keys) => {
                assert!(self.settings.track_estimated_time);
                self.apply_time(t);
                self.excess += 16;
                for k in keys {
                    self.excess += 16;
                    if let Some(e) = self.map.get_mut(k) {
                        e.use_time = t.relative;
                    }
                }
                true
            }
        }
    }
    fn process_store(&mut self, s: Store) -> bool {
        if s.size == 0 || s.size > self.settings.max_data_size {
            return false;
        }
        let mut time = self.time.relative;
        if let Some(t) = s.time {
            self.apply_time(t);
            time = t.relative;
        }
        self.set_entry(
            s.key,
            Entry {
                use_time: time,
                size: s.size,
                checksum: s.checksum,
                place: s.place,
                tag: s.tag,
            },
        );
        true
    }
    fn apply_time(&mut self, t: EstimatedTimePoint) {
        if t.relative > self.time.relative {
            self.time = t;
        }
    }
    fn count_time_point(&self) -> EstimatedTimePoint {
        EstimatedTimePoint {
            system: self.now(),
            relative: self
                .time
                .relative
                .wrapping_add((i64::from(self.now()) - i64::from(self.time.system)).max(0) as u64),
        }
    }
    fn store_time(&self) -> Option<EstimatedTimePoint> {
        self.settings.track_estimated_time.then(|| {
            let t = self.count_time_point();
            assert!(t.relative >= self.time.relative);
            if (t.relative - self.time.relative).wrapping_mul(1000)
                < self.settings.write_bundle_delay as u64
            {
                self.time
            } else {
                t
            }
        })
    }
    fn set_entry(&mut self, key: Key, entry: Entry) {
        let old = self.map.get(key).cloned().unwrap_or_default();
        self.update_stats(&old, &entry);
        if old.size != 0 {
            self.excess += if self.settings.track_estimated_time {
                48
            } else {
                32
            };
        }
        if entry.use_time != 0 && (entry.use_time < self.minimal_time || self.minimal_time == 0) {
            self.minimal_time = entry.use_time;
            self.minimal_count = 1;
        } else if self.minimal_time != 0 && old.use_time != entry.use_time {
            if entry.use_time == self.minimal_time {
                assert!(self.minimal_count > 0);
                self.minimal_count += 1;
            } else if old.use_time == self.minimal_time {
                assert!(self.minimal_count > 0);
                self.minimal_count -= 1;
                if self.minimal_count == 0 {
                    self.minimal_time = 0;
                }
            }
        }
        self.map.insert(key, entry);
    }
    fn update_stats(&mut self, old: &Entry, new: &Entry) {
        self.total_size += new.size as i64 - old.size as i64;
        let update = |tagged: &mut BTreeMap<u8, TaggedSummary>, e: &Entry, add: bool| {
            if e.tag != 0 {
                let t = tagged.entry(e.tag).or_default();
                if add {
                    t.count += usize::from(e.size != 0);
                    t.total_size += e.size as i64;
                } else {
                    t.count -= usize::from(e.size != 0);
                    t.total_size -= e.size as i64;
                }
            }
        };
        update(&mut self.tagged, old, false);
        update(&mut self.tagged, new, true);
        self.push_stats_delayed();
    }
    fn erase(&mut self, key: Key) {
        if let Some(e) = self.map.get(key).cloned() {
            self.update_stats(&e, &Entry::default());
            if self.minimal_time != 0 && e.use_time == self.minimal_time {
                assert!(self.minimal_count > 0);
                self.minimal_count -= 1;
                if self.minimal_count == 0 {
                    self.minimal_time = 0;
                }
            }
            self.map.remove(key);
        }
    }
    pub fn collect_stats(&self) -> Stats {
        Stats {
            full: TaggedSummary {
                count: self.map.len(),
                total_size: self.total_size,
            },
            tagged: self.tagged.clone(),
            clearing: self.cleaner.object.is_some() || !self.stale.is_empty(),
        }
    }
    pub fn emit_stats(&self, id: u64) {
        if let Some(queue) = self.subscribers.get(&id) {
            let s = self.collect_stats();
            queue.post(move || deliver(id, Some(s)));
        }
    }
    fn push_stats(&self) {
        for id in self.subscribers.keys() {
            self.emit_stats(*id);
        }
    }
    fn push_stats_delayed(&mut self) {
        if self.pushing_stats {
            return;
        }
        self.pushing_stats = true;
        self.weak.with(|d| {
            if std::mem::take(&mut d.pushing_stats) {
                d.push_stats();
            }
        });
    }
    pub fn close(&mut self, done: Ready) {
        if self.binlog.is_open() {
            self.write_bundles();
            self.binlog.close();
        }
        if let Some(done) = done {
            done();
        }
        self.path = PathBuf::new();
        self.key = EncryptionKey::default();
        // `_map = {}` selects unordered_map's initializer-list assignment:
        // it clears entries but retains the bucket array (and tie ordering).
        self.map.clear();
        self.removing.clear();
        self.accessed.clear();
        self.stale.clear();
        self.time = EstimatedTimePoint::default();
        self.excess = 0;
        self.total_size = 0;
        self.minimal_time = 0;
        self.minimal_count = 0;
        self.tagged.clear();
        self.pushing_stats = false;
        self.bundles_timer.cancel();
        self.prune_timer.cancel();
        self.compactor = CompactorWrap::default();
    }
    pub fn put(&mut self, key: Key, value: TaggedValue, done: Done) {
        if !self.binlog.is_open() {
            Self::callback(done, self.io(self.version_path()));
            return;
        }
        if value.bytes.is_empty() {
            let e = self.remove(key);
            Self::callback(done, e);
            return;
        }
        assert!(value.bytes.len() <= self.settings.max_data_size);
        self.removing.remove(&key);
        self.stale.retain(|k| *k != key);
        let checksum = count_checksum(&value.bytes);
        let old = self.map.get(key).cloned();
        if let Some(e) = &old {
            if e.tag == value.tag
                && e.size == value.bytes.len()
                && e.checksum == checksum
                && self.read_value(e) == value.bytes
            {
                Self::callback(done, Error::no_error());
                self.record_access(key);
                return;
            }
        }
        let mut place = [0; 7];
        loop {
            self.env.random.fill(&mut place);
            if !self.place_path(place).exists() {
                break;
            }
        }
        let path = self.place_path(place);
        let mut content = value.bytes;
        let size = content.len();
        let result = self.write_new_entry(&path, &mut content);
        if !result.is_ok() {
            let _ = fs::remove_file(&path);
            Self::callback(done, result);
            return;
        }
        let result = self.write_store(Store {
            tag: value.tag,
            size,
            place,
            checksum,
            key,
            time: self.store_time(),
        });
        if !result.is_ok() {
            let _ = fs::remove_file(&path);
            Self::callback(done, result);
            return;
        }
        if let Some(e) = old {
            let _ = fs::remove_file(self.place_path(e.place));
        }
        Self::callback(done, Error::no_error());
        self.optimize();
    }
    fn write_new_entry(&self, path: &Path, content: &mut [u8]) -> Error {
        let mut f = File::with_random(self.env.random.clone());
        match f.open(path, Mode::Write, &self.key) {
            FileResult::Success => {
                if f.write_with_padding(content) && f.flush() {
                    Error::no_error()
                } else {
                    self.io(path)
                }
            }
            FileResult::LockFailed => Error::new(ErrorType::LockFailed, path),
            FileResult::Failed => self.io(path),
            FileResult::WrongKey => unreachable!("new value WrongKey"),
        }
    }
    fn write_store(&mut self, s: Store) -> Error {
        if !self.binlog.write(&mut s.encode()) {
            self.binlog.close();
            return self.io(self.binlog_path());
        }
        self.binlog.flush();
        assert!(self.process_store(s));
        Error::no_error()
    }
    fn read_value(&self, entry: &Entry) -> Vec<u8> {
        let mut f = File::default();
        match f.open(&self.place_path(entry.place), Mode::Read, &self.key) {
            FileResult::Failed | FileResult::WrongKey => Vec::new(),
            FileResult::Success => {
                let mut b = vec![0; entry.size];
                if f.read_with_padding(&mut b) == entry.size as isize {
                    b
                } else {
                    Vec::new()
                }
            }
            FileResult::LockFailed => unreachable!("read-only LockFailed"),
        }
    }
    fn get_value(&mut self, key: Key) -> TaggedValue {
        let Some(e) = self.map.get(key).cloned() else {
            return TaggedValue::default();
        };
        let bytes = self.read_value(&e);
        if bytes.is_empty() || count_checksum(&bytes) != e.checksum {
            self.remove(key);
            TaggedValue::default()
        } else {
            TaggedValue::new(bytes, e.tag)
        }
    }
    pub fn get(&mut self, key: Key, done: impl FnOnce(TaggedValue)) {
        let value = self.get_value(key);
        let good = !value.bytes.is_empty();
        done(value);
        if good {
            self.record_access(key);
        }
    }
    pub fn get_with_sizes(
        &mut self,
        key: Key,
        keys: Vec<Key>,
        done: impl FnOnce(Vec<u8>, Vec<usize>),
    ) {
        let value = self.get_value(key);
        if value.bytes.is_empty() {
            done(Vec::new(), Vec::new());
            return;
        }
        let sizes = keys
            .into_iter()
            .map(|k| self.map.get(k).map_or(0, |e| e.size))
            .collect();
        done(value.bytes, sizes);
        self.record_access(key);
    }
    fn record_access(&mut self, key: Key) {
        if !self.settings.track_estimated_time {
            return;
        }
        self.accessed.insert(key);
        if self.accessed.len() == self.settings.max_bundled_records {
            self.write_multi_access();
        } else {
            self.write_bundles_lazy();
        }
        self.optimize();
    }
    pub fn remove(&mut self, key: Key) -> Error {
        if !self.binlog.is_open() {
            return self.io(self.version_path());
        }
        if let Some(e) = self.map.get(key).cloned() {
            self.removing.insert(key);
            if self.removing.len() == self.settings.max_bundled_records {
                self.write_multi_remove();
            } else {
                self.write_bundles_lazy();
            }
            let p = self.place_path(e.place);
            self.erase(key);
            if fs::remove_file(&p).is_err() && p.exists() {
                return self.io(p);
            }
        }
        Error::no_error()
    }
    pub fn put_if_empty(&mut self, key: Key, value: TaggedValue, done: Done) {
        if self.map.get(key).is_some() {
            Self::callback(done, Error::no_error());
        } else {
            self.put(key, value, done);
        }
    }
    pub fn copy_if_empty(&mut self, from: Key, to: Key, done: Done) {
        if self.map.get(to).is_some() {
            Self::callback(done, Error::no_error());
            return;
        }
        let value = self.get_value(from);
        let accessed = !value.bytes.is_empty();
        self.put(to, value, done);
        if accessed {
            self.record_access(from);
        }
    }
    pub fn move_if_empty(&mut self, from: Key, to: Key) -> Error {
        if self.map.get(to).is_some() {
            return Error::no_error();
        }
        let Some(e) = self.map.get(from).cloned() else {
            return Error::no_error();
        };
        self.removing.insert(from);
        self.erase(from);
        let result = self.write_multi_remove();
        if !result.is_ok() {
            return result;
        }
        self.removing.remove(&to);
        self.stale.retain(|k| *k != to);
        self.write_existing_place(to, &e)
    }
    fn write_existing_place(&mut self, to: Key, e: &Entry) -> Error {
        if let Some(already) = self.map.get(to) {
            if already.tag == e.tag
                && already.size == e.size
                && already.checksum == e.checksum
                && self.read_value(already) == self.read_value(e)
            {
                return Error::no_error();
            }
        }
        self.write_store(Store {
            tag: e.tag,
            size: e.size,
            place: e.place,
            checksum: e.checksum,
            key: to,
            time: self.store_time(),
        })
    }
    fn write_bundles_lazy(&self) {
        if !self.bundles_timer.is_active() {
            self.bundles_timer
                .call_once(self.settings.write_bundle_delay);
        }
    }
    fn write_multi_remove(&mut self) -> Error {
        assert!(self.removing.len() <= self.settings.max_bundled_records);
        if self.removing.is_empty() {
            return Error::no_error();
        }
        let keys = std::mem::take(&mut self.removing).into_iter().collect();
        self.write_bundle(Record::MultiRemove(keys))
    }
    fn write_multi_access(&mut self) -> Error {
        if self.accessed.is_empty() {
            Error::no_error()
        } else {
            self.write_multi_access_block()
        }
    }
    fn write_multi_access_block(&mut self) -> Error {
        assert!(self.settings.track_estimated_time);
        assert!(self.accessed.len() <= self.settings.max_bundled_records);
        let keys: Vec<_> = std::mem::take(&mut self.accessed).into_iter().collect();
        self.time = self.count_time_point();
        for k in &keys {
            if let Some(e) = self.map.get_mut(*k) {
                e.use_time = self.time.relative;
            }
        }
        self.write_bundle(Record::MultiAccess(self.time, keys))
    }
    fn write_bundle(&mut self, record: Record) -> Error {
        let mut b = record.encode();
        let size = b.len();
        let (header, parts) = b.split_at_mut(16);
        if self.binlog.write(header) && (parts.is_empty() || self.binlog.write(parts)) {
            self.binlog.flush();
            self.excess += size as i64;
            Error::no_error()
        } else {
            self.binlog.close();
            self.io(self.binlog_path())
        }
    }
    fn write_bundles(&mut self) {
        self.write_multi_remove();
        if self.settings.track_estimated_time {
            self.write_multi_access();
        }
    }
    fn prune_before(&self) -> u64 {
        let relative = self.count_time_point().relative;
        if self.settings.total_time_limit != 0 && relative > self.settings.total_time_limit {
            relative - self.settings.total_time_limit
        } else {
            0
        }
    }
    fn optimize(&mut self) {
        if !self.start_delayed_pruning() {
            self.check_compactor();
        }
    }
    fn start_delayed_pruning(&self) -> bool {
        if !self.settings.track_estimated_time || self.map.is_empty() {
            return false;
        }
        let before = self.prune_before();
        if (self.settings.total_size_limit > 0 && self.total_size > self.settings.total_size_limit)
            || self.minimal_time == 0
            || self.minimal_time <= before
        {
            if !self.prune_timer.is_active()
                || self.prune_timer.remaining_time() > self.settings.prune_timeout
            {
                self.prune_timer.call_once(self.settings.prune_timeout);
            }
            true
        } else {
            if self.minimal_time != 0 && !self.prune_timer.is_active() {
                self.prune_timer.call_once(
                    ((self.minimal_time - before) as i64 * 1000)
                        .min(self.settings.max_prune_check_timeout),
                );
            }
            false
        }
    }
    fn prune(&mut self) {
        if !self.stale.is_empty() {
            return;
        }
        let mut stale = BTreeSet::new();
        let mut total = 0;
        self.collect_time_stale(&mut stale, &mut total);
        self.collect_size_stale(&mut stale, &mut total);
        if stale.len() <= self.settings.stale_remove_chunk {
            if stale.is_empty() {
                return;
            }
            self.stale.push(*stale.first().unwrap());
            self.push_stats();
            for key in stale {
                self.remove(key);
            }
            self.stale.clear();
            self.optimize();
        } else {
            self.stale = stale.into_iter().collect();
            self.start_stale_clear();
        }
    }
    fn collect_time_stale(&mut self, stale: &mut BTreeSet<Key>, total: &mut i64) {
        if self.settings.total_time_limit == 0 {
            return;
        }
        let before = self.prune_before();
        if self.minimal_time == 0 || self.minimal_time > before {
            return;
        }
        self.minimal_time = 0;
        self.minimal_count = 0;
        for k in self.map.order() {
            let e = self.map.get(k).unwrap();
            if e.use_time <= before {
                stale.insert(k);
                *total += e.size as i64;
            } else if self.minimal_time == 0 || self.minimal_time > e.use_time {
                self.minimal_time = e.use_time;
                self.minimal_count = 1;
            } else if self.minimal_time == e.use_time {
                self.minimal_count += 1;
            }
        }
    }
    fn collect_size_stale(&self, stale: &mut BTreeSet<Key>, total: &mut i64) {
        let remove = if self.settings.total_size_limit > 0 {
            self.total_size - *total - self.settings.total_size_limit
        } else {
            0
        };
        if remove <= 0 {
            return;
        }
        let mut oldest: Vec<(Key, &Entry)> = Vec::new();
        let mut size = 0;
        for key in self.map.order() {
            let e = self.map.get(key).unwrap();
            if stale.contains(&key) {
                continue;
            }
            if size >= remove && e.use_time >= oldest[0].1.use_time {
                continue;
            }
            while !oldest.is_empty()
                && e.use_time <= oldest[0].1.use_time
                && size + e.size as i64 - remove >= oldest[0].1.size as i64
            {
                size -= oldest.remove(0).1.size as i64;
            }
            size += e.size as i64;
            let at =
                oldest.partition_point(|(_, other)| (other.use_time as i64) >= (e.use_time as i64));
            oldest.insert(at, (key, e));
        }
        for (k, _) in oldest {
            stale.insert(k);
        }
        *total += size;
    }
    fn start_stale_clear(&mut self) {
        self.push_stats();
        self.clear_stale_chunk();
    }
    fn clear_stale_chunk(&mut self) {
        if self.stale.is_empty() {
            return;
        }
        let start = self
            .stale
            .len()
            .saturating_sub(self.settings.stale_remove_chunk);
        let keys = self.stale[start..].to_vec();
        for k in keys {
            self.remove(k);
        }
        self.stale.truncate(start);
        if self.stale.is_empty() {
            self.optimize();
        } else if !self.clearing_stale {
            self.clearing_stale = true;
            self.weak.with(|d| {
                if std::mem::take(&mut d.clearing_stale) {
                    d.clear_stale_chunk();
                }
            });
        }
    }
    pub fn clear_by_tag(&mut self, tag: u8) {
        let had = !self.stale.is_empty();
        for k in self.map.order() {
            if self.map.get(k).unwrap().tag == tag {
                self.stale.push(k);
            }
        }
        if !had {
            self.start_stale_clear();
        }
    }
    pub fn wait_for_cleaner(&mut self, done: Ready) {
        while !self.stale.is_empty() {
            self.clear_stale_chunk();
        }
        if self.cleaner.object.is_some() {
            self.cleaner.done = done;
        } else if let Some(done) = done {
            done();
        }
    }
    fn create_cleaner(&mut self) {
        let guard = self.cleaner.guard.make_guard();
        self.cleaner.object = Some(storage_cache_cleaner::start(
            self.base.clone(),
            guard,
            self.weak.clone(),
        ));
        self.push_stats_delayed();
    }
    pub(super) fn cleaner_done(&mut self, _error: Error) {
        if let Some(done) = self.cleaner.done.take() {
            done();
        }
        self.cleaner = CleanerWrap::default();
        self.push_stats_delayed();
    }
    fn find_available_version(&self) -> i32 {
        let mut versions = BTreeSet::new();
        for name in storage_cache_cleaner::directories(&self.base) {
            versions.insert(name.trim().parse::<i32>().unwrap_or(0));
        }
        let mut result = 0i32;
        for v in versions {
            if result != v {
                break;
            }
            result = result.wrapping_add(1);
        }
        result
    }
    pub fn clear(&mut self, done: Done) {
        let key = std::mem::take(&mut self.key);
        if !key.empty() {
            self.close(None);
        }
        let v = self.find_available_version();
        if !write_version_value(&self.base, v) {
            Self::callback(done, self.io(self.version_path()));
            return;
        }
        if key.empty() {
            Self::callback(done, Error::no_error());
            self.create_cleaner();
        } else {
            let e = self.open(key);
            Self::callback(done, e);
        }
    }
    fn check_compactor(&mut self) {
        if self.compactor.object.is_some()
            || self.settings.compact_after_excess == 0
            || self.excess < self.settings.compact_after_excess
        {
            return;
        }
        if self.settings.compact_after_full_size != 0
            && self.excess * self.settings.compact_after_full_size
                < self.settings.compact_after_excess * self.binlog.size()
        {
            return;
        }
        if self.env.clock.monotonic_ms() < self.compactor.next_attempt || !self.binlog.is_open() {
            return;
        }
        let info = CompactorInfo {
            till: self.binlog.size(),
            system_time: self.time.system,
            keys_count: self.map.len(),
        };
        self.compactor.object = Some(storage_cache_compactor::start(
            self.weak.clone(),
            self.compactor.guard.make_guard(),
            self.path.clone(),
            self.settings.clone(),
            self.key.clone(),
            info,
            self.env.clone(),
        ));
        self.compactor.excess_length = self.excess;
    }
    pub(super) fn get_many_raw(&self, keys: Vec<Key>) -> Vec<(Key, Entry)> {
        keys.into_iter()
            .filter_map(|k| self.map.get(k).cloned().map(|e| (k, e)))
            .collect()
    }
    pub(super) fn compactor_done(&mut self, path: PathBuf, mut till: i64) {
        let size = self.binlog.size();
        let binlog = self.binlog_path();
        let ready = self.ready_path();
        if till != size {
            till = storage_cache_compactor::catch_up(
                &path,
                &binlog,
                &self.key,
                till,
                self.settings.read_block_size,
            );
            if till != size {
                self.compactor_fail();
                return;
            }
        }
        if !File::move_file(&path, &ready) {
            self.compactor_fail();
            return;
        }
        self.binlog.close();
        if !File::move_file(&ready, &binlog) {
            self.compactor_fail();
            self.compactor = CompactorWrap::default();
            return;
        }
        if self.binlog.open(&binlog, Mode::ReadAppend, &self.key) != FileResult::Success {
            self.compactor_fail();
            self.compactor = CompactorWrap::default();
            return;
        }
        if !self.binlog.seek(self.binlog.size()) {
            self.binlog.close();
            self.compactor_fail();
            self.compactor = CompactorWrap::default();
            return;
        }
        self.excess -= self.compactor.excess_length;
        assert!(self.excess >= 0);
        self.compactor = CompactorWrap::default();
    }
    pub(super) fn compactor_fail(&mut self) {
        let delay = self.compactor.delay_after_failure;
        self.compactor = CompactorWrap::default();
        self.compactor.next_attempt = self.env.clock.monotonic_ms() + delay;
        self.compactor.delay_after_failure = (delay * 2).min(24 * 60 * 60 * 1000);
        let _ = fs::remove_file(self.ready_path());
    }
}
impl Drop for DatabaseObject {
    fn drop(&mut self) {
        self.close(None);
        for (id, queue) in &self.subscribers {
            let id = *id;
            queue.post(move || deliver(id, None));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{storage_cache_database::CacheClock, Database};
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    };
    use tg_base::timer::ManualClock;
    struct Clock(ManualClock);
    impl CacheClock for Clock {
        fn unix_time(&self) -> i64 {
            1700000000
        }
        fn monotonic_ms(&self) -> i64 {
            tg_base::timer::Clock::now(&self.0)
        }
    }
    #[test]
    fn gnu_hash_buckets_survive_clear_and_replay() {
        let mut map = Map::default();
        for i in 0..1000 {
            map.insert(Key::new(i, 0), Entry::default());
        }
        assert_eq!(map.buckets, 1109);
        map.clear();
        assert_eq!(map.buckets, 1109);
        assert!(map.order().is_empty());
        for i in 0..20 {
            map.insert(Key::new(i, 0), Entry::default());
        }
        assert_eq!(
            map.order(),
            (0..20).rev().map(|i| Key::new(i, 0)).collect::<Vec<_>>()
        );
    }
    #[test]
    fn compactor_failed_destination_retries_after_backoff() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "cache-compactor-retry-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let clock = ManualClock::new(10000);
        let env = Environment {
            clock: Arc::new(Clock(clock.clone())),
            ..Environment::default()
        };
        let settings = Settings {
            compact_after_excess: 48,
            ..Settings::default()
        };
        let db = Database::with_environment(&root, settings, env);
        let (tx, rx) = mpsc::channel();
        db.open(
            EncryptionKey::new(vec![1; 64]),
            Some(Box::new(move |e| tx.send(e).unwrap())),
        );
        assert!(rx.recv().unwrap().is_ok());
        let put = |byte| {
            let (tx, rx) = mpsc::channel();
            db.put(
                Key::new(1, 0),
                vec![byte; 21],
                Some(Box::new(move |e| tx.send(e).unwrap())),
            );
            assert!(rx.recv().unwrap().is_ok());
            db.sync();
        };
        put(0);
        fs::create_dir(root.join("0/binlog-temp")).unwrap();
        put(1);
        let start = std::time::Instant::now();
        while db.wrapped.with_sync(|d| d.compactor.next_attempt).unwrap() == 0 {
            assert!(start.elapsed().as_secs() < 20);
        }
        fs::remove_dir(root.join("0/binlog-temp")).unwrap();
        put(2);
        assert!(db
            .wrapped
            .with_sync(|d| d.compactor.object.is_none())
            .unwrap());
        let initial = db.wrapped.with_sync(|d| d.compactor.next_attempt).unwrap();
        assert!(initial >= 20000);
        let mut expected = 2u8;
        let start = std::time::Instant::now();
        loop {
            let (active, next, excess) = db
                .wrapped
                .with_sync(|d| {
                    (
                        d.compactor.object.is_some(),
                        d.compactor.next_attempt,
                        d.excess,
                    )
                })
                .unwrap();
            if !active && excess == 0 {
                break;
            }
            if !active {
                // start() continues after fail() in the original, so a late
                // unguarded failure may extend backoff. Follow its actual
                // virtual deadline rather than assuming one notification.
                clock.set(next);
                expected = expected.checked_add(1).unwrap();
                put(expected);
            }
            assert!(start.elapsed().as_secs() < 20);
        }
        let (tx, rx) = mpsc::channel();
        db.get(Key::new(1, 0), move |v| tx.send(v).unwrap());
        assert_eq!(rx.recv().unwrap(), vec![expected; 21]);
        db.close(None);
        db.sync();
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn compactor_backoff_doubles_and_caps_virtual_delay() {
        let root = std::env::temp_dir().join(format!("cache-backoff-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let env = Environment {
            clock: Arc::new(Clock(ManualClock::new(1000))),
            ..Environment::default()
        };
        let mut object = DatabaseObject::new(
            WeakOnQueue::default(),
            root.clone(),
            Settings::default(),
            env,
        );
        object.path = root.clone();
        let delays = [
            10000, 20000, 40000, 80000, 160000, 320000, 640000, 1280000, 2560000, 5120000,
            10240000, 20480000, 40960000, 81920000, 86400000, 86400000,
        ];
        for pair in delays.windows(2) {
            object.compactor_fail();
            assert_eq!(object.compactor.next_attempt, 1000 + pair[0]);
            assert_eq!(object.compactor.delay_after_failure, pair[1]);
        }
        drop(object);
        fs::remove_dir(root).unwrap();
    }
}
