//! Value/model slice of core/file_location.{h,cpp}. Inspection and bookmark
//! access are supplied by the real platform/storage service (package 33).
//! This preserves the fields without constructing a fake QFileInfo/Session.
pub const MAX_FILE_SIZE: i64 = 4000 * 1024 * 1024;
const MEDIA_CACHE: &str = "*media_cache*";
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileLocation {
    pub fname: String,
    pub modified_ms: Option<i64>,
    pub size: i64,
}
/// Scalar projection of a real QFileInfo/filesystem result. The platform
/// supplies readability and resolved bookmark name; no default provider exists.
#[derive(Clone, Debug)]
pub struct FileInfo {
    pub path: String,
    pub exists: bool,
    pub readable: bool,
    pub modified_ms: Option<i64>,
    pub size: i64,
}
impl FileLocation {
    pub fn from_info(info: &FileInfo) -> Self {
        if info.path.is_empty() {
            return Self::default();
        }
        if !info.exists || info.size > MAX_FILE_SIZE {
            return Self::default();
        }
        Self {
            fname: info.path.clone(),
            modified_ms: info.modified_ms,
            size: info.size,
        }
    }
    pub fn in_media_cache_location() -> Self {
        Self {
            fname: MEDIA_CACHE.into(),
            ..Default::default()
        }
    }
    pub fn name(&self) -> &str {
        &self.fname
    }
    pub fn is_empty(&self) -> bool {
        self.fname.is_empty()
    }
    pub fn in_media_cache(&self) -> bool {
        self.fname == MEDIA_CACHE
    }
    /// FileLocation::check after actual bookmark access and QFileInfo lookup.
    pub fn check(&self, info: &FileInfo) -> bool {
        !self.is_empty()
            && !self.in_media_cache()
            && info.readable
            && info.size >= 0
            && info.size <= MAX_FILE_SIZE
            && info.size == self.size
            && info.modified_ms == self.modified_ms
    }
    pub fn verified_by(&self, files: &(impl FileLocations + ?Sized)) -> bool {
        !self.is_empty() && !self.in_media_cache() && self.check(&files.inspect(self))
    }
}
/// Storage-side service boundary used by DocumentData/PhotoData location paths.
/// Implementations must apply the actual platform bookmark/access rules.
pub trait FileLocations {
    fn inspect(&self, location: &FileLocation) -> FileInfo;
    fn read(&self, key: (u64, u64)) -> FileLocation;
}
