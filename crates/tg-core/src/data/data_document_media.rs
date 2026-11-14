//! Independent byte lease. No Session/DocumentData pointer is retained.
//! Pixels and real loader adapters remain storage/file_download and stage 5.
#[derive(Default)]
pub struct DocumentMedia {
    bytes: Vec<u8>,
    video_thumbnail_bytes: Vec<u8>,
    good_thumbnail_wanted: bool,
}
impl DocumentMedia {
    pub fn may_have_good_thumbnail(owner: &super::data_document::DocumentData) -> bool {
        owner.is_video_file()
            || owner.is_animation()
            || owner.is_wallpaper()
            || owner.is_theme()
            || owner.is_svg_image()
            || owner.sticker().is_some_and(|s| s.kind.is_animated())
    }
    pub fn video_thumbnail_size(
        owner: &super::data_document::DocumentData,
    ) -> super::data_media_types::MediaSize {
        let l = &owner.video_thumbnail.location;
        super::data_media_types::MediaSize::new(l.width, l.height)
    }
    pub fn loaded(&self, filepath: &str) -> bool {
        !self.bytes.is_empty() || !filepath.is_empty()
    }
    pub fn can_be_played(
        &self,
        owner: &super::data_document::DocumentData,
        filepath: &str,
    ) -> bool {
        !owner.inapp_playback_failed()
            && owner.use_streaming_loader()
            && (self.loaded(filepath) || owner.can_be_streamed())
    }
    pub fn good_thumbnail_wanted(&mut self) {
        self.good_thumbnail_wanted = true;
    }
    pub fn is_good_thumbnail_wanted(&self) -> bool {
        self.good_thumbnail_wanted
    }
    pub fn set_bytes(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.bytes = bytes.to_vec();
        }
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn video_thumbnail_content(&self) -> &[u8] {
        &self.video_thumbnail_bytes
    }
    pub fn set_video_thumbnail(&mut self, bytes: Vec<u8>) {
        self.video_thumbnail_bytes = bytes;
    }
    /// Pixel fields are intentionally absent until the actual image package.
    pub fn collect_local_bytes(&mut self, local: &Self) {
        self.bytes.clone_from(&local.bytes);
        self.video_thumbnail_bytes
            .clone_from(&local.video_thumbnail_bytes);
        self.good_thumbnail_wanted = local.good_thumbnail_wanted;
    }
}
