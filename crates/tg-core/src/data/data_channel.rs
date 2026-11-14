//! Real channel model fields and predicates; mutations live on the sole Peers owner.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelDataFlag {
    Left = 1 << 0,
    Creator = 1 << 1,
    Forbidden = 1 << 2,
    CallActive = 1 << 3,
    CallNotEmpty = 1 << 4,
    Signatures = 1 << 5,
    Verified = 1 << 6,
    Scam = 1 << 7,
    Fake = 1 << 8,
    Megagroup = 1 << 9,
    Broadcast = 1 << 10,
    Gigagroup = 1 << 11,
    Username = 1 << 12,
    Location = 1 << 13,
    CanSetUsername = 1 << 14,
    CanSetStickers = 1 << 15,
    PreHistoryHidden = 1 << 16,
    CanViewParticipants = 1 << 17,
    HasLink = 1 << 18,
    SlowmodeEnabled = 1 << 19,
    NoForwards = 1 << 20,
    JoinToWrite = 1 << 21,
    RequestToJoin = 1 << 22,
    Forum = 1 << 23,
    AntiSpam = 1 << 24,
    ParticipantsHidden = 1 << 25,
    StoriesHidden = 1 << 26,
    HasActiveStories = 1 << 27,
    HasUnreadStories = 1 << 28,
    CanGetStatistics = 1 << 29,
    ViewAsMessages = 1 << 30,
    SimilarExpanded = 1 << 31,
    CanViewRevenue = 1 << 32,
    PaidMediaAllowed = 1 << 33,
    CanViewCreditsRevenue = 1 << 34,
    SignatureProfiles = 1 << 35,
    StargiftsAvailable = 1 << 36,
    PaidMessagesAvailable = 1 << 37,
    AutoTranslation = 1 << 38,
    Monoforum = 1 << 39,
    MonoforumAdmin = 1 << 40,
    MonoforumDisabled = 1 << 41,
    ForumTabs = 1 << 42,
    HasStarsPerMessage = 1 << 43,
    StarsPerMessageKnown = 1 << 44,
    HasActiveVideoStream = 1 << 45,
    Community = 1 << 46,
    CommunityCollapsed = 1 << 47,
    HasWelcomeMessages = 1 << 48,
    AyuNoForwards = 1 << 63,
}
tg_base::flag_type!(ChannelDataFlag: u64);
pub type ChannelDataFlags = tg_base::flags::Flags<ChannelDataFlag>;

use super::{
    data_chat_participant_status::*, data_flags::Flags as ReactiveFlags,
    data_location::LocationPoint, data_msg_id::MsgId, data_peer::*, data_peer_bot_commands::*,
    data_peer_fields::*, data_peer_id::*, data_store_keys::*, data_types::StickerSetIdentifier,
    data_user_names::*,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Clone, Debug, Default)]
