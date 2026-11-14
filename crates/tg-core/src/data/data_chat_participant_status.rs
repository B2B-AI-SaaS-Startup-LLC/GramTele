//! ChatAdminRight value contract from data_chat_participant_status.h.
//! Scalar rights conversions and sending contracts; UI errors are tracked separately.
use tg_base::flags::Flags;
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatAdminRight {
    ChangeInfo = 1 << 0,
    PostMessages = 1 << 1,
    EditMessages = 1 << 2,
    DeleteMessages = 1 << 3,
    BanUsers = 1 << 4,
    InviteByLinkOrAdd = 1 << 5,
    PinMessages = 1 << 7,
    AddAdmins = 1 << 9,
    Anonymous = 1 << 10,
    ManageCall = 1 << 11,
    Other = 1 << 12,
    ManageTopics = 1 << 13,
    PostStories = 1 << 14,
    EditStories = 1 << 15,
    DeleteStories = 1 << 16,
    ManageDirect = 1 << 17,
    ManageRanks = 1 << 18,
    ProcessJoinRequests = 1 << 19,
    ManageLinkedPeers = 1 << 20,
    ManageWelcomeMessages = 1 << 21,
}
tg_base::flag_type!(ChatAdminRight: i32);
pub type ChatAdminRights = Flags<ChatAdminRight>;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatRestriction {
    ViewMessages = 1 << 0,
    SendStickers = 1 << 3,
    SendGifs = 1 << 4,
    SendGames = 1 << 5,
    SendInline = 1 << 6,
    SendPolls = 1 << 8,
    SendPhotos = 1 << 19,
    SendVideos = 1 << 20,
    SendVideoMessages = 1 << 21,
    SendMusic = 1 << 22,
    SendVoiceMessages = 1 << 23,
    SendFiles = 1 << 24,
    SendOther = 1 << 25,
    EmbedLinks = 1 << 7,
    ChangeInfo = 1 << 10,
    AddParticipants = 1 << 15,
    PinMessages = 1 << 17,
    CreateTopics = 1 << 18,
    EditRank = 1 << 26,
    SendReactions = 1 << 27,
    ManageLinkedPeers = 1 << 28,
}
tg_base::flag_type!(ChatRestriction:i32);
pub type ChatRestrictions = Flags<ChatRestriction>;

