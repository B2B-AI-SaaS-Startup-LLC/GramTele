//! Real chat model fields and predicates; mutations live on the sole Peers owner.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatDataFlag {
    Left = 1 << 0,
    Creator = 1 << 2,
    Deactivated = 1 << 3,
    Forbidden = 1 << 4,
    CallActive = 1 << 5,
    CallNotEmpty = 1 << 6,
    CanSetUsername = 1 << 7,
    NoForwards = 1 << 8,
    HasWelcomeMessages = 1 << 9,
    AyuNoForwards = 1 << 31,
}
tg_base::flag_type!(ChatDataFlag: i32);
pub type ChatDataFlags = tg_base::flags::Flags<ChatDataFlag>;

use super::{
    data_chat_participant_status::*, data_flags::Flags as ReactiveFlags, data_peer::*,
    data_peer_bot_commands::*, data_peer_fields::*, data_peer_id::*, data_store_keys::*,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Good,
    TooOld,
    Skipped,
}
/// The real ChatData-specific fields. PeerData is owned once, as the C++ base.
pub struct ChatData {
    pub peer: PeerData,
    pub count: i32,
    pub date: i32,
    pub creator: UserId,
    pub participants: BTreeSet<UserKey>,
    pub invited_by_me: BTreeSet<UserKey>,
    pub admins: BTreeSet<UserKey>,
    pub last_authors: VecDeque<UserKey>,
    pub markup_senders: BTreeSet<PeerKey>,
    pub member_ranks: BTreeMap<UserId, String>,
    pub bot_status: BotStatus,
    _lifetime: tg_rpl::Lifetime,
    pub(crate) flags: ReactiveFlags<ChatDataFlag>,
    pub(crate) admin_rights: ReactiveFlags<ChatAdminRight>,
    pub(crate) default_restrictions: ReactiveFlags<ChatRestriction>,
    pub(crate) version: i32,
    pub(crate) invite_link: String,
    pub(crate) pending_requests_count: i32,
    pub(crate) recent_requesters: Vec<UserId>,
    pub(crate) allowed_reactions: AllowedReactions,
    pub(crate) call: Option<GroupCallKey>,
    pub(crate) call_default_join_as: PeerId,
    pub(crate) bot_commands: ChatBotCommands,
    pub(crate) migrated_to: Option<ChannelKey>,
}
impl ChatData {
    pub(crate) fn new(
        peer: PeerData,
        events: std::rc::Rc<tg_rpl::EventStream<super::data_group_model::GroupEffect>>,
    ) -> Self {
        assert!(peer.is_chat());
        let flags = ReactiveFlags::<ChatDataFlag>::default();
        let lifetime = tg_rpl::Lifetime::new();
        let key = peer.key();
        flags.changes().on_next(
            move |c| {
                if (c.diff & ChatDataFlag::CallNotEmpty).any() {
                    events.fire(super::data_group_model::GroupEffect::UpdateChatListEntry(
                        key,
                    ));
                }
            },
            &lifetime,
        );
        Self {
            peer,
            _lifetime: lifetime,
            count: 0,
            date: 0,
            creator: UserId::new(0),
            participants: BTreeSet::new(),
            invited_by_me: BTreeSet::new(),
            admins: BTreeSet::new(),
            last_authors: VecDeque::new(),
            markup_senders: BTreeSet::new(),
            member_ranks: BTreeMap::new(),
            bot_status: BotStatus::Unknown,
            flags,
            admin_rights: ReactiveFlags::default(),
            default_restrictions: ReactiveFlags::default(),
            version: 0,
            invite_link: String::new(),
            pending_requests_count: 0,
            recent_requesters: Vec::new(),
            allowed_reactions: AllowedReactions::default(),
            call: None,
            call_default_join_as: PeerId::new(0),
            bot_commands: ChatBotCommands::default(),
            migrated_to: None,
        }
    }
    pub fn key(&self) -> ChatKey {
        ChatKey::try_from(self.peer.key()).unwrap()
    }
    pub fn flags(&self) -> ChatDataFlags {
        self.flags.current()
    }
    pub fn flags_value(&self) -> tg_rpl::Producer<super::data_flags::Change<ChatDataFlag>> {
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
    pub fn default_restrictions(&self) -> ChatRestrictions {
        self.default_restrictions.current()
    }
    pub fn default_restrictions_value(
        &self,
    ) -> tg_rpl::Producer<super::data_flags::Change<ChatRestriction>> {
        self.default_restrictions.value()
    }
    pub fn peer_fields(&self) -> PeerFields<'_> {
        PeerFields::Chat(ChatPeerFields {
            key: self.key(),
            flags: &self.flags,
            admin_rights: self.admin_rights(),
            default_restrictions: self.default_restrictions(),
            migrated_to: self.migrated_to,
            group_call: self.call,
            group_call_default_join_as: self.call_default_join_as,
        })
    }
    pub fn has_admin_rights(&self) -> bool {
        self.admin_rights().any()
    }
    pub fn is_forbidden(&self) -> bool {
        (self.flags() & ChatDataFlag::Forbidden).any()
    }
    pub fn have_left(&self) -> bool {
        (self.flags() & ChatDataFlag::Left).any()
    }
    pub fn am_creator(&self) -> bool {
        (self.flags() & ChatDataFlag::Creator).any()
    }
    pub fn is_deactivated(&self) -> bool {
        (self.flags() & ChatDataFlag::Deactivated).any()
    }
    pub fn am_in(&self) -> bool {
        !self.is_forbidden() && !self.is_deactivated() && !self.have_left()
    }
    pub fn is_migrated(&self) -> bool {
        self.migrated_to.is_some()
    }
    pub fn has_welcome_messages(&self) -> bool {
        (self.flags() & ChatDataFlag::HasWelcomeMessages).any()
    }
    pub fn no_participant_info(&self) -> bool {
        (self.count > 0 || self.am_in()) && self.participants.is_empty()
    }
    pub fn am_restricted(&self, right: ChatRestriction) -> RestrictionCheckResult {
        self.peer.am_restricted(&self.peer_fields(), right, false)
    }
    pub fn default_admin_rights(&self, user: UserKey, is_self: bool) -> ChatAdminRightsInfo {
        assert_eq!(user.scope(), self.key().scope());
        use ChatAdminRight as R;
        ChatAdminRightsInfo {
            flags: R::Other
                | R::ChangeInfo
                | R::DeleteMessages
                | R::BanUsers
                | R::InviteByLinkOrAdd
                | R::PinMessages
                | R::ManageCall
                | R::ManageWelcomeMessages
                | if self.creator == peer_to_user(user.peer().id())
                    || (is_self && self.am_creator())
                {
                    R::AddAdmins.into()
                } else {
                    ChatAdminRights::empty()
                },
        }
    }
    pub fn is_ayu_no_forwards(&self) -> bool {
        (self.flags() & ChatDataFlag::AyuNoForwards).any()
    }
    pub fn allows_forwarding(&self) -> bool {
        (self.flags() & ChatDataFlag::NoForwards).is_empty()
    }
    pub fn can_edit_information(&self) -> bool {
        self.am_in() && !self.am_restricted(ChatRestriction::ChangeInfo).restricted()
    }
    pub fn can_edit_permissions(&self) -> bool {
        self.am_in()
            && (self.am_creator() || (self.admin_rights() & ChatAdminRight::BanUsers).any())
    }
    pub fn can_edit_username(&self) -> bool {
        self.am_creator() && (self.flags() & ChatDataFlag::CanSetUsername).any()
    }
    pub fn can_edit_pre_history_hidden(&self) -> bool {
        self.am_creator()
    }
    pub fn can_delete_messages(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::DeleteMessages).any()
    }
    pub fn can_add_members(&self) -> bool {
        self.am_in()
            && !self
                .am_restricted(ChatRestriction::AddParticipants)
                .restricted()
    }
    pub fn can_add_admins(&self) -> bool {
        self.am_in() && self.am_creator()
    }
    pub fn can_ban_members(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::BanUsers).any()
    }
    pub fn anyone_can_add_members(&self) -> bool {
        (self.default_restrictions() & ChatRestriction::AddParticipants).is_empty()
    }
    pub fn can_have_invite_link(&self) -> bool {
        self.am_creator() || (self.admin_rights() & ChatAdminRight::InviteByLinkOrAdd).any()
    }
    pub fn invite_link(&self) -> &str {
        &self.invite_link
    }
    pub fn version(&self) -> i32 {
        self.version
    }
    pub fn get_migrate_to_channel(&self) -> Option<ChannelKey> {
        self.migrated_to
    }
    pub fn group_call(&self) -> Option<GroupCallKey> {
        self.call
    }
    pub fn group_call_default_join_as(&self) -> PeerId {
        self.call_default_join_as
    }
    pub fn bot_commands(&self) -> &ChatBotCommands {
        &self.bot_commands
    }
    pub fn pending_requests_count(&self) -> i32 {
        self.pending_requests_count
    }
    pub fn recent_requesters(&self) -> &[UserId] {
        &self.recent_requesters
    }
    pub fn allowed_reactions(&self) -> &AllowedReactions {
        &self.allowed_reactions
    }
    pub fn input_chat(&self) -> u64 {
        peer_to_chat(self.peer.id()).bare
    }
}