pub struct ChannelLocation {
    pub address: String,
    pub point: LocationPoint,
}
impl PartialEq for ChannelLocation {
    fn eq(&self, o: &Self) -> bool {
        if self.address.is_empty() {
            o.address.is_empty()
        } else {
            self.address == o.address && self.point == o.point
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Admin {
    pub rights: ChatAdminRightsInfo,
    pub can_edit: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Restricted {
    pub rights: ChatRestrictionsInfo,
}
pub struct MegagroupInfo {
    pub last_participants: VecDeque<UserKey>,
    pub last_admins: BTreeMap<UserKey, Admin>,
    pub last_restricted: BTreeMap<UserKey, Restricted>,
    pub markup_senders: BTreeSet<PeerKey>,
    pub bots: BTreeSet<UserKey>,
    pub admins: BTreeSet<UserId>,
    pub member_ranks: BTreeMap<UserId, String>,
    pub creator: Option<UserKey>,
    pub bot_status: BotStatus,
    pub joined_message_found: bool,
    pub admins_loaded: bool,
    pub sticker_set: StickerSetIdentifier,
    pub emoji_set: StickerSetIdentifier,
    pub last_participants_status: i32,
    pub last_participants_count: i32,
    pub boosts_applied: i32,
    pub boosts_unrestrict: i32,
    pub slowmode_seconds: i32,
    pub slowmode_last_message: i32,
    pub(crate) migrated_from: Option<ChatKey>,
    pub(crate) location: ChannelLocation,
    pub(crate) bot_commands: ChatBotCommands,
    /// Owner keys for the future real Forum/SavedMessages stores; no fake records.
    pub(crate) forum: Option<PeerKey>,
    pub(crate) monoforum: Option<PeerKey>,
    pub(crate) boosts_changes: std::rc::Rc<tg_rpl::EventStream<bool>>,
}
impl Default for MegagroupInfo {
    fn default() -> Self {
        Self {
            last_participants: VecDeque::new(),
            last_admins: BTreeMap::new(),
            last_restricted: BTreeMap::new(),
            markup_senders: BTreeSet::new(),
            bots: BTreeSet::new(),
            admins: BTreeSet::new(),
            member_ranks: BTreeMap::new(),
            creator: None,
            bot_status: BotStatus::Unknown,
            joined_message_found: false,
            admins_loaded: false,
            sticker_set: Default::default(),
            emoji_set: Default::default(),
            last_participants_status: 0,
            last_participants_count: 0,
            boosts_applied: 0,
            boosts_unrestrict: 0,
            slowmode_seconds: 0,
            slowmode_last_message: 0,
            migrated_from: None,
            location: Default::default(),
            bot_commands: Default::default(),
            forum: None,
            monoforum: None,
            boosts_changes: std::rc::Rc::new(tg_rpl::EventStream::new()),
        }
    }
}
impl MegagroupInfo {
    pub const LAST_PARTICIPANTS_UP_TO_DATE: i32 = 0;
    pub const LAST_PARTICIPANTS_ONCE_RECEIVED: i32 = 1;
    pub const LAST_PARTICIPANTS_COUNT_OUTDATED: i32 = 2;
    pub fn get_migrate_from_chat(&self) -> Option<ChatKey> {
        self.migrated_from
    }
    pub fn set_migrate_from_chat(&mut self, chat: Option<ChatKey>) {
        self.migrated_from = chat;
    }
    pub fn get_location(&self) -> Option<&ChannelLocation> {
        (!self.location.address.is_empty()).then_some(&self.location)
    }
    pub fn set_location(&mut self, location: ChannelLocation) {
        self.location = location;
    }
    pub fn set_bot_commands(&mut self, list: &[BotCommands]) -> bool {
        self.bot_commands.update(list)
    }
    pub fn bot_commands(&self) -> &ChatBotCommands {
        &self.bot_commands
    }
    pub fn forum(&self) -> Option<PeerKey> {
        self.forum
    }
    pub fn monoforum(&self) -> Option<PeerKey> {
        self.monoforum
    }
}
pub struct ChannelData {
    pub peer: PeerData,
    pub date: i32,
    pub mg_info: Option<MegagroupInfo>,
    pub inviter: UserId,
    pub invite_date: i32,
    pub invite_via_request: bool,
    pub(crate) flags: ReactiveFlags<ChannelDataFlag>,
    pub(crate) username: UsernamesInfo,
    pub(crate) recent_requesters: Vec<UserId>,
    pub(crate) available_min_id: MsgId,
    pub(crate) access_hash: u64,
    pub(crate) default_restrictions: ReactiveFlags<ChatRestriction>,
    pub(crate) admin_rights: ReactiveFlags<ChatAdminRight>,
    pub(crate) restrictions: ReactiveFlags<ChatRestriction>,
    pub(crate) restricted_until: i32,
    pub(crate) subscription_until_date: i32,
    pub(crate) unavailable_reasons: Vec<UnavailableReason>,
    pub(crate) invite_peek: Option<(String, i32)>,
    pub(crate) invite_link: String,
    pub(crate) discussion_link: Option<ChannelKey>,
    pub(crate) monoforum_link: Option<ChannelKey>,
    pub(crate) linked_community_id: ChannelId,
    pub(crate) community_owner: Option<ChannelKey>,
    pub(crate) discussion_link_known: bool,
    pub(crate) peer_gifts_count: i32,
    pub(crate) members_count: i32,
    pub(crate) admins_count: i32,
    pub(crate) restricted_count: i32,
    pub(crate) kicked_count: i32,
    pub(crate) pending_requests_count: i32,
    pub(crate) level_hint: i32,
    pub(crate) stars_per_message: i32,
    pub(crate) guard_bot_id: UserId,
    pub(crate) allowed_reactions: AllowedReactions,
    pub(crate) call: Option<GroupCallKey>,
    pub(crate) call_default_join_as: PeerId,
    // Real PtsWaiter is a separate model in package12; no fabricated sequence state.
}
impl ChannelData {
    pub(crate) fn new(peer: PeerData) -> Self {
        assert!(peer.is_channel());
        Self {
            peer,
            date: 0,
            mg_info: None,
            inviter: UserId::new(0),
            invite_date: 0,
            invite_via_request: false,
            flags: ReactiveFlags::new(ChannelDataFlag::Forbidden.into()),
            username: UsernamesInfo::new(),
            recent_requesters: Vec::new(),
            available_min_id: MsgId::new(0),
            access_hash: 0,
            default_restrictions: ReactiveFlags::default(),
            admin_rights: ReactiveFlags::default(),
            restrictions: ReactiveFlags::default(),
            restricted_until: 0,
            subscription_until_date: 0,
            unavailable_reasons: Vec::new(),
            invite_peek: None,
            invite_link: String::new(),
            discussion_link: None,
            monoforum_link: None,
            linked_community_id: ChannelId::new(0),
            community_owner: None,
            discussion_link_known: false,
            peer_gifts_count: 0,
            members_count: -1,
            admins_count: 1,
            restricted_count: 0,
            kicked_count: 0,
            pending_requests_count: 0,
            level_hint: 0,
            stars_per_message: 0,
            guard_bot_id: UserId::new(0),
            allowed_reactions: AllowedReactions::default(),
            call: None,
            call_default_join_as: PeerId::new(0),
        }
    }
    pub fn key(&self) -> ChannelKey {
        ChannelKey::try_from(self.peer.key()).unwrap()
    }
    pub fn flags(&self) -> ChannelDataFlags {
        self.flags.current()
    }
    pub fn flags_value(&self) -> tg_rpl::Producer<super::data_flags::Change<ChannelDataFlag>> {
        self.flags.value()
    }
    pub fn admin_rights(&self) -> ChatAdminRights {
        self.admin_rights.current()
    }
    pub fn admin_rights_value(
        &self,
    ) -> tg_rpl::Producer<super::data_flags::Change<ChatAdminRight>> {
        self.admin_rights.value()
    }
    pub fn restrictions(&self) -> ChatRestrictions {
        self.restrictions.current()
    }
    pub fn restrictions_value(
        &self,
    ) -> tg_rpl::Producer<super::data_flags::Change<ChatRestriction>> {
        self.restrictions.value()
    }
    pub fn default_restrictions(&self) -> ChatRestrictions {
        self.default_restrictions.current()
    }
    pub fn default_restrictions_value(
        &self,
    ) -> tg_rpl::Producer<super::data_flags::Change<ChatRestriction>> {
        self.default_restrictions.value()
    }
    pub fn has_admin_rights(&self) -> bool {
        self.admin_rights().any()
    }
    pub fn has_restrictions(&self) -> bool {
        self.restrictions().any()
    }
    pub fn has_restrictions_at(&self, now: i32) -> bool {
        self.has_restrictions() && self.restricted_until > now
    }
    pub fn restricted_until(&self) -> i32 {
        self.restricted_until
    }
    pub fn am_in(&self) -> bool {
        !self.is_forbidden() && !self.have_left() && !self.is_community()
    }
    pub fn is_public(&self) -> bool {
        self.has_username() || self.has_location()
    }
    pub fn is_restricted_forever(until: i32) -> bool {
        until == 0 || until == i32::MAX
    }
    pub fn members_count(&self) -> i32 {
        self.members_count.max(1)
    }
    pub fn members_count_known(&self) -> bool {
        self.members_count >= 0
    }
    pub fn admins_count(&self) -> i32 {
        self.admins_count
    }
    pub fn restricted_count(&self) -> i32 {
        self.restricted_count
    }
    pub fn kicked_count(&self) -> i32 {
        self.kicked_count
    }
    pub fn pending_requests_count(&self) -> i32 {
        self.pending_requests_count
    }
    pub fn recent_requesters(&self) -> &[UserId] {
        &self.recent_requesters
    }
    pub fn access_hash(&self) -> u64 {
        self.access_hash
    }
    pub fn username(&self) -> &str {
        self.username.username()
    }
    pub fn editable_username(&self) -> &str {
        self.username.editable_username()
    }
    pub fn usernames(&self) -> &[String] {
        self.username.usernames()
    }
    pub fn is_username_editable(&self, name: &str) -> bool {
        self.username.is_editable(name)
    }
    pub fn peer_fields(&self) -> PeerFields<'_> {
        PeerFields::Channel(ChannelPeerFields {
            key: self.key(),
            flags: &self.flags,
            usernames: &self.username,
            admin_rights: self.admin_rights(),
            default_restrictions: self.default_restrictions(),
            restrictions: self.restrictions(),
            boosts_unrestrict: self.boosts_unrestrict(),
            boosts_applied: self.boosts_applied(),
            slowmode_seconds: self.slowmode_seconds(),
            slowmode_last_message: self.mg_info.as_ref().map_or(0, |i| i.slowmode_last_message),
            has_megagroup_info: self.mg_info.is_some(),
            inviter: self.inviter,
            stars_per_message: self.stars_per_message,
            peer_gifts_count: self.peer_gifts_count,
            can_post_messages: self.can_post_messages(),
            can_edit_messages: self.can_edit_messages(),
            can_pin_messages: self.can_pin_messages(),
            can_post_stories: self.can_post_stories(),
            can_edit_stories: self.can_edit_stories(),
            can_delete_stories: self.can_delete_stories(),
            can_delete: self.can_delete(),
            members_count_known: self.members_count_known(),
            migrated_from: self.get_migrate_from_chat(),
            monoforum_link: self.monoforum_link,
            forum: self.forum(),
            monoforum: self.monoforum(),
            group_call: self.call,
            group_call_default_join_as: self.call_default_join_as,
        })
    }
    pub fn am_restricted(&self, right: ChatRestriction) -> RestrictionCheckResult {
        self.peer.am_restricted(&self.peer_fields(), right, false)
    }
    pub fn can_ban_members(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::BanUsers).any()
    }
    pub fn can_post_messages(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::PostMessages).any()
    }
    pub fn can_edit_messages(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::EditMessages).any()
    }
    pub fn can_delete_messages(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::DeleteMessages).any()
    }
    pub fn can_post_stories(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::PostStories).any()
    }
    pub fn can_edit_stories(&self) -> bool {
        !self.is_monoforum()
            && (self.am_creator() || (self.admin_rights() & ChatAdminRight::EditStories).any())
    }
    pub fn can_delete_stories(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::DeleteStories).any()
    }
    pub fn can_access_monoforum(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::ManageDirect).any()
    }
    pub fn can_post_paid_media(&self) -> bool {
        self.can_post_messages() && (self.flags() & ChannelDataFlag::PaidMediaAllowed).any()
    }
    pub fn can_have_invite_link(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::InviteByLinkOrAdd).any()
    }
    pub fn can_manage_linked_peers(&self) -> bool {
        self.is_community()
            && (self.am_creator()
                || (self.admin_rights() & ChatAdminRight::ManageLinkedPeers).any())
    }
    pub fn community_anyone_can_add_peers(&self) -> bool {
        self.is_community()
            && (self.default_restrictions() & ChatRestriction::ManageLinkedPeers).is_empty()
    }
    pub fn anyone_can_add_members(&self) -> bool {
        (self.default_restrictions() & ChatRestriction::AddParticipants).is_empty()
    }
    pub fn can_add_members(&self) -> bool {
        if self.is_monoforum() {
            false
        } else if self.is_megagroup() {
            !self
                .am_restricted(ChatRestriction::AddParticipants)
                .restricted()
        } else {
            self.am_creator() || (self.admin_rights() & ChatAdminRight::InviteByLinkOrAdd).any()
        }
    }
    pub fn can_add_admins(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::AddAdmins).any()
    }
    pub fn is_ayu_no_forwards(&self) -> bool {
        (self.flags() & ChannelDataFlag::AyuNoForwards).any()
    }
    pub fn allows_forwarding(&self) -> bool {
        (self.flags() & ChannelDataFlag::NoForwards).is_empty()
    }
    pub fn can_view_members(&self) -> bool {
        (self.flags() & ChannelDataFlag::CanViewParticipants).any()
            && ((self.flags() & ChannelDataFlag::ParticipantsHidden).is_empty()
                || self.am_creator()
                || self.has_admin_rights())
    }
    pub fn can_view_admins(&self) -> bool {
        self.is_megagroup() || self.has_admin_rights() || self.am_creator()
    }
    pub fn can_view_banned(&self) -> bool {
        self.has_admin_rights() || self.am_creator()
    }
    pub fn can_edit_information(&self) -> bool {
        if self.is_megagroup() {
            !self.am_restricted(ChatRestriction::ChangeInfo).restricted()
        } else {
            self.am_creator() || (self.admin_rights() & ChatAdminRight::ChangeInfo).any()
        }
    }
    pub fn can_edit_permissions(&self) -> bool {
        self.is_megagroup()
            && !self.is_gigagroup()
            && (self.am_creator() || (self.admin_rights() & ChatAdminRight::BanUsers).any())
    }
    pub fn can_edit_signatures(&self) -> bool {
        self.is_broadcast() && self.can_edit_information()
    }
    pub fn can_edit_auto_translate(&self) -> bool {
        self.is_broadcast() && self.can_edit_information()
    }
    pub fn can_edit_pre_history_hidden(&self) -> bool {
        self.is_megagroup()
            && (self.am_creator() || (self.admin_rights() & ChatAdminRight::BanUsers).any())
            && (!self.is_public() || self.can_edit_username())
    }
    pub fn can_edit_username(&self) -> bool {
        self.am_creator() && (self.flags() & ChannelDataFlag::CanSetUsername).any()
    }
    pub fn can_edit_stickers(&self) -> bool {
        (self.flags() & ChannelDataFlag::CanSetStickers).any()
    }
    pub fn can_edit_emoji(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::ChangeInfo).any()
    }
    pub fn can_delete(&self) -> bool {
        self.am_creator()
    }
    pub fn can_edit_last_admin(&self, user: UserKey) -> bool {
        assert_eq!(user.scope(), self.key().scope());
        self.mg_info.as_ref().is_some_and(|i| {
            i.last_admins
                .get(&user)
                .map_or(i.creator != Some(user), |a| a.can_edit)
        })
    }
    pub fn can_edit_admin(&self, user: UserKey, is_self: bool) -> bool {
        assert_eq!(user.scope(), self.key().scope());
        !is_self
            && (self.am_creator()
                || (self.can_edit_last_admin(user)
                    && (self.admin_rights() & ChatAdminRight::AddAdmins).any()))
    }
    pub fn can_restrict_participant(&self, participant: PeerKey, is_self: bool) -> bool {
        assert_eq!(participant.scope(), self.key().scope());
        !is_self
            && (self.am_creator()
                || (UserKey::try_from(participant).map_or(true, |u| self.can_edit_last_admin(u))
                    && (self.admin_rights() & ChatAdminRight::BanUsers).any()))
    }
    pub fn use_subsection_tabs(&self) -> bool {
        self.am_monoforum_admin()
            || (self.is_forum() && (self.flags() & ChannelDataFlag::ForumTabs).any())
    }
    pub fn is_group_admin(&self, user: UserKey) -> bool {
        assert_eq!(user.scope(), self.key().scope());
        self.mg_info
            .as_ref()
            .is_some_and(|i| i.admins.contains(&peer_to_user(user.peer().id())))
    }
    pub fn last_participants_request_needed(&mut self) -> bool {
        if !self.can_view_members() {
            return false;
        }
        let count = self.members_count();
        let Some(i) = self.mg_info.as_mut() else {
            return false;
        };
        if i.last_participants_count == count {
            i.last_participants_status &= !MegagroupInfo::LAST_PARTICIPANTS_COUNT_OUTDATED;
        }
        i.last_participants.is_empty()
            || i.last_participants_status & MegagroupInfo::LAST_PARTICIPANTS_ONCE_RECEIVED == 0
            || i.last_participants_status & MegagroupInfo::LAST_PARTICIPANTS_COUNT_OUTDATED != 0
    }
    pub fn get_migrate_from_chat(&self) -> Option<ChatKey> {
        self.mg_info
            .as_ref()
            .and_then(MegagroupInfo::get_migrate_from_chat)
    }
    pub fn get_location(&self) -> Option<&ChannelLocation> {
        self.mg_info.as_ref().and_then(MegagroupInfo::get_location)
    }
    pub fn forum(&self) -> Option<PeerKey> {
        self.mg_info.as_ref().and_then(MegagroupInfo::forum)
    }
    pub fn monoforum(&self) -> Option<PeerKey> {
        self.mg_info.as_ref().and_then(MegagroupInfo::monoforum)
    }
    pub fn slowmode_seconds(&self) -> i32 {
        self.mg_info.as_ref().map_or(0, |i| i.slowmode_seconds)
    }
    pub fn slowmode_last_message(&self) -> i32 {
        if self.has_admin_rights() || self.am_creator() || self.unrestricted_by_boosts() {
            0
        } else {
            self.mg_info.as_ref().map_or(0, |i| i.slowmode_last_message)
        }
    }
    pub fn boosts_applied(&self) -> i32 {
        self.mg_info.as_ref().map_or(0, |i| i.boosts_applied)
    }
    pub fn boosts_unrestrict(&self) -> i32 {
        self.mg_info.as_ref().map_or(0, |i| i.boosts_unrestrict)
    }
    pub fn unrestricted_by_boosts(&self) -> bool {
        self.boosts_unrestrict() > 0 && self.boosts_applied() >= self.boosts_unrestrict()
    }
    pub fn unrestricted_by_boosts_value(&self) -> tg_rpl::Producer<bool> {
        self.mg_info.as_ref().map_or_else(
            || tg_rpl::single(false),
            |i| {
                i.boosts_changes
                    .events_starting_with(self.unrestricted_by_boosts())
            },
        )
    }
    pub fn stars_per_message(&self) -> i32 {
        self.stars_per_message
    }
    pub fn peer_gifts_count(&self) -> i32 {
        self.peer_gifts_count
    }
    pub fn invite_peek_expires(&self) -> i32 {
        self.invite_peek.as_ref().map_or(0, |i| i.1)
    }
    pub fn invite_peek_hash(&self) -> &str {
        self.invite_peek.as_ref().map_or("", |i| i.0.as_str())
    }
    pub fn invite_link(&self) -> &str {
        &self.invite_link
    }
    pub fn discussion_link(&self) -> Option<ChannelKey> {
        self.discussion_link
    }
    pub fn discussion_link_known(&self) -> bool {
        self.discussion_link_known
    }
    pub fn monoforum_link(&self) -> Option<ChannelKey> {
        self.monoforum_link
    }
    pub fn linked_community_id(&self) -> ChannelId {
        self.linked_community_id
    }
    pub fn community_info(&self) -> Option<ChannelKey> {
        self.community_owner
    }
    pub fn available_min_id(&self) -> MsgId {
        self.available_min_id
    }
    pub fn group_call(&self) -> Option<GroupCallKey> {
        self.call
    }
    pub fn group_call_default_join_as(&self) -> PeerId {
        self.call_default_join_as
    }
    pub fn allowed_reactions(&self) -> &AllowedReactions {
        &self.allowed_reactions
    }
    pub fn level_hint(&self) -> i32 {
        self.level_hint
    }
    pub fn subscription_until_date(&self) -> i32 {
        self.subscription_until_date
    }
    pub fn guard_bot_id(&self) -> UserId {
        self.guard_bot_id
    }
    pub fn unavailable_reasons(&self) -> &[UnavailableReason] {
        &self.unavailable_reasons
    }
    pub fn kicked_restricted_rights(is_user: bool) -> ChatRestrictionsInfo {
        use ChatRestriction as R;
        ChatRestrictionsInfo {
            flags: if is_user {
                R::ViewMessages
                    | R::SendStickers
                    | R::SendGifs
                    | R::SendGames
                    | R::SendInline
                    | R::SendPhotos
                    | R::SendVideos
                    | R::SendVideoMessages
                    | R::SendMusic
                    | R::SendVoiceMessages
                    | R::SendFiles
                    | R::SendOther
                    | R::EmbedLinks
            } else {
                R::ViewMessages.into()
            },
            until: i32::MAX,
        }
    }
}

