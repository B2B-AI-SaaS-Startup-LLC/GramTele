//! Facade::Impl adds no behavior: ownership and delegation replace the PImpl.
use super::{storage_shared_media::*, storage_user_photos::UserPhotos};
#[derive(Default)]
pub struct Facade {
    pub shared_media: SharedMedia,
    pub user_photos: UserPhotos,
}
impl std::ops::Deref for Facade {
    type Target = SharedMedia;
    fn deref(&self) -> &SharedMedia {
        &self.shared_media
    }
}
impl std::ops::DerefMut for Facade {
    fn deref_mut(&mut self) -> &mut SharedMedia {
        &mut self.shared_media
    }
}
