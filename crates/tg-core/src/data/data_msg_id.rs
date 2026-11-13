use super::data_peer_id::PeerId;
use std::ops::{Add, Neg, Not, Sub};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MsgId {
    pub bare: i64,
}

impl MsgId {
    pub const fn new(bare: i64) -> Self {
        Self { bare }
    }
    pub const fn is_zero(self) -> bool {
        self.bare == 0
    }
    pub fn increment(&mut self) -> Self {
        self.bare = self.bare.checked_add(1).expect("MsgId increment overflow");
        *self
    }
    pub fn post_increment(&mut self) -> Self {
        let previous = *self;
        self.increment();
        previous
    }
    pub fn decrement(&mut self) -> Self {
        self.bare = self.bare.checked_sub(1).expect("MsgId decrement overflow");
        *self
    }
    pub fn post_decrement(&mut self) -> Self {
        let previous = *self;
        self.decrement();
        previous
    }
}
impl From<i64> for MsgId {
    fn from(bare: i64) -> Self {
        Self::new(bare)
    }
}
impl Not for MsgId {
    type Output = bool;
    fn not(self) -> bool {
        self.is_zero()
    }
}
impl Neg for MsgId {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(self.bare.checked_neg().expect("MsgId negation overflow"))
    }
}
impl Add for MsgId {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(
            self.bare
                .checked_add(other.bare)
                .expect("MsgId addition overflow"),
        )
    }
}
impl Sub for MsgId {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(
            self.bare
                .checked_sub(other.bare)
                .expect("MsgId subtraction overflow"),
        )
    }
}

pub type StoryId = i32;
pub type BusinessShortcutId = i32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FullStoryId {
    pub peer: PeerId,
    pub story: StoryId,
}
impl FullStoryId {
    pub const fn valid(self) -> bool {
        !self.peer.is_zero() && self.story != 0
    }
}
impl Not for FullStoryId {
    type Output = bool;
    fn not(self) -> bool {
        !self.valid()
    }
}

pub const START_CLIENT_MSG_ID: MsgId = MsgId::new(1 - (1_i64 << 58));
pub const CLIENT_MSG_IDS: i64 = 1_i64 << 31;
pub const END_CLIENT_MSG_ID: MsgId = MsgId::new(START_CLIENT_MSG_ID.bare + CLIENT_MSG_IDS);
pub const START_STORY_MSG_ID: MsgId = MsgId::new(END_CLIENT_MSG_ID.bare + 1);
pub const SERVER_MAX_STORY_ID: StoryId = 1 << 30;
pub const STORY_MSG_IDS: i64 = SERVER_MAX_STORY_ID as i64;
pub const END_STORY_MSG_ID: MsgId = MsgId::new(START_STORY_MSG_ID.bare + STORY_MSG_IDS);
pub const SERVER_MAX_MSG_ID: MsgId = MsgId::new(1_i64 << 56);
pub const SCHEDULED_MAX_MSG_ID: MsgId = MsgId::new(SERVER_MAX_MSG_ID.bare + (1_i64 << 32));
pub const SHORTCUT_MAX_MSG_ID: MsgId = MsgId::new(SCHEDULED_MAX_MSG_ID.bare + (1_i64 << 32));
pub const WELCOME_MAX_MSG_ID: MsgId = MsgId::new(SHORTCUT_MAX_MSG_ID.bare + (1_i64 << 32));
pub const SHOW_AT_UNREAD_MSG_ID: MsgId = MsgId::new(0);
pub const SPECIAL_MSG_ID_SHIFT: i64 = END_STORY_MSG_ID.bare;
pub const SHOW_AT_THE_END_MSG_ID: MsgId = MsgId::new(SPECIAL_MSG_ID_SHIFT + 1);
pub const SWITCH_AT_TOP_MSG_ID: MsgId = MsgId::new(SPECIAL_MSG_ID_SHIFT + 2);
pub const SHOW_AND_START_BOT_MSG_ID: MsgId = MsgId::new(SPECIAL_MSG_ID_SHIFT + 4);
pub const SHOW_AND_MAYBE_START_BOT_MSG_ID: MsgId = MsgId::new(SPECIAL_MSG_ID_SHIFT + 5);
pub const SHOW_FOR_CHOOSE_MESSAGES_MSG_ID: MsgId = MsgId::new(SPECIAL_MSG_ID_SHIFT + 6);
pub const SEARCH_QUERY_OFFSET_HINT: i32 = -1;

const _: () = assert!(SPECIAL_MSG_ID_SHIFT + 0xff < 0);
const _: () = assert!(-(SPECIAL_MSG_ID_SHIFT + 0xff) > SERVER_MAX_MSG_ID.bare);

pub const fn is_client_msg_id(id: MsgId) -> bool {
    id.bare >= START_CLIENT_MSG_ID.bare && id.bare < END_CLIENT_MSG_ID.bare
}
pub const fn client_msg_index(id: MsgId) -> i32 {
    assert!(is_client_msg_id(id));
    (id.bare - START_CLIENT_MSG_ID.bare) as i32
}
pub const fn client_msg_by_index(index: i32) -> MsgId {
    assert!(index >= 0);
    MsgId::new(START_CLIENT_MSG_ID.bare + index as i64)
}
pub const fn is_story_msg_id(id: MsgId) -> bool {
    id.bare >= START_STORY_MSG_ID.bare && id.bare < END_STORY_MSG_ID.bare
}
pub const fn story_id_from_msg_id(id: MsgId) -> StoryId {
    assert!(is_story_msg_id(id));
    (id.bare - START_STORY_MSG_ID.bare) as StoryId
}
pub const fn story_id_to_msg_id(id: StoryId) -> MsgId {
    assert!(id >= 0);
    MsgId::new(START_STORY_MSG_ID.bare + id as i64)
}
pub const fn is_server_msg_id(id: MsgId) -> bool {
    id.bare > 0 && id.bare < SERVER_MAX_MSG_ID.bare
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MsgRange {
    pub from: MsgId,
    pub till: MsgId,
}
impl MsgRange {
    pub const fn new(from: MsgId, till: MsgId) -> Self {
        Self { from, till }
    }
}

/// ```compile_fail
/// use tg_core::data::{data_peer_id::ChannelId, data_msg_id::{FullMsgId, MsgId}};
/// let invalid = FullMsgId::new(ChannelId::new(42), MsgId::new(7));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FullMsgId {
    pub peer: PeerId,
    pub msg: MsgId,
}
impl FullMsgId {
    pub const fn new(peer: PeerId, msg: MsgId) -> Self {
        Self { peer, msg }
    }
    pub const fn is_zero(self) -> bool {
        self.msg.is_zero()
    }
}
impl Not for FullMsgId {
    type Output = bool;
    fn not(self) -> bool {
        self.is_zero()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalMsgId {
    pub item_id: FullMsgId,
    pub session_unique_id: u64,
}
impl GlobalMsgId {
    pub const fn is_zero(self) -> bool {
        self.item_id.is_zero() || self.session_unique_id == 0
    }
}
impl Not for GlobalMsgId {
    type Output = bool;
    fn not(self) -> bool {
        self.is_zero()
    }
}
