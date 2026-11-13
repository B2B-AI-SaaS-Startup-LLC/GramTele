#[path = "support/database.rs"]
mod support;
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    sync::{mpsc, Arc, Mutex},
};
use support::*;
use tg_cache::{
    cache::{storage_cache_binlog_reader::*, storage_cache_types::*},
    cache::{Database, ErrorType, Key, Settings, SettingsUpdate, TaggedValue},
    storage_databases::Databases,
    storage_encrypted_file::{File, Mode, Result as FileResult},
};

#[test]
fn native_databases_read_and_byte_identical_writes() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/database");
    let temp = Temp::new("goldens");
    for name in SCENARIOS {
        scenario(&temp.0, name, true);
        let native = tree(&fixtures.join(name));
        assert_eq!(tree(&temp.0.join(name)), native, "native bytes: {name}");
    }
    let copies = Temp::new("native-reader");
    copy_tree(&fixtures, &copies.0);
    for name in SCENARIOS {
        scenario(&copies.0, name, false);
    }
}
#[test]
fn packed_types_key_order_and_checksum_boundaries() {
    for n in [0, 1, 255, 256, 65535, 65536, 0xffffff] {
        assert_eq!(unpack24(&pack24(n)), n);
    }
    assert!(std::panic::catch_unwind(|| pack24(1 << 24)).is_err());
    assert!(!Key::default().valid());
    assert!(Key::new(0, 1).valid());
    assert!(Key::new(1, 0) > Key::new(0, u64::MAX));
    assert_eq!(
        Key::decode(&Key::new(u64::MAX, 0x123456789abcdef0).encode()),
        Key::new(u64::MAX, 0x123456789abcdef0)
    );
    assert_eq!(
        place_from_id([0x12, 0xab, 0, 0xff, 0x01, 0x34, 0x56]),
        "21/BA00FF104365"
    );
    assert_eq!(count_checksum(b""), 0x02cc5d05);
    assert_eq!(count_checksum(b"123456789"), 0x937bad67);
    let h = BasicHeader {
        format: 0x12,
        flags: 0xabcdef,
        system_time: 0x12345678,
        reserved1: 0x87654321,
        reserved2: u32::MAX,
    };
    assert_eq!(h.encode()[..4], [0x12, 0xef, 0xcd, 0xab]);
    assert_eq!(BasicHeader::decode(&h.encode()), h);
    for t in [
        EstimatedTimePoint::default(),
        EstimatedTimePoint {
            relative: u64::MAX,
            system: u32::MAX,
        },
        EstimatedTimePoint {
            relative: 1 << 32,
            system: 1,
        },
    ] {
        assert_eq!(EstimatedTimePoint::decode(&t.encode()), t);
    }
}

#[test]
fn binlog_reader_sizes_reserved_flags_eof_and_invalid_state() {
    let s = settings("stores");
    assert_eq!(record_size(&[], &s), RecordSize::Unknown);
    assert_eq!(record_size(&[1], &s), RecordSize::Known(48));
    assert_eq!(record_size(&[2], &s), RecordSize::Unknown);
    assert_eq!(record_size(&[255], &s), RecordSize::Invalid);
    assert_eq!(
        record_size(
            &Record::MultiAccess(EstimatedTimePoint::default(), vec![]).encode(),
            &s
        ),
        RecordSize::Known(16)
    );
    assert_eq!(
        record_size(&Record::MultiStore(vec![]).encode(), &s),
        RecordSize::Invalid
    );
    assert_eq!(record_size(&[4], &settings("plain")), RecordSize::Invalid);
    let t = Temp::new("reader-state");
    let mut file = File::default();
    assert_eq!(
        file.open(&t.0.join("binlog"), Mode::Write, &key()),
        FileResult::Success
    );
    assert!(file.write(
        &mut BasicHeader {
            flags: 0xff0001,
            system_time: 42,
            reserved1: 7,
            reserved2: 9,
            ..BasicHeader::default()
        }
        .encode()
    ));
    assert!(file.flush());
    file.close();
    assert_eq!(
        file.open(&t.0.join("binlog"), Mode::ReadAppend, &key()),
        FileResult::Success
    );
    assert_eq!(
        BinlogWrapper::read_header(&mut file, &s).unwrap().reserved1,
        7
    );
    assert_eq!(BinlogWrapper::read_header(&mut file, &s), None);
    let mut wrapper = BinlogWrapper::new(&file, &s, 0);
    assert!(wrapper.read_till_end(&mut file, |_| panic!("empty binlog")));
    assert!(!wrapper.finished());
    assert!(!wrapper.failed());
    assert!(file.write(&mut [255; 16]));
    assert!(file.seek(16));
    let mut wrapper = BinlogWrapper::new(&file, &s, 0);
    assert!(!wrapper.read_till_end(&mut file, |_| panic!("bad type")));
    assert!(wrapper.failed());
    assert!(wrapper.finished());
    assert!(wrapper.read_record(&mut file).is_none());
    assert!(wrapper.read_till_end(&mut file, |_| false));
}

