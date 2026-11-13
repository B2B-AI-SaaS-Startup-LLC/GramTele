//! `BinlogWrapper` and `BinlogReader`: retain incomplete tails for overwrite.
use super::storage_cache_types::*;
use crate::storage_encrypted_file::File;

#[derive(Debug, PartialEq, Eq)]
pub enum RecordSize {
    Unknown,
    Invalid,
    Known(usize),
}
pub fn record_size(bytes: &[u8], settings: &Settings) -> RecordSize {
    if bytes.is_empty() {
        return RecordSize::Unknown;
    }
    let store_size = if settings.track_estimated_time {
        48
    } else {
        32
    };
    if bytes[0] == 1 {
        return RecordSize::Known(store_size);
    }
    if !(2..=4).contains(&bytes[0]) || (bytes[0] == 4 && !settings.track_estimated_time) {
        return RecordSize::Invalid;
    }
    if bytes.len() < 16 {
        return RecordSize::Unknown;
    }
    let n = unpack24(&bytes[1..4]);
    if n > settings.max_bundled_records || (n == 0 && bytes[0] != 4) {
        return RecordSize::Invalid;
    }
    RecordSize::Known(16 + n * if bytes[0] == 2 { store_size } else { 16 })
}
pub struct BinlogWrapper {
    settings: Settings,
    till: i64,
    part: Vec<u8>,
    finished: bool,
    failed: bool,
}
impl BinlogWrapper {
    pub fn new(file: &File, settings: &Settings, till: i64) -> Self {
        assert!(
            settings.read_block_size
                >= if settings.track_estimated_time {
                    48
                } else {
                    32
                }
        );
        Self {
            settings: settings.clone(),
            till: if till == 0 { file.size() } else { till },
            part: Vec::new(),
            finished: false,
            failed: false,
        }
    }
    pub fn read_header(file: &mut File, settings: &Settings) -> Option<BasicHeader> {
        if file.offset() != 0 {
            return None;
        }
        let mut b = [0; 16];
        if file.read(&mut b) != 16 {
            return None;
        }
        let h = BasicHeader::decode(&b);
        (h.format == 0
            && (h.flags & BasicHeader::TRACK_ESTIMATED_TIME != 0) == settings.track_estimated_time)
            .then_some(h)
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    pub fn finished(&self) -> bool {
        self.finished
    }
    pub fn read_part(&mut self, file: &mut File) -> bool {
        if self.finished {
            return false;
        }
        let left = self.till - file.offset();
        if left == 0 {
            self.finish(file, 0);
            return false;
        }
        let amount = (left as usize).min(self.settings.read_block_size - self.part.len());
        assert!(amount > 0);
        let old = self.part.len();
        self.part.resize(old + amount, 0);
        let count = file.read(&mut self.part[old..]);
        if count <= 0 {
            self.part.truncate(old);
            self.finish(file, 0);
            return false;
        }
        self.part.truncate(old + count as usize);
        true
    }
    pub fn read_record(&mut self, file: &mut File) -> Option<Vec<u8>> {
        if self.finished {
            return None;
        }
        match record_size(&self.part, &self.settings) {
            RecordSize::Invalid => {
                self.finish(file, 0);
                self.finished = true;
                self.failed = true;
                None
            }
            RecordSize::Known(size) if size <= self.part.len() => {
                Some(self.part.drain(..size).collect())
            }
            _ => None,
        }
    }
    pub fn finish(&mut self, file: &mut File, rollback: usize) {
        if rollback > 0 {
            self.failed = true;
        }
        file.seek(file.offset() - (rollback + self.part.len()) as i64);
        // Original finish neither clears part nor sets finished.
    }
    /// A block per call. `true` means done, including an invalid handler.
    pub fn read_till_end(
        &mut self,
        file: &mut File,
        mut handler: impl FnMut(Record) -> bool,
    ) -> bool {
        if !self.read_part(file) {
            return true;
        }
        while let Some(bytes) = self.read_record(file) {
            if !handler(Record::decode(&bytes, self.settings.track_estimated_time)) {
                self.finish(file, bytes.len());
                return true;
            }
        }
        false
    }
}
