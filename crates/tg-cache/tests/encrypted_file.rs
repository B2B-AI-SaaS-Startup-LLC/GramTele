use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
use tg_cache::{
    storage_encrypted_file::{File, Mode, Result},
    storage_encryption::{CtrState, EncryptionKey},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ayu-cache-file-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("cache.tdef")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn key(size: usize) -> EncryptionKey {
    EncryptionKey::new((0..size).map(|v| v as u8).collect())
}
fn pattern(size: usize, start: u8) -> Vec<u8> {
    (0..size).map(|v| start.wrapping_add(v as u8)).collect()
}
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
fn hex(data: &[u8]) -> String {
    data.iter().map(|v| format!("{v:02x}")).collect()
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn bytes(&mut self, size: usize) -> &'a [u8] {
        let (result, rest) = self.0.split_at(size);
        self.0 = rest;
        result
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.bytes(4).try_into().unwrap())
    }
    fn i64(&mut self) -> i64 {
        i64::from_le_bytes(self.bytes(8).try_into().unwrap())
    }
    fn vector(&mut self) -> &'a [u8] {
        let size = self.u32() as usize;
        self.bytes(size)
    }
}

#[test]
fn original_openssl_ctr_vectors_offsets_wrap_and_key_derivation() {
    let mut input = Cursor(include_bytes!("fixtures/ctr.bin"));
    let count = input.u32();
    assert_eq!(count, 30);
    for index in 0..count {
        let derived = input.u32() != 0;
        let key_bytes = input.vector();
        let iv_or_salt = input.vector();
        let offset = input.i64();
        let size = input.u32() as usize;
        let plain = input.bytes(size);
        let expected = input.bytes(size);
        let state = if derived {
            EncryptionKey::new(key_bytes.to_vec()).prepare_ctr_state(iv_or_salt)
        } else {
            CtrState::new(key_bytes, iv_or_salt)
        };
        let mut data = plain.to_vec();
        state.encrypt(&mut data, offset);
        assert_eq!(data, expected, "native vector {index}");
        state.decrypt(&mut data, offset);
        assert_eq!(data, plain);
        if offset <= i64::MAX - 48 {
            state.encrypt(&mut data[..48], offset);
            state.encrypt(&mut data[48..], offset + 48);
            assert_eq!(data, expected, "split native vector {index}");
        }
    }
    assert!(input.0.is_empty());
}

#[test]
fn native_file_headers_damage_truncation_and_read_results() {
    for line in include_str!("fixtures/probes.txt").lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        let mut file = File::default();
        let result = file.open(
            &fixtures().join(parts[0]),
            Mode::Read,
            &key(parts[1].parse().unwrap()),
        );
        let result = match result {
            Result::Failed => 0,
            Result::LockFailed => 1,
            Result::WrongKey => 2,
            Result::Success => 3,
        };
        let mut data = [0; 32];
        let count = if result == 3 { file.read(&mut data) } else { 0 };
        assert_eq!(
            format!(
                "{result} {} {} {} {count} {}",
                u8::from(file.is_open()),
                file.size(),
                file.offset(),
                hex(&data)
            ),
            parts[2..].join(" "),
            "native probe {} key {}",
            parts[0],
            parts[1]
        );
    }
}

#[test]
fn original_sections_write_append_seek_read_and_move() {
    let temp = Temp::new();
    let path = temp.file();
    let key = EncryptionKey::new(b"abcdefgh01234567".repeat(16));
    let mut file = File::default();
    assert_eq!(file.open(&path, Mode::Write, &key), Result::Success);
    let mut data = b"testbytetestbyte".to_vec();
    assert!(file.write(&mut data));
    file.close();
    assert_eq!(file.open(&path, Mode::ReadAppend, &key), Result::Success);
    let mut data = [0; 16];
    assert_eq!(file.read(&mut data), 16);
    assert_eq!(&data, b"testbytetestbyte");
    let mut data = b"bytetestbytetest".to_vec();
    assert!(file.write(&mut data));
    file.close();
    assert_eq!(file.open(&path, Mode::ReadAppend, &key), Result::Success);
    assert_eq!((file.offset(), file.size()), (0, 32));
    assert!(file.seek(16));
    let mut data = [0; 16];
    assert_eq!(file.read(&mut data), 16);
    assert_eq!(&data, b"bytetestbytetest");
    assert_eq!((file.offset(), file.size()), (32, 32));
    assert!(file.seek(16));
    let mut data = *b"testbytetestbyte";
    assert!(file.write(&mut data));
    let first_ciphertext = data;
    assert!(file.write(&mut data));
    assert_eq!((file.offset(), file.size()), (48, 48));
    file.close();
    assert_eq!(file.open(&path, Mode::Read, &key), Result::Success);
    let mut read = [0; 48];
    assert_eq!(file.read(&mut read), 48);
    assert_eq!(&read[..32], b"testbytetestbytetestbytetestbyte");
    assert_eq!(&read[32..], &first_ciphertext);
    file.close();
    let moved = temp.0.join("other.tdef");
    assert!(File::move_file(&path, &moved));
    assert!(!path.exists());
    assert_eq!(file.open(&moved, Mode::Read, &key), Result::Success);
}

