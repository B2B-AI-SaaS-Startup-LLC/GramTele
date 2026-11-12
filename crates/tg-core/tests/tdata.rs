use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tg_core::{
    main::{
        main_account::{compose_data_string, Authorization},
        main_domain::Domain as MainDomain,
    },
    storage::{
        details::storage_file_utilities::*,
        serialize_common::*,
        storage_account::{compute_data_name_key, Account, AccountMap},
        storage_domain::Domain,
    },
};
use tg_mtproto::{
    core_types::Random,
    mtp_instance::{Fields, Instance, Mode},
    mtproto_auth_key::{AuthKey, KeyType},
    mtproto_dc_options::Environment,
};
#[derive(Default)]
struct Entropy(u8);
impl Random for Entropy {
    fn fill(&mut self, data: &mut [u8]) {
        for b in data {
            *b = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ayugram-synthetic-tdata-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn expected(index: i32) -> Authorization {
    let key = |dc, slot| {
        Arc::new(AuthKey::new(
            KeyType::ReadFromFile,
            dc,
            std::array::from_fn(|i| (i as i32 + index * 17 + slot * 31) as u8),
            0,
        ))
    };
    Authorization {
        user_id: 1234567890123 + index as u64,
        main_dc_id: 2,
        keys: vec![key(2, 0), key(4, 1)],
        keys_to_destroy: vec![key(1, 2)],
    }
}
fn write_synthetic(root: &Path, pass: bool, count: i32) {
    let mut domain = Domain::new(root, "data");
    domain.local_key = Some(Arc::new(AuthKey::local(std::array::from_fn(|i| i as u8))));
    let mut random = Entropy::default();
    let mut manager = WriteManager::default();
    domain
        .encrypt_local_key(if pass { b"test-passcode" } else { b"" }, &mut random)
        .unwrap();
    for i in 0..count {
        let mut a = Account::new(root, &compose_data_string("data", i));
        domain.start_added(&mut a);
        a.authorization = expected(i);
        a.write_map(&mut manager, &mut random).unwrap();
        a.write_mtp_data(&mut manager, &mut random).unwrap();
        domain
            .owner
            .account_added_in_storage(i, a.authorization)
            .unwrap();
    }
    domain.owner.activate_from_storage(count - 1);
    domain.write_accounts(&mut manager, &mut random).unwrap();
    manager.stop().unwrap();
}
fn check_domain(root: &Path, pass: bool, count: i32) {
    let mut d = Domain::new(root, "data");
    d.start_modern(if pass { b"test-passcode" } else { b"" })
        .unwrap();
    assert_eq!(d.owner.accounts.len(), count as usize);
    assert_eq!(d.owner.active_for_storage(), count - 1);
    for (i, a) in &d.owner.accounts {
        assert_eq!(
            a.serialize_mtp_authorization(),
            expected(*i).serialize_mtp_authorization()
        );
    }
    if pass && count == 1 {
        assert!(matches!(
            d.start_modern(b"wrong password"),
            Err(Error::IncorrectPasscode)
        ));
    }
}
fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out)
            } else {
                out.push((p.strip_prefix(root).unwrap().into(), fs::read(p).unwrap()));
            }
        }
    }
    let mut out = vec![];
    walk(root, root, &mut out);
    out.sort();
    out
}
#[test]
fn original_directories_read_and_exact_write() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tdata");
    for pass in [false, true] {
        for count in [1, 3] {
            let name = format!("{}-{count}", if pass { "pass" } else { "nopass" });
            let source = fixture.join(name);
            check_domain(&source, pass, count);
            let dest = Temp::new();
            write_synthetic(&dest.0, pass, count);
            assert_eq!(tree(&source), tree(&dest.0));
        }
    }
}
#[test]
#[ignore = "manual synthetic directory export for the native reader; requires AYUGRAM_SYNTHETIC_REFERENCE_OUTPUT"]
fn export_directories_for_native_reader() {
    // Explicit opt-in writes only synthetic fixtures to a fresh test output path.
    if let Some(root) = std::env::var_os("AYUGRAM_SYNTHETIC_REFERENCE_OUTPUT") {
        let root = PathBuf::from(root);
        assert!(!root.exists());
        for pass in [false, true] {
            for count in [1, 3] {
                let dir = root.join(format!("{}-{count}", if pass { "pass" } else { "nopass" }));
                write_synthetic(&dir, pass, count);
            }
        }
    }
}
#[test]
fn envelopes_corruption_truncation_and_local_lengths() {
    let key = AuthKey::local([7; 256]);
    let mut entropy = Entropy::default();
    for len in 0..65 {
        let mut data = EncryptedDescriptor::new(len);
        data.stream.0.extend(vec![19; len]);
        let cipher = prepare_encrypted(&mut data, &key, &mut entropy);
        let plain = decrypt_local(&cipher, &key).unwrap();
        assert_eq!(&plain.stream.0[4..], vec![19; len]);
        assert!(decrypt_local(&cipher, &AuthKey::local([8; 256])).is_err());
        for n in 0..cipher.len() {
            assert!(decrypt_local(&cipher[..n], &key).is_err());
        }
    }
    let bytes = encode_file(b"payload", APP_VERSION);
    assert_eq!(decode_file(&bytes).unwrap().data, b"payload");
    for n in 0..bytes.len() {
        assert!(decode_file(&bytes[..n]).is_err());
    }
    for i in 0..bytes.len() {
        let mut bad = bytes.clone();
        bad[i] ^= 1;
        assert!(decode_file(&bad).is_err());
    }
    assert!(decode_file(&encode_file(b"", APP_VERSION + 1)).is_err());
    assert_eq!(
        decode_file(&encode_file(b"legacy", 8000)).unwrap().version,
        8000
    );
}
#[test]
fn suffix_selection_safe_priority_and_queued_write_coalescing() {
    let temp = Temp::new();
    let root = &temp.0;
    fs::write(root.join("x0"), encode_file(b"older", 1)).unwrap();
    fs::write(root.join("x1"), encode_file(b"newer", 2)).unwrap();
    let set = |name, seconds| {
        fs::File::options()
            .write(true)
            .open(root.join(name))
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
            )
            .unwrap();
    };
    set("x0", 1);
    set("x1", 2);
    assert_eq!(read_file("x", root).unwrap().data, b"newer");
    assert!(!root.join("x0").exists());
    fs::write(root.join("xs"), b"bad modern").unwrap();
    assert!(read_file("x", root).is_err());
    fs::remove_file(root.join("xs")).unwrap();
    fs::write(root.join("x0"), encode_file(b"fallback", 3)).unwrap();
    fs::write(root.join("x1"), b"bad newer").unwrap();
    set("x0", 1);
    set("x1", 2);
    assert_eq!(read_file("x", root).unwrap().data, b"fallback");
    assert!(!root.join("x1").exists());
    let mut m = WriteManager::default();
    for value in [b"first".as_slice(), b"last".as_slice()] {
        m.write(WriteEntry {
            base: root.join("queued"),
            data: encode_file(value, APP_VERSION),
        });
    }
    m.sync().unwrap();
    assert_eq!(read_file("queued", root).unwrap().data, b"last");
    assert!(!m.write_one_scheduled_now().unwrap());
    m.write(WriteEntry {
        base: root.join("queued"),
        data: encode_file(b"old", 1),
    });
    m.write_sync(WriteEntry {
        base: root.join("queued"),
        data: encode_file(b"sync", 2),
    })
    .unwrap();
    m.stop().unwrap();
    assert_eq!(read_file("queued", root).unwrap().data, b"sync");
}
#[test]
fn authorization_versions_instance_keys_and_account_names() {
    let a = expected(0);
    assert_eq!(
        Authorization::from_bytes(&a.serialize_mtp_authorization())
            .unwrap()
            .user_id,
        a.user_id
    );
    let mut old = ByteArrayWriter::default();
    old.i32(12345);
    old.i32(2);
    old.0.extend(&a.serialize_mtp_authorization()[20..]);
    assert_eq!(Authorization::from_bytes(&old.0).unwrap().user_id, 12345);
    for n in 0..20 {
        assert!(Authorization::from_bytes(&a.serialize_mtp_authorization()[..n]).is_err());
    }
    let mut fields = a.fields(Environment::Test);
    fields.device_model = "Desktop".into();
    let instance = Instance::new(Mode::Normal, fields);
    let destroyer = Instance::new(Mode::KeysDestroyer, Fields::new(Environment::Test));
    a.apply_keys_to_destroy(&destroyer);
    let got = Authorization::from_instance(a.user_id, &instance, Some(&destroyer));
    assert_eq!(
        got.serialize_mtp_authorization(),
        a.serialize_mtp_authorization()
    );
    instance.shutdown();
    destroyer.shutdown();
    assert_eq!(compose_data_string("d#a#ta", 0), "data");
    assert_eq!(compose_data_string("data", 2), "data#3");
    assert_eq!(to_file_part(0x123456789abcdef0), "0FEDCBA987654321");
    assert_ne!(
        compute_data_name_key("data"),
        compute_data_name_key("data#2")
    );
    assert_eq!(a.session_unique_id(true), a.user_id | (1 << 56));
}
#[test]
fn qt_null_empty_utf16_bytes_color_and_bad_lengths() {
    let mut w = ByteArrayWriter::default();
    w.nullable_bytes(None);
    w.nullable_bytes(Some(b""));
    w.nullable_utf16(Some(&[0xD800, 0x41, 0xDC00]));
    write_bytes(&mut w, &[]);
    write_color(&mut w, [1, 2, 3, 4]);
    let mut r = ByteArrayReader::new(&w.0);
    assert_eq!(r.nullable_bytes(), Some(None));
    assert_eq!(r.nullable_bytes(), Some(Some(vec![])));
    assert_eq!(r.nullable_utf16(), Some(Some(vec![0xD800, 0x41, 0xDC00])));
    assert_eq!(r.nullable_bytes(), Some(None));
    assert_eq!(read_color(&mut r), Some([1, 2, 3, 4]));
    assert!(r.at_end());
    assert_eq!(string_size(Some(&[1, 2])), 8);
    assert_eq!(bytearray_size(&[]), 4);
    assert_eq!(bytes_size(&[1]), 5);
    assert_eq!(color_size(), 4);
    assert_eq!(date_time_size(), 13);
    assert!(ByteArrayReader::new(&[0, 0, 0, 1, 0])
        .nullable_utf16()
        .is_none());
    assert!(ByteArrayReader::new(&[0xff, 0xff, 0xff, 0xfe])
        .bytes()
        .is_none());
}
#[test]
fn legacy_local_keys_map_handles_and_passcode_changes() {
    let temp = Temp::new();
    let mut random = Entropy::default();
    let mut manager = WriteManager::default();
    let mut domain = Domain::new(&temp.0, "data");
    domain
        .start_from_scratch(&mut random, &mut manager)
        .unwrap();
    assert!(!domain.has_local_passcode());
    let changes = std::rc::Rc::new(std::cell::Cell::new(0));
    let notified = changes.clone();
    let lifetime = tg_rpl::Lifetime::new();
    domain
        .local_passcode_changed()
        .on_next(move |_| notified.set(notified.get() + 1), &lifetime);
    domain
        .set_passcode(b"password", &mut manager, &mut random)
        .unwrap();
    assert_eq!(changes.get(), 1);
    assert!(domain.has_local_passcode());
    assert!(domain.check_passcode(b"password"));
    assert!(!domain.check_passcode(b"bad"));
    domain.clear_old_version();
    assert_eq!(domain.old_version(), 0);
    manager.sync().unwrap();
    let mut account = Account::new(&temp.0, "legacy");
    let salt = [4; 32];
    let local = Arc::new(AuthKey::local([5; 256]));
    let mut key = EncryptedDescriptor::new(256);
    key.stream.0.extend(local.data());
    let mut map = EncryptedDescriptor::new(0);
    map.stream.i32(0x1e);
    map.stream.u64(42);
    let mut f = FileWriteDescriptor::new("map", &account.base_path);
    f.write_data(Some(&salt));
    f.write_encrypted(
        &mut key,
        &create_legacy_local_key(b"pass", &salt),
        &mut random,
    );
    f.write_encrypted(&mut map, &local, &mut random);
    f.finish(&mut manager, true).unwrap();
    assert!(account.read_map_with(None, b"wrong").is_err());
    account.read_map_with(None, b"pass").unwrap();
    assert!(account.local_key.as_ref().unwrap().equals(&local));
    assert_eq!(account.map.data.len(), 12);
    account.authorization = expected(0);
    account.write_mtp_data(&mut manager, &mut random).unwrap();
    manager.sync().unwrap();
    let mut migrated = Domain::new(&temp.0, "legacy");
    assert!(matches!(
        migrated.start(b"wrong", &mut random, &mut manager),
        Err(Error::IncorrectPasscodeLegacy)
    ));
    migrated.start(b"pass", &mut random, &mut manager).unwrap();
    assert_eq!(migrated.owner.accounts[0].1.user_id, expected(0).user_id);
    assert_eq!(migrated.old_version(), APP_VERSION);
    manager.sync().unwrap();

    assert!(account.temp_directory().ends_with("temp_legacy"));
    assert!(account.support_mode_path().ends_with("user_legacy/support"));
    assert!(AccountMap::parse(vec![0, 0, 0, 0]).is_err());
    assert!(AccountMap::parse(vec![0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff]).is_err());
    let k = generate_key(&temp.0, &mut random);
    let mut p = temp.0.join(to_file_part(k)).to_str().unwrap().to_string();
    assert!(!key_already_used(&mut p));
    fs::write(temp.0.join(format!("{}s", to_file_part(k))), b"").unwrap();
    let mut p = temp.0.join(to_file_part(k)).to_str().unwrap().to_string();
    assert!(key_already_used(&mut p));
    clear_key(k, &temp.0);
    assert!(!temp.0.join(format!("{}s", to_file_part(k))).exists());
    let mut main = MainDomain::default();
    main.account_added_in_storage(0, expected(0)).unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || main.account_added_in_storage(0, expected(1))
    ))
    .is_err());
}

