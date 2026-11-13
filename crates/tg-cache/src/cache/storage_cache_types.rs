//! `storage_cache_types.{h,cpp}`. All disk integers are explicitly LE.
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub high: u64,
    pub low: u64,
}
impl Key {
    pub const fn new(high: u64, low: u64) -> Self {
        Self { high, low }
    }
    pub fn valid(self) -> bool {
        self.high != 0 || self.low != 0
    }
    pub fn encode(self) -> [u8; 16] {
        let mut b = [0; 16];
        b[..8].copy_from_slice(&self.high.to_le_bytes());
        b[8..].copy_from_slice(&self.low.to_le_bytes());
        b
    }
    pub fn decode(b: &[u8]) -> Self {
        Self::new(
            u64::from_le_bytes(b[..8].try_into().unwrap()),
            u64::from_le_bytes(b[8..16].try_into().unwrap()),
        )
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ErrorType {
    #[default]
    None,
    Io,
    WrongKey,
    LockFailed,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorType,
    pub path: PathBuf,
}
impl Error {
    pub fn no_error() -> Self {
        Self::default()
    }
    pub fn new(kind: ErrorType, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }
    pub fn is_ok(&self) -> bool {
        self.kind == ErrorType::None
    }
}
pub type PlaceId = [u8; 7];
pub const DATA_SIZE_LIMIT: usize = 1 << 24;
pub const BUNDLED_RECORDS_LIMIT: usize = 1 << 24;
#[derive(Clone, Debug)]
pub struct Settings {
    pub max_bundled_records: usize,
    pub read_block_size: usize,
    pub max_data_size: usize,
    pub write_bundle_delay: i64,
    pub stale_remove_chunk: usize,
    pub compact_after_excess: i64,
    pub compact_after_full_size: i64,
    pub compact_chunk_size: usize,
    pub track_estimated_time: bool,
    pub total_size_limit: i64,
    pub total_time_limit: u64,
    pub prune_timeout: i64,
    pub max_prune_check_timeout: i64,
    pub clear_on_wrong_key: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            max_bundled_records: 16384,
            read_block_size: 8 * 1024 * 1024,
            max_data_size: DATA_SIZE_LIMIT - 1,
            write_bundle_delay: 15 * 60 * 1000,
            stale_remove_chunk: 256,
            compact_after_excess: 8 * 1024 * 1024,
            compact_after_full_size: 0,
            compact_chunk_size: 16384,
            track_estimated_time: true,
            total_size_limit: 1024 * 1024 * 1024,
            total_time_limit: 31 * 24 * 60 * 60,
            prune_timeout: 5000,
            max_prune_check_timeout: 3600 * 1000,
            clear_on_wrong_key: false,
        }
    }
}
impl Settings {
    pub(crate) fn check(&self) {
        assert!(self.stale_remove_chunk > 0);
        assert!(self.max_data_size > 0 && self.max_data_size < DATA_SIZE_LIMIT);
        assert!(self.max_bundled_records > 0 && self.max_bundled_records < BUNDLED_RECORDS_LIMIT);
        assert!(self.total_size_limit == 0 || self.total_size_limit > self.max_data_size as i64);
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SettingsUpdate {
    pub total_size_limit: i64,
    pub total_time_limit: u64,
}
impl Default for SettingsUpdate {
    fn default() -> Self {
        let s = Settings::default();
        Self {
            total_size_limit: s.total_size_limit,
            total_time_limit: s.total_time_limit,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaggedValue {
    pub bytes: Vec<u8>,
    pub tag: u8,
}
impl TaggedValue {
    pub fn new(bytes: Vec<u8>, tag: u8) -> Self {
        Self { bytes, tag }
    }
}
impl From<Vec<u8>> for TaggedValue {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes, 0)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaggedSummary {
    pub count: usize,
    pub total_size: i64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub full: TaggedSummary,
    pub tagged: BTreeMap<u8, TaggedSummary>,
    pub clearing: bool,
}
pub fn compute_base_path(path: &Path) -> PathBuf {
    // QDir::absolutePath cleans '.', '..' without resolving symlinks.
    let absolute = std::path::absolute(path).expect("absolute cache path");
    let mut result = PathBuf::new();
    for part in absolute.components() {
        match part {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    result
}
pub fn version_file_path(base: &Path) -> PathBuf {
    base.join("version")
}
pub fn read_version_value(base: &Path) -> Option<i32> {
    let mut b = [0; 4];
    fs::File::open(version_file_path(base))
        .ok()?
        .read_exact(&mut b)
        .ok()?;
    Some(i32::from_le_bytes(b))
}
pub fn write_version_value(base: &Path, value: i32) -> bool {
    (|| -> std::io::Result<()> {
        fs::create_dir_all(base)?;
        let mut f = fs::File::create(version_file_path(base))?;
        f.write_all(&value.to_le_bytes())?;
        f.flush()
    })()
    .is_ok()
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BasicHeader {
    pub format: u8,
    pub flags: u32,
    pub system_time: u32,
    pub reserved1: u32,
    pub reserved2: u32,
}
impl BasicHeader {
    pub const TRACK_ESTIMATED_TIME: u32 = 1;
    pub fn encode(self) -> [u8; 16] {
        let mut b = [0; 16];
        b[..4].copy_from_slice(
            &(u32::from(self.format) | ((self.flags & 0xffffff) << 8)).to_le_bytes(),
        );
        b[4..8].copy_from_slice(&self.system_time.to_le_bytes());
        b[8..12].copy_from_slice(&self.reserved1.to_le_bytes());
        b[12..].copy_from_slice(&self.reserved2.to_le_bytes());
        b
    }
    pub fn decode(b: &[u8]) -> Self {
        let flags = u32::from_le_bytes(b[..4].try_into().unwrap());
        Self {
            format: flags as u8,
            flags: flags >> 8,
            system_time: u32_at(b, 4),
            reserved1: u32_at(b, 8),
            reserved2: u32_at(b, 12),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EstimatedTimePoint {
    pub relative: u64,
    pub system: u32,
}
impl EstimatedTimePoint {
    pub fn encode(self) -> [u8; 12] {
        let mut b = [0; 12];
        b[..8].copy_from_slice(&self.relative.to_le_bytes());
        b[8..].copy_from_slice(&self.system.to_le_bytes());
        b
    }
    pub fn decode(b: &[u8]) -> Self {
        Self {
            relative: u64::from_le_bytes(b[..8].try_into().unwrap()),
            system: u32_at(b, 8),
        }
    }
}
pub fn pack24(v: usize) -> [u8; 3] {
    assert!(v < DATA_SIZE_LIMIT);
    [v as u8, (v >> 8) as u8, (v >> 16) as u8]
}
pub fn unpack24(b: &[u8]) -> usize {
    usize::from(b[0]) | (usize::from(b[1]) << 8) | (usize::from(b[2]) << 16)
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    pub tag: u8,
    pub size: usize,
    pub place: PlaceId,
    pub checksum: u32,
    pub key: Key,
    pub time: Option<EstimatedTimePoint>,
}
impl Store {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![0; if self.time.is_some() { 48 } else { 32 }];
        b[0] = 1;
        b[1] = self.tag;
        b[2..5].copy_from_slice(&pack24(self.size));
        b[5..12].copy_from_slice(&self.place);
        b[12..16].copy_from_slice(&self.checksum.to_le_bytes());
        b[16..32].copy_from_slice(&self.key.encode());
        if let Some(t) = self.time {
            b[32..44].copy_from_slice(&t.encode());
        }
        b
    }
    pub fn decode(b: &[u8], timed: bool) -> Self {
        Self {
            tag: b[1],
            size: unpack24(&b[2..5]),
            place: b[5..12].try_into().unwrap(),
            checksum: u32_at(b, 12),
            key: Key::decode(&b[16..32]),
            time: timed.then(|| EstimatedTimePoint::decode(&b[32..44])),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    Store(Store),
    MultiStore(Vec<Store>),
    MultiRemove(Vec<Key>),
    MultiAccess(EstimatedTimePoint, Vec<Key>),
}
impl Record {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Store(s) => s.encode(),
            Self::MultiStore(list) => {
                let mut b = bundle_header(2, list.len());
                for s in list {
                    b.extend(s.encode());
                }
                b
            }
            Self::MultiRemove(keys) => {
                let mut b = bundle_header(3, keys.len());
                for k in keys {
                    b.extend(k.encode());
                }
                b
            }
            Self::MultiAccess(t, keys) => {
                let mut b = bundle_header(4, keys.len());
                b[4..].copy_from_slice(&t.encode());
                for k in keys {
                    b.extend(k.encode());
                }
                b
            }
        }
    }
    pub fn decode(b: &[u8], timed: bool) -> Self {
        let n = if b[0] == 1 { 0 } else { unpack24(&b[1..4]) };
        match b[0] {
            1 => Self::Store(Store::decode(b, timed)),
            2 => Self::MultiStore(
                (0..n)
                    .map(|i| {
                        let size = if timed { 48 } else { 32 };
                        Store::decode(&b[16 + i * size..], timed)
                    })
                    .collect(),
            ),
            3 => Self::MultiRemove((0..n).map(|i| Key::decode(&b[16 + i * 16..])).collect()),
            4 => Self::MultiAccess(
                EstimatedTimePoint::decode(&b[4..16]),
                (0..n).map(|i| Key::decode(&b[16 + i * 16..])).collect(),
            ),
            _ => unreachable!("validated record type"),
        }
    }
}
fn bundle_header(kind: u8, count: usize) -> Vec<u8> {
    let mut b = vec![0; 16];
    b[0] = kind;
    b[1..4].copy_from_slice(&pack24(count));
    b
}
pub fn place_from_id(place: PlaceId) -> String {
    let mut s = String::with_capacity(15);
    const HEX: &[u8] = b"0123456789ABCDEF";
    for (i, b) in place.into_iter().enumerate() {
        if i == 1 {
            s.push('/');
        }
        s.push(HEX[(b & 15) as usize] as char);
        s.push(HEX[(b >> 4) as usize] as char);
    }
    s
}

/// XXH32, seed=0, used by lib_storage, not a cryptographic authenticator.
pub fn count_checksum(data: &[u8]) -> u32 {
    const P1: u32 = 0x9E3779B1;
    const P2: u32 = 0x85EBCA77;
    const P3: u32 = 0xC2B2AE3D;
    const P4: u32 = 0x27D4EB2F;
    const P5: u32 = 0x165667B1;
    fn round(a: u32, b: u32) -> u32 {
        a.wrapping_add(b.wrapping_mul(P2))
            .rotate_left(13)
            .wrapping_mul(P1)
    }
    let mut at = 0;
    let mut hash = if data.len() >= 16 {
        let mut v = [P1.wrapping_add(P2), P2, 0, 0u32.wrapping_sub(P1)];
        while at + 16 <= data.len() {
            for (i, a) in v.iter_mut().enumerate() {
                *a = round(*a, u32_at(data, at + i * 4));
            }
            at += 16;
        }
        v[0].rotate_left(1)
            .wrapping_add(v[1].rotate_left(7))
            .wrapping_add(v[2].rotate_left(12))
            .wrapping_add(v[3].rotate_left(18))
    } else {
        P5
    };
    hash = hash.wrapping_add(data.len() as u32);
    while at + 4 <= data.len() {
        hash = hash
            .wrapping_add(u32_at(data, at).wrapping_mul(P3))
            .rotate_left(17)
            .wrapping_mul(P4);
        at += 4;
    }
    for b in &data[at..] {
        hash = hash
            .wrapping_add(u32::from(*b).wrapping_mul(P5))
            .rotate_left(11)
            .wrapping_mul(P1);
    }
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(P2);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(P3);
    hash ^ (hash >> 16)
}