#[test]
fn padding_all_remainders_mutates_full_blocks_and_preserves_partial_input() {
    let temp = Temp::new();
    let key = key(64);
    for size in 0..=65 {
        let path = temp.0.join(size.to_string());
        let mut file = File::default();
        assert_eq!(file.open(&path, Mode::Write, &key), Result::Success);
        let plain = pattern(size, 0x10);
        let mut data = plain.clone();
        assert!(file.write_with_padding(&mut data));
        assert!(file.flush());
        let padded = size.div_ceil(16) * 16;
        assert_eq!((file.size(), file.offset()), (padded as i64, padded as i64));
        assert_eq!(&data[size - size % 16..], &plain[size - size % 16..]);
        if size >= 16 {
            assert_ne!(data[..16], plain[..16]);
        }
        file.close();
        assert_eq!(fs::metadata(&path).unwrap().len(), 116 + padded as u64);
        assert_eq!(file.open(&path, Mode::Read, &key), Result::Success);
        let mut read = vec![0; size];
        assert_eq!(file.read_with_padding(&mut read), size as isize);
        assert_eq!(read, plain);
        assert_eq!(file.offset(), padded as i64);
        let mut end = [0; 16];
        assert_eq!(file.read(&mut end), 0);
    }
}

#[test]
fn append_creates_truncate_replaces_and_bad_opens_reset_state() {
    let temp = Temp::new();
    let path = temp.0.join("nested/cache");
    let key = key(256);
    let mut file = File::default();
    assert_eq!(file.open(&path, Mode::Read, &key), Result::Failed);
    assert!(!path.parent().unwrap().exists());
    assert_eq!(file.open(&path, Mode::ReadAppend, &key), Result::Success);
    assert_eq!(file.size(), 0);
    assert!(file.write(&mut [7; 32]));
    file.close();
    assert_eq!(file.open(&path, Mode::Write, &key), Result::Success);
    assert_eq!(file.size(), 0);
    file.close();
    assert_eq!(fs::metadata(&path).unwrap().len(), 116);
    fs::write(&path, b"bad").unwrap();
    assert_eq!(file.open(&path, Mode::ReadAppend, &key), Result::Failed);
    assert!(!file.is_open());
    assert_eq!((file.size(), file.offset()), (0, 0));
    assert_eq!(fs::read(&path).unwrap(), b"bad");
    assert_eq!(file.open(&temp.0, Mode::Write, &key), Result::LockFailed);
    let parent = temp.0.join("ordinary-file");
    fs::write(&parent, b"x").unwrap();
    assert_eq!(
        file.open(&parent.join("child"), Mode::Write, &key),
        Result::Failed
    );
}

#[test]
fn read_only_failed_writes_restore_buffer_and_encryption_offset() {
    let mut file = File::default();
    assert_eq!(
        file.open(&fixtures().join("key64.tdef"), Mode::Read, &key(64)),
        Result::Success
    );
    assert!(file.seek(16));
    let mut data = pattern(32, 0x17);
    let plain = data.clone();
    assert!(!file.write(&mut data));
    assert_eq!(data, plain);
    assert_eq!((file.offset(), file.size()), (16, 80));
    data = pattern(19, 0x90);
    let plain = data.clone();
    assert!(!file.write_with_padding(&mut data));
    assert_eq!(data, plain);
    assert_eq!(file.offset(), 16);
    let mut read = [0; 16];
    assert_eq!(file.read(&mut read), 16);
    assert_eq!(read, pattern(16, 0x40).as_slice());
}

#[test]
fn closed_and_invalid_seek_alignment_contracts() {
    let mut file = File::default();
    assert!(!file.is_open());
    assert_eq!((file.size(), file.offset()), (0, 0));
    assert!(!file.seek(0));
    assert!(!file.flush());
    assert_eq!(file.read(&mut [0; 16]), 0);
    assert!(!file.write(&mut [0; 16]));
    assert!(file.write_with_padding(&mut []));
    assert_eq!(file.read_with_padding(&mut []), 0);
    assert_eq!(
        file.open(&fixtures().join("key64.tdef"), Mode::Read, &key(64)),
        Result::Success
    );
    assert!(!file.seek(-1));
    assert!(!file.seek(81));
    assert_eq!(file.offset(), 0);
    assert!(file.seek(1));
    assert_eq!(file.offset(), 1);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| file.read(&mut [0; 16]))).is_err()
    );
    file.close();
    assert_eq!((file.size(), file.offset()), (0, 0));
}

