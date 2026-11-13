//! Scenario port of tdesktop fe618bd652 (2019-02-19), not a source of
//! database semantics. All operations execute the ccdc725 port; time is virtual.
#[path = "support/database.rs"]
mod support;
use std::{
    fs,
    path::Path,
    sync::{mpsc, Arc, Mutex},
};
use support::*;
use tg_cache::{
    cache::{Database, Key, Settings, TaggedValue},
    storage_encrypted_file::SystemRandom,
};
fn legacy_settings() -> Settings {
    Settings {
        track_estimated_time: false,
        write_bundle_delay: 1000,
        prune_timeout: 1500,
        max_data_size: 20,
        ..Settings::default()
    }
}
fn old_key(i: u32) -> Key {
    Key::new(u64::from(i), u64::from(i + 1))
}
fn old_value(i: u32, second: bool) -> Vec<u8> {
    let mut v = if second {
        b"bytetestbytetestb".to_vec()
    } else {
        b"testbytetestbyt".to_vec()
    };
    v[0] = b'A'.wrapping_add(i as u8);
    v
}
fn put_old(db: &Database, from: u32, till: u32, second: bool) {
    for i in from..till {
        db.put(old_key(i), old_value(i, second), None);
        db.sync();
    }
}
fn get_old(db: &Database, key: Key) -> Vec<u8> {
    let (tx, rx) = mpsc::channel();
    db.get(key, move |v| tx.send(v).unwrap());
    let v = rx.recv().unwrap();
    db.sync();
    v
}
fn check_old(db: &Database, from: u32, till: u32, second: Option<bool>) {
    for i in from..till {
        assert_eq!(
            get_old(db, old_key(i)),
            second.map_or_else(Vec::new, |v| old_value(i, v)),
            "key {i}"
        );
    }
}
fn remove_old(db: &Database, from: u32, till: u32) {
    for i in from..till {
        db.remove(old_key(i), None);
    }
    db.sync();
}
fn touch_old(db: &Database, from: u32, till: u32) {
    for i in from..till {
        db.get(old_key(i), |_| {});
    }
    db.sync();
}
fn advance(host: &Host, db: &Database, seconds: i64) {
    db.sync();
    let start = tg_cache::cache::storage_cache_database::CacheClock::unix_time(host.clock.as_ref());
    // Sleep in the 2019 scenarios allowed timers to run *during* the
    // interval. Process their deadlines, rather than jumping every callback
    // to the final wall time (which would spuriously refresh old accesses).
    for elapsed in (100..=seconds * 1000).step_by(100) {
        host.wall(start + elapsed / 1000);
        host.advance(db, 100);
    }
}
fn clear(db: &Database) {
    let (tx, rx) = mpsc::channel();
    db.clear(Some(Box::new(move |e| tx.send(e).unwrap())));
    assert!(rx.recv().unwrap().is_ok());
    db.sync();
}
fn binlog_size(base: &Path) -> u64 {
    fs::metadata(base.join("0/binlog")).unwrap().len()
}
fn wait_compact(host: &Host, db: &Database, base: &Path, before: u64) {
    until(host, db, || binlog_size(base) < before && compacted(base));
    db.sync();
}

#[test]
fn legacy_init_timers() {
    let host = Host::new();
    let t = Temp::new("legacy-init");
    let db = Database::with_environment(&t.0, legacy_settings(), host.env.clone());
    clear(&db);
    assert!(open(&db, key()).is_ok());
    put_old(&db, 0, 1, false);
    remove_old(&db, 0, 1);
    let before = binlog_size(&t.0);
    host.advance(&db, 1000);
    assert!(binlog_size(&t.0) > before);
    closed(&db);
}

