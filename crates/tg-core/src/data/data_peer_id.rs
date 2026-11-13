use std::ops::Not;
use tg_scheme::{mtp_peer_channel, mtp_peer_chat, mtp_peer_user, Peer};

pub type BareId = u64;

/// ```compile_fail
/// use tg_core::data::data_peer_id::{UserId, ChatId};
/// let user: UserId = ChatId::new(42);
/// ```
/// ```compile_fail
/// use tg_core::data::data_peer_id::ChatIdType;
/// let invalid = ChatIdType::<128>::new(42);
/// ```
#[repr(transparent)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChatIdType<const SHIFT: u8> {
    pub bare: BareId,
}

impl<const SHIFT: u8> ChatIdType<SHIFT> {
    pub const SHIFT: u8 = SHIFT;
    pub const RESERVED_BIT: BareId = 0x80;
    const CHECK: () = assert!((SHIFT as u64 & Self::RESERVED_BIT) == 0);

    pub const fn new(bare: BareId) -> Self {
        let () = Self::CHECK;
        Self { bare }
    }

    pub const fn is_zero(self) -> bool {
        self.bare == 0
    }
}

impl<const SHIFT: u8> Default for ChatIdType<SHIFT> {
    fn default() -> Self {
        Self::new(0)
    }
}
impl<const SHIFT: u8> From<u64> for ChatIdType<SHIFT> {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}
impl<const SHIFT: u8> From<u32> for ChatIdType<SHIFT> {
    fn from(value: u32) -> Self {
        Self::new(value as u64)
    }
}
impl<const SHIFT: u8> From<i64> for ChatIdType<SHIFT> {
    fn from(value: i64) -> Self {
        Self::new(value as u64)
    }
}
impl<const SHIFT: u8> From<i32> for ChatIdType<SHIFT> {
    fn from(value: i32) -> Self {
        Self::new(value as u64)
    }
}
impl<const SHIFT: u8> Not for ChatIdType<SHIFT> {
    type Output = bool;
    fn not(self) -> bool {
        self.is_zero()
    }
}

pub type UserId = ChatIdType<0>;
pub type ChatId = ChatIdType<1>;
pub type ChannelId = ChatIdType<2>;
pub type FakeChatId = ChatIdType<0x7f>;

/// ```compile_fail
/// use tg_core::data::data_peer_id::PeerId;
/// let ordered = PeerId::new(42) < 0;
/// ```
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId {
    pub value: BareId,
}

impl PeerId {
    pub const CHAT_TYPE_MASK: BareId = 0xffff_ffff_ffff;

    pub const fn new(value: BareId) -> Self {
        Self { value }
    }
    pub const fn from_chat_id<const SHIFT: u8>(id: ChatIdType<SHIFT>) -> Self {
        Self::new(id.bare | ((SHIFT as BareId) << 48))
    }
    pub const fn is<const SHIFT: u8>(self) -> bool {
        let () = ChatIdType::<SHIFT>::CHECK;
        ((self.value >> 48) & 0xff) == SHIFT as BareId
    }
    pub const fn to<const SHIFT: u8>(self) -> ChatIdType<SHIFT> {
        ChatIdType::new(if self.is::<SHIFT>() {
            self.value & Self::CHAT_TYPE_MASK
        } else {
            0
        })
    }
    pub const fn is_zero(self) -> bool {
        self.value == 0
    }
}

impl<const SHIFT: u8> From<ChatIdType<SHIFT>> for PeerId {
    fn from(id: ChatIdType<SHIFT>) -> Self {
        Self::from_chat_id(id)
    }
}
impl Not for PeerId {
    type Output = bool;
    fn not(self) -> bool {
        self.is_zero()
    }
}

pub const fn peer_is_user(id: PeerId) -> bool {
    id.is::<0>()
}
pub const fn peer_is_chat(id: PeerId) -> bool {
    id.is::<1>()
}
pub const fn peer_is_channel(id: PeerId) -> bool {
    id.is::<2>()
}

pub fn peer_from_user(id: impl Into<UserId>) -> PeerId {
    PeerId::from_chat_id(id.into())
}
pub fn peer_from_chat(id: impl Into<ChatId>) -> PeerId {
    PeerId::from_chat_id(id.into())
}
pub fn peer_from_channel(id: impl Into<ChannelId>) -> PeerId {
    PeerId::from_chat_id(id.into())
}

pub const fn peer_to_user(id: PeerId) -> UserId {
    id.to::<0>()
}
pub const fn peer_to_chat(id: PeerId) -> ChatId {
    id.to::<1>()
}
pub const fn peer_to_channel(id: PeerId) -> ChannelId {
    id.to::<2>()
}
pub const fn peer_to_bare_mtp_int(id: PeerId) -> u64 {
    id.value & PeerId::CHAT_TYPE_MASK
}

pub fn peer_from_mtp(peer: &Peer) -> PeerId {
    match peer {
        Peer::PeerUser(data) => peer_from_user(data.user_id),
        Peer::PeerChat(data) => peer_from_chat(data.chat_id),
        Peer::PeerChannel(data) => peer_from_channel(data.channel_id),
    }
}

pub fn peer_to_mtp(id: PeerId) -> Peer {
    if peer_is_user(id) {
        mtp_peer_user(peer_to_bare_mtp_int(id))
    } else if peer_is_chat(id) {
        mtp_peer_chat(peer_to_bare_mtp_int(id))
    } else if peer_is_channel(id) {
        mtp_peer_channel(peer_to_bare_mtp_int(id))
    } else {
        mtp_peer_user(0)
    }
}

const MODERN_FLAG: u64 = UserId::RESERVED_BIT << 48;

pub const fn deserialize_peer_id(serialized: u64) -> PeerId {
    if serialized & MODERN_FLAG != 0 {
        return PeerId::new(serialized & !MODERN_FLAG);
    }
    let bare = serialized & 0xffff_ffff;
    match serialized & 0xf_0000_0000 {
        0 => PeerId::from_chat_id(UserId::new(bare)),
        0x1_0000_0000 => PeerId::from_chat_id(ChatId::new(bare)),
        0x2_0000_0000 => PeerId::from_chat_id(ChannelId::new(bare)),
        0xf_0000_0000 => PeerId::from_chat_id(FakeChatId::new(bare)),
        _ => PeerId::new(0),
    }
}

pub const fn serialize_peer_id(id: PeerId) -> u64 {
    assert!(id.value & MODERN_FLAG == 0);
    id.value | MODERN_FLAG
}