#[test]
fn encryption_preconditions_match_key_salt_and_block_contracts() {
    assert!(EncryptionKey::default().empty());
    assert!(EncryptionKey::default().data().is_empty());
    for size in [0, 32, 63, 65, 255, 257] {
        assert!(std::panic::catch_unwind(|| EncryptionKey::new(vec![0; size])).is_err());
    }
    for (length, offset) in [(17, 0), (16, 1), (16, -16)] {
        assert!(std::panic::catch_unwind(
            || CtrState::new(&[0; 32], &[0; 16]).encrypt(&mut vec![0; length], offset)
        )
        .is_err());
    }
    assert!(std::panic::catch_unwind(|| key(64).prepare_ctr_state(&[0; 63])).is_err());
    assert!(
        std::panic::catch_unwind(|| EncryptionKey::default().prepare_ctr_state(&[0; 64])).is_err()
    );
    assert!(std::panic::catch_unwind(|| CtrState::new(&[0; 31], &[0; 16])).is_err());
    assert!(std::panic::catch_unwind(|| CtrState::new(&[0; 32], &[0; 15])).is_err());
}

#[test]
fn move_replaces_destination_and_preserves_failure_behavior() {
    let temp = Temp::new();
    let source = temp.0.join("from");
    let target = temp.0.join("to");
    assert!(!File::move_file(&source, &target));
    fs::write(&source, b"source").unwrap();
    fs::write(&target, b"target").unwrap();
    assert!(File::move_file(&source, &target));
    assert_eq!(fs::read(&target).unwrap(), b"source");
    assert!(!source.exists());
    fs::write(&source, b"source").unwrap();
    assert!(!File::move_file(&source, &temp.0.join("missing/sub")));
    assert_eq!(fs::read(&source).unwrap(), b"source");
    assert!(!File::move_file(&source, &source));
    assert!(!source.exists());
}

