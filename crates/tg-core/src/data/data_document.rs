//! Headless DocumentData. File/cache/network and pixel effects are separate
//! packages; there is no artificial Main::Session or image object here.
pub use super::data_media_types::{DocumentType, FileStatus};
use super::{data_image_location::ImageLocation, data_media_location::*};
use super::{data_media_types::*, data_store_keys::*, data_types::StickerSetIdentifier};
use crate::core::file_location::{FileLocation, FileLocations};
use crate::core::mime_type::*;
use std::rc::{Rc, Weak};
pub const MAX_FILE_IN_MEMORY: i64 = 10 * 1024 * 1024;
pub const MAX_STICKER_BYTES: i64 = 2 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StickerType {
    #[default]
    Webp,
    Tgs,
    Webm,
}
impl StickerType {
    pub fn is_static(self) -> bool {
        self == Self::Webp
    }
    pub fn is_lottie(self) -> bool {
        self == Self::Tgs
    }
    pub fn is_webm(self) -> bool {
        self == Self::Webm
    }
    pub fn is_animated(self) -> bool {
        !self.is_static()
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StickersType {
    #[default]
    Stickers,
    Masks,
    Emoji,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StickerData {
    pub alt: String,
    pub set: StickerSetIdentifier,
    pub kind: StickerType,
    pub set_type: StickersType,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SongData {
    pub title: String,
    pub performer: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoiceData {
    pub waveform: Vec<i8>,
    pub wavemax: i8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoData {
    pub codec: String,
    pub qualities: Vec<DocumentRecordKey>,
    pub real_video_size: MediaSize,
    pub start_ts: i64,
}
/// Immutable projection of an actual document for cross-record video choices.
/// The owning Documents store supplies these after looking up scoped keys.
#[derive(Clone, Copy, Debug)]
pub struct VideoVariant {
    pub key: DocumentRecordKey,
    pub quality: i32,
    pub size: i64,
    pub good: bool,
}
impl Default for VideoData {
    fn default() -> Self {
        Self {
            codec: String::new(),
            qualities: Vec::new(),
            real_video_size: MediaSize::INVALID,
            start_ts: 0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentAdditional {
    Sticker(StickerData),
    Song(SongData),
    Voice(VoiceData),
    Round(VoiceData),
    Video(VideoData),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentEffect {
    WriteFileLocation {
        key: (u64, u64),
        location: FileLocation,
    },
    RemoveFileLocation {
        key: (u64, u64),
    },
    RequestViewRepaint(DocumentRecordKey),
    CachePut {
        key: CacheKey,
        bytes: Vec<u8>,
        tag: u8,
    },
    CheckGoodThumbnail,
    LoadThumbnail,
    ClearThumbnails,
    CachePutIfEmpty {
        key: CacheKey,
        bytes: Vec<u8>,
        animation: bool,
    },
}
pub struct DocumentData {
    key: DocumentRecordKey,
    pub(crate) id: u64,
    pub size: i64,
    pub dimensions: MediaSize,
    pub date: i32,
    pub kind: DocumentType,
    pub status: FileStatus,
    pub uploading_data: Option<UploadState>,
    pub(crate) dc_id: i32,
    pub(crate) access_hash: u64,
    pub(crate) file_reference: Vec<u8>,
    pub(crate) content_url: String,
    pub(crate) web_location: Option<(Vec<u8>, u64)>,
    filename: String,
    mime: String,
    name_type: NameType,
    flags: u16,
    good_thumbnail_flags: u8,
    duration: i64,
    pub thumbnail: CloudFileMetadata,
    pub video_thumbnail: CloudFileMetadata,
    inline_thumbnail_bytes: Vec<u8>,
    good_thumbnail_photo: Option<PhotoRecordKey>,
    location: FileLocation,
    pub(crate) video_preload_prefix: i32,
    pub(crate) additional: Option<DocumentAdditional>,
    pub(crate) media: Weak<std::cell::RefCell<super::data_document_media::DocumentMedia>>,
}
const STREAM_MASK: u16 = 3;
const PLAYBACK_FAILED: u16 = 4;
const IMAGE_TYPE: u16 = 8;
const CANCELLED: u16 = 0x10;
const IN_CACHE: u16 = 0x20;
const ATTACHED_STICKERS: u16 = 0x40;
const INLINE_PATH: u16 = 0x80;
const FORCE_CACHE: u16 = 0x100;
const PREMIUM: u16 = 0x200;
const POSSIBLE_COVER: u16 = 0x400;
const TEXT_COLOR: u16 = 0x800;
const STORY: u16 = 0x1000;
const SILENT: u16 = 0x2000;
const FORBID_SAVE: u16 = 0x4000;
impl DocumentData {
    pub fn new(scope: &SessionScope, id: u64) -> Self {
        Self::with_key(scope.new_document_record_key(), id)
    }
    pub(crate) fn with_key(key: DocumentRecordKey, id: u64) -> Self {
        Self {
            key,
            id,
            size: 0,
            dimensions: MediaSize::INVALID,
            date: 0,
            kind: DocumentType::File,
            status: FileStatus::Ready,
            uploading_data: None,
            dc_id: 0,
            access_hash: 0,
            file_reference: Vec::new(),
            content_url: String::new(),
            web_location: None,
            filename: String::new(),
            mime: String::new(),
            name_type: NameType::Unknown,
            flags: STREAM_MASK,
            good_thumbnail_flags: 0,
            duration: -1,
            video_preload_prefix: 0,
            additional: None,
            media: Weak::new(),
            thumbnail: CloudFileMetadata::default(),
            video_thumbnail: CloudFileMetadata::default(),
            inline_thumbnail_bytes: Vec::new(),
            good_thumbnail_photo: None,
            location: FileLocation::default(),
        }
    }
    pub fn wire_key(&self) -> DocumentKey {
        DocumentKey::from_scoped_id(self.key.scope(), self.id)
    }
    pub fn key(&self) -> DocumentRecordKey {
        self.key
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn dc_id(&self) -> i32 {
        self.dc_id
    }
    pub fn filename(&self) -> &str {
        &self.filename
    }
    pub fn name_type(&self) -> NameType {
        self.name_type
    }
    pub fn mime_string(&self) -> &str {
        &self.mime
    }
    pub fn has_mime_type(&self, mime: &str) -> bool {
        self.mime == mime
    }
    pub fn set_mime_string(&mut self, mime: &str) {
        self.mime = mime.to_lowercase();
    }
    pub fn set_file_name(&mut self, name: &str) {
        self.filename = sanitize_remote_filename(name);
        self.name_type = detect_name_type(&self.filename);
    }
    pub fn enforce_name_type(&mut self, wanted: NameType, database: &impl MimeDatabase) -> bool {
        if self.name_type == wanted {
            return true;
        }
        let base = if self.filename.is_empty() {
            "file"
        } else {
            &self.filename
        };
        for glob in mime_type_for_name(&self.mime, database).glob_patterns {
            let candidate = format!("{base}{}", glob.replace('*', ""));
            if detect_name_type(&candidate) == wanted {
                self.filename = candidate;
                self.name_type = wanted;
                return true;
            }
        }
        false
    }
    pub fn sticker(&self) -> Option<&StickerData> {
        if self.kind != DocumentType::Sticker {
            return None;
        }
        if let Some(DocumentAdditional::Sticker(d)) = &self.additional {
            Some(d)
        } else {
            None
        }
    }
    pub fn song(&self) -> Option<&SongData> {
        if !self.is_song() {
            return None;
        }
        if let Some(DocumentAdditional::Song(d)) = &self.additional {
            Some(d)
        } else {
            None
        }
    }
    pub fn voice(&self) -> Option<&VoiceData> {
        if !self.is_voice_message() {
            return None;
        }
        if let Some(DocumentAdditional::Voice(d)) = &self.additional {
            Some(d)
        } else {
            None
        }
    }
    pub fn round(&self) -> Option<&VoiceData> {
        if !self.is_video_message() {
            return None;
        }
        if let Some(DocumentAdditional::Round(d)) = &self.additional {
            Some(d)
        } else {
            None
        }
    }
    pub fn video(&self) -> Option<&VideoData> {
        if !self.is_video_file() {
            return None;
        }
        if let Some(DocumentAdditional::Video(d)) = &self.additional {
            Some(d)
        } else {
            None
        }
    }
    pub fn video_mut(&mut self) -> Option<&mut VideoData> {
        if !self.is_video_file() {
            return None;
        }
        if let Some(DocumentAdditional::Video(d)) = &mut self.additional {
            Some(d)
        } else {
            None
        }
    }
    pub fn has_remote_location(&self) -> bool {
        self.dc_id != 0 && self.access_hash != 0
    }
    pub fn has_web_location(&self) -> bool {
        self.web_location
            .as_ref()
            .is_some_and(|(url, _)| !url.is_empty())
    }
    pub fn is_null(&self) -> bool {
        !self.has_remote_location()
            && !self.has_web_location()
            && self.content_url.is_empty()
            && !self.uploading()
            && self.location.is_empty()
    }
    pub fn video_preload_location(&self) -> super::data_image_location::StorageFileLocation {
        if self.has_remote_location() {
            super::data_image_location::StorageFileLocation {
                dc_id: self.dc_id as u16,
                kind: super::data_image_location::StorageFileType::Document,
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference.clone(),
                ..Default::default()
            }
        } else {
            super::data_image_location::StorageFileLocation::default()
        }
    }
    pub fn set_content_url(&mut self, url: String) {
        self.content_url = url;
    }
    pub fn set_web_location(&mut self, url: Vec<u8>, access_hash: u64) {
        self.web_location = Some((url, access_hash));
    }
    pub fn mtp_input(&self) -> tg_scheme::InputDocument {
        if self.access_hash != 0 {
            tg_scheme::mtp_input_document(self.id, self.access_hash, self.file_reference.clone())
        } else {
            tg_scheme::mtp_input_document_empty()
        }
    }
    pub fn duration(&self) -> i64 {
        self.duration.max(0)
    }
    pub fn has_duration(&self) -> bool {
        self.duration >= 0
    }
    pub fn is_voice_message(&self) -> bool {
        self.kind == DocumentType::Voice
    }
    pub fn is_video_message(&self) -> bool {
        self.kind == DocumentType::RoundVideo
    }
    pub fn is_video_file(&self) -> bool {
        self.kind == DocumentType::Video
    }
    pub fn is_song(&self) -> bool {
        self.kind == DocumentType::Song
    }
    pub fn is_shared_media_music(&self) -> bool {
        self.is_song()
    }
    pub fn is_music_for_profile(&self) -> bool {
        self.is_song()
    }
    pub fn is_wallpaper(&self) -> bool {
        self.kind == DocumentType::Wallpaper
    }
    pub fn is_animation(&self) -> bool {
        self.kind == DocumentType::Animated
            || self.is_video_message()
            || ((self.filename.is_empty()
                || matches!(self.name_type, NameType::Image | NameType::Video))
                && self.has_mime_type("image/gif")
                && !self.inapp_playback_failed())
    }
    pub fn is_gifv(&self) -> bool {
        self.kind == DocumentType::Animated && self.has_mime_type("video/mp4")
    }
    pub fn is_theme(&self) -> bool {
        qt_ascii_ci_suffix(&self.filename, ".tdesktop-theme")
            || qt_ascii_ci_suffix(&self.filename, ".tdesktop-palette")
            || (self.has_mime_type("application/x-tgtheme-tdesktop")
                && (self.filename.is_empty()
                    || !self.filename.contains('.')
                    || self.name_type == NameType::ThemeFile))
    }
    pub fn is_audio_file(&self) -> bool {
        if self.is_voice_message() || self.is_video_file() {
            return false;
        }
        if self.is_song() {
            return true;
        }
        if !self.mime.starts_with("audio/") {
            return qt_ascii_ci_suffix(&self.filename, ".opus");
        }
        if !self.filename.is_empty() && !matches!(self.name_type, NameType::Audio | NameType::Video)
        {
            return false;
        }
        matches!(&self.mime[6..], "x-wav" | "wav" | "mp4")
    }
    pub fn is_svg_image(&self) -> bool {
        self.has_mime_type("image/svg+xml") || qt_ascii_ci_suffix(&self.filename, ".svg")
    }
    pub fn is_pattern_wallpaper_png(&self) -> bool {
        self.is_wallpaper() && self.has_mime_type("image/png")
    }
    pub fn is_pattern_wallpaper_svg(&self) -> bool {
        self.is_wallpaper() && self.has_mime_type("application/x-tgwallpattern")
    }
    pub fn is_premium_sticker(&self) -> bool {
        self.flag(PREMIUM)
            && self
                .sticker()
                .is_some_and(|s| s.set_type == StickersType::Stickers)
    }
    pub fn is_premium_emoji(&self) -> bool {
        self.flag(PREMIUM)
            && self
                .sticker()
                .is_some_and(|s| s.set_type == StickersType::Emoji)
    }
    pub fn uses_text_color(&self) -> bool {
        self.flag(TEXT_COLOR)
    }
    pub fn override_emoji_uses_text_color(&mut self, value: bool) {
        self.set_flag(TEXT_COLOR, value);
    }
    pub fn has_thumbnail(&self) -> bool {
        self.thumbnail.location.valid() && !self.thumbnail.failed && !self.flag(POSSIBLE_COVER)
    }
    pub fn has_video_thumbnail(&self) -> bool {
        self.video_thumbnail.location.valid()
    }
    pub fn is_song_with_cover(&self) -> bool {
        self.is_song() && self.has_thumbnail()
    }
    pub fn inline_thumbnail_bytes(&self) -> &[u8] {
        &self.inline_thumbnail_bytes
    }
    pub fn inline_thumbnail_is_path(&self) -> bool {
        self.flag(INLINE_PATH)
    }
    pub fn clear_inline_thumbnail_bytes(&mut self) {
        self.inline_thumbnail_bytes.clear();
    }
    pub fn is_pattern_wallpaper(&self) -> bool {
        self.is_wallpaper() && (self.is_pattern_wallpaper_png() || self.is_pattern_wallpaper_svg())
    }
    pub fn good_thumbnail_photo(&self) -> Option<PhotoRecordKey> {
        self.good_thumbnail_photo
    }
    pub fn set_good_thumbnail_photo(&mut self, photo: PhotoRecordKey) {
        assert_eq!(self.key.scope(), photo.scope());
        self.good_thumbnail_photo = Some(photo);
    }
    pub fn thumbnail_location(&self) -> &ImageLocation {
        &self.thumbnail.location
    }
    pub fn thumbnail_failed(&self) -> bool {
        self.thumbnail.failed
    }
    pub fn thumbnail_byte_size(&self) -> i32 {
        self.thumbnail.byte_size
    }
    pub fn video_thumbnail_location(&self) -> &ImageLocation {
        &self.video_thumbnail.location
    }
    pub fn video_thumbnail_failed(&self) -> bool {
        self.video_thumbnail.failed
    }
    pub fn video_thumbnail_byte_size(&self) -> i32 {
        self.video_thumbnail.byte_size
    }
    pub fn refresh_file_reference(&mut self, value: &[u8]) {
        self.file_reference = value.to_vec();
        self.thumbnail.location.file.refresh_file_reference(value);
        self.video_thumbnail
            .location
            .file
            .refresh_file_reference(value);
    }
    pub fn file_reference(&self) -> &[u8] {
        &self.file_reference
    }
    pub fn cache_key(&self) -> CacheKey {
        if let Some((url, _)) = &self.web_location {
            if !url.is_empty() {
                return web_document_cache_key(url);
            }
        }
        if self.access_hash == 0 && !self.content_url.is_empty() {
            url_cache_key(&self.content_url)
        } else {
            document_cache_key(self.dc_id, self.id)
        }
    }
    pub fn good_thumbnail_cache_key(&self) -> CacheKey {
        document_thumb_cache_key(self.dc_id, self.id)
    }
    pub fn location(&mut self, check: bool, files: &impl FileLocations) -> &FileLocation {
        if check && !self.location.verified_by(files) {
            let location = files.read(self.media_key());
            if location.in_media_cache() {
                self.set_loaded_in_media_cache_location();
            } else {
                self.location = location;
            }
        }
        &self.location
    }
    pub fn filepath(&mut self, check: bool, files: &impl FileLocations) -> String {
        if check && self.location.name().is_empty() {
            return String::new();
        }
        self.location(check, files).name().to_owned()
    }
    pub fn set_location(&mut self, location: FileLocation, files: &impl FileLocations) {
        if location.in_media_cache() {
            self.set_loaded_in_media_cache_location();
        } else if location.verified_by(files) {
            self.location = location;
        }
    }
    pub fn set_loaded_in_media_cache_location(&mut self) {
        self.location = FileLocation::default();
        self.flags |= IN_CACHE;
    }
    pub fn set_loaded_in_media_cache(&mut self, loaded: bool) -> Vec<DocumentEffect> {
        if self.loaded_in_media_cache() == loaded {
            return Vec::new();
        }
        self.set_flag(IN_CACHE, loaded);
        let mut effects = Vec::new();
        if self.location.is_empty() {
            effects.push(if loaded {
                DocumentEffect::WriteFileLocation {
                    key: self.media_key(),
                    location: FileLocation::in_media_cache_location(),
                }
            } else {
                DocumentEffect::RemoveFileLocation {
                    key: self.media_key(),
                }
            });
            effects.push(DocumentEffect::RequestViewRepaint(self.key));
        }
        effects
    }
    pub fn set_remote_location(
        &mut self,
        dc: i32,
        access: u64,
        reference: &[u8],
        files: &(impl FileLocations + ?Sized),
    ) -> Vec<DocumentEffect> {
        self.file_reference = reference.to_vec();
        let mut effects = Vec::new();
        if self.dc_id != dc || self.access_hash != access {
            self.dc_id = dc;
            self.access_hash = access;
            if !self.is_null() {
                if self.location.verified_by(files) {
                    effects.push(DocumentEffect::WriteFileLocation {
                        key: self.media_key(),
                        location: self.location.clone(),
                    });
                } else {
                    self.location = files.read(self.media_key());
                    if self.location.in_media_cache() {
                        self.set_loaded_in_media_cache_location();
                    } else if self.location.is_empty() && self.loaded_in_media_cache() {
                        effects.push(DocumentEffect::WriteFileLocation {
                            key: self.media_key(),
                            location: FileLocation::in_media_cache_location(),
                        });
                    }
                }
            }
        }
        effects
    }
    pub fn big_file_base_cache_key(&self) -> CacheKey {
        if !self.has_remote_location() {
            return CacheKey::default();
        }
        super::data_image_location::StorageFileLocation {
            kind: super::data_image_location::StorageFileType::Document,
            dc_id: self.dc_id as u16,
            id: self.id,
            ..Default::default()
        }
        .big_file_base_cache_key()
    }
    pub fn video_preload_prefix(&self) -> i32 {
        self.video_preload_prefix
    }
    fn attribute_video_quality(&self) -> i32 {
        if !self.is_video_file() || self.dimensions.is_empty() {
            0
        } else {
            self.dimensions.width.min(self.dimensions.height)
        }
    }
    pub fn resolve_original_video_quality(&self) -> i32 {
        if let Some(v) = self.video().filter(|v| !v.real_video_size.is_empty()) {
            v.real_video_size.width.min(v.real_video_size.height)
        } else {
            self.attribute_video_quality()
        }
    }
    pub fn resolve_video_quality(&self, mut quality: impl FnMut(DocumentRecordKey) -> i32) -> i32 {
        if let Some(v) = self.video() {
            if !v.real_video_size.is_empty() {
                return v.real_video_size.width.min(v.real_video_size.height);
            }
            let max = v
                .qualities
                .iter()
                .filter(|&&k| k != self.key)
                .map(|&k| quality(k))
                .max()
                .unwrap_or_default()
                .max(0);
            if max > 0 {
                return resolve_attribute_vs_transcode_quality(self.attribute_video_quality(), max);
            }
        }
        self.attribute_video_quality()
    }
    pub fn initial_playback_video_quality(
        &self,
        request: VideoQuality,
        filepath: &str,
    ) -> VideoQuality {
        if self.is_video_file() && !filepath.is_empty() {
            VideoQuality::new(
                true,
                self.resolve_original_video_quality().max(0) as u32,
                true,
            )
        } else {
            request
        }
    }
    pub fn video_variant(&self, quality: i32) -> VideoVariant {
        VideoVariant {
            key: self.key,
            quality,
            size: self.size,
            good: self.is_video_file()
                && !self.dimensions.is_empty()
                && !self.inapp_playback_failed()
                && self.use_streaming_loader(),
        }
    }
    pub fn set_video_qualities(&mut self, mut variants: Vec<VideoVariant>) {
        if self.video().is_none() || variants.is_empty() {
            return;
        }
        assert!(variants.iter().all(|v| v.key.scope() == self.key.scope()));
        variants.sort_unstable_by_key(|v| std::cmp::Reverse(v.quality));
        let mut i = 0;
        while i + 1 < variants.len() {
            let a = variants[i];
            let b = variants[i + 1];
            if !a.good || !b.good || a.quality == b.quality {
                variants.remove(i + usize::from(a.good && !(b.good && a.size > b.size)));
            } else {
                i += 1;
            }
        }
        if variants.last().is_some_and(|v| v.quality == 0) {
            variants.pop();
        }
        let max = variants
            .iter()
            .map(|v| v.quality)
            .max()
            .unwrap_or_default()
            .max(0);
        let insert_self = !variants.is_empty()
            && resolve_attribute_vs_transcode_quality(self.attribute_video_quality(), max) != 0;
        let mut keys: Vec<_> = variants.iter().map(|v| v.key).collect();
        if insert_self {
            keys.insert(0, self.key);
        }
        self.video_mut().unwrap().qualities = keys;
    }
    /// History/Media list eligibility is checked by the actual history owner
    /// (package 16/18), before calling this pure closest-variant selection.
    pub fn choose_quality(
        &self,
        list: &[VideoVariant],
        request: VideoQuality,
    ) -> DocumentRecordKey {
        if list.is_empty() || request.height == 0 || request.original {
            return self.key;
        }
        let mut closest: Option<VideoVariant> = None;
        let mut distance = 0i64;
        for &v in list {
            assert_eq!(v.key.scope(), self.key.scope());
            let d = (i64::from(request.height) - i64::from(v.quality)).abs();
            if closest.is_none_or(|c| {
                d < distance
                    || (d == distance
                        && (v.size < c.size || (c.key == self.key && v.key != self.key)))
            }) {
                closest = Some(v);
                distance = d;
            }
        }
        closest.map_or(self.key, |v| v.key)
    }
    pub fn required_send_right(&self) -> super::data_chat_participant_status::ChatRestriction {
        use super::data_chat_participant_status::ChatRestriction as R;
        if self.is_video_file() {
            R::SendVideos
        } else if self.is_song() {
            R::SendMusic
        } else if self.is_voice_message() {
            R::SendVoiceMessages
        } else if self.is_video_message() {
            R::SendVideoMessages
        } else if self.sticker().is_some() {
            R::SendStickers
        } else if self.is_animation() {
            R::SendGifs
        } else {
            R::SendFiles
        }
    }
    pub fn location_type(&self) -> u32 {
        if self.is_voice_message() {
            0x74dc404d
        } else if self.is_video_file() {
            0x3d0364ec
        } else {
            0x4e45abe9
        }
    }
    pub fn media_key(&self) -> (u64, u64) {
        (
            (u64::from(self.location_type()) << 32) | u64::from(self.dc_id as u32),
            self.id,
        )
    }
    pub fn cache_tag(&self) -> u8 {
        if self.kind == DocumentType::Sticker {
            2
        } else if self.is_voice_message() {
            3
        } else if self.is_video_message() {
            4
        } else if self.is_animation() {
            5
        } else if self.is_wallpaper() {
            1
        } else {
            0
        }
    }
    pub fn check_wallpaper_properties(&mut self) -> bool {
        if self.is_wallpaper() {
            return true;
        }
        if self.kind != DocumentType::File
            || !self.has_thumbnail()
            || self.dimensions.is_empty()
            || self.dimensions.width > 4096
            || self.dimensions.height > 4096
            || self.size > MAX_FILE_IN_MEMORY
        {
            return false;
        }
        self.kind = DocumentType::Wallpaper;
        true
    }
    pub fn update_thumbnails(
        &mut self,
        inline: &[u8],
        path: bool,
        thumbnail: &ImageWithLocation,
        video: &ImageWithLocation,
        premium: bool,
    ) -> Vec<DocumentEffect> {
        if !self.filename.is_empty()
            && !name_type_allows_thumbnail(detect_name_type(&self.filename))
        {
            return Vec::new();
        }
        if !inline.is_empty() && self.inline_thumbnail_bytes.is_empty() {
            self.inline_thumbnail_bytes = inline.to_vec();
            self.set_flag(INLINE_PATH, path);
        }
        if self
            .sticker()
            .is_none_or(|s| s.set_type != StickersType::Emoji)
        {
            self.set_flag(PREMIUM, premium);
        }
        let mut effects = Vec::new();
        for (file, input, animation) in [
            (&mut self.thumbnail, thumbnail, false),
            (&mut self.video_thumbnail, video, true),
        ] {
            if let Some((key, bytes)) = file.update(input).cache_put_if_empty {
                effects.push(DocumentEffect::CachePutIfEmpty {
                    key,
                    bytes,
                    animation,
                });
            }
        }
        effects
    }
    pub fn refresh_possible_cover_thumbnail(&mut self) -> Vec<DocumentEffect> {
        let Some(song) = self.song() else {
            return Vec::new();
        };
        if self.thumbnail.location.valid()
            || (song.performer.is_empty() && song.title.is_empty())
            || self.has_mime_type("audio/ogg")
        {
            return Vec::new();
        }
        self.flags |= POSSIBLE_COVER;
        let data = ImageWithLocation {
            location: ImageLocation::new(
                super::data_image_location::DownloadLocation::AudioAlbumThumb {
                    document_id: self.id,
                },
                100,
                100,
            ),
            ..Default::default()
        };
        let mut effects =
            self.update_thumbnails(&[], false, &data, &ImageWithLocation::default(), false);
        effects.push(DocumentEffect::LoadThumbnail);
        effects
    }
    /// Keep attribute order and pre-existing type/additional data. This is not
    /// a fresh classification on every call (data_document.cpp:344).
    pub fn set_attributes(
        &mut self,
        attributes: &[tg_scheme::DocumentAttribute],
        database: &impl MimeDatabase,
    ) -> Vec<DocumentEffect> {
        use tg_scheme::DocumentAttribute::*;
        use DocumentAdditional as A;
        use DocumentType as T;
        let mut effects = Vec::new();
        self.duration = -1;
        self.flags = (self.flags
            & !(IMAGE_TYPE | ATTACHED_STICKERS | TEXT_COLOR | SILENT | STREAM_MASK))
            | STREAM_MASK;
        if self.kind == T::File && is_mime_sticker_lottie(&self.mime) {
            self.kind = T::Sticker;
            self.additional = Some(A::Sticker(StickerData {
                kind: StickerType::Tgs,
                ..Default::default()
            }));
            self.dimensions = MediaSize::new(512, 512);
        }
        let mut was_video = if self.is_video_file() {
            self.additional.take()
        } else {
            None
        };
        self.video_preload_prefix = 0;
        for attribute in attributes {
            match attribute {
                DocumentAttributeImageSize(d) => self.dimensions = MediaSize::new(d.w, d.h),
                DocumentAttributeAnimated(_) => {
                    if matches!(self.kind, T::File | T::Video)
                        || self.sticker().is_some_and(|s| s.kind != StickerType::Webm)
                    {
                        self.kind = T::Animated;
                        self.additional = None;
                    }
                }
                DocumentAttributeSticker(d) => {
                    let was = self.kind;
                    if matches!(was, T::File | T::Video) {
                        self.kind = T::Sticker;
                        self.additional = Some(A::Sticker(StickerData::default()));
                    }
                    if let Some(A::Sticker(s)) = &mut self.additional {
                        s.set_type = if d.is_mask() {
                            StickersType::Masks
                        } else {
                            StickersType::Stickers
                        };
                        if was == T::Video {
                            s.kind = StickerType::Webm;
                        }
                        s.alt = String::from_utf8_lossy(&d.alt).into_owned();
                        s.set = sticker_set_identifier(&d.stickerset);
                    }
                }
                DocumentAttributeCustomEmoji(d) => {
                    let was = self.kind;
                    if matches!(was, T::File | T::Video) {
                        self.kind = T::Sticker;
                        self.additional = Some(A::Sticker(StickerData::default()));
                    }
                    if let Some(A::Sticker(s)) = &mut self.additional {
                        s.set_type = StickersType::Emoji;
                        if was == T::Video {
                            s.kind = StickerType::Webm;
                        }
                        s.alt = String::from_utf8_lossy(&d.alt).into_owned();
                        s.set = sticker_set_identifier(&d.stickerset);
                        self.set_flag(PREMIUM, !d.is_free());
                        if d.is_text_color() {
                            self.flags |= TEXT_COLOR;
                        }
                    }
                }
                DocumentAttributeVideo(d) => {
                    if self.kind == T::File {
                        if d.is_round_message() {
                            self.kind = T::RoundVideo;
                            self.additional = Some(A::Round(VoiceData::default()));
                        } else {
                            self.kind = T::Video;
                            if let Some(size) = d.preload_prefix_size {
                                if size > 0 && size < 6 * 1024 * 1024 {
                                    self.video_preload_prefix = size;
                                }
                            }
                            self.additional = was_video
                                .take()
                                .or_else(|| Some(A::Video(VideoData::default())));
                            if let Some(v) = self.video_mut() {
                                v.codec = String::from_utf8_lossy(
                                    d.video_codec.as_deref().unwrap_or_default(),
                                )
                                .into_owned();
                            }
                        }
                    } else if self.kind == T::Video && was_video.is_some() {
                        self.additional = was_video.take();
                    } else if let Some(A::Sticker(s)) = &mut self.additional {
                        s.kind = StickerType::Webm;
                    }
                    self.duration = milliseconds_from_seconds(d.duration);
                    self.set_maybe_supports_streaming(d.is_supports_streaming());
                    if d.is_nosound() {
                        self.flags |= SILENT;
                    }
                    self.dimensions = MediaSize::new(d.w, d.h);
                    if let Some(v) = self.video_mut() {
                        v.start_ts =
                            milliseconds_from_seconds(d.video_start_ts.unwrap_or_default());
                    }
                }
                DocumentAttributeAudio(d) => {
                    if self.kind == T::File {
                        if d.is_voice() {
                            self.kind = T::Voice;
                            self.additional = Some(A::Voice(VoiceData::default()));
                        } else {
                            self.kind = T::Song;
                            self.additional = Some(A::Song(SongData::default()));
                        }
                    }
                    match &mut self.additional {
                        Some(A::Voice(v) | A::Round(v)) => {
                            self.duration = i64::from(d.duration) * 1000;
                            v.waveform =
                                document_waveform_decode(d.waveform.as_deref().unwrap_or_default());
                            v.wavemax = v.waveform.iter().copied().max().unwrap_or(0);
                        }
                        Some(A::Song(s)) => {
                            self.duration = i64::from(d.duration) * 1000;
                            s.title =
                                String::from_utf8_lossy(d.title.as_deref().unwrap_or_default())
                                    .into_owned();
                            s.performer =
                                String::from_utf8_lossy(d.performer.as_deref().unwrap_or_default())
                                    .into_owned();
                            effects.extend(self.refresh_possible_cover_thumbnail());
                        }
                        _ => {}
                    }
                }
                DocumentAttributeFilename(d) => {
                    self.set_file_name(&String::from_utf8_lossy(&d.file_name))
                }
                DocumentAttributeHasStickers(_) => self.flags |= ATTACHED_STICKERS,
            }
        }
        if is_mime_sticker_webm(&self.mime) {
            if self.kind == T::File {
                self.kind = T::Sticker;
                self.additional = Some(A::Sticker(StickerData::default()));
            }
            if let Some(A::Sticker(s)) = &mut self.additional {
                s.kind = StickerType::Webm;
            }
        }
        if let Some(s) = self.sticker() {
            if !s.set.empty() && s.kind == StickerType::Webm && self.dimensions.is_empty() {
                match s.set_type {
                    StickersType::Emoji => self.dimensions = MediaSize::new(100, 100),
                    StickersType::Stickers => {
                        let thumb = MediaSize::new(
                            self.thumbnail.location.width,
                            self.thumbnail.location.height,
                        )
                        .fit(MediaSize::new(512, 512));
                        if !thumb.is_empty() {
                            self.dimensions = thumb;
                        }
                    }
                    StickersType::Masks => {}
                }
            }
        }
        if self.sticker().is_some_and(|s| {
            self.size > MAX_STICKER_BYTES
                || (!s.kind.is_lottie()
                    && !good_sticker_dimensions(self.dimensions.width, self.dimensions.height))
        }) {
            self.kind = T::File;
            self.additional = None;
        }
        if !self.filename.is_empty() {
            if (matches!(self.kind, T::Video | T::Animated | T::RoundVideo) || self.is_animation())
                && !self.enforce_name_type(NameType::Video, database)
            {
                self.kind = T::File;
                self.additional = None;
            }
            if (matches!(self.kind, T::Song | T::Voice) || self.is_audio_file())
                && !self.enforce_name_type(NameType::Audio, database)
            {
                self.kind = T::File;
                self.additional = None;
            }
            if !name_type_allows_thumbnail(self.name_type) {
                self.inline_thumbnail_bytes.clear();
                self.flags &= !INLINE_PATH;
                self.thumbnail.clear();
                self.video_thumbnail.clear();
                effects.push(DocumentEffect::ClearThumbnails);
            }
        }
        if self.is_audio_file()
            || self.is_animation()
            || self.is_voice_message()
            || self.story_media()
        {
            self.set_maybe_supports_streaming(true);
        }
        effects
    }
    pub fn is_image(&self) -> bool {
        self.flag(IMAGE_TYPE)
    }
    pub fn has_attached_stickers(&self) -> bool {
        self.flag(ATTACHED_STICKERS)
    }
    pub fn is_silent_video(&self) -> bool {
        self.flag(SILENT)
    }
    pub fn recount_is_image(&mut self) {
        self.set_flag(
            IMAGE_TYPE,
            !self.is_animation()
                && !self.is_video_file()
                && file_is_image(&self.filename, &self.mime),
        );
    }
    pub fn supports_streaming(&self) -> bool {
        self.flags & STREAM_MASK == 1
    }
    pub fn set_not_supports_streaming(&mut self) {
        self.flags &= !STREAM_MASK;
    }
    pub fn set_maybe_supports_streaming(&mut self, yes: bool) {
        if self.flags & STREAM_MASK != 0 {
            self.flags = (self.flags & !STREAM_MASK) | if yes { 1 } else { 2 };
        }
    }
    pub fn inapp_playback_failed(&self) -> bool {
        self.flag(PLAYBACK_FAILED)
    }
    pub fn set_inapp_playback_failed(&mut self) {
        self.flags |= PLAYBACK_FAILED;
    }
    pub fn force_is_streamed_animation(&mut self) {
        self.kind = DocumentType::Animated;
        self.additional = None;
        self.set_maybe_supports_streaming(true);
    }
    pub fn use_streaming_loader(&self) -> bool {
        self.size > 0
            && self.sticker().map_or_else(
                || {
                    self.is_animation()
                        || self.is_video_file()
                        || self.is_audio_file()
                        || self.is_voice_message()
                },
                |s| s.kind.is_webm(),
            )
    }
    pub fn can_be_streamed(&self) -> bool {
        self.has_remote_location() && self.supports_streaming()
    }
    pub fn story_media(&self) -> bool {
        self.flag(STORY)
    }
    pub fn set_story_media(&mut self, yes: bool) {
        self.set_flag(STORY, yes);
        if yes {
            self.set_maybe_supports_streaming(true);
        }
    }
    pub fn force_to_cache(&mut self, yes: bool) {
        self.set_flag(FORCE_CACHE, yes);
    }
    pub fn save_to_cache(&self) -> bool {
        self.size < MAX_FILE_IN_MEMORY
            && (self.sticker().is_some()
                || self.flag(FORCE_CACHE)
                || self.is_animation()
                || self.is_voice_message()
                || self.is_wallpaper()
                || self.is_theme()
                || (self.has_mime_type("image/png") && self.filename.starts_with("image_")))
    }
    pub fn set_data_and_cache(&self, data: &[u8]) -> Vec<DocumentEffect> {
        if let Some(media) = self.active_media_view() {
            media.borrow_mut().set_bytes(data);
        }
        if self.save_to_cache() && data.len() <= MAX_FILE_IN_MEMORY as usize {
            vec![DocumentEffect::CachePut {
                key: self.cache_key(),
                bytes: data.to_vec(),
                tag: self.cache_tag(),
            }]
        } else {
            Vec::new()
        }
    }
    pub fn uploading(&self) -> bool {
        self.uploading_data.is_some()
    }
    pub fn uploading_progress(&self) -> Option<f64> {
        self.uploading_data.as_ref().map(|u| u.progress(true))
    }
    pub fn set_waiting_for_album(&mut self) {
        if let Some(u) = &mut self.uploading_data {
            u.waiting_for_album = true;
        }
    }
    pub fn waiting_for_album(&self) -> bool {
        self.uploading_data
            .as_ref()
            .is_some_and(|u| u.waiting_for_album)
    }
    pub fn cancelled(&self) -> bool {
        self.flag(CANCELLED)
    }
    pub fn reset_cancelled(&mut self) {
        self.flags &= !CANCELLED;
    }
    pub fn loaded_in_media_cache(&self) -> bool {
        self.flag(IN_CACHE)
    }
    pub fn set_forbids_file_save(&mut self) {
        self.flags |= FORBID_SAVE;
    }
    pub fn forbids_file_save(&self) -> bool {
        self.flag(FORBID_SAVE)
    }
    pub fn good_thumbnail_checked(&self) -> bool {
        self.good_thumbnail_flags & 3 == 1
    }
    pub fn good_thumbnail_generating(&self) -> bool {
        self.good_thumbnail_flags & 3 == 2
    }
    pub fn good_thumbnail_no_data(&self) -> bool {
        self.good_thumbnail_flags & 3 == 3
    }
    pub fn set_good_thumbnail_generating(&mut self) {
        self.good_thumbnail_flags = (self.good_thumbnail_flags & !3) | 2;
    }
    pub fn set_good_thumbnail_data_ready(&mut self) {
        self.good_thumbnail_flags = 4 | if self.good_thumbnail_no_data() {
            0
        } else {
            self.good_thumbnail_flags & 3
        };
    }
    pub fn set_good_thumbnail_checked(&mut self, has_data: bool) -> Vec<DocumentEffect> {
        if !has_data && self.good_thumbnail_flags & 4 != 0 {
            self.good_thumbnail_flags &= !7;
            vec![DocumentEffect::CheckGoodThumbnail]
        } else {
            self.good_thumbnail_flags =
                (self.good_thumbnail_flags & !3) | if has_data { 1 } else { 3 };
            Vec::new()
        }
    }
    pub fn create_media_view(
        &mut self,
    ) -> Rc<std::cell::RefCell<super::data_document_media::DocumentMedia>> {
        if let Some(m) = self.media.upgrade() {
            return m;
        }
        let m = Rc::new(std::cell::RefCell::new(
            super::data_document_media::DocumentMedia::default(),
        ));
        self.media = Rc::downgrade(&m);
        m
    }
    pub fn active_media_view(
        &self,
    ) -> Option<Rc<std::cell::RefCell<super::data_document_media::DocumentMedia>>> {
        self.media.upgrade()
    }
    fn flag(&self, bit: u16) -> bool {
        self.flags & bit != 0
    }
    fn set_flag(&mut self, bit: u16, yes: bool) {
        if yes {
            self.flags |= bit;
        } else {
            self.flags &= !bit;
        }
    }
}
pub fn sticker_set_identifier(set: &tg_scheme::InputStickerSet) -> StickerSetIdentifier {
    if let tg_scheme::InputStickerSet::InputStickerSetID(d) = set {
        StickerSetIdentifier {
            id: d.id,
            access_hash: d.access_hash,
            short_name: String::new(),
        }
    } else {
        StickerSetIdentifier::default()
    }
}
