//! Borrowed projections of actual subclass fields, not UserData/ChatData/ChannelData
//! stand-ins. No defaults: future real stores must supply every requested value.
use super::data_flags::Flags;
use super::data_store_keys::{ChannelKey, ChatKey, GroupCallKey, PeerKey, UserKey};
use super::{
    data_channel::*,
    data_chat::*,
    data_chat_participant_status::*,
    data_peer_common::StarsRating,
    data_peer_id::{PeerId, UserId},
    data_user::*,
    data_user_names::UsernamesInfo,
};
pub struct UserPeerFields<'a> {
    pub forum: Option<PeerKey>,
    pub key: UserKey,
    pub user_creates_topics: bool,
    pub flags: &'a Flags<UserDataFlag>,
    pub usernames: &'a UsernamesInfo,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub name_or_phone: &'a str,
    pub bot_info_present: bool,
    pub effective_premium: bool,
    pub stars_per_message: i32,
    pub stars_rating: StarsRating,
    pub peer_gifts_count: i32,
}
pub struct ChatPeerFields<'a> {
    pub migrated_to: Option<ChannelKey>,
    pub group_call: Option<GroupCallKey>,
    pub group_call_default_join_as: PeerId,
    pub key: ChatKey,
    pub flags: &'a Flags<ChatDataFlag>,
    pub admin_rights: ChatAdminRights,
    pub default_restrictions: ChatRestrictions,
}
pub struct ChannelPeerFields<'a> {
    pub migrated_from: Option<ChatKey>,
    pub monoforum_link: Option<ChannelKey>,
    pub forum: Option<PeerKey>,
    pub monoforum: Option<PeerKey>,
    pub group_call: Option<GroupCallKey>,
    pub group_call_default_join_as: PeerId,
    pub key: ChannelKey,
    pub can_post_messages: bool,
    pub can_edit_messages: bool,
    pub can_pin_messages: bool,
    pub can_post_stories: bool,
    pub can_edit_stories: bool,
    pub can_delete_stories: bool,
    pub can_delete: bool,
    pub members_count_known: bool,
    pub flags: &'a Flags<ChannelDataFlag>,
    pub usernames: &'a UsernamesInfo,
    pub admin_rights: ChatAdminRights,
    pub default_restrictions: ChatRestrictions,
    pub restrictions: ChatRestrictions,
    pub boosts_unrestrict: i32,
    pub boosts_applied: i32,
    pub slowmode_seconds: i32,
    pub slowmode_last_message: i32,
    pub has_megagroup_info: bool,
    pub inviter: UserId,
    pub stars_per_message: i32,
    pub peer_gifts_count: i32,
}
pub enum PeerFields<'a> {
    User(UserPeerFields<'a>),
    Chat(ChatPeerFields<'a>),
    Channel(ChannelPeerFields<'a>),
}
impl PeerFields<'_> {
    pub fn key(&self) -> PeerKey {
        match self {
            Self::User(v) => v.key.peer(),
            Self::Chat(v) => v.key.peer(),
            Self::Channel(v) => v.key.peer(),
        }
    }
    pub fn usernames(&self) -> Option<&UsernamesInfo> {
        match self {
            Self::User(v) => Some(v.usernames),
            Self::Channel(v) => Some(v.usernames),
            Self::Chat(_) => None,
        }
    }
    pub fn is_creator(&self) -> bool {
        match self {
            Self::User(_) => false,
            Self::Chat(v) => (v.flags.current() & ChatDataFlag::Creator).any(),
            Self::Channel(v) => (v.flags.current() & ChannelDataFlag::Creator).any(),
        }
    }
    pub fn admin_rights(&self) -> ChatAdminRights {
        match self {
            Self::User(_) => ChatAdminRights::empty(),
            Self::Chat(v) => v.admin_rights,
            Self::Channel(v) => v.admin_rights,
        }
    }
}