fn read_plain_binlog(base: &Path) -> Vec<u8> {
    let mut f = File::default();
    assert_eq!(
        f.open(&base.join("0/binlog"), Mode::Read, &key()),
        FileResult::Success
    );
    let mut b = vec![0; f.size() as usize];
    assert_eq!(f.read(&mut b), b.len() as isize);
    b
}
fn replace_binlog(base: &Path, bytes: &[u8]) {
    let mut f = File::with_random(Arc::new(Random::default()));
    assert_eq!(
        f.open(&base.join("0/binlog"), Mode::Write, &key()),
        FileResult::Success
    );
    assert!(f.write(&mut bytes.to_vec()));
    assert!(f.flush());
}
#[test]
fn version_paths_extra_bytes_clear_and_cleaner() {
    let t = Temp::new("version");
    assert_eq!(read_version_value(&t.0), None);
    assert!(write_version_value(&t.0, -123));
    let path = version_file_path(&t.0);
    assert_eq!(fs::read(&path).unwrap(), (-123i32).to_le_bytes());
    fs::write(&path, [0xff, 0xff, 0xff, 0x7f, 42]).unwrap();
    assert_eq!(read_version_value(&t.0), Some(i32::MAX));
    assert_eq!(compute_base_path(&t.0.join("a/../b/./")), t.0.join("b"));
    fs::remove_file(&path).unwrap();
    for d in ["0", "2", "junk", ".hidden"] {
        fs::create_dir(t.0.join(d)).unwrap();
        fs::write(t.0.join(d).join("old"), b"old").unwrap();
    }
    let host = Host::new();
    let db = Database::with_environment(&t.0, settings("stores"), host.env.clone());
    assert!(open(&db, key()).is_ok());
    put(&db, 0, 0, 21);
    let (tx, rx) = mpsc::channel();
    db.clear(Some(Box::new(move |e| tx.send(e).unwrap())));
    assert!(rx.recv().unwrap().is_ok());
    cleaned(&db);
    assert_eq!(read_version_value(&t.0), Some(1));
    assert!(t.0.join("1/binlog").exists());
    for d in ["0", "2", "junk"] {
        assert!(!t.0.join(d).exists());
    }
    assert!(t.0.join(".hidden").exists());
    assert!(get(&db, 0).bytes.is_empty());
    closed(&db);
}
#[test]
fn client_api_copy_move_if_empty_sizes_tags_stats_and_subscription_lifetime() {
    let t = Temp::new("api");
    let host = Host::new();
    let db = Database::with_environment(&t.0, settings("stores"), host.env.clone());
    assert!(open(&db, key()).is_ok());
    let records = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let r = records.clone();
    let life = db
        .stats_on_main()
        .on_next_returning(move |s| r.borrow_mut().push((std::thread::current().id(), s)));
    host.step(&db);
    assert_eq!(records.borrow().last().unwrap().1.full.count, 0);
    put(&db, 0, 0, 21);
    db.put_if_empty(id(0), value(9, 30), None);
    db.copy_if_empty(id(0), id(1), None);
    db.copy_if_empty(id(9), id(2), None);
    db.move_if_empty(id(0), id(2), None);
    db.move_if_empty(id(1), id(2), None);
    db.put_if_empty(id(3), TaggedValue::new(value(3, 11), 7), None);
    db.sync();
    assert!(get(&db, 0).bytes.is_empty());
    assert_eq!(get(&db, 1), TaggedValue::new(value(0, 21), 1));
    assert_eq!(get(&db, 2), TaggedValue::new(value(0, 21), 1));
    let (tx, rx) = mpsc::channel();
    db.get_with_sizes(
        id(1),
        vec![id(0), id(1), id(2), id(3), id(99)],
        move |b, s| tx.send((b, s)).unwrap(),
    );
    assert_eq!(rx.recv().unwrap(), (value(0, 21), vec![0, 21, 21, 11, 0]));
    let (tx, rx) = mpsc::channel();
    db.get_with_sizes(id(99), vec![id(1)], move |b, s| tx.send((b, s)).unwrap());
    assert_eq!(rx.recv().unwrap(), (vec![], vec![]));
    db.move_if_empty(id(99), id(8), None);
    db.put(id(3), Vec::new(), None);
    db.sync();
    let stats = host.stats(&db);
    assert_eq!(
        stats.full,
        TaggedSummary {
            count: 2,
            total_size: 42
        }
    );
    assert_eq!(stats.tagged[&1].count, 2);
    assert_eq!(stats.tagged[&7].count, 0);
    host.step(&db);
    assert!(records
        .borrow()
        .iter()
        .all(|(id, _)| *id == std::thread::current().id()));
    let len = records.borrow().len();
    drop(life);
    put(&db, 4, 4, 21);
    host.step(&db);
    assert_eq!(records.borrow().len(), len);
    closed(&db);
}
#[test]
fn delayed_bundles_virtual_time_sorted_keys_and_backward_wall_clock() {
    let t = Temp::new("bundles");
    let host = Host::new();
    let s = Settings {
        max_bundled_records: 2,
        write_bundle_delay: 100,
        ..settings("stores")
    };
    let db = Database::with_environment(&t.0, s, host.env.clone());
    assert!(open(&db, key()).is_ok());
    for i in 0..3 {
        put(&db, i, i, 21);
    }
    let original = read_plain_binlog(&t.0).len();
    get(&db, 2);
    host.wall(1700000002);
    host.advance(&db, 99);
    assert_eq!(read_plain_binlog(&t.0).len(), original);
    host.advance(&db, 1);
    assert_eq!(read_plain_binlog(&t.0).len(), original + 32);
    get(&db, 1);
    get(&db, 0);
    db.sync();
    let bytes = read_plain_binlog(&t.0);
    assert_eq!(bytes.len(), original + 32 + 48);
    assert_eq!(Key::decode(&bytes[bytes.len() - 32..]), id(0));
    assert_eq!(Key::decode(&bytes[bytes.len() - 16..]), id(1));
    db.remove(id(2), None);
    db.remove(id(0), None);
    db.sync();
    assert_eq!(read_plain_binlog(&t.0).len(), bytes.len() + 48);
    closed(&db);
    host.wall(1600000000);
    assert!(open(&db, key()).is_ok());
    closed(&db);
    let bytes = read_plain_binlog(&t.0);
    assert_eq!(bytes[bytes.len() - 16], 4);
    assert_eq!(unpack24(&bytes[bytes.len() - 15..]), 0);
}
#[test]
fn checksum_missing_short_and_wrong_key_values_are_removed() {
    for kind in ["missing", "short", "checksum", "wrong-key"] {
        let t = Temp::new(kind);
        let host = Host::new();
        let db = Database::with_environment(&t.0, settings("stores"), host.env.clone());
        assert!(open(&db, key()).is_ok());
        put(&db, 0, 0, 21);
        closed(&db);
        let bytes = read_plain_binlog(&t.0);
        let store = Store::decode(&bytes[16..], true);
        let path = t.0.join("0").join(place_from_id(store.place));
        match kind {
            "missing" => fs::remove_file(&path).unwrap(),
            "short" => fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(124)
                .unwrap(),
            "checksum" => {
                let mut b = fs::read(&path).unwrap();
                b[116] ^= 1;
                fs::write(&path, b).unwrap();
            }
            _ => {
                let mut b = fs::read(&path).unwrap();
                b[4] ^= 1;
                fs::write(&path, b).unwrap();
            }
        }
        assert!(open(&db, key()).is_ok());
        assert!(get(&db, 0).bytes.is_empty());
        assert_eq!(host.stats(&db).full.count, 0);
        closed(&db);
        assert!(!path.exists());
        assert_eq!(read_plain_binlog(&t.0)[64], 3);
    }
}
#[test]
fn binlog_truncated_tail_invalid_type_count_and_store_rollback() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/database/stores");
    for kind in [
        "partial",
        "unknown",
        "zero-store",
        "zero-remove",
        "oversize",
        "zero-size",
        "multi-partial",
    ] {
        let t = Temp::new(kind);
        copy_tree(&fixtures, &t.0);
        let mut bytes = read_plain_binlog(&t.0);
        bytes.truncate(16 + 4 * 48);
        let valid = bytes.len();
        match kind {
            "partial" => bytes.extend([1; 16]),
            "unknown" => bytes.extend([0xff; 16]),
            "zero-store" => bytes.extend(Record::MultiStore(vec![]).encode()),
            "zero-remove" => bytes.extend(Record::MultiRemove(vec![]).encode()),
            "oversize" => {
                let mut b = Record::MultiRemove(vec![id(0)]).encode();
                b[1..4].copy_from_slice(&pack24(16385));
                bytes.extend(b);
            }
            "zero-size" => {
                let mut s = Store::decode(&bytes[16..], true);
                s.size = 0;
                bytes.extend(s.encode());
            }
            _ => {
                let mut a = Store::decode(&bytes[16..], true);
                a.key = id(9);
                let mut b = a.clone();
                b.key = id(10);
                b.size = 0;
                bytes.extend(Record::MultiStore(vec![a, b]).encode());
            }
        }
        replace_binlog(&t.0, &bytes);
        let host = Host::new();
        let db = Database::with_environment(&t.0, settings("stores"), host.env.clone());
        assert!(open(&db, key()).is_ok());
        assert_eq!(
            host.stats(&db).full.count,
            if kind == "multi-partial" { 5 } else { 4 },
            "{kind}"
        );
        // A fresh Store overwrites the rejected record at its start; no truncate.
        put(&db, 20, 20, 21);
        closed(&db);
        let actual = read_plain_binlog(&t.0);
        assert_eq!(Store::decode(&actual[valid..], true).key, id(20));
        assert!(actual.len() >= valid + 48);
    }
}
#[test]
fn wrong_key_binlog_clear_flag_short_headers_and_closed_errors() {
    let t = Temp::new("wrong-key");
    let host = Host::new();
    let mut s = settings("stores");
    let db = Database::with_environment(&t.0, s.clone(), host.env.clone());
    assert!(open(&db, key()).is_ok());
    put(&db, 0, 0, 21);
    closed(&db);
    let wrong = tg_cache::storage_encryption::EncryptionKey::new(vec![99; 64]);
    let e = open(&db, wrong.clone());
    assert_eq!(e.kind, ErrorType::WrongKey);
    assert_eq!(e.path, t.0.join("0/binlog"));
    let (tx, rx) = mpsc::channel();
    db.put(id(0), vec![1], Some(Box::new(move |e| tx.send(e).unwrap())));
    assert_eq!(rx.recv().unwrap().path, t.0.join("version"));
    s.clear_on_wrong_key = true;
    db.reconfigure(s);
    assert!(open(&db, wrong).is_ok());
    cleaned(&db);
    assert_eq!(read_version_value(&t.0), Some(1));
    assert!(!t.0.join("0").exists());
    closed(&db);
    for kind in ["short-file", "bad-format", "track-mismatch"] {
        let t = Temp::new(kind);
        let host = Host::new();
        let db = Database::with_environment(&t.0, settings("stores"), host.env.clone());
        assert!(open(&db, key()).is_ok());
        closed(&db);
        if kind == "short-file" {
            fs::write(t.0.join("0/binlog"), b"bad").unwrap();
        } else {
            let mut b = read_plain_binlog(&t.0);
            if kind == "bad-format" {
                b[0] = 1;
            } else {
                b[1] = 0;
            }
            replace_binlog(&t.0, &b);
        }
        assert!(open(&db, key()).is_ok());
        cleaned(&db);
        assert_eq!(read_version_value(&t.0), Some(1));
        closed(&db);
    }
}
#[test]
fn pruning_chunks_clear_by_tag_and_disabled_time_limits() {
    let t = Temp::new("prune-chunks");
    let host = Host::new();
    let db = Database::with_environment(
        &t.0,
        Settings {
            stale_remove_chunk: 1,
            ..settings("stores")
        },
        host.env.clone(),
    );
    assert!(open(&db, key()).is_ok());
    for i in 0..8 {
        db.put(
            id(i),
            TaggedValue::new(value(i, 21), if i % 2 == 0 { 7 } else { 8 }),
            None,
        );
    }
    db.clear_by_tag(7, None);
    cleaned(&db);
    assert_eq!(host.stats(&db).full.count, 4);
    for i in 0..8 {
        assert_eq!(get(&db, i).bytes.is_empty(), i % 2 == 0);
    }
    host.wall(1700000010);
    db.update_settings(SettingsUpdate {
        total_size_limit: 0,
        total_time_limit: 1,
    });
    until(&host, &db, || host.stats(&db).full.count == 0);
    closed(&db);
    let t = Temp::new("plain-no-pruning");
    let mut s = settings("plain");
    s.total_size_limit = 128;
    s.total_time_limit = 1;
    let db = Database::with_environment(&t.0, s, host.env.clone());
    assert!(open(&db, key()).is_ok());
    for i in 0..3 {
        put(&db, i, i, 120);
    }
    host.wall(1900000000);
    host.advance(&db, 100000);
    assert_eq!(host.stats(&db).full.count, 3);
    closed(&db);
}
#[test]
fn databases_exclusive_pointer_reacquisition_cancels_deferred_erase() {
    let t = Temp::new("shared-owner");
    let host = Host::new();
    let owner = Databases::with_environment(host.env.clone());
    let mut first = owner.get(&t.0, settings("stores"));
    assert!(first.is_valid());
    let address = first.get().unwrap() as *const Database;
    assert!(open(&first, key()).is_ok());
    put(&first, 0, 0, 21);
    first.destroy();
    assert!(!first.is_valid());
    let second = owner.get(&t.0, settings("stores"));
    assert_eq!(second.get().unwrap() as *const Database, address);
    second.sync();
    host.env.main.process_all();
    assert!(open(&second, key()).is_ok());
    assert_eq!(get(&second, 0).bytes, value(0, 21));
    cleaned(&second);
    second.sync();
    drop(second);
    // Close + cleaner callback queue the erase on main; processing it permits
    // a completely fresh pointer, with the same persisted database contents.
    for _ in 0..4 {
        host.env.main.process_all();
    }
    let third = owner.get(&t.0, settings("stores"));
    assert!(open(&third, key()).is_ok());
    closed(&third);
    let mut replacement = owner.get(t.0.join("move-a"), settings("stores"));
    assert!(replacement.is_valid());
    let moved = owner.get(t.0.join("move-b"), settings("stores"));
    let address = moved.get().unwrap() as *const Database;
    replacement = moved;
    assert_eq!(replacement.get().unwrap() as *const Database, address);
    let literal = owner.get(t.0.join("aliases"), settings("stores"));
    let alias = owner.get(t.0.join("./aliases"), settings("stores"));
    assert!(!std::ptr::eq(literal.get().unwrap(), alias.get().unwrap()));
}
#[test]
fn legacy_collection_preserves_good_files_directories_symlinks_and_batches() {
    use tg_cache::storage_clear_legacy::*;
    let t = Temp::new("legacy");
    let base = format!("{}/", t.0.display());
    fs::create_dir(t.0.join("dir")).unwrap();
    for name in ["good", "bad", ".hidden"] {
        fs::write(t.0.join(name), b"x").unwrap();
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("bad", t.0.join("file-link")).unwrap();
        std::os::unix::fs::symlink("dir", t.0.join("dir-link")).unwrap();
        std::os::unix::fs::symlink("absent", t.0.join("broken-link")).unwrap();
    }
    let skip = BTreeSet::from(["good".to_owned()]);
    let files = collect_files(&base, 100, &skip);
    assert!(files.contains(&"bad".to_owned()));
    assert!(files.contains(&".hidden".to_owned()));
    assert!(!files.contains(&"dir".to_owned()));
    assert!(!files.contains(&"good".to_owned()));
    assert_eq!(collect_files(&base, 1, &skip).len(), 1);
    assert!(!remove_legacy_file(&t.0.join("absent")));
    // The filter can complete asynchronously, as the client does on main.
    let (tx, rx) = mpsc::channel();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let filter: CollectGoodFiles = Arc::new(move |done| {
        done(BTreeSet::from(["good".to_owned()]));
        if let Some(tx) = tx.lock().unwrap().take() {
            tx.send(()).unwrap();
        }
    });
    clear_legacy_files(base.clone(), filter);
    rx.recv().unwrap();
    // A queue barrier is unavailable for crl::async; collect through the same
    // pool until deletion is observable, without a clock or sleep.
    let start = std::time::Instant::now();
    while t.0.join("bad").exists() {
        std::thread::yield_now();
        assert!(start.elapsed().as_secs() < 20);
    }
    assert!(t.0.join("good").exists());
    assert!(t.0.join("dir").exists());
    #[cfg(unix)]
    assert!(collect_files("/", 1, &BTreeSet::new()).is_empty());
}