impl ChannelData {
    pub fn have_left(&self) -> bool {
        (self.flags() & ChannelDataFlag::Left).any()
    }
    pub fn adds_signature(&self) -> bool {
        (self.flags() & ChannelDataFlag::Signatures).any()
    }
    pub fn signature_profiles(&self) -> bool {
        (self.flags() & ChannelDataFlag::SignatureProfiles).any()
    }
    pub fn is_forbidden(&self) -> bool {
        (self.flags() & ChannelDataFlag::Forbidden).any()
    }
    pub fn is_verified(&self) -> bool {
        (self.flags() & ChannelDataFlag::Verified).any()
    }
    pub fn is_scam(&self) -> bool {
        (self.flags() & ChannelDataFlag::Scam).any()
    }
    pub fn is_fake(&self) -> bool {
        (self.flags() & ChannelDataFlag::Fake).any()
    }
    pub fn has_stories_hidden(&self) -> bool {
        (self.flags() & ChannelDataFlag::StoriesHidden).any()
    }
    pub fn view_forum_as_messages(&self) -> bool {
        (self.flags() & ChannelDataFlag::ViewAsMessages).any()
    }
    pub fn stargifts_available(&self) -> bool {
        (self.flags() & ChannelDataFlag::StargiftsAvailable).any()
    }
    pub fn paid_messages_available(&self) -> bool {
        (self.flags() & ChannelDataFlag::PaidMessagesAvailable).any()
    }
    pub fn has_stars_per_message(&self) -> bool {
        (self.flags() & ChannelDataFlag::HasStarsPerMessage).any()
    }
    pub fn stars_per_message_known(&self) -> bool {
        (self.flags() & ChannelDataFlag::StarsPerMessageKnown).any()
    }
    pub fn has_welcome_messages(&self) -> bool {
        (self.flags() & ChannelDataFlag::HasWelcomeMessages).any()
    }
    pub fn is_megagroup(&self) -> bool {
        (self.flags() & ChannelDataFlag::Megagroup).any()
    }
    pub fn is_broadcast(&self) -> bool {
        (self.flags() & ChannelDataFlag::Broadcast).any()
    }
    pub fn is_gigagroup(&self) -> bool {
        (self.flags() & ChannelDataFlag::Gigagroup).any()
    }
    pub fn is_forum(&self) -> bool {
        (self.flags() & ChannelDataFlag::Forum).any()
    }
    pub fn is_monoforum(&self) -> bool {
        (self.flags() & ChannelDataFlag::Monoforum).any()
    }
    pub fn is_community(&self) -> bool {
        (self.flags() & ChannelDataFlag::Community).any()
    }
    pub fn collapsed_in_dialogs(&self) -> bool {
        (self.flags() & ChannelDataFlag::CommunityCollapsed).any()
    }
    pub fn has_username(&self) -> bool {
        (self.flags() & ChannelDataFlag::Username).any()
    }
    pub fn has_location(&self) -> bool {
        (self.flags() & ChannelDataFlag::Location).any()
    }
    pub fn am_creator(&self) -> bool {
        (self.flags() & ChannelDataFlag::Creator).any()
    }
    pub fn join_to_write(&self) -> bool {
        (self.flags() & ChannelDataFlag::JoinToWrite).any()
    }
    pub fn request_to_join(&self) -> bool {
        (self.flags() & ChannelDataFlag::RequestToJoin).any()
    }
    pub fn anti_spam_mode(&self) -> bool {
        (self.flags() & ChannelDataFlag::AntiSpam).any()
    }
    pub fn auto_translation(&self) -> bool {
        (self.flags() & ChannelDataFlag::AutoTranslation).any()
    }
    pub fn am_monoforum_admin(&self) -> bool {
        (self.flags() & ChannelDataFlag::MonoforumAdmin).any()
    }
}