#[test]
fn legacy_compacting_min_size_without_min_double_and_time_tracking() {
    for variant in ["min-size", "without-min-size", "double", "time"] {
        let t = Temp::new(variant);
        let host = Host::new();
        let mut s = legacy_settings();
        s.write_bundle_delay = 100;
        s.read_block_size = 512;
        s.max_bundled_records = 5;
        s.compact_after_excess = 3 * (16 * 5 + 16) + 15 * 32;
        if variant == "min-size" {
            s.compact_after_excess /= 2;
            s.compact_after_full_size = (16 + 40 * 32) / 2 + s.compact_after_excess;
        }
        if variant == "time" {
            s.track_estimated_time = true;
            s.compact_after_excess =
                6 * (16 * 5 + 16) + 3 * (16 * 5 + 16) + 15 * 48 + 3 * (16 * 5 + 16) + (16 + 16);
        }
        let db = Database::with_environment(&t.0, s, host.env.clone());
        clear(&db);
        assert!(open(&db, key()).is_ok());
        put_old(&db, 0, 30, false);
        if variant == "time" {
            touch_old(&db, 0, 30);
            remove_old(&db, 0, 15);
            put_old(&db, 15, 30, true);
            touch_old(&db, 0, 30);
            advance(&host, &db, 1);
            let before = binlog_size(&t.0);
            touch_old(&db, 29, 30);
            advance(&host, &db, 2);
            wait_compact(&host, &db, &t.0, before);
            check_old(&db, 15, 30, Some(true));
            closed(&db);
            assert!(open(&db, key()).is_ok());
            check_old(&db, 15, 30, Some(true));
            closed(&db);
            continue;
        }
        remove_old(&db, 0, 15);
        if variant != "double" {
            put_old(&db, 30, 40, false);
        }
        put_old(&db, 15, 29, true);
        advance(&host, &db, 1);
        let before = binlog_size(&t.0);
        put_old(&db, 29, 30, true);
        wait_compact(&host, &db, &t.0, before);
        if variant == "double" {
            put_old(&db, 30, 45, false);
            remove_old(&db, 20, 35);
            put_old(&db, 15, 20, false);
            put_old(&db, 35, 44, true);
            let second = binlog_size(&t.0);
            put_old(&db, 44, 45, true);
            wait_compact(&host, &db, &t.0, second);
            assert!(binlog_size(&t.0) < before);
            for reopen in [false, true] {
                if reopen {
                    assert!(open(&db, key()).is_ok());
                }
                check_old(&db, 0, 15, None);
                check_old(&db, 15, 20, Some(false));
                check_old(&db, 20, 35, None);
                check_old(&db, 35, 45, Some(true));
                closed(&db);
            }
        } else {
            remove_old(&db, 30, 35);
            put_old(&db, 35, 37, true);
            put_old(&db, 15, 20, false);
            put_old(&db, 40, 45, false);
            for reopen in [false, true] {
                if reopen {
                    assert!(open(&db, key()).is_ok());
                }
                check_old(&db, 0, 15, None);
                check_old(&db, 15, 20, Some(false));
                check_old(&db, 20, 30, Some(true));
                check_old(&db, 30, 35, None);
                check_old(&db, 35, 37, Some(true));
                check_old(&db, 37, 45, Some(false));
                closed(&db);
            }
        }
    }
}
#[test]
fn legacy_encrypted_write_read_tag_delete_overwrite_and_chunked_reads() {
    let t = Temp::new("legacy-encrypted");
    let host = Host::new();
    let db = Database::with_environment(&t.0, legacy_settings(), host.env.clone());
    clear(&db);
    assert!(open(&db, key()).is_ok());
    let a = b"testbytetestbyt".to_vec();
    let b = b"bytetestbytetestb".to_vec();
    let k = |h, l| Key::new(h, l);
    db.put(k(0, 1), b.clone(), None);
    db.put(k(0, 1), TaggedValue::new(a.clone(), 1), None);
    db.put_if_empty(k(0, 2), b.clone(), None);
    db.put_if_empty(k(0, 2), a.clone(), None);
    db.copy_if_empty(k(0, 1), k(2, 0), None);
    db.copy_if_empty(k(0, 2), k(2, 0), None);
    db.put(k(0, 3), a.clone(), None);
    db.move_if_empty(k(0, 3), k(3, 0), None);
    db.move_if_empty(k(0, 2), k(3, 0), None);
    closed(&db);
    assert!(open(&db, key()).is_ok());
    assert_eq!(get_old(&db, k(0, 1)), a);
    let (tx, rx) = mpsc::channel();
    db.get_with_tag(k(0, 1), move |v| tx.send(v).unwrap());
    assert_eq!(rx.recv().unwrap(), TaggedValue::new(a.clone(), 1));
    db.put(k(1, 0), b.clone(), None);
    db.sync();
    let (tx, rx) = mpsc::channel();
    db.get_with_tag(k(1, 0), move |v| tx.send(v).unwrap());
    assert_eq!(rx.recv().unwrap(), TaggedValue::new(b.clone(), 0));
    assert!(get_old(&db, k(1, 1)).is_empty());
    assert_eq!(get_old(&db, k(0, 2)), b);
    assert_eq!(get_old(&db, k(2, 0)), a);
    assert!(get_old(&db, k(0, 3)).is_empty());
    assert_eq!(get_old(&db, k(3, 0)), a);
    for tag in 1..=3 {
        db.put(k(5, tag), TaggedValue::new(a.clone(), tag as u8), None);
        db.put(k(6, tag), TaggedValue::new(b.clone(), tag as u8), None);
    }
    closed(&db);
    assert!(open(&db, key()).is_ok());
    assert_eq!(get_old(&db, k(0, 1)), a);
    assert_eq!(get_old(&db, k(1, 0)), b);
    db.clear_by_tag(2, None);
    cleaned(&db);
    for tag in 1..=3 {
        assert_eq!(
            get_old(&db, k(5, tag)),
            if tag == 2 { vec![] } else { a.clone() }
        );
        assert_eq!(
            get_old(&db, k(6, tag)),
            if tag == 2 { vec![] } else { b.clone() }
        );
    }
    let before = binlog_size(&t.0);
    db.put(k(0, 1), b.clone(), None);
    db.sync();
    let after = binlog_size(&t.0);
    assert!(after > before);
    assert_eq!(get_old(&db, k(0, 1)), b);
    db.put(k(0, 1), b.clone(), None);
    db.sync();
    assert_eq!(binlog_size(&t.0), after);
    closed(&db);
    let s = Settings {
        read_block_size: 512,
        max_bundled_records: 5,
        track_estimated_time: true,
        ..legacy_settings()
    };
    db.reconfigure(s);
    clear(&db);
    assert!(open(&db, key()).is_ok());
    for i in 0..30 {
        db.put(Key::new(i, i * 2), old_value(i as u32, false), None);
    }
    closed(&db);
    assert!(open(&db, key()).is_ok());
    for i in 0..30 {
        assert_eq!(get_old(&db, Key::new(i, i * 2)), old_value(i as u32, false));
    }
    closed(&db);
}
#[test]
fn legacy_remove_persists_and_bundled_touch_remove_lazy_or_on_close() {
    for touch in [false, true] {
        for on_close in [false, true] {
            let t = Temp::new("legacy-bundles");
            let host = Host::new();
            let s = Settings {
                track_estimated_time: touch,
                ..legacy_settings()
            };
            let db = Database::with_environment(&t.0, s, host.env.clone());
            clear(&db);
            assert!(open(&db, key()).is_ok());
            put_old(&db, 0, 2, false);
            let before = binlog_size(&t.0);
            if touch {
                touch_old(&db, 0, 1);
            } else {
                remove_old(&db, 0, 1);
            }
            assert_eq!(binlog_size(&t.0), before);
            if on_close {
                closed(&db);
            } else {
                advance(&host, &db, 2);
            }
            assert!(binlog_size(&t.0) > before);
            if !on_close {
                closed(&db);
            }
            assert!(open(&db, key()).is_ok());
            check_old(&db, 0, 1, if touch { Some(false) } else { None });
            check_old(&db, 1, 2, Some(false));
            closed(&db);
        }
    }
}
#[test]
fn legacy_both_size_and_time_limits_with_access_updates() {
    let k = |i| old_key(i);
    for variant in ["both", "size", "time"] {
        let t = Temp::new(variant);
        let host = Host::new();
        let mut s = legacy_settings();
        s.track_estimated_time = true;
        if variant != "time" {
            s.total_size_limit = 17 * 3 + 1;
        }
        if variant == "both" {
            s.total_time_limit = 4;
        } else if variant == "time" {
            s.total_time_limit = 3;
        }
        let db = Database::with_environment(&t.0, s, host.env.clone());
        clear(&db);
        assert!(open(&db, key()).is_ok());
        if variant == "both" {
            put_old(&db, 0, 2, false);
            advance(&host, &db, 2);
            touch_old(&db, 1, 2);
            advance(&host, &db, 3);
            put_old(&db, 2, 5, false);
            advance(&host, &db, 2);
            check_old(&db, 0, 2, None);
            check_old(&db, 2, 5, Some(false));
        } else if variant == "size" {
            for i in 0..3 {
                put_old(&db, i, i + 1, false);
                if i == 2 {
                    touch_old(&db, 0, 1);
                }
                advance(&host, &db, 2);
            }
            put_old(&db, 3, 4, false);
            advance(&host, &db, 2);
            assert!(get_old(&db, k(1)).is_empty());
            check_old(&db, 2, 3, Some(false));
            put_old(&db, 4, 6, false);
            advance(&host, &db, 2);
            assert!(get_old(&db, k(0)).is_empty());
            assert!(get_old(&db, k(3)).is_empty());
            for i in [2, 4, 5] {
                check_old(&db, i, i + 1, Some(false));
            }
        } else {
            put_old(&db, 0, 4, false);
            advance(&host, &db, 1);
            touch_old(&db, 1, 3);
            advance(&host, &db, 1);
            touch_old(&db, 0, 2);
            advance(&host, &db, 1);
            touch_old(&db, 0, 2);
            advance(&host, &db, 3);
            check_old(&db, 2, 4, None);
            // ccdc725 updates useTime only when a touch bundle is written.
            // At t=3 the earlier prune timer precedes the touch timer. Key 0
            // is still old and expires; key 1 had a persisted touch at t=2.
            // The old sleep-based assertion depended on the opposite order.
            check_old(&db, 0, 1, None);
            check_old(&db, 1, 2, Some(false));
        }
        closed(&db);
    }
}
#[test]
fn legacy_large_database_102400_records() {
    let t = Temp::new("legacy-large");
    let mut host = Host::new();
    host.env.random = Arc::new(SystemRandom);
    let s = Settings {
        write_bundle_delay: 1000,
        max_data_size: 20,
        total_size_limit: 1024 * 1024,
        total_time_limit: 120,
        prune_timeout: 1500,
        compact_after_excess: 1024 * 1024,
        ..Settings::default()
    };
    let db = Database::with_environment(&t.0, s, host.env.clone());
    assert!(open(&db, key()).is_ok());
    let key_at = |i: u64| Key::new(i * 2, (i << 32) + 3);
    let plain = b"testbytetestbyt".to_vec();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let mut seed = 1u64;
    for i in 0..100 * 1024u64 {
        let errors = errors.clone();
        db.put(
            key_at(i),
            plain.clone(),
            Some(Box::new(move |e| {
                if !e.is_ok() {
                    errors.lock().unwrap().push(e);
                }
            })),
        );
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let j = if i == 0 { 0 } else { (seed >> 16) % i };
        if i % 1024 == 1023 {
            host.advance(&db, 100);
            let sample = get_old(&db, key_at(j));
            assert!(sample.is_empty() || sample == plain);
        } else {
            db.get(key_at(j), |_| {});
        }
    }
    db.sync();
    assert!(errors.lock().unwrap().is_empty());
    cleaned(&db);
    let last = get_old(&db, key_at(100 * 1024 - 1));
    assert!(last.is_empty() || last == plain);
    let before = host.stats(&db);
    assert_eq!(
        before.full.total_size,
        before.full.count as i64 * plain.len() as i64
    );
    closed(&db);
    assert!(open(&db, key()).is_ok());
    assert_eq!(host.stats(&db).full, before.full);
    assert_eq!(get_old(&db, key_at(100 * 1024 - 1)), last);
    closed(&db);
}
