#![allow(dead_code)]
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicI64, AtomicUsize, Ordering},
        mpsc, Arc,
    },
};
use tg_base::{concurrent_timer::ConcurrentTimerEnvironment, timer::ManualClock};
use tg_cache::{
    cache::storage_cache_database::CacheClock,
    cache::{Database, Environment, Key, Settings, SettingsUpdate, Stats, TaggedValue},
    storage_encrypted_file::RandomSource,
    storage_encryption::EncryptionKey,
};
pub struct Clock {
    pub wall: AtomicI64,
    pub mono: ManualClock,
}
impl CacheClock for Clock {
    fn unix_time(&self) -> i64 {
        self.wall.load(Ordering::SeqCst)
    }
    fn monotonic_ms(&self) -> i64 {
        tg_base::timer::Clock::now(&self.mono)
    }
}
#[derive(Default)]
pub struct Random(AtomicUsize);
impl RandomSource for Random {
    fn fill(&self, bytes: &mut [u8]) {
        for b in bytes {
            *b = self.0.fetch_add(1, Ordering::SeqCst) as u8;
        }
    }
}
#[derive(Default)]
pub struct WideRandom(AtomicUsize);
impl RandomSource for WideRandom {
    fn fill(&self, b: &mut [u8]) {
        for v in b {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            *v = (n ^ (n >> 8) ^ (n >> 16)) as u8;
        }
    }
}
pub struct Host {
    pub clock: Arc<Clock>,
    pub timers: Arc<ConcurrentTimerEnvironment>,
    pub env: Environment,
}
impl Host {
    pub fn new() -> Self {
        let clock = Arc::new(Clock {
            wall: AtomicI64::new(1700000000),
            mono: ManualClock::new(10000),
        });
        let timers = Arc::new(ConcurrentTimerEnvironment::manual(Arc::new(
            clock.mono.clone(),
        )));
        let main = tg_runtime::MainQueue::new();
        main.bind_current_thread();
        let env = Environment {
            clock: clock.clone(),
            timers: Some(timers.clone()),
            random: Arc::new(Random::default()),
            main,
        };
        Self { clock, timers, env }
    }
    pub fn wall(&self, time: i64) {
        self.clock.wall.store(time, Ordering::SeqCst);
    }
    pub fn step(&self, db: &Database) {
        db.sync();
        self.timers.process();
        db.sync();
        db.sync();
        self.env.main.process_all();
    }
    pub fn advance(&self, db: &Database, ms: i64) {
        db.sync();
        self.clock.mono.advance(ms);
        self.step(db);
    }
    pub fn stats(&self, db: &Database) -> Stats {
        let result = Rc::new(RefCell::new(None));
        let r = result.clone();
        let life = db.stats_on_main().on_next_returning(move |v| {
            *r.borrow_mut() = Some(v);
        });
        db.sync();
        db.sync();
        self.env.main.process_all();
        drop(life);
        result.take().expect("initial stats")
    }
}
pub fn key() -> EncryptionKey {
    EncryptionKey::new((0..64).collect())
}
pub fn id(i: usize) -> Key {
    Key::new(0x123456789abcdef0, (i + 1) as u64)
}
pub fn value(i: usize, n: usize) -> Vec<u8> {
    (0..n).map(|j| (i * 31 + j) as u8).collect()
}
pub fn open(db: &Database, k: EncryptionKey) -> tg_cache::cache::Error {
    let (tx, rx) = mpsc::channel();
    db.open(k, Some(Box::new(move |e| tx.send(e).unwrap())));
    rx.recv().unwrap()
}
pub fn put(db: &Database, i: usize, v: usize, n: usize) {
    let (tx, rx) = mpsc::channel();
    db.put(
        id(i),
        TaggedValue::new(value(v, n), (i + 1) as u8),
        Some(Box::new(move |e| tx.send(e).unwrap())),
    );
    assert!(rx.recv().unwrap().is_ok());
    db.sync();
}
pub fn get(db: &Database, i: usize) -> TaggedValue {
    let (tx, rx) = mpsc::channel();
    db.get_with_tag(id(i), move |v| tx.send(v).unwrap());
    let result = rx.recv().unwrap();
    db.sync();
    result
}
pub fn closed(db: &Database) {
    let (tx, rx) = mpsc::channel();
    db.close(Some(Box::new(move || tx.send(()).unwrap())));
    rx.recv().unwrap();
    db.sync();
}
pub fn cleaned(db: &Database) {
    let (tx, rx) = mpsc::channel();
    db.wait_for_cleaner(Some(Box::new(move || tx.send(()).unwrap())));
    rx.recv().unwrap();
    db.sync();
}
pub fn settings(name: &str) -> Settings {
    let mut s = Settings {
        max_data_size: 127,
        prune_timeout: 0,
        ..Settings::default()
    };
    if name == "plain" {
        s.track_estimated_time = false;
    }
    if name == "size" || name == "size-ties" || name == "time" {
        s.total_time_limit = 0;
        s.write_bundle_delay = 0;
        s.total_size_limit = if name == "size" || name == "size-ties" {
            200
        } else {
            0
        };
    }
    if name == "compact" || name == "all-records" {
        s.compact_after_excess = 48;
    }
    if name == "wide" {
        s.max_data_size = (1 << 24) - 1;
        s.max_bundled_records = 257;
        s.compact_after_excess = 48;
    }
    s
}
pub fn compacted(base: &Path) -> bool {
    let Ok(bytes) = fs::read(base.join("0/binlog")) else {
        return false;
    };
    if bytes.len() < 148 {
        return false;
    }
    let mut b = bytes[132..148].to_vec();
    key().prepare_ctr_state(&bytes[4..68]).decrypt(&mut b, 64);
    b[0] == 2 && !base.join("0/binlog-temp").exists() && !base.join("0/binlog-ready").exists()
}
pub fn until(host: &Host, db: &Database, mut predicate: impl FnMut() -> bool) {
    let start = std::time::Instant::now();
    while !predicate() {
        host.step(db);
        assert!(start.elapsed().as_secs() < 20, "queue/timer deadline");
    }
}
pub fn scenario(root: &Path, name: &str, generate: bool) {
    let mut host = Host::new();
    if name == "wide" {
        host.env.random = Arc::new(WideRandom::default());
    }
    let base = root.join(name);
    let mut config = settings(name);
    if !generate {
        config.compact_after_excess = 0;
    }
    if generate && (name == "size" || name == "size-ties") {
        config.total_size_limit = 0;
    }
    let db = Database::with_environment(&base, config, host.env.clone());
    assert!(open(&db, key()).is_ok());
    if generate {
        match name {
            "empty" => {}
            "wide" => {
                for i in 0..257 {
                    put(&db, i, i, if i == 0 { 0x12345 } else { 21 });
                }
                put(&db, 0, 9, 0x12345);
                until(&host, &db, || compacted(&base));
            }
            "size" | "size-ties" | "time" => {
                for i in 0..3 {
                    host.wall(1700000000 + if name == "size-ties" { 0 } else { i as i64 * 3 });
                    put(&db, i, i, 120);
                }
                if name == "time" {
                    host.wall(1700000010);
                    db.update_settings(SettingsUpdate {
                        total_size_limit: 0,
                        total_time_limit: 5,
                    });
                } else {
                    db.update_settings(SettingsUpdate {
                        total_size_limit: 200,
                        total_time_limit: 0,
                    });
                }
                until(&host, &db, || host.stats(&db).full.count == 1);
            }
            _ => {
                for i in 0..4 {
                    put(&db, i, i, 21);
                }
                match name {
                    "removed" => {
                        for i in 0..4 {
                            db.remove(id(i), None);
                        }
                    }
                    "compact" | "all-records" => {
                        put(&db, 0, 9, 21);
                        until(&host, &db, || compacted(&base));
                        if name == "all-records" {
                            put(&db, 4, 4, 21);
                            db.remove(id(1), None);
                            host.wall(1700000002);
                            assert_eq!(get(&db, 2).bytes, value(2, 21));
                        }
                    }
                    _ => {
                        db.remove(id(1), None);
                        host.wall(1700000002);
                        assert_eq!(get(&db, 0).bytes, value(0, 21));
                    }
                }
            }
        }
    } else {
        let stats = host.stats(&db);
        let expected = match name {
            "empty" | "removed" => 0,
            "size" | "size-ties" | "time" => 1,
            "compact" | "all-records" => 4,
            "wide" => 257,
            _ => 3,
        };
        assert_eq!(stats.full.count, expected, "{name}");
        let checks: Vec<_> = match name {
            "empty" | "removed" => vec![],
            "size" | "size-ties" | "time" => vec![if name == "size-ties" {
                (0, 0, 120)
            } else {
                (2, 2, 120)
            }],
            "compact" => vec![(0, 9, 21), (1, 1, 21), (2, 2, 21), (3, 3, 21)],
            "all-records" => vec![(0, 9, 21), (2, 2, 21), (3, 3, 21), (4, 4, 21)],
            "wide" => (0..257)
                .map(|i| {
                    (
                        i,
                        if i == 0 { 9 } else { i },
                        if i == 0 { 0x12345 } else { 21 },
                    )
                })
                .collect(),
            _ => vec![(0, 0, 21), (2, 2, 21), (3, 3, 21)],
        };
        for (i, v, n) in checks {
            assert_eq!(
                get(&db, i),
                TaggedValue::new(value(v, n), (i + 1) as u8),
                "{name} key {i}"
            );
        }
    }
    cleaned(&db);
    closed(&db);
}
pub const SCENARIOS: &[&str] = &[
    "empty",
    "stores",
    "plain",
    "removed",
    "size",
    "size-ties",
    "time",
    "compact",
    "all-records",
    "wide",
];
pub fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in fs::read_dir(at).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                visit(root, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_owned(),
                    fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for (p, b) in tree(from) {
        let p = to.join(p);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b).unwrap();
    }
}
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tg-cache-db-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