#[test]
fn queued_account_list_and_map_virtual_timers() {
    use std::sync::Mutex;
    use tg_base::timer::{ManualClock, TimerHost};
    use tg_core::storage::storage_account::AccountWrites;
    tg_runtime::main_queue().bind_current_thread();
    let temp = Temp::new();
    let clock = ManualClock::new(0);
    let host = TimerHost::new(Arc::new(clock.clone()));
    let mut random = Entropy::default();
    let mut a = Account::new(&temp.0, "data");
    a.local_key = Some(Arc::new(AuthKey::local([2; 256])));
    let mut writes = AccountWrites::new(&host);
    let mut manager = WriteManager::default();
    writes.write_map_delayed();
    clock.advance(999);
    host.process();
    assert!(!writes.process(&a, &mut manager, &mut random).unwrap());
    clock.advance(1);
    host.process();
    assert!(writes.process(&a, &mut manager, &mut random).unwrap());
    assert!(!writes.process(&a, &mut manager, &mut random).unwrap());
    writes.write_map_queued();
    tg_runtime::process_main_queue();
    assert!(writes.process(&a, &mut manager, &mut random).unwrap());
    manager.sync().unwrap();
    let mut d = Domain::new(&temp.0, "data");
    d.local_key = a.local_key.clone();
    d.encrypt_local_key(&[], &mut random).unwrap();
    d.owner.account_added_in_storage(0, expected(0)).unwrap();
    let d = std::rc::Rc::new(std::cell::RefCell::new(d));
    let manager = Arc::new(Mutex::new(manager));
    for _ in 0..3 {
        tg_core::main::main_domain::schedule_write_accounts(&d, manager.clone());
    }
    tg_runtime::process_main_queue();
    manager.lock().unwrap().sync().unwrap();
    assert!(read_file("key_data", &temp.0).is_ok());
    tg_core::main::main_domain::schedule_write_accounts(&d, manager.clone());
    drop(d);
    tg_runtime::process_main_queue();
}

