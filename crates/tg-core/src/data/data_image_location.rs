//! Headless value slice of ui/image/image_location.{h,cpp}; no image/cache objects.
//! Equality deliberately ignores image dimensions, access hashes and file refs.
use super::data_peer_id::{peer_from_channel, peer_from_chat, peer_from_user, PeerId, UserId};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum StorageFileType {
    #[default]
    Legacy = 0,
    Encrypted = 1,
    Document = 2,
    Secure = 3,
    Takeout = 4,
    Photo = 5,
    PeerPhoto = 6,
    StickerSetThumb = 7,
    GroupCallStream = 8,
}
#[derive(Clone, Debug, Default)]
pub struct StorageFileLocation {
    pub dc_id: u16,
    pub kind: StorageFileType,
    pub size_letter: u8,
    pub local_id: i32,
    pub id: u64,
    pub access_hash: u64,
    pub volume_id: u64,
    pub in_message_peer_id: PeerId,
    pub in_message_id: u32,
    pub file_reference: Vec<u8>,
}
impl StorageFileLocation {
    pub fn valid(&self) -> bool {
        use StorageFileType::*;
        match self.kind {
            Legacy => self.dc_id != 0 && self.volume_id != 0 && self.local_id != 0,
            Takeout => true,
            Photo => self.dc_id != 0 && self.id != 0 && self.size_letter != 0,
            GroupCallStream => self.dc_id != 0 && self.id != 0 && self.volume_id != 0,
            _ => self.dc_id != 0 && self.id != 0,
        }
    }
    pub fn is_legacy(&self) -> bool {
        self.kind == StorageFileType::Legacy
    }
    pub fn peer_photo(
        dc_id: i32,
        self_id: UserId,
        peer: &tg_scheme::InputPeer,
        photo_id: u64,
        big: bool,
    ) -> Self {
        let mut r = Self {
            dc_id: dc_id as u16,
            kind: StorageFileType::PeerPhoto,
            volume_id: photo_id,
            size_letter: if big { b'c' } else { b'a' },
            ..Self::default()
        };
        fn fill(r: &mut StorageFileLocation, p: &tg_scheme::InputPeer, self_id: UserId) -> bool {
            use tg_scheme::InputPeer::*;
            match p {
                InputPeerEmpty(_) => r.id = 0,
                InputPeerSelf(_) => r.id = peer_from_user(self_id).value,
                InputPeerChat(d) => r.id = peer_from_chat(d.chat_id).value,
                InputPeerUser(d) => {
                    r.id = peer_from_user(d.user_id).value;
                    r.access_hash = d.access_hash;
                }
                InputPeerChannel(d) => {
                    r.id = peer_from_channel(d.channel_id).value;
                    r.access_hash = d.access_hash;
                }
                _ => return false,
            };
            true
        }
        use tg_scheme::InputPeer::*;
        match peer {
            InputPeerUserFromMessage(d) => {
                if !fill(&mut r, &d.peer, self_id) {
                    r.id = 0;
                    r.access_hash = 0;
                }
                r.in_message_peer_id = peer_from_user(d.user_id);
                r.in_message_id = d.msg_id as u32;
            }
            InputPeerChannelFromMessage(d) => {
                if !fill(&mut r, &d.peer, self_id) {
                    r.id = 0;
                    r.access_hash = 0;
                }
                r.in_message_peer_id = peer_from_channel(d.channel_id);
                r.in_message_id = d.msg_id as u32;
            }
            _ => {
                fill(&mut r, peer, self_id);
            }
        }
        r
    }
}
impl PartialEq for StorageFileLocation {
    fn eq(&self, b: &Self) -> bool {
        use StorageFileType::*;
        let a = self;
        if a.valid() != b.valid() {
            return false;
        }
        if !a.valid() {
            return true;
        }
        if a.kind != b.kind {
            return false;
        }
        match a.kind {
            Legacy => (a.dc_id, a.volume_id, a.local_id) == (b.dc_id, b.volume_id, b.local_id),
            Encrypted | Secure => (a.dc_id, a.id) == (b.dc_id, b.id),
            Photo | Document => (a.dc_id, a.id, a.size_letter) == (b.dc_id, b.id, b.size_letter),
            Takeout => true,
            PeerPhoto => {
                (a.dc_id, a.volume_id, a.local_id, a.id, a.size_letter)
                    == (b.dc_id, b.volume_id, b.local_id, b.id, b.size_letter)
            }
            StickerSetThumb => {
                (a.dc_id, a.volume_id, a.local_id, a.id) == (b.dc_id, b.volume_id, b.local_id, b.id)
            }
            GroupCallStream => {
                (a.dc_id, a.id, a.local_id, a.size_letter)
                    == (b.dc_id, b.id, b.local_id, b.size_letter)
            }
        }
    }
}
impl Eq for StorageFileLocation {}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeoPointLocation {
    pub lat: f64,
    pub lon: f64,
    pub access: u64,
    pub width: i32,
    pub height: i32,
    pub zoom: i32,
    pub scale: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DownloadLocation {
    Storage(StorageFileLocation),
    Web { access_hash: u64, url: Vec<u8> },
    Geo(GeoPointLocation),
    Url(String),
    AudioAlbumThumb { document_id: u64 },
    InMemory(Vec<u8>),
}
impl Default for DownloadLocation {
    fn default() -> Self {
        Self::Storage(StorageFileLocation::default())
    }
}
impl DownloadLocation {
    pub fn valid(&self) -> bool {
        match self {
            Self::Storage(s) => s.valid(),
            Self::Web { url, .. } => !url.is_empty(),
            Self::Geo(_) => true,
            Self::Url(s) => !s.is_empty(),
            Self::AudioAlbumThumb { document_id } => *document_id != 0,
            Self::InMemory(s) => !s.is_empty(),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct ImageLocation {
    pub file: DownloadLocation,
    pub width: i32,
    pub height: i32,
}
impl ImageLocation {
    pub fn new(file: DownloadLocation, width: i32, height: i32) -> Self {
        Self {
            file,
            width,
            height,
        }
    }
    pub fn valid(&self) -> bool {
        self.file.valid()
    }
    pub fn set_size(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
    }
}
impl PartialEq for ImageLocation {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
    }
}
