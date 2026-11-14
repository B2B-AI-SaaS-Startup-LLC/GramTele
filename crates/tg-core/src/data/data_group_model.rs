//! Ordered service boundaries for real chat/channel records. No Session/History stand-ins.
use super::{data_peer::PeerEffect, data_peer_id::*, data_store_keys::*};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug)]
pub struct UserParticipantFields {
    pub key: UserKey,
    pub is_self: bool,
    pub is_bot: bool,
    pub bot_info_inited: bool,
}
/// Required facts from the actual Session/User store, never default model objects.
pub struct GroupContext<'a> {
    pub self_user: UserKey,
    pub now: i32,
    pub now_millis: i64,
    pub filter_zalgo: bool,
    pub theme_facts: &'a dyn Fn(&str, &str) -> (bool, bool),
    pub user_loaded: &'a dyn Fn(UserId) -> Option<UserParticipantFields>,
}
#[derive(Clone)]
pub enum GroupEffect {
    Peer(PeerEffect),
    CloseChatIfStillNotMember(PeerKey),
    UpdateChatListEntry(PeerKey),
    UpdateChatListEntryPostponed(PeerKey),
    ForceHistoryFullResize(PeerKey),
    HistoryFlagsChanged {
        peer: PeerKey,
        diff: u64,
        which: u64,
    },
    CommunityMembershipChanged {
        peer: PeerKey,
        community: ChannelId,
    },
    CommunityFlagsChanged(PeerKey),
    LinkedCommunityChanged(PeerKey),
    EnsureForum(PeerKey),
    DetachForum(PeerKey),
    ClearForumUserpics(PeerKey),
    EnsureMonoforum(PeerKey),
    DetachMonoforum(PeerKey),
    EnsureCommunity(PeerKey),
    ApplyGroupAdminChanges {
        peer: PeerKey,
        users: BTreeSet<UserId>,
    },
    ClearKeyboardIfFrom {
        peer: PeerKey,
        user: PeerKey,
    },
    CheckKeyboardParticipants {
        peer: PeerKey,
        users: BTreeSet<PeerKey>,
    },
    RemoveMegagroupParticipant {
        peer: PeerKey,
        user: UserKey,
    },
    BotCommandsChanged(PeerKey),
    ReactionsEnabled {
        peer: PeerKey,
        enabled: bool,
    },
    LoadPaidReactionShownPeer,
    GroupCallChanged {
        peer: PeerKey,
        old: Option<GroupCallKey>,
        new: Option<GroupCallKey>,
        input: Option<tg_scheme::InputGroupCall>,
        schedule_date: i32,
        rtmp: bool,
    },
    MigrateCall {
        from: PeerKey,
        to: PeerKey,
        call: GroupCallKey,
    },
    ProcessPhoto {
        peer: PeerKey,
        photo: tg_scheme::Photo,
    },
    SetPermanentInvite {
        peer: PeerKey,
        invite: Option<tg_scheme::ExportedChatInvite>,
    },
    SetTopPinned {
        peer: PeerKey,
        message: i32,
    },
    CheckFolder {
        peer: PeerKey,
        folder: i32,
    },
    ApplyTheme {
        peer: PeerKey,
        token: String,
    },
    ApplyNotify {
        peer: PeerKey,
        settings: tg_scheme::PeerNotifySettings,
    },
    CheckTrustedPay(PeerKey),
    ChannelDialogFields {
        peer: PeerKey,
        available_min: Option<i32>,
        folder: i32,
        pts: i32,
        unread: i32,
        inbox: i32,
        outbox: i32,
    },
    ApplyStories {
        peer: PeerKey,
        stories: Option<tg_scheme::PeerStories>,
    },
    BotVerification {
        peer: PeerKey,
        verification: Option<tg_scheme::BotVerification>,
    },
    StatsDc {
        peer: PeerKey,
        dc: i32,
    },
    DefaultSendAs {
        peer: PeerKey,
        send_as: PeerId,
    },
    Wallpaper {
        peer: PeerKey,
        paper: Option<tg_scheme::WallPaper>,
    },
    LoadRevenue {
        peer: PeerKey,
        timeout_ms: i64,
    },
    SuggestGigagroup {
        peer: PeerKey,
        suggest: bool,
    },
    SendHistoryChangeNotifications,
    CommunityLinkedPeers {
        peer: PeerKey,
        peers: Vec<tg_scheme::CommunityPeer>,
    },
    PrivateInviteCheck {
        peer: PeerKey,
        hash: String,
    },
    ProcessForbidden(tg_scheme::Chat),
}
pub(crate) fn changed(peer: PeerKey, flags: super::data_changes::PeerFlags) -> GroupEffect {
    GroupEffect::Peer(PeerEffect::Updated(peer, flags))
}