impl ChannelData {
    pub fn can_pin_messages(&self) -> bool {
        if !self.is_megagroup() {
            return self.am_creator() || (self.admin_rights() & ChatAdminRight::EditMessages).any();
        }
        if self.monoforum_disabled() {
            return false;
        }
        if self.am_creator() || (self.admin_rights() & ChatAdminRight::PinMessages).any() {
            return true;
        }
        let default =
            (self.default_restrictions() & ChatRestriction::PinMessages).any() || self.is_public();
        (!default || self.unrestricted_by_boosts())
            && (self.restrictions() & ChatRestriction::PinMessages).is_empty()
    }
}
impl ChannelData {
    pub fn monoforum_disabled(&self) -> bool {
        (self.flags() & ChannelDataFlag::MonoforumDisabled).any()
    }
    pub fn input_channel(
        &self,
        from_message: Option<(tg_scheme::InputPeer, MsgId)>,
    ) -> tg_scheme::InputChannel {
        if !self.peer.is_loaded() {
            if let Some((peer, msg)) = from_message {
                return tg_scheme::mtp_input_channel_from_message(
                    peer,
                    msg.bare as i32,
                    peer_to_channel(self.peer.id()).bare,
                );
            }
        }
        tg_scheme::mtp_input_channel(peer_to_channel(self.peer.id()).bare, self.access_hash)
    }
}

impl ChannelData {
    pub fn hidden_pre_history(&self) -> bool {
        (self.flags() & ChannelDataFlag::PreHistoryHidden).any()
    }
    pub fn guard_bot(&self, loaded: impl Fn(UserId) -> Option<UserKey>) -> Option<UserKey> {
        if self.guard_bot_id.is_zero() {
            return None;
        }
        let key = loaded(self.guard_bot_id);
        if let Some(key) = key {
            assert_eq!(key.scope(), self.key().scope());
        }
        key
    }
    pub fn common_stars_per_message(&self, read: impl Fn(PeerKey) -> i32) -> i32 {
        read(self.peer.key())
    }
}

impl ChannelData {
    pub fn has_active_stories(&self) -> bool {
        (self.flags() & ChannelDataFlag::HasActiveStories).any()
    }
    pub fn has_unread_stories(&self) -> bool {
        (self.flags() & ChannelDataFlag::HasUnreadStories).any()
    }
    pub fn has_active_video_stream(&self) -> bool {
        (self.flags() & ChannelDataFlag::HasActiveVideoStream).any()
    }
}
