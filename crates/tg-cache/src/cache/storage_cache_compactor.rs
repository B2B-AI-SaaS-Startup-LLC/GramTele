//! Queue-to-queue compaction with live lookups, tail catch-up and ready rename.
use super::{
    storage_cache_binlog_reader::BinlogWrapper,
    storage_cache_database::Environment,
    storage_cache_database_object::{DatabaseObject, Entry},
    storage_cache_types::*,
};
use crate::{
    storage_encrypted_file::{File, Mode, Result as FileResult},
    storage_encryption::EncryptionKey,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use tg_base::binary_guard::BinaryGuard;
use tg_runtime::{ObjectOnQueue, WeakOnQueue};
#[derive(Clone, Copy)]
pub(super) struct CompactorInfo {
    pub till: i64,
    pub system_time: u32,
    pub keys_count: usize,
}
pub(super) struct Compactor {
    weak: WeakOnQueue<Self>,
    database: WeakOnQueue<DatabaseObject>,
    guard: BinaryGuard,
    base: PathBuf,
    settings: Settings,
    key: EncryptionKey,
    info: CompactorInfo,
    binlog: File,
    compact: File,
    wrapper: BinlogWrapper,
    written: HashSet<Key>,
    list: Vec<Store>,
}
pub(super) fn start(
    database: WeakOnQueue<DatabaseObject>,
    guard: BinaryGuard,
    base: PathBuf,
    settings: Settings,
    key: EncryptionKey,
    info: CompactorInfo,
    env: Environment,
) -> ObjectOnQueue<Compactor> {
    ObjectOnQueue::new_with_weak(move |weak| {
        assert!(settings.compact_chunk_size > 0);
        let binlog = File::default();
        let wrapper = BinlogWrapper::new(&binlog, &settings, info.till);
        let mut result = Compactor {
            weak,
            database,
            guard,
            base,
            settings,
            key,
            info,
            binlog,
            compact: File::with_random(env.random),
            wrapper,
            written: HashSet::with_capacity(info.keys_count),
            list: Vec::new(),
        };
        result.start();
        result
    })
}
impl Compactor {
    fn binlog_path(&self) -> PathBuf {
        self.base.join("binlog")
    }
    fn compact_path(&self) -> PathBuf {
        self.base.join("binlog-temp")
    }
    fn start(&mut self) {
        let success = self.binlog.open(&self.binlog_path(), Mode::Read, &self.key)
            == FileResult::Success
            && self.binlog.size() >= self.info.till;
        let header = if success {
            BinlogWrapper::read_header(&mut self.binlog, &self.settings)
        } else {
            None
        };
        let success = header.is_some_and(|h| {
            self.compact
                .open(&self.compact_path(), Mode::Write, &self.key)
                == FileResult::Success
                && self.compact.write(&mut h.encode())
        });
        if !success {
            self.fail();
        }
        // Original start() continues after fail(); keep its queued failure
        // semantics. An unreadable source is handled by File::read's EOF path.
        self.list = Vec::with_capacity(self.settings.max_bundled_records);
        self.parse_chunk();
    }
    fn fail(&mut self) {
        self.compact.close();
        let _ = fs::remove_file(self.compact_path());
        self.database.with(|d| d.compactor_fail());
    }
    fn done(&mut self, till: i64) {
        let path = self.compact_path();
        let guard = std::mem::take(&mut self.guard);
        self.database.with(move |d| {
            if guard.alive() {
                d.compactor_done(path, till);
            }
        });
    }
    fn write_list(&mut self) -> bool {
        if self.list.is_empty() {
            return true;
        }
        let list = std::mem::take(&mut self.list);
        let mut bytes = Record::MultiStore(list).encode();
        let (header, parts) = bytes.split_at_mut(16);
        if !self.compact.write(header) || !self.compact.write(parts) {
            return false;
        }
        self.compact.flush();
        true
    }
    fn finish(&mut self) {
        if self.write_list() {
            self.finalize();
        } else {
            self.fail();
        }
    }
    fn finalize(&mut self) {
        self.binlog.close();
        self.compact.close();
        let mut last = 0i32;
        let mut from = self.info.till;
        loop {
            let till = catch_up(
                &self.compact_path(),
                &self.binlog_path(),
                &self.key,
                from,
                self.settings.read_block_size,
            );
            if till == 0 {
                self.fail();
                return;
            }
            if till == from || (last > 0 && till - from >= i64::from(last)) {
                self.done(till);
                return;
            }
            last = (till - from) as i32;
            from = till;
        }
    }
    fn read_chunk(&mut self) -> Vec<Key> {
        let mut keys = Vec::new();
        while keys.len() < self.settings.compact_chunk_size {
            let done = self.wrapper.read_till_end(&mut self.binlog, |record| {
                match record {
                    Record::Store(s) => keys.push(s.key),
                    Record::MultiStore(list) => keys.extend(list.into_iter().map(|s| s.key)),
                    _ => {}
                }
                true
            });
            if done {
                break;
            }
        }
        keys
    }
    fn parse_chunk(&mut self) {
        let keys = self.read_chunk();
        if self.wrapper.failed() {
            self.fail();
        } else if keys.is_empty() {
            self.finish();
        } else {
            let weak = self.weak.clone();
            self.database.with(move |d| {
                let values = d.get_many_raw(keys);
                weak.with(move |s| s.process_values(values));
            });
        }
    }
    fn process_values(&mut self, values: Vec<(Key, Entry)>) {
        let mut from = 0;
        while from < values.len() {
            while from < values.len() && self.list.len() < self.settings.max_bundled_records {
                let (k, e) = &values[from];
                from += 1;
                if !self.written.insert(*k) {
                    continue;
                }
                self.list.push(Store {
                    tag: e.tag,
                    size: e.size,
                    place: e.place,
                    checksum: e.checksum,
                    key: *k,
                    time: self
                        .settings
                        .track_estimated_time
                        .then_some(EstimatedTimePoint {
                            relative: e.use_time,
                            system: self.info.system_time,
                        }),
                });
            }
            if from < values.len() && !self.write_list() {
                self.fail();
                return;
            }
        }
        self.parse_chunk();
    }
}
pub(super) fn catch_up(
    compact: &Path,
    binlog: &Path,
    key: &EncryptionKey,
    from: i64,
    block_size: usize,
) -> i64 {
    let mut original = File::default();
    if original.open(binlog, Mode::Read, key) != FileResult::Success {
        return 0;
    }
    let till = original.size();
    if till == from {
        return till;
    }
    if till < from || !original.seek(from) {
        return 0;
    }
    let mut target = File::default();
    if target.open(compact, Mode::ReadAppend, key) != FileResult::Success
        || !target.seek(target.size())
    {
        return 0;
    }
    let mut left = till - from;
    while left > 0 {
        let size = (left as usize).min(block_size);
        let mut bytes = vec![0; size];
        let read = original.read(&mut bytes);
        if read <= 0 || !target.write(&mut bytes[..read as usize]) {
            return 0;
        }
        left -= read as i64;
    }
    till
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catch_up_copies_native_tail_and_preserves_early_return_branches() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/database/stores/0/binlog");
        let key = EncryptionKey::new((0..64).collect());
        let root = std::env::temp_dir().join(format!("cache-catch-up-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let target = root.join("compact");
        let mut f = File::default();
        assert_eq!(f.open(&source, Mode::Read, &key), FileResult::Success);
        let mut original = vec![0; f.size() as usize];
        assert_eq!(f.read(&mut original), original.len() as isize);
        f.close();
        assert_eq!(f.open(&target, Mode::Write, &key), FileResult::Success);
        assert!(f.write(&mut original[..16].to_vec()));
        f.close();
        assert_eq!(
            catch_up(&target, &source, &key, 16, 64),
            original.len() as i64
        );
        assert_eq!(f.open(&target, Mode::Read, &key), FileResult::Success);
        let mut bytes = vec![0; f.size() as usize];
        assert_eq!(f.read(&mut bytes), bytes.len() as isize);
        assert_eq!(bytes, original);
        f.close();
        let absent = root.join("absent");
        assert_eq!(
            catch_up(&absent, &source, &key, original.len() as i64, 64),
            original.len() as i64
        );
        assert!(!absent.exists());
        assert_eq!(
            catch_up(&target, &source, &key, original.len() as i64 + 16, 64),
            0
        );
        assert_eq!(catch_up(&target, &source, &key, -16, 64), 0);
        assert_eq!(catch_up(&target, &absent, &key, 16, 64), 0);
        assert_eq!(
            catch_up(&target, &source, &EncryptionKey::new(vec![99; 64]), 16, 64),
            0
        );
        assert_eq!(catch_up(&root, &source, &key, 16, 64), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
