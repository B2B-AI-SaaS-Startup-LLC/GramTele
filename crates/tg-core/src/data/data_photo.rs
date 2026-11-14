//! Photo model metadata. UI previews and actual loader/pixel ownership are
//! explicitly deferred, rather than represented with dummy image objects.
use super::{
    data_image_location::*, data_media_location::*, data_media_types::*, data_store_keys::*,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum PhotoSize {
    #[default]
    Small = 0,
    Thumbnail = 1,
    Large = 2,
}
impl PhotoSize {
    pub fn index(self) -> usize {
        self as usize
    }
}
#[derive(Clone, Debug, Default)]
pub struct PhotoVideoSizes {
    pub small: CloudFileMetadata,
    pub large: CloudFileMetadata,
    pub start_time: i64,
    pub playback_failed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhotoEffect {
    WriteFileLocation {
        key: (u64, u64),
        location: crate::core::file_location::FileLocation,
    },
    CachePutIfEmpty {
        key: CacheKey,
        bytes: Vec<u8>,
        tag: u8,
    },
}
pub struct PhotoData {
    key: PhotoRecordKey,
    pub(crate) id: u64,
    pub peer: Option<PeerKey>,
    pub uploading_data: Option<UploadState>,
    pub(crate) dc_id: i32,
    pub(crate) access_hash: u64,
    pub(crate) file_reference: Vec<u8>,
    date_or_extended_video_duration: i32,
    has_stickers: bool,
    extended_media_preview: bool,
    inline_thumbnail_bytes: Vec<u8>,
    pub images: [CloudFileMetadata; 3],
    pub video_sizes: Option<PhotoVideoSizes>,
    media: Weak<RefCell<super::data_photo_media::PhotoMedia>>,
    location: crate::core::file_location::FileLocation,
}
impl PhotoData {
    pub const SIDE_LIMIT: i32 = 2560;
    pub fn new(scope: &SessionScope, id: u64) -> Self {
        Self::with_key(scope.new_photo_record_key(), id)
    }
    pub(crate) fn with_key(key: PhotoRecordKey, id: u64) -> Self {
        Self {
            key,
            id,
            peer: None,
            uploading_data: None,
            dc_id: 0,
            access_hash: 0,
            file_reference: Vec::new(),
            date_or_extended_video_duration: 0,
            has_stickers: false,
            extended_media_preview: false,
            inline_thumbnail_bytes: Vec::new(),
            images: Default::default(),
            video_sizes: None,
            media: Weak::new(),
            location: Default::default(),
        }
    }
    pub fn wire_key(&self) -> PhotoKey {
        PhotoKey::from_scoped_id(self.key.scope(), self.id)
    }
    pub fn key(&self) -> PhotoRecordKey {
        self.key
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn dc_id(&self) -> i32 {
        self.dc_id
    }
    pub fn media_key(&self) -> (u64, u64) {
        (u64::from(self.dc_id as u32), self.id)
    }
    pub fn location(
        &mut self,
        check: bool,
        files: &impl crate::core::file_location::FileLocations,
    ) -> &crate::core::file_location::FileLocation {
        if check && !self.location.verified_by(files) {
            let l = files.read(self.media_key());
            if !l.in_media_cache() {
                self.location = l;
            }
        }
        &self.location
    }
    pub fn set_location(
        &mut self,
        location: crate::core::file_location::FileLocation,
        files: &impl crate::core::file_location::FileLocations,
    ) -> Vec<PhotoEffect> {
        if !location.in_media_cache() && location.verified_by(files) {
            self.location = location.clone();
            vec![PhotoEffect::WriteFileLocation {
                key: self.media_key(),
                location,
            }]
        } else {
            Vec::new()
        }
    }
    pub fn date(&self) -> i32 {
        if self.extended_media_preview {
            0
        } else {
            self.date_or_extended_video_duration
        }
    }
    pub fn set_fields(&mut self, date: i32, has_stickers: bool) {
        self.date_or_extended_video_duration = date;
        self.has_stickers = has_stickers;
        self.extended_media_preview = false;
    }
    pub fn extended_media_preview(&self) -> bool {
        self.extended_media_preview
    }
    pub fn extended_media_video_duration(&self) -> Option<i32> {
        (self.extended_media_preview && self.date_or_extended_video_duration != 0)
            .then(|| self.date_or_extended_video_duration.wrapping_sub(1))
    }
    pub fn set_extended_media_preview(
        &mut self,
        inline: &[u8],
        dimensions: MediaSize,
        duration: Option<i32>,
    ) -> Vec<PhotoEffect> {
        self.extended_media_preview = true;
        let r = self.update_images(
            inline,
            &Default::default(),
            &Default::default(),
            &ImageWithLocation {
                location: ImageLocation::new(
                    DownloadLocation::default(),
                    dimensions.width,
                    dimensions.height,
                ),
                ..Default::default()
            },
            &Default::default(),
            &Default::default(),
            0,
        );
        self.date_or_extended_video_duration = duration.map_or(0, |n| n.wrapping_add(1));
        r
    }
    pub fn has_attached_stickers(&self) -> bool {
        self.has_stickers
    }
    pub fn set_has_attached_stickers(&mut self, value: bool) {
        self.has_stickers = value;
    }
    pub fn has_exact(&self, size: PhotoSize) -> bool {
        self.images[size.index()].location.valid()
    }
    pub fn valid_size_index(&self, size: PhotoSize) -> usize {
        (size.index()..3)
            .find(|&i| self.images[i].location.valid())
            .unwrap_or(2)
    }
    pub fn existing_size_index(&self, size: PhotoSize) -> usize {
        (size.index()..3)
            .find(|&i| self.images[i].location.valid() || self.images[i].progressive_part_size != 0)
            .unwrap_or(2)
    }
    pub fn image_location(&self, size: PhotoSize) -> &ImageLocation {
        &self.images[self.valid_size_index(size)].location
    }
    pub fn image_byte_size(&self, size: PhotoSize) -> i32 {
        let n = self.images[self.existing_size_index(size)].progressive_part_size;
        if n != 0 {
            n
        } else {
            self.images[self.valid_size_index(size)].byte_size
        }
    }
    pub fn size(&self, size: PhotoSize) -> Option<MediaSize> {
        let l = self.image_location(size);
        MediaSize::new(l.width, l.height).photo_size()
    }
    pub fn width(&self) -> i32 {
        self.images[2].location.width
    }
    pub fn height(&self) -> i32 {
        self.images[2].location.height
    }
    pub fn is_null(&self) -> bool {
        !self.images[2].location.valid()
    }
    pub fn failed(&self, size: PhotoSize) -> bool {
        self.images[self.valid_size_index(size)].failed
    }
    pub fn clear_failed(&mut self, size: PhotoSize) {
        let i = self.valid_size_index(size);
        self.images[i].failed = false;
    }
    pub fn cancelled(&self) -> bool {
        self.images[2].cancelled
    }
    pub fn uploading(&self) -> bool {
        self.uploading_data.is_some()
    }
    pub fn uploading_progress(&self) -> Option<f64> {
        self.uploading_data.as_ref().map(|u| u.progress(false))
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
    pub fn set_remote_location(&mut self, dc: i32, access: u64, reference: &[u8]) {
        self.dc_id = dc;
        self.access_hash = access;
        self.file_reference = reference.to_vec();
    }
    pub fn mtp_input(&self) -> tg_scheme::InputPhoto {
        tg_scheme::mtp_input_photo(self.id, self.access_hash, self.file_reference.clone())
    }
    pub fn file_reference(&self) -> &[u8] {
        &self.file_reference
    }
    /// The original refreshes image locations only, not video locations.
    pub fn refresh_file_reference(&mut self, value: &[u8]) {
        self.file_reference = value.to_vec();
        for i in &mut self.images {
            i.location.file.refresh_file_reference(value);
        }
    }
    pub fn inline_thumbnail_bytes(&self) -> &[u8] {
        &self.inline_thumbnail_bytes
    }
    pub fn clear_inline_thumbnail_bytes(&mut self) {
        self.inline_thumbnail_bytes.clear();
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update_images(
        &mut self,
        inline: &[u8],
        small: &ImageWithLocation,
        thumbnail: &ImageWithLocation,
        large: &ImageWithLocation,
        video_small: &ImageWithLocation,
        video_large: &ImageWithLocation,
        video_start_time: i64,
    ) -> Vec<PhotoEffect> {
        if !inline.is_empty() && self.inline_thumbnail_bytes.is_empty() {
            self.inline_thumbnail_bytes = inline.to_vec();
        }
        let mut effects = Vec::new();
        for (image, data) in self.images.iter_mut().zip([small, thumbnail, large]) {
            if let Some(p) = image.update(data).cache_put_if_empty {
                effects.push(PhotoEffect::CachePutIfEmpty {
                    key: p.0,
                    bytes: p.1,
                    tag: 1,
                });
            }
        }
        if !video_large.location.valid() {
            self.video_sizes = None;
        } else {
            let v = self.video_sizes.get_or_insert_with(Default::default);
            v.start_time = video_start_time;
            for (file, data) in [(&mut v.large, video_large), (&mut v.small, video_small)] {
                if let Some(p) = file.update(data).cache_put_if_empty {
                    effects.push(PhotoEffect::CachePutIfEmpty {
                        key: p.0,
                        bytes: p.1,
                        tag: 5,
                    });
                }
            }
        }
        effects
    }
    pub fn has_video(&self) -> bool {
        self.video_sizes.is_some()
    }
    pub fn has_video_small(&self) -> bool {
        self.video_sizes
            .as_ref()
            .is_some_and(|s| s.small.location.valid())
    }
    pub fn video_file(&self, size: PhotoSize) -> Option<&CloudFileMetadata> {
        self.video_sizes.as_ref().map(|v| {
            if size == PhotoSize::Small && self.has_video_small() {
                &v.small
            } else {
                &v.large
            }
        })
    }
    pub fn video_location(&self, size: PhotoSize) -> Option<&ImageLocation> {
        self.video_file(size).map(|f| &f.location)
    }
    pub fn video_byte_size(&self, size: PhotoSize) -> i32 {
        self.video_file(size).map_or(0, |f| f.byte_size)
    }
    pub fn video_start_position(&self) -> i64 {
        self.video_sizes.as_ref().map_or(0, |v| v.start_time)
    }
    pub fn video_failed(&self, size: PhotoSize) -> bool {
        self.video_file(size).is_some_and(|f| f.failed)
    }
    pub fn video_loaded(&self, size: PhotoSize) -> bool {
        self.video_file(size).is_some_and(|f| f.loaded)
    }
    pub fn set_video_playback_failed(&mut self) {
        if let Some(v) = &mut self.video_sizes {
            v.playback_failed = true;
        }
    }
    pub fn video_playback_failed(&self) -> bool {
        self.video_sizes.as_ref().is_some_and(|v| v.playback_failed)
    }
    pub fn video_can_be_played(&self) -> bool {
        self.has_video() && !self.video_playback_failed()
    }
    pub fn create_media_view(&mut self) -> Rc<RefCell<super::data_photo_media::PhotoMedia>> {
        if let Some(m) = self.media.upgrade() {
            return m;
        }
        let m = Rc::new(RefCell::new(super::data_photo_media::PhotoMedia::default()));
        self.media = Rc::downgrade(&m);
        m
    }
    pub fn active_media_view(&self) -> Option<Rc<RefCell<super::data_photo_media::PhotoMedia>>> {
        self.media.upgrade()
    }
}