#[cfg(unix)]
#[test]
fn native_move_file_directory_and_symlink_cases() {
    let temp = Temp::new();
    for line in include_str!("fixtures/moves.txt").lines() {
        let kind: usize = line.split_whitespace().next().unwrap().parse().unwrap();
        let base = temp.0.join(kind.to_string());
        fs::create_dir(&base).unwrap();
        let source = base.join("from");
        let target = if kind == 3 {
            base.join("missing/to")
        } else if kind == 4 {
            source.clone()
        } else {
            base.join("to")
        };
        let aux = base.join("aux");
        if kind == 5 {
            fs::create_dir(&source).unwrap();
            fs::write(source.join("child"), b"source").unwrap();
        } else if kind == 7 {
            fs::write(&aux, b"source").unwrap();
            std::os::unix::fs::symlink(&aux, &source).unwrap();
        } else if kind != 2 {
            fs::write(&source, b"source").unwrap();
        }
        if kind == 1 {
            fs::write(&target, b"target").unwrap();
        } else if kind == 6 {
            fs::write(&aux, b"target").unwrap();
            std::os::unix::fs::symlink(&aux, &target).unwrap();
        }
        let result = File::move_file(&source, &target);
        let content = if target.is_file() {
            hex(&fs::read(&target).unwrap())
        } else {
            "-".into()
        };
        let aux_content = if aux.exists() {
            let value = fs::read(&aux).unwrap();
            if value.is_empty() {
                "empty".into()
            } else {
                hex(&value)
            }
        } else {
            "-".into()
        };
        let symlink =
            fs::symlink_metadata(&target).is_ok_and(|value| value.file_type().is_symlink());
        assert_eq!(
            format!(
                "{kind} {} {} {} {} {} {content} {aux_content}",
                u8::from(result),
                u8::from(source.exists()),
                u8::from(target.exists()),
                u8::from(target.is_dir()),
                u8::from(symlink)
            ),
            line
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn native_move_across_filesystems_copies_then_removes_source() {
    use std::os::unix::fs::MetadataExt;
    let temp = Temp::new();
    let Ok(other) = fs::metadata("/dev/shm") else {
        return;
    };
    if fs::metadata(&temp.0).unwrap().dev() == other.dev() {
        return;
    }
    let path = Path::new("/dev/shm").join(format!(
        "ayu-cache-cross-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    let other = Temp(path);
    let source = temp.0.join("from");
    let target = other.0.join("to");
    fs::write(&source, b"source").unwrap();
    let result = File::move_file(&source, &target);
    assert_eq!(
        format!(
            "cross {} {} {} {}\n",
            u8::from(result),
            u8::from(source.exists()),
            u8::from(target.exists()),
            hex(&fs::read(&target).unwrap())
        ),
        include_str!("fixtures/cross.txt")
    );
}

#[test]
fn original_child_process_file_holder() {
    let Some(path) = std::env::var_os("TG_CACHE_TEST_HOLDER") else {
        return;
    };
    let path = PathBuf::from(path);
    let mut file = File::default();
    assert_eq!(
        file.open(&path, Mode::ReadAppend, &key(64)),
        Result::Success
    );
    let mut bytes = [0; 16];
    assert_eq!(file.read(&mut bytes), 16);
    assert_eq!(bytes, [3; 16]);
    assert!(file.write(&mut bytes) && file.flush());
    use std::io::{Read, Write};
    println!("holder-ready");
    std::io::stdout().flush().unwrap();
    // An open pipe keeps the holder alive until FileLock terminates it.
    let mut byte = [0; 1];
    std::io::stdin().read_exact(&mut byte).unwrap();
}

#[test]
fn original_two_process_read_and_writer_takeover() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
    };
    let temp = Temp::new();
    let path = temp.file();
    let mut file = File::default();
    assert_eq!(file.open(&path, Mode::Write, &key(64)), Result::Success);
    assert!(file.write(&mut [3; 16]));
    file.close();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "original_child_process_file_holder",
            "--test-threads=1",
            "--nocapture",
        ])
        .env("TG_CACHE_TEST_HOLDER", &path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert!(
            output.read_line(&mut line).unwrap() > 0,
            "holder exited before ready"
        );
        if line.contains("holder-ready") {
            break;
        }
    }
    assert_eq!(file.open(&path, Mode::Read, &key(64)), Result::Success);
    let mut data = [0; 32];
    assert_eq!(file.read(&mut data), 32);
    assert_eq!(data, [3; 32]);
    file.close();
    // Child::wait closes its stdin. Keep our pipe outside Child so the
    // helper remains blocked until FileLock actually terminates it.
    let _keep_stdin_open = child.stdin.take().unwrap();
    let reaper = std::thread::spawn(move || child.wait().unwrap());
    assert_eq!(
        file.open(&path, Mode::ReadAppend, &key(64)),
        Result::Success
    );
    assert_eq!(file.read(&mut data), 32);
    assert_eq!(data, [3; 32]);
    let status = reaper.join().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(15));
    }
    #[cfg(not(unix))]
    {
        let _ = status;
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
fn file_fault_child() {
    let Some(root) = std::env::var_os("TG_CACHE_FAULT_CHILD") else {
        return;
    };
    use nix::sys::{
        resource::{getrlimit, setrlimit, Resource},
        signal::{SigSet, Signal},
    };
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGXFSZ);
    mask.thread_block().unwrap();
    let root = PathBuf::from(root);
    for large in [false, true] {
        let (soft, hard) = getrlimit(Resource::RLIMIT_FSIZE).unwrap();
        setrlimit(Resource::RLIMIT_FSIZE, 124, hard).unwrap();
        let path = root.join(if large { "large" } else { "small" });
        let mut f = File::default();
        let opened = f.open(&path, Mode::Write, &key(64));
        let mut b = vec![7; if large { 32768 } else { 16 }];
        let original = b.clone();
        let wrote = f.write(&mut b);
        let flushed = f.flush();
        println!(
            "oracle:{},{},{},{},{},{},{},{}",
            if large { "large" } else { "small" },
            opened as u8,
            u8::from(wrote),
            u8::from(flushed),
            f.offset(),
            f.size(),
            u8::from(b == original),
            fs::metadata(&path).unwrap().len()
        );
        f.close();
        setrlimit(Resource::RLIMIT_FSIZE, soft, hard).unwrap();
    }
    let mut f = File::default();
    let opened = f.open(Path::new("/dev/full"), Mode::Write, &key(64));
    let wrote = f.write(&mut [7; 16]);
    let flushed = f.flush();
    println!(
        "oracle:full,{},{},{},{},{}",
        opened as u8,
        u8::from(wrote),
        u8::from(flushed),
        f.offset(),
        f.size()
    );
    assert!(!f.seek(0));
    assert_eq!(f.read(&mut [0; 16]), 0);
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
fn native_qfile_buffering_short_write_enospc_and_flush_failures() {
    let t = Temp::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "file_fault_child", "--nocapture"])
        .env("TG_CACHE_FAULT_CHILD", &t.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let actual = stdout
        .lines()
        .filter_map(|s| s.strip_prefix("oracle:"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(actual, include_str!("fixtures/file-faults.csv"));
}