#[test]
fn account_config_legacy_blocks_and_partial_authorization() {
    let temp = Temp::new();
    let mut random = Entropy::default();
    let mut manager = WriteManager::default();
    let mut a = Account::new(&temp.0, "data");
    a.local_key = Some(Arc::new(AuthKey::local([3; 256])));
    let config = tg_mtproto::mtproto_config::Config::new(Environment::Test);
    a.write_mtp_config(&config, &mut manager, &mut random)
        .unwrap();
    manager.sync().unwrap();
    assert!(a.read_mtp_config().unwrap().unwrap().is_test_mode());
    let mut inner = EncryptedDescriptor::new(0);
    inner.stream.i32(1);
    inner.stream.i32(123);
    inner.stream.i32(2);
    inner.stream.i32(0);
    inner.stream.i32(2);
    inner.stream.0.extend([42; 256]);
    let mut f = FileWriteDescriptor::new(&to_file_part(a.data_name_key), &temp.0);
    f.write_encrypted(&mut inner, a.local_key.as_ref().unwrap(), &mut random);
    f.finish(&mut manager, true).unwrap();
    a.read_mtp_data().unwrap();
    assert_eq!(a.authorization.user_id, 123);
    assert_eq!(a.authorization.keys[0].data(), &[42; 256]);
    let modern = expected(0);
    a.authorization = Authorization::default();
    a.apply_read_authorization_context(&modern.serialize_mtp_authorization(), expected(1))
        .unwrap();
    assert_eq!(a.authorization.user_id, modern.user_id);
    assert_eq!(a.authorization.keys.len(), 2);
    let mut partial = Authorization::default();
    let full = modern.serialize_mtp_authorization();
    assert!(partial.set_mtp_authorization(&full[..284]).is_err());
    assert_eq!(partial.user_id, modern.user_id);
    assert_eq!(partial.keys.len(), 1);
    assert_eq!(Authorization::default().session_unique_id(true), 0);
    let mut lock = tg_core::storage::storage_file_lock::FileLock::new();
    let mut file = None;
    let mut opts = fs::OpenOptions::new();
    opts.read(true).write(true).create(true);
    assert!(lock.lock(&mut file, &temp.0.join("lock"), &opts));
    assert!(lock.locked());
    lock.unlock();
    assert!(!lock.locked());
}

