//! File-location/cache metadata. Actual loaders and pixels are not model data.
use super::data_image_location::*;
pub use tg_cache::cache::storage_cache_types::Key as CacheKey;
pub fn document_cache_key(dc: i32, id: u64) -> CacheKey {
    CacheKey::new(0x100 | (dc as u64 & 255), id)
}
pub fn document_thumb_cache_key(dc: i32, id: u64) -> CacheKey {
    CacheKey::new(0x200 | (dc as u64 & 255), id)
}
pub fn web_document_cache_key(url: &[u8]) -> CacheKey {
    let hash = tg_base::openssl_help::sha256(url);
    CacheKey::new(
        0x0000_0200_0000_0000
            | (4 << 32)
            | u64::from(u32::from_le_bytes(hash[..4].try_into().unwrap())),
        u64::from_le_bytes(hash[4..12].try_into().unwrap()),
    )
}
pub fn url_cache_key(url: &str) -> CacheKey {
    let hash = tg_base::openssl_help::sha256(url.as_bytes());
    CacheKey::new(
        0x0000_0300_0000_0000
            | (u64::from(u16::from_le_bytes(hash[12..14].try_into().unwrap())) << 32)
            | u64::from(u32::from_le_bytes(hash[..4].try_into().unwrap())),
        u64::from_le_bytes(hash[4..12].try_into().unwrap()),
    )
}
impl StorageFileLocation {
    pub fn big_file_base_cache_key(&self) -> CacheKey {
        match self.kind {
            StorageFileType::Document | StorageFileType::Photo => {
                let tag = if self.kind == StorageFileType::Document {
                    0x10000
                } else {
                    0x20000
                };
                // Preserve image_location.cpp's actual <<16 followed by FF00
                // mask (the dc contribution is zero); do not silently fix it.
                CacheKey::new(
                    tag | ((u64::from(self.dc_id) << 16) & 0xff00) | (self.id >> 48),
                    self.id << 16,
                )
            }
            StorageFileType::StickerSetThumb => CacheKey::new(
                ((self.local_id as u32 as u64) << 24)
                    | ((self.kind as u64 + 1) << 16)
                    | ((u64::from(self.dc_id) & 255) << 8)
                    | (self.volume_id >> 56),
                self.volume_id << 8,
            ),
            _ => panic!("Not implemented file location type, as in image_location.cpp"),
        }
    }
    pub fn cache_key(&self) -> CacheKey {
        use StorageFileType::*;
        let shifted = (self.kind as u64 + 3) << 8;
        let dc = u64::from(self.dc_id) & 255;
        match self.kind {
            Legacy | PeerPhoto | StickerSetThumb => CacheKey::new(
                shifted | dc | ((self.local_id as u32 as u64) << 16),
                self.volume_id,
            ),
            Encrypted | Secure => CacheKey::new(shifted | dc, self.id),
            Document if self.size_letter == 0 => document_cache_key(i32::from(self.dc_id), self.id),
            Document | Photo => {
                CacheKey::new(shifted | dc | (u64::from(self.size_letter) << 16), self.id)
            }
            Takeout => CacheKey::new(shifted, 0),
            GroupCallStream => CacheKey::new(
                shifted
                    | dc
                    | u64::from((self.local_id as u32) << 16)
                    | (self.volume_id << 20)
                    | (u64::from(self.size_letter) << 56),
                self.id,
            ),
        }
    }
}
impl DownloadLocation {
    pub fn cache_key(&self) -> CacheKey {
        match self {
            Self::Storage(s) if s.valid() => s.cache_key(),
            Self::Web { url, .. } if !url.is_empty() => web_document_cache_key(url),
            Self::Url(s) if !s.is_empty() => url_cache_key(s),
            Self::AudioAlbumThumb { document_id } => CacheKey::new(0x300, *document_id),
            Self::Geo(g) => CacheKey::new(
                0x0000_0400_0000_0000
                    | (u64::from(((g.zoom as u32 & 15) << 8) | (g.scale as u32 & 15)) << 32)
                    | u64::from(((g.width as u32 & 65535) << 16) | (g.height as u32 & 65535)),
                ((((g.lat + 360.).abs() * 1_000_000.).round() as u64) << 32)
                    | (((g.lon + 360.).abs() * 1_000_000.).round() as u64),
            ),
            _ => CacheKey::default(),
        }
    }
    pub fn refresh_file_reference(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() {
            if let Self::Storage(s) = self {
                s.file_reference = bytes.to_vec();
            }
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct ImageWithLocation {
    pub location: ImageLocation,
    pub bytes: Vec<u8>,
    pub bytes_count: i32,
    pub progressive_part_size: i32,
}
#[derive(Clone, Debug, Default)]
pub struct CloudFileMetadata {
    pub location: ImageLocation,
    pub byte_size: i32,
    pub progressive_part_size: i32,
    pub cancelled: bool,
    pub failed: bool,
    pub loaded: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CloudMetadataUpdate {
    pub changed: bool,
    pub cache_put_if_empty: Option<(CacheKey, Vec<u8>)>,
}
impl CloudFileMetadata {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    /// Metadata branch of UpdateCloudFile. Loader restart and preloaded pixels
    /// must be handled by the storage/file_download adapter (map marks partial).
    pub fn update(&mut self, data: &ImageWithLocation) -> CloudMetadataUpdate {
        if !data.location.valid() {
            if data.progressive_part_size != 0 && !self.location.valid() {
                self.progressive_part_size = data.progressive_part_size;
            }
            if data.location.width != 0
                && data.location.height != 0
                && !self.location.valid()
                && self.location.width == 0
            {
                self.location = data.location.clone();
            }
            return CloudMetadataUpdate::default();
        }
        let cache_key = data.location.file.cache_key();
        let was_key = self.location.file.cache_key();
        let sticker_changed = matches!(&self.location.file, DownloadLocation::Storage(s) if s.kind == StorageFileType::StickerSetThumb)
            && matches!(&data.location.file, DownloadLocation::Storage(s) if s.valid() && (s.kind != StorageFileType::StickerSetThumb || s.cache_key() != was_key));
        let update = !self.location.valid()
            || (cache_key.valid()
                && (!was_key.valid()
                    || self.location.width < data.location.width
                    || self.location.height < data.location.height
                    || sticker_changed));
        if !update {
            return CloudMetadataUpdate::default();
        }
        let bytes = if !data.bytes.is_empty() {
            data.bytes.clone()
        } else if let DownloadLocation::InMemory(b) = &self.location.file {
            b.clone()
        } else {
            Vec::new()
        };
        self.location = data.location.clone();
        self.byte_size = data.bytes_count;
        self.failed = false;
        CloudMetadataUpdate {
            changed: true,
            cache_put_if_empty: (cache_key.valid() && !bytes.is_empty())
                .then_some((cache_key, bytes)),
        }
    }
}