#[test]
fn native_recovery_branches_stats_values_and_final_bytes() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let rows = fs::read_to_string(fixtures.join("database-probes/results.csv")).unwrap();
    for row in rows.lines() {
        let fields: Vec<_> = row.split(',').collect();
        let kind = fields[0];
        let t = Temp::new(kind);
        copy_tree(&fixtures.join("database/stores"), &t.0);
        let mut plain = read_plain_binlog(&t.0);
        let original = Store::decode(&plain[16..], true);
        let value_path = t.0.join("0").join(place_from_id(original.place));
        match kind {
            "torn-tail" => {
                let path = t.0.join("0/binlog");
                let len = fs::metadata(&path).unwrap().len();
                fs::OpenOptions::new()
                    .write(true)
                    .open(path)
                    .unwrap()
                    .set_len(len - 5)
                    .unwrap();
            }
            "unknown-tail" | "zero-store" | "zero-remove" | "large-count" | "zero-size"
            | "large-size" | "multi-partial" => {
                plain.truncate(16 + 4 * 48);
                match kind {
                    "unknown-tail" => plain.extend([255; 16]),
                    "zero-store" => plain.extend(Record::MultiStore(vec![]).encode()),
                    "zero-remove" => plain.extend(Record::MultiRemove(vec![]).encode()),
                    "large-count" => {
                        let mut b = Record::MultiRemove(vec![id(0)]).encode();
                        b.truncate(16);
                        b[1..4].copy_from_slice(&pack24(16385));
                        plain.extend(b);
                    }
                    "zero-size" | "large-size" => {
                        let mut s = original.clone();
                        s.size = if kind == "zero-size" { 0 } else { 128 };
                        plain.extend(s.encode());
                    }
                    _ => {
                        let mut s = original.clone();
                        s.key = id(9);
                        let mut z = s.clone();
                        z.key = id(10);
                        z.size = 0;
                        plain.extend(Record::MultiStore(vec![s, z]).encode());
                    }
                }
                replace_binlog(&t.0, &plain);
            }
            "missing-value" => fs::remove_file(&value_path).unwrap(),
            "short-value" | "short-value-header" => fs::OpenOptions::new()
                .write(true)
                .open(&value_path)
                .unwrap()
                .set_len(if kind == "short-value" { 124 } else { 110 })
                .unwrap(),
            "checksum" | "wrong-value-key" => {
                let mut b = fs::read(&value_path).unwrap();
                b[if kind == "checksum" { 116 } else { 4 }] ^= 1;
                fs::write(&value_path, b).unwrap();
            }
            "short-binlog" => fs::write(t.0.join("0/binlog"), b"bad").unwrap(),
            "bad-format" | "track-mismatch" => {
                if kind == "bad-format" {
                    plain[0] = 1;
                } else {
                    plain[1] = 0;
                }
                replace_binlog(&t.0, &plain);
            }
            "bad-version" => fs::write(t.0.join("version"), b"bad").unwrap(),
            "negative-version" => assert!(write_version_value(&t.0, -1)),
            "ready-recovery" => {
                fs::copy(t.0.join("0/binlog"), t.0.join("0/binlog-ready")).unwrap();
                fs::write(t.0.join("0/binlog"), b"bad").unwrap();
            }
            "wrong-binlog-key" | "wrong-key-clear" | "pending-access-prune" => {}
            _ => panic!("unmapped native probe {kind}"),
        }
        let host = Host::new();
        host.wall(if kind == "pending-access-prune" {
            1700000005
        } else {
            1700000010
        });
        let mut s = settings("stores");
        s.clear_on_wrong_key = kind == "wrong-key-clear";
        let db = Database::with_environment(&t.0, s, host.env.clone());
        let k = if kind == "wrong-binlog-key" || kind == "wrong-key-clear" {
            tg_cache::storage_encryption::EncryptionKey::new(vec![99; 64])
        } else {
            key()
        };
        let e = open(&db, k);
        let before = host.stats(&db).full.count;
        let v = get(&db, 0);
        if kind == "pending-access-prune" {
            db.update_settings(SettingsUpdate {
                total_size_limit: 0,
                total_time_limit: 3,
            });
            until(&host, &db, || host.stats(&db).full.count == 0);
        }
        cleaned(&db);
        let after = host.stats(&db).full.count;
        let error_path = if e.path.as_os_str().is_empty() {
            String::new()
        } else {
            e.path
                .strip_prefix(&t.0)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        };
        let bytes = v
            .bytes
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        assert_eq!(
            format!(
                "{kind},{},{error_path},{before},{after},{bytes}",
                e.kind as u8
            ),
            row,
            "native result {kind}"
        );
        closed(&db);
        assert_eq!(
            tree(&t.0),
            tree(&fixtures.join("database-probes").join(kind)),
            "native final bytes {kind}"
        );
    }
}