use tg_scheme as tl;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatAdminRightsInfo {
    pub flags: ChatAdminRights,
}
impl ChatAdminRightsInfo {
    pub fn from_mtp(value: &tl::ChatAdminRights) -> Self {
        let tl::ChatAdminRights::ChatAdminRights(d) = value;
        let mut flags = ChatAdminRights::empty();
        if d.is_change_info() {
            flags |= ChatAdminRight::ChangeInfo;
        }
        if d.is_post_messages() {
            flags |= ChatAdminRight::PostMessages;
        }
        if d.is_edit_messages() {
            flags |= ChatAdminRight::EditMessages;
        }
        if d.is_delete_messages() {
            flags |= ChatAdminRight::DeleteMessages;
        }
        if d.is_ban_users() {
            flags |= ChatAdminRight::BanUsers;
        }
        if d.is_invite_users() {
            flags |= ChatAdminRight::InviteByLinkOrAdd;
        }
        if d.is_pin_messages() {
            flags |= ChatAdminRight::PinMessages;
        }
        if d.is_add_admins() {
            flags |= ChatAdminRight::AddAdmins;
        }
        if d.is_anonymous() {
            flags |= ChatAdminRight::Anonymous;
        }
        if d.is_manage_call() {
            flags |= ChatAdminRight::ManageCall;
        }
        if d.is_other() {
            flags |= ChatAdminRight::Other;
        }
        if d.is_manage_topics() {
            flags |= ChatAdminRight::ManageTopics;
        }
        if d.is_post_stories() {
            flags |= ChatAdminRight::PostStories;
        }
        if d.is_edit_stories() {
            flags |= ChatAdminRight::EditStories;
        }
        if d.is_delete_stories() {
            flags |= ChatAdminRight::DeleteStories;
        }
        if d.is_manage_direct_messages() {
            flags |= ChatAdminRight::ManageDirect;
        }
        if d.is_manage_ranks() {
            flags |= ChatAdminRight::ManageRanks;
        }
        if d.is_manage_linked_peers() {
            flags |= ChatAdminRight::ManageLinkedPeers;
        }
        if d.is_manage_welcome_messages() {
            flags |= ChatAdminRight::ManageWelcomeMessages;
        }
        Self { flags }
    }
}
pub fn admin_rights_to_mtp(info: ChatAdminRightsInfo) -> tl::ChatAdminRights {
    let mut flags = tl::tl::Flags::<tl::ChatAdminRightsData>::empty();
    if (info.flags & ChatAdminRight::ChangeInfo).any() {
        flags |= tl::ChatAdminRightsData::F_CHANGE_INFO;
    }
    if (info.flags & ChatAdminRight::PostMessages).any() {
        flags |= tl::ChatAdminRightsData::F_POST_MESSAGES;
    }
    if (info.flags & ChatAdminRight::EditMessages).any() {
        flags |= tl::ChatAdminRightsData::F_EDIT_MESSAGES;
    }
    if (info.flags & ChatAdminRight::DeleteMessages).any() {
        flags |= tl::ChatAdminRightsData::F_DELETE_MESSAGES;
    }
    if (info.flags & ChatAdminRight::BanUsers).any() {
        flags |= tl::ChatAdminRightsData::F_BAN_USERS;
    }
    if (info.flags & ChatAdminRight::InviteByLinkOrAdd).any() {
        flags |= tl::ChatAdminRightsData::F_INVITE_USERS;
    }
    if (info.flags & ChatAdminRight::PinMessages).any() {
        flags |= tl::ChatAdminRightsData::F_PIN_MESSAGES;
    }
    if (info.flags & ChatAdminRight::AddAdmins).any() {
        flags |= tl::ChatAdminRightsData::F_ADD_ADMINS;
    }
    if (info.flags & ChatAdminRight::Anonymous).any() {
        flags |= tl::ChatAdminRightsData::F_ANONYMOUS;
    }
    if (info.flags & ChatAdminRight::ManageCall).any() {
        flags |= tl::ChatAdminRightsData::F_MANAGE_CALL;
    }
    if (info.flags & ChatAdminRight::Other).any() {
        flags |= tl::ChatAdminRightsData::F_OTHER;
    }
    if (info.flags & ChatAdminRight::ManageTopics).any() {
        flags |= tl::ChatAdminRightsData::F_MANAGE_TOPICS;
    }
    if (info.flags & ChatAdminRight::PostStories).any() {
        flags |= tl::ChatAdminRightsData::F_POST_STORIES;
    }
    if (info.flags & ChatAdminRight::EditStories).any() {
        flags |= tl::ChatAdminRightsData::F_EDIT_STORIES;
    }
    if (info.flags & ChatAdminRight::DeleteStories).any() {
        flags |= tl::ChatAdminRightsData::F_DELETE_STORIES;
    }
    if (info.flags & ChatAdminRight::ManageDirect).any() {
        flags |= tl::ChatAdminRightsData::F_MANAGE_DIRECT_MESSAGES;
    }
    if (info.flags & ChatAdminRight::ManageRanks).any() {
        flags |= tl::ChatAdminRightsData::F_MANAGE_RANKS;
    }
    if (info.flags & ChatAdminRight::ManageLinkedPeers).any() {
        flags |= tl::ChatAdminRightsData::F_MANAGE_LINKED_PEERS;
    }
    if (info.flags & ChatAdminRight::ManageWelcomeMessages).any() {
        flags |= tl::ChatAdminRightsData::F_MANAGE_WELCOME_MESSAGES;
    }
    tl::mtp_chat_admin_rights(flags)
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatRestrictionsInfo {
    pub flags: ChatRestrictions,
    pub until: i32,
}
impl ChatRestrictionsInfo {
    pub fn from_mtp(value: &tl::ChatBannedRights) -> Self {
        let tl::ChatBannedRights::ChatBannedRights(d) = value;
        let mut flags = ChatRestrictions::empty();
        if d.is_view_messages() {
            flags |= ChatRestriction::ViewMessages;
        }
        if d.is_send_stickers() {
            flags |= ChatRestriction::SendStickers;
        }
        if d.is_send_gifs() {
            flags |= ChatRestriction::SendGifs;
        }
        if d.is_send_games() {
            flags |= ChatRestriction::SendGames;
        }
        if d.is_send_inline() {
            flags |= ChatRestriction::SendInline;
        }
        if d.is_send_polls() {
            flags |= ChatRestriction::SendPolls;
        }
        if d.is_send_photos() {
            flags |= ChatRestriction::SendPhotos;
        }
        if d.is_send_videos() {
            flags |= ChatRestriction::SendVideos;
        }
        if d.is_send_roundvideos() {
            flags |= ChatRestriction::SendVideoMessages;
        }
        if d.is_send_audios() {
            flags |= ChatRestriction::SendMusic;
        }
        if d.is_send_voices() {
            flags |= ChatRestriction::SendVoiceMessages;
        }
        if d.is_send_docs() {
            flags |= ChatRestriction::SendFiles;
        }
        if d.is_send_plain() {
            flags |= ChatRestriction::SendOther;
        }
        if d.is_embed_links() {
            flags |= ChatRestriction::EmbedLinks;
        }
        if d.is_send_reactions() {
            flags |= ChatRestriction::SendReactions;
        }
        if d.is_change_info() {
            flags |= ChatRestriction::ChangeInfo;
        }
        if d.is_invite_users() {
            flags |= ChatRestriction::AddParticipants;
        }
        if d.is_pin_messages() {
            flags |= ChatRestriction::PinMessages;
        }
        if d.is_manage_topics() {
            flags |= ChatRestriction::CreateTopics;
        }
        if d.is_edit_rank() {
            flags |= ChatRestriction::EditRank;
        }
        if d.is_manage_linked_peers() {
            flags |= ChatRestriction::ManageLinkedPeers;
        }
        Self {
            flags,
            until: d.until_date,
        }
    }
}
pub fn restrictions_to_mtp(info: ChatRestrictionsInfo) -> tl::ChatBannedRights {
    let mut flags = tl::tl::Flags::<tl::ChatBannedRightsData>::empty();
    if (info.flags & ChatRestriction::ViewMessages).any() {
        flags |= tl::ChatBannedRightsData::F_VIEW_MESSAGES;
    }
    if (info.flags & ChatRestriction::SendStickers).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_STICKERS;
    }
    if (info.flags & ChatRestriction::SendGifs).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_GIFS;
    }
    if (info.flags & ChatRestriction::SendGames).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_GAMES;
    }
    if (info.flags & ChatRestriction::SendInline).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_INLINE;
    }
    if (info.flags & ChatRestriction::SendPolls).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_POLLS;
    }
    if (info.flags & ChatRestriction::SendPhotos).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_PHOTOS;
    }
    if (info.flags & ChatRestriction::SendVideos).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_VIDEOS;
    }
    if (info.flags & ChatRestriction::SendVideoMessages).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_ROUNDVIDEOS;
    }
    if (info.flags & ChatRestriction::SendMusic).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_AUDIOS;
    }
    if (info.flags & ChatRestriction::SendVoiceMessages).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_VOICES;
    }
    if (info.flags & ChatRestriction::SendFiles).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_DOCS;
    }
    if (info.flags & ChatRestriction::SendOther).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_PLAIN;
    }
    if (info.flags & ChatRestriction::EmbedLinks).any() {
        flags |= tl::ChatBannedRightsData::F_EMBED_LINKS;
    }
    if (info.flags & ChatRestriction::SendReactions).any() {
        flags |= tl::ChatBannedRightsData::F_SEND_REACTIONS;
    }
    if (info.flags & ChatRestriction::ChangeInfo).any() {
        flags |= tl::ChatBannedRightsData::F_CHANGE_INFO;
    }
    if (info.flags & ChatRestriction::AddParticipants).any() {
        flags |= tl::ChatBannedRightsData::F_INVITE_USERS;
    }
    if (info.flags & ChatRestriction::PinMessages).any() {
        flags |= tl::ChatBannedRightsData::F_PIN_MESSAGES;
    }
    if (info.flags & ChatRestriction::CreateTopics).any() {
        flags |= tl::ChatBannedRightsData::F_MANAGE_TOPICS;
    }
    if (info.flags & ChatRestriction::EditRank).any() {
        flags |= tl::ChatBannedRightsData::F_EDIT_RANK;
    }
    if (info.flags & ChatRestriction::ManageLinkedPeers).any() {
        flags |= tl::ChatBannedRightsData::F_MANAGE_LINKED_PEERS;
    }
    tl::mtp_chat_banned_rights(flags, info.until)
}
pub const fn all_send_restrictions_list() -> [ChatRestriction; 12] {
    [
        ChatRestriction::SendOther,
        ChatRestriction::SendStickers,
        ChatRestriction::SendGifs,
        ChatRestriction::SendGames,
        ChatRestriction::SendInline,
        ChatRestriction::SendPolls,
        ChatRestriction::SendPhotos,
        ChatRestriction::SendVideos,
        ChatRestriction::SendVideoMessages,
        ChatRestriction::SendMusic,
        ChatRestriction::SendVoiceMessages,
        ChatRestriction::SendFiles,
    ]
}
pub fn all_send_restrictions() -> ChatRestrictions {
    all_send_restrictions_list()
        .into_iter()
        .fold(ChatRestrictions::empty(), |r, f| r | f)
}
pub const fn files_send_restrictions_list() -> [ChatRestriction; 6] {
    [
        ChatRestriction::SendStickers,
        ChatRestriction::SendGifs,
        ChatRestriction::SendPhotos,
        ChatRestriction::SendVideos,
        ChatRestriction::SendMusic,
        ChatRestriction::SendFiles,
    ]
}
pub fn files_send_restrictions() -> ChatRestrictions {
    files_send_restrictions_list()
        .into_iter()
        .fold(ChatRestrictions::empty(), |r, f| r | f)
}
pub const fn tabbed_panel_send_restrictions_list() -> [ChatRestriction; 3] {
    [
        ChatRestriction::SendStickers,
        ChatRestriction::SendGifs,
        ChatRestriction::SendOther,
    ]
}
pub fn tabbed_panel_send_restrictions() -> ChatRestrictions {
    tabbed_panel_send_restrictions_list()
        .into_iter()
        .fold(ChatRestrictions::empty(), |r, f| r | f)
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdminRightsSetOptions {
    pub is_group: bool,
    pub is_forum: bool,
    pub is_community: bool,
    pub anyone_can_add_members: bool,
    pub can_process_join_requests: bool,
    pub is_bot: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RestrictionsSetOptions {
    pub is_forum: bool,
    pub is_user_specific: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SendError {
    pub text: String,
    pub boosts_to_lift: i32,
    pub monoforum_admin: bool,
    pub premium_to_lift: bool,
    pub frozen: bool,
}
impl SendError {
    pub fn is_present(&self) -> bool {
        self.monoforum_admin || !self.text.is_empty()
    }
    pub fn has_value(&self) -> bool {
        !self.text.is_empty()
    }
    pub fn value_or(self, other: Self) -> Self {
        if self.is_present() {
            self
        } else {
            other
        }
    }
}

/// Flags-only projection of RestrictionLabels: labels themselves remain UI/lang.
pub fn list_of_restrictions(options: RestrictionsSetOptions) -> Vec<ChatRestrictions> {
    use ChatRestriction as R;
    let mut r = vec![
        R::SendOther.into(),
        R::SendPhotos.into(),
        R::SendVideos.into(),
        R::SendVideoMessages.into(),
        R::SendMusic.into(),
        R::SendVoiceMessages.into(),
        R::SendFiles.into(),
        R::SendStickers | R::SendGifs | R::SendGames | R::SendInline,
        R::EmbedLinks.into(),
        R::SendPolls.into(),
        R::SendReactions.into(),
        R::AddParticipants.into(),
    ];
    if options.is_forum {
        r.push(R::CreateTopics.into());
    }
    r.extend([R::PinMessages, R::EditRank, R::ChangeInfo].map(ChatRestrictions::from));
    r
}
/// Actual session/Ayu application facts needed by CanSendAnyOf, no default owner.
pub struct SendContext {
    pub forwarding: bool,
    pub frozen: bool,
    pub session_premium: bool,
    pub production: bool,
    pub user_inaccessible: bool,
}
pub fn can_send_any_of(
    peer: &super::data_peer::PeerData,
    fields: &super::data_peer_fields::PeerFields<'_>,
    rights: ChatRestrictions,
    forbid_in_forums: bool,
    ctx: &SendContext,
) -> bool {
    use super::{
        data_channel::ChannelDataFlag as G, data_chat::ChatDataFlag as C,
        data_peer_fields::PeerFields, data_user::UserDataFlag as U,
    };
    assert_eq!(peer.key(), fields.key());
    if ctx.forwarding {
        return false;
    }
    if ctx.frozen && !peer.is_freeze_appeal_chat(fields) {
        return false;
    }
    match fields {
        PeerFields::User(v) => {
            if ctx.user_inaccessible
                || peer.is_replies_chat(ctx.production)
                || peer.is_verify_codes()
            {
                return false;
            }
            if !peer.is_self(fields)
                && (v.flags.current() & U::RequiresPremiumToWrite).any()
                && !ctx.session_premium
            {
                return false;
            }
            let special = ChatRestriction::SendVoiceMessages
                | ChatRestriction::SendVideoMessages
                | ChatRestriction::SendPolls;
            if (rights & !special).any() {
                return true;
            }
            [
                ChatRestriction::SendVoiceMessages,
                ChatRestriction::SendVideoMessages,
                ChatRestriction::SendPolls,
            ]
            .into_iter()
            .any(|r| {
                (rights & r).any()
                    && !peer
                        .am_restricted(fields, r, ctx.session_premium)
                        .restricted()
            })
        }
        PeerFields::Chat(v) => {
            let flags = v.flags.current();
            (flags & (C::Forbidden | C::Deactivated | C::Left)).is_empty()
                && (fields.is_creator()
                    || v.admin_rights.any()
                    || (rights & !v.default_restrictions).any())
        }
        PeerFields::Channel(v) => {
            let flags = v.flags.current();
            if (flags & G::MonoforumDisabled).any() {
                return false;
            }
            let am_in = (flags & (G::Forbidden | G::Left | G::Community)).is_empty();
            let allowed = am_in
                || ((flags & G::HasLink).any() && (flags & G::JoinToWrite).is_empty())
                || (flags & G::Monoforum).any();
            if !allowed || (forbid_in_forums && (flags & G::Forum).any()) {
                return false;
            }
            let boosts = v.has_megagroup_info
                && v.boosts_unrestrict > 0
                && v.boosts_applied >= v.boosts_unrestrict;
            let restricted = v.restrictions
                | if boosts {
                    ChatRestrictions::empty()
                } else {
                    v.default_restrictions
                };
            v.can_post_messages
                || ((flags & G::Broadcast).is_empty()
                    && (v.admin_rights.any() || (rights & !restricted).any()))
        }
    }
}
pub fn can_send(
    peer: &super::data_peer::PeerData,
    fields: &super::data_peer_fields::PeerFields<'_>,
    right: ChatRestriction,
    forbid_in_forums: bool,
    ctx: &SendContext,
) -> bool {
    can_send_any_of(peer, fields, right.into(), forbid_in_forums, ctx)
}
pub fn can_send_texts(
    peer: &super::data_peer::PeerData,
    fields: &super::data_peer_fields::PeerFields<'_>,
    forbid_in_forums: bool,
    ctx: &SendContext,
) -> bool {
    can_send(
        peer,
        fields,
        ChatRestriction::SendOther,
        forbid_in_forums,
        ctx,
    )
}
pub fn can_send_anything(
    peer: &super::data_peer::PeerData,
    fields: &super::data_peer_fields::PeerFields<'_>,
    forbid_in_forums: bool,
    ctx: &SendContext,
) -> bool {
    can_send_any_of(peer, fields, all_send_restrictions(), forbid_in_forums, ctx)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SendErrorArgs {
    pub text: String,
    pub boosts_to_lift: i32,
    pub monoforum_admin: bool,
    pub premium_to_lift: bool,
    pub frozen: bool,
}
impl SendError {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
    pub fn from_args(args: SendErrorArgs) -> Self {
        Self {
            text: args.text,
            boosts_to_lift: args.boosts_to_lift,
            monoforum_admin: args.monoforum_admin,
            premium_to_lift: args.premium_to_lift,
            frozen: args.frozen,
        }
    }
}
impl std::ops::Deref for SendError {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}
