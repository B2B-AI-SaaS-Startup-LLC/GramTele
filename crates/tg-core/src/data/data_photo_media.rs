//! Standalone byte lease; actual decoded image slots are stage 5.
use super::data_photo::PhotoSize;
#[derive(Default)]
struct EncodedPhotoImage {
    bytes: Vec<u8>,
    good_for: PhotoSize,
}
#[derive(Default)]
pub struct PhotoMedia {
    images: [EncodedPhotoImage; 3],
    video_bytes: [Vec<u8>; 2],
}
impl PhotoMedia {
    /// Byte/metadata phase of PhotoMedia::set. This does not claim a decoded
    /// Image exists and does not implement image()/loaded() (actual pixels5).
    pub fn set_encoded_image(&mut self, size: PhotoSize, good_for: PhotoSize, bytes: Vec<u8>) {
        self.images[size.index()] = EncodedPhotoImage { bytes, good_for };
    }
    pub fn encoded_image_slot(&self, size: PhotoSize) -> (&[u8], PhotoSize) {
        let i = &self.images[size.index()];
        (&i.bytes, i.good_for)
    }
    pub fn video_size(
        owner: &super::data_photo::PhotoData,
        size: PhotoSize,
    ) -> super::data_media_types::MediaSize {
        owner
            .video_location(size)
            .map_or(super::data_media_types::MediaSize::new(0, 0), |l| {
                super::data_media_types::MediaSize::new(l.width, l.height)
            })
    }
    fn video_index(size: PhotoSize, has_small: bool) -> usize {
        usize::from(size != PhotoSize::Small || !has_small)
    }
    pub fn video_content(&self, size: PhotoSize, has_small: bool) -> &[u8] {
        &self.video_bytes[Self::video_index(size, has_small)]
    }
    pub fn set_video(&mut self, size: PhotoSize, has_small: bool, bytes: Vec<u8>) {
        self.video_bytes[Self::video_index(size, has_small)] = bytes;
    }
}