#[test]
fn asynchronous_manager_drop_flush_and_io_failures() {
    let temp = Temp::new();
    let root = &temp.0;
    let mut manager = AsyncWriteManager::default();
    manager.write(WriteEntry {
        base: root.join("async"),
        data: encode_file(b"first", 1),
    });
    manager
        .write_sync(WriteEntry {
            base: root.join("async"),
            data: encode_file(b"sync", 2),
        })
        .unwrap();
    manager.sync().unwrap();
    assert_eq!(read_file("async", root).unwrap().data, b"sync");
    manager.stop().unwrap();
    {
        let mut file = FileWriteDescriptor::new("drop", root);
        file.write_data(Some(b"saved by destructor"));
    }
    tg_core::storage::details::storage_file_utilities::sync().unwrap();
    let file = read_file("drop", root).unwrap();
    assert_eq!(file.reader().bytes().unwrap(), b"saved by destructor");
    tg_core::storage::details::storage_file_utilities::finish().unwrap();
    fs::write(root.join("blocked"), b"file").unwrap();
    let mut queue = WriteManager::default();
    queue.write(WriteEntry {
        base: root.join("blocked/x"),
        data: encode_file(b"bad", 1),
    });
    queue.write(WriteEntry {
        base: root.join("good"),
        data: encode_file(b"good", 1),
    });
    assert!(queue.sync().is_err());
    assert_eq!(read_file("good", root).unwrap().data, b"good");
}