#[test]
fn legacy_full_batches_and_failed_unlink_preserve_directories() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tg_cache::storage_clear_legacy::*;
    let t = Temp::new("legacy-full");
    fs::write(t.0.join("failed"), b"x").unwrap();
    fs::write(t.0.join("good"), b"x").unwrap();
    for i in 0..10003 {
        fs::write(t.0.join(format!("old-{i}")), b"x").unwrap();
    }
    let base = format!("{}/", t.0.display());
    let root = t.0.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let filter: CollectGoodFiles = Arc::new(move |done| {
        if count.fetch_add(1, Ordering::SeqCst) == 0 {
            fs::remove_file(root.join("failed")).unwrap();
            fs::create_dir(root.join("failed")).unwrap();
        }
        done(BTreeSet::from(["good".to_owned()]));
    });
    clear_legacy_files(base.clone(), filter);
    let start = std::time::Instant::now();
    while calls.load(Ordering::SeqCst) < 2
        || !collect_files(&base, 1, &BTreeSet::from(["good".to_owned()])).is_empty()
    {
        std::thread::yield_now();
        assert!(start.elapsed().as_secs() < 20);
    }
    assert!(t.0.join("good").is_file());
    assert!(t.0.join("failed").is_dir());
}

#[cfg(unix)]
#[test]
fn native_cleaner_root_and_nested_symlink_removal() {
    use std::os::unix::{fs::symlink, fs::PermissionsExt};
    let t = Temp::new("cleaner-links");
    for dir in ["0/target", "0/keep", "2/nested", ".hidden"] {
        fs::create_dir_all(t.0.join(dir)).unwrap();
    }
    for name in [
        "0/target/deleted",
        "0/keep/kept",
        "2/nested/readonly",
        ".hidden/kept",
    ] {
        fs::write(t.0.join(name), b"x").unwrap();
    }
    fs::set_permissions(
        t.0.join("2/nested/readonly"),
        fs::Permissions::from_mode(0o444),
    )
    .unwrap();
    symlink("0/target", t.0.join("link")).unwrap();
    symlink("../0/keep", t.0.join("2/nested-link")).unwrap();
    assert!(write_version_value(&t.0, 0));
    let host = Host::new();
    let db = Database::with_environment(&t.0, settings("stores"), host.env.clone());
    assert!(open(&db, key()).is_ok());
    cleaned(&db);
    let result = format!(
        "cleaner,{},{},{},{},{},{}\n",
        u8::from(t.0.join("2").exists()),
        u8::from(t.0.join(".hidden/kept").exists()),
        u8::from(t.0.join("0/keep/kept").exists()),
        u8::from(t.0.join("0/target/deleted").exists()),
        u8::from(
            fs::symlink_metadata(t.0.join("link"))
                .unwrap()
                .file_type()
                .is_symlink()
        ),
        u8::from(t.0.join("0/target").is_dir())
    );
    assert_eq!(result, include_str!("fixtures/cleaner-check.csv"));
    closed(&db);
}