#[test]
fn original_qt_stream_and_both_pbkdf_variants() {
    let bytes = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qt-pbkdf.bin"))
        .unwrap();
    let mut r = ByteArrayReader::new(&bytes);
    assert_eq!(r.nullable_bytes(), Some(None));
    assert_eq!(r.nullable_bytes(), Some(Some(vec![])));
    assert_eq!(r.nullable_utf16(), Some(None));
    assert_eq!(r.nullable_utf16(), Some(Some(vec![])));
    assert_eq!(r.nullable_utf16(), Some(Some(vec![0xd800, 0x41, 0xdc00])));
    assert_eq!(read_color(&mut r), Some([1, 2, 3, 4]));
    let salt: Vec<_> = (0..32).collect();
    for pass in [b"".as_slice(), b"legacy-pass".as_slice()] {
        let key = create_legacy_local_key(pass, &salt);
        assert_eq!(r.raw(256).unwrap(), key.data());
    }
    for pass in [b"".as_slice(), b"pass".as_slice()] {
        let key = create_local_key(pass, &salt);
        assert_eq!(r.raw(256).unwrap(), key.data());
    }
    let narrow = r.bytes().unwrap();
    let expected_wide = r.bytes().unwrap();
    assert_eq!(
        Authorization::from_bytes(&narrow)
            .unwrap()
            .serialize_mtp_authorization(),
        expected_wide
    );
    assert!(r.at_end());
}

#[test]
fn malformed_modern_info_preserves_original_start_contract() {
    let temp = Temp::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tdata/nopass-1");
    let file = read_file("key_data", &fixture).unwrap();
    let mut r = file.reader();
    let salt = r.bytes().unwrap();
    let key = r.bytes().unwrap();
    let mut payload = ByteArrayWriter::default();
    payload.bytes(&salt);
    payload.bytes(&key);
    payload.bytes(b"broken info");
    fs::write(
        temp.0.join("key_datas"),
        encode_file(&payload.0, APP_VERSION),
    )
    .unwrap();
    let mut domain = Domain::new(&temp.0, "data");
    assert!(matches!(
        domain.start_modern(&[]),
        Err(Error::Invalid("could not decrypt info"))
    ));
    assert!(domain.local_key.is_some());
    // Original start() falls back to generateLocalKey with a key already assigned.
    // Preserve that Expects failure as a Rust panic, rather than silently wiping it.
    let mut again = Domain::new(&temp.0, "data");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        again.start(&[], &mut Entropy::default(), &mut WriteManager::default())
    }));
    assert!(result.is_err());
}
