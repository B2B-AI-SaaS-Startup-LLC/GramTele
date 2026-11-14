//! Phased mutations on the sole Peers owner: no store borrow crosses fire.
use super::{
    data_changes::PeerFlag as F,
    data_changes::PeerFlags,
    data_channel::*,
    data_chat::*,
    data_chat_participant_status::*,
    data_group_model::*,
    data_msg_id::MsgId,
    data_peer::*,
    data_peer_bot_commands::*,
    data_peer_id::*,
    data_session::{NameFields, NameIndex, PeerRecord, Peers},
    data_store_keys::*,
    data_user_names::Usernames,
};
impl Peers {
    pub fn group_effects(&self) -> tg_rpl::Producer<GroupEffect> {
        self.group_effects.events()
    }
    pub(crate) fn emit_group(&self, e: GroupEffect) {
        match e {
            GroupEffect::Peer(p) => self.dispatch(vec![p]),
            e => self.group_effects.fire(e),
        }
    }
    pub(crate) fn group_changed(&self, k: PeerKey, f: PeerFlags) {
        self.emit_group(changed(k, f));
    }
    pub fn ensure_chat(&self, id: ChatId) -> ChatKey {
        ChatKey::try_from(self.ensure_peer(peer_from_chat(id))).unwrap()
    }
    pub fn ensure_channel(&self, id: ChannelId) -> ChannelKey {
        ChannelKey::try_from(self.ensure_peer(peer_from_channel(id))).unwrap()
    }
    pub fn read_chat<R>(&self, k: ChatKey, f: impl FnOnce(&ChatData) -> R) -> R {
        self.check_scope(k.peer());
        let p = self.peers.borrow();
        match p.get(&k.peer().id()).expect("chat not ensured") {
            PeerRecord::Chat(c) => f(c),
            _ => unreachable!(),
        }
    }
    pub fn read_channel<R>(&self, k: ChannelKey, f: impl FnOnce(&ChannelData) -> R) -> R {
        self.check_scope(k.peer());
        let p = self.peers.borrow();
        match p.get(&k.peer().id()).expect("channel not ensured") {
            PeerRecord::Channel(c) => f(c),
            _ => unreachable!(),
        }
    }
    pub fn edit_chat<R>(&self, k: ChatKey, f: impl FnOnce(&mut ChatData) -> R) -> R {
        self.check_scope(k.peer());
        let mut p = self.peers.borrow_mut();
        match p.get_mut(&k.peer().id()).expect("chat not ensured") {
            PeerRecord::Chat(c) => f(c),
            _ => unreachable!(),
        }
    }
    pub fn edit_channel<R>(&self, k: ChannelKey, f: impl FnOnce(&mut ChannelData) -> R) -> R {
        self.check_scope(k.peer());
        let mut p = self.peers.borrow_mut();
        match p.get_mut(&k.peer().id()).expect("channel not ensured") {
            PeerRecord::Channel(c) => f(c),
            _ => unreachable!(),
        }
    }
    pub fn set_chat_flags(&self, k: ChatKey, which: ChatDataFlags) {
        let (was, flags) = self.read_chat(k, |c| (c.am_in(), c.flags.clone()));
        flags.set(which);
        if was && !self.read_chat(k, ChatData::am_in) {
            self.emit_group(GroupEffect::CloseChatIfStillNotMember(k.peer()));
        }
    }
    pub fn add_chat_flags(&self, k: ChatKey, which: ChatDataFlags) {
        let flags = self.read_chat(k, |c| c.flags.clone());
        flags.add(which);
    }
    pub fn remove_chat_flags(&self, k: ChatKey, which: ChatDataFlags) {
        let flags = self.read_chat(k, |c| c.flags.clone());
        flags.remove(which);
    }
    pub fn set_chat_admin_rights(&self, k: ChatKey, rights: ChatAdminRights) {
        let flags = self.read_chat(k, |c| c.admin_rights.clone());
        if flags.current() == rights {
            return;
        }
        flags.set(rights);
        if !self.read_chat(k, ChatData::can_have_invite_link) {
            self.set_chat_pending_requests(k, 0, Vec::new());
        }
        self.group_changed(k.peer(), F::Rights | F::Admins | F::BannedUsers);
    }
    pub fn set_chat_default_restrictions(&self, k: ChatKey, rights: ChatRestrictions) {
        let flags = self.read_chat(k, |c| c.default_restrictions.clone());
        if flags.current() != rights {
            flags.set(rights);
            self.group_changed(k.peer(), F::Rights.into());
        }
    }
    pub fn invalidate_chat_participants(&self, k: ChatKey) {
        self.edit_chat(k, |c| {
            c.participants.clear();
            c.admins.clear();
        });
        self.set_chat_admin_rights(k, ChatAdminRights::empty());
        self.edit_chat(k, |c| {
            c.invited_by_me.clear();
            c.bot_status = BotStatus::Unknown;
        });
        self.group_changed(k.peer(), F::Members | F::Admins);
    }
    pub fn set_chat_pending_requests(&self, k: ChatKey, count: i32, recent: Vec<UserId>) {
        let changed = self.edit_chat(k, |c| {
            if c.pending_requests_count == count && c.recent_requesters == recent {
                return false;
            }
            c.pending_requests_count = count;
            c.recent_requesters = recent;
            true
        });
        if changed {
            self.group_changed(k.peer(), F::PendingRequests.into());
        }
    }
    pub fn set_chat_invite_link(&self, k: ChatKey, link: &str) {
        self.edit_chat(k, |c| c.invite_link = link.into());
    }
    pub fn apply_chat_edit_admin(&self, k: ChatKey, user: UserKey, admin: bool) {
        self.check_scope(user.peer());
        self.edit_chat(k, |c| {
            if admin {
                c.admins.insert(user);
            } else {
                c.admins.remove(&user);
            }
        });
        self.group_changed(k.peer(), F::Admins.into());
    }
    pub fn refresh_chat_bot_status(&self, k: ChatKey, is_bot: &impl Fn(UserKey) -> bool) {
        let users = self.read_chat(k, |c| c.participants.iter().copied().collect::<Vec<_>>());
        let status = if users.is_empty() {
            BotStatus::Unknown
        } else if users.iter().copied().any(is_bot) {
            BotStatus::HasBots
        } else {
            BotStatus::NoBots
        };
        self.edit_chat(k, |c| c.bot_status = status);
    }
    pub fn apply_chat_update_version(&self, k: ChatKey, version: i32) -> UpdateStatus {
        let old = self.read_chat(k, ChatData::version);
        if old > version {
            UpdateStatus::TooOld
        } else if old.wrapping_add(1) < version {
            self.invalidate_chat_participants(k);
            self.dispatch(vec![PeerEffect::RequestFullPeer(k.peer())]);
            UpdateStatus::Skipped
        } else {
            self.edit_chat(k, |c| c.version = version);
            UpdateStatus::Good
        }
    }
    pub fn set_chat_version(&self, k: ChatKey, version: i32) {
        self.edit_chat(k, |c| c.version = version);
    }
    pub fn set_chat_migrate_to_channel(&self, k: ChatKey, target: ChannelKey) {
        self.check_scope(target.peer());
        let changed = self.edit_chat(k, |c| {
            let diff = c.migrated_to != Some(target);
            c.migrated_to = Some(target);
            diff
        });
        if changed && self.read_channel(target, ChannelData::am_in) {
            self.group_changed(k.peer(), F::Migration.into());
        }
    }
    pub fn set_chat_name(
        &self,
        k: ChatKey,
        name: &str,
        filter: bool,
        index: impl Fn() -> NameIndex,
    ) {
        let filtered = if filter {
            crate::ayu::utils::telegram_helpers::filter_zalgo(name)
        } else {
            name.into()
        };
        let value = if filtered.is_empty() {
            self.resolved_group_name(k.peer())
        } else {
            filtered
        };
        self.update_name_delayed(k.peer(), &value, "", "", NameFields::Chat, index);
    }
    pub fn set_channel_name(
        &self,
        k: ChannelKey,
        name: &str,
        username: &str,
        filter: bool,
        index: impl Fn() -> NameIndex,
    ) {
        let filtered = if filter {
            crate::ayu::utils::telegram_helpers::filter_zalgo(name)
        } else {
            name.into()
        };
        let value = if filtered.is_empty() {
            self.resolved_group_name(k.peer())
        } else {
            filtered
        };
        self.update_name_delayed(
            k.peer(),
            &value,
            "",
            username,
            NameFields::Channel {
                editable_username: &|| self.read_channel(k, |c| c.editable_username().to_owned()),
                username: &|| self.read_channel(k, |c| c.username().to_owned()),
                set_username: &|s| self.set_channel_username(k, s),
                set_username_flag: &|has| {
                    let old = self.read_channel(k, ChannelData::flags);
                    self.set_channel_flags(
                        k,
                        (old & !ChannelDataFlags::from(ChannelDataFlag::Username))
                            | if has {
                                ChannelDataFlag::Username.into()
                            } else {
                                ChannelDataFlags::empty()
                            },
                    );
                },
            },
            index,
        );
    }
    pub fn set_channel_username(&self, k: ChannelKey, name: &str) {
        self.edit_channel(k, |c| c.username.set_username(name));
    }
    pub fn set_channel_usernames(&self, k: ChannelKey, names: &Usernames) {
        let flags = self.edit_channel(k, |c| {
            let old = c.username().to_owned();
            let olds = c.usernames().to_vec();
            c.username.set_usernames(names);
            let mut f = PeerFlags::empty();
            if old != c.username() {
                f |= F::Username;
            }
            if olds != c.usernames() {
                f |= F::Usernames;
            }
            f
        });
        self.group_changed(k.peer(), flags);
    }
    pub fn set_channel_flags(&self, k: ChannelKey, mut which: ChannelDataFlags) {
        use ChannelDataFlag as G;
        if (which & G::MonoforumAdmin).any() {
            which |= G::Monoforum;
        }
        if (which & (G::Forum | G::Monoforum)).any() {
            which |= G::Megagroup;
        }
        if (which & G::Monoforum).any() {
            which &= !ChannelDataFlags::from(G::Forum);
        }
        let (diff, was_in, flags, pre, taken_forum) = self.edit_channel(k, |c| {
            let diff = c.flags() ^ which;
            let mut pre = Vec::new();
            if (which & G::Megagroup).any() && c.mg_info.is_none() {
                c.mg_info = Some(MegagroupInfo::default());
            }
            if (which & G::Community).any() && c.community_owner.is_none() {
                c.community_owner = Some(k);
                pre.push(GroupEffect::EnsureCommunity(k.peer()));
            }
            let taken = (diff & G::Forum).any()
                && (which & G::Forum).is_empty()
                && c.mg_info.as_mut().and_then(|i| i.forum.take()).is_some();
            if taken {
                pre.push(GroupEffect::DetachForum(k.peer()));
            }
            if (diff & G::MonoforumAdmin).any()
                && (which & G::MonoforumAdmin).is_empty()
                && c.mg_info
                    .as_mut()
                    .and_then(|i| i.monoforum.take())
                    .is_some()
            {
                pre.push(GroupEffect::DetachMonoforum(k.peer()));
            }
            let was = c.am_in();
            if (diff & G::MonoforumAdmin).any() && (which & G::MonoforumAdmin).any() {
                let i = c.mg_info.as_mut().unwrap();
                if i.monoforum.is_none() {
                    i.monoforum = Some(k.peer());
                    pre.push(GroupEffect::EnsureMonoforum(k.peer()));
                }
            } else if (diff & G::Forum).any() && (which & G::Forum).any() {
                let i = c.mg_info.as_mut().unwrap();
                if i.forum.is_none() {
                    i.forum = Some(k.peer());
                    pre.push(GroupEffect::EnsureForum(k.peer()));
                }
            }
            (diff, was, c.flags.clone(), pre, taken)
        });
        for e in pre {
            self.emit_group(e);
        }
        flags.set(which);
        if (diff & (G::Left | G::Forbidden)).any() {
            if let Some(chat) = self.read_channel(k, ChannelData::get_migrate_from_chat) {
                self.group_changed(chat.peer(), F::Migration.into());
                self.group_changed(k.peer(), F::Migration.into());
            }
            if was_in && !self.read_channel(k, ChannelData::am_in) {
                self.emit_group(GroupEffect::CloseChatIfStillNotMember(k.peer()));
            }
            let community = self.read_channel(k, ChannelData::linked_community_id);
            if !community.is_zero() {
                self.emit_group(GroupEffect::CommunityMembershipChanged {
                    peer: k.peer(),
                    community,
                });
            }
        }
        if (diff
            & (G::Forum
                | G::MonoforumAdmin
                | G::CallNotEmpty
                | G::SimilarExpanded
                | G::Signatures
                | G::SignatureProfiles
                | G::ForumTabs))
            .any()
        {
            self.emit_group(GroupEffect::HistoryFlagsChanged {
                peer: k.peer(),
                diff: diff.value(),
                which: which.value(),
            });
        }
        if (which & G::Community).any()
            && (diff & (G::Community | G::CommunityCollapsed | G::Forbidden | G::Left)).any()
        {
            self.emit_group(GroupEffect::CommunityFlagsChanged(k.peer()));
        }
        if taken_forum {
            self.emit_group(GroupEffect::ClearForumUserpics(k.peer()));
        }
    }
    pub fn add_channel_flags(&self, k: ChannelKey, f: ChannelDataFlags) {
        let old = self.read_channel(k, ChannelData::flags);
        self.set_channel_flags(k, old | f);
    }
    pub fn remove_channel_flags(&self, k: ChannelKey, f: ChannelDataFlags) {
        let old = self.read_channel(k, ChannelData::flags);
        self.set_channel_flags(k, old & !f);
    }
    pub fn set_channel_pending_requests(&self, k: ChannelKey, count: i32, recent: Vec<UserId>) {
        let diff = self.edit_channel(k, |c| {
            if c.pending_requests_count == count && c.recent_requesters == recent {
                return false;
            }
            c.pending_requests_count = count;
            c.recent_requesters = recent;
            true
        });
        if diff {
            self.group_changed(k.peer(), F::PendingRequests.into());
        }
    }
    pub fn set_channel_members_count(&self, k: ChannelKey, count: i32) {
        let diff = self.edit_channel(k, |c| {
            if c.members_count == count {
                return false;
            }
            if c.is_megagroup() && c.can_view_members() {
                let old = c.members_count();
                let i = c.mg_info.as_mut().unwrap();
                if !i.last_participants.is_empty() {
                    i.last_participants_status |= MegagroupInfo::LAST_PARTICIPANTS_COUNT_OUTDATED;
                    i.last_participants_count = old;
                }
            }
            c.members_count = count;
            true
        });
        if diff {
            self.group_changed(k.peer(), F::Members.into());
        }
    }
    pub fn set_channel_admins_count(&self, k: ChannelKey, count: i32) {
        let diff = self.edit_channel(k, |c| {
            let diff = c.admins_count != count;
            c.admins_count = count;
            diff
        });
        if diff {
            self.group_changed(k.peer(), F::Admins.into());
        }
    }
    pub fn set_channel_restricted_count(&self, k: ChannelKey, count: i32) {
        let diff = self.edit_channel(k, |c| {
            let diff = c.restricted_count != count;
            c.restricted_count = count;
            diff
        });
        if diff {
            self.group_changed(k.peer(), F::BannedUsers.into());
        }
    }
    pub fn set_channel_kicked_count(&self, k: ChannelKey, count: i32) {
        let diff = self.edit_channel(k, |c| {
            let diff = c.kicked_count != count;
            c.kicked_count = count;
            diff
        });
        if diff {
            self.group_changed(k.peer(), F::BannedUsers.into());
        }
    }
    pub fn set_channel_admin_rights(
        &self,
        k: ChannelKey,
        rights: ChatAdminRights,
        self_user: UserKey,
    ) {
        self.check_scope(self_user.peer());
        let flags = self.read_channel(k, |c| c.admin_rights.clone());
        if flags.current() == rights {
            return;
        }
        flags.set(rights);
        if !self.read_channel(k, ChannelData::can_have_invite_link) {
            self.set_channel_pending_requests(k, 0, Vec::new());
        }
        self.edit_channel(k, |c| {
            if c.is_megagroup() {
                let has = c.has_admin_rights();
                let creator = c.am_creator();
                let i = c.mg_info.as_mut().unwrap();
                if has {
                    if !creator {
                        i.last_admins.entry(self_user).or_insert(Admin {
                            rights: ChatAdminRightsInfo { flags: rights },
                            can_edit: false,
                        });
                    }
                    i.last_restricted.remove(&self_user);
                } else {
                    i.last_admins.remove(&self_user);
                }
            }
        });
        self.group_changed(k.peer(), F::Rights | F::Admins | F::BannedUsers);
        if self.read_channel(k, ChannelData::is_broadcast) {
            if let Some(link) = self.read_channel(k, ChannelData::monoforum_link) {
                let can = self.read_channel(k, ChannelData::can_access_monoforum);
                let old = self.read_channel(link, ChannelData::flags);
                self.set_channel_flags(
                    link,
                    (old & !ChannelDataFlags::from(ChannelDataFlag::MonoforumAdmin))
                        | if can {
                            ChannelDataFlag::MonoforumAdmin.into()
                        } else {
                            ChannelDataFlags::empty()
                        },
                );
            }
        }
    }
    pub fn set_channel_restrictions(
        &self,
        k: ChannelKey,
        rights: ChatRestrictionsInfo,
        self_user: UserKey,
    ) {
        self.check_scope(self_user.peer());
        let (old, until, flags) = self.read_channel(k, |c| {
            (c.restrictions(), c.restricted_until, c.restrictions.clone())
        });
        if old == rights.flags && until == rights.until {
            return;
        }
        self.edit_channel(k, |c| c.restricted_until = rights.until);
        flags.set(rights.flags);
        let notify = self.edit_channel(k, |c| {
            if !c.is_megagroup() {
                return None;
            }
            let has = c.has_restrictions();
            let creator = c.am_creator();
            let i = c.mg_info.as_mut().unwrap();
            if has {
                if !creator {
                    i.last_restricted
                        .entry(self_user)
                        .or_insert(Restricted { rights });
                }
                i.last_admins.remove(&self_user);
                let mut changes = super::data_channel_admins::ChannelAdminChanges::new(i);
                changes.remove(i, peer_to_user(self_user.peer().id()));
                changes.finish(i, peer_to_user(self_user.peer().id()))
            } else {
                i.last_restricted.remove(&self_user);
                None
            }
        });
        if let Some(users) = notify {
            self.emit_group(GroupEffect::ApplyGroupAdminChanges {
                peer: k.peer(),
                users,
            });
        }
        self.group_changed(k.peer(), F::Rights | F::Admins | F::BannedUsers);
    }
    pub fn set_channel_default_restrictions(&self, k: ChannelKey, rights: ChatRestrictions) {
        let flags = self.read_channel(k, |c| c.default_restrictions.clone());
        if flags.current() != rights {
            flags.set(rights);
            self.group_changed(k.peer(), F::Rights.into());
        }
    }
    pub fn set_channel_migrate_from_chat(&self, k: ChannelKey, chat: Option<ChatKey>) {
        if let Some(chat) = chat {
            self.check_scope(chat.peer());
        }
        let changed = self.edit_channel(k, |c| {
            let i = c.mg_info.as_mut().expect("MegagroupInfo required");
            let diff = i.migrated_from != chat;
            i.set_migrate_from_chat(chat);
            diff
        });
        if changed && self.read_channel(k, ChannelData::am_in) {
            self.group_changed(k.peer(), F::Migration.into());
        }
    }
    pub fn apply_migration(&self, chat: ChatKey, channel: ChannelKey) {
        assert!(self.read_channel(channel, ChannelData::is_megagroup));
        self.set_chat_migrate_to_channel(chat, channel);
        self.set_channel_migrate_from_chat(channel, Some(chat));
    }
    pub fn set_channel_slowmode_seconds(&self, k: ChannelKey, seconds: i32) {
        let diff = self.edit_channel(k, |c| {
            let Some(i) = c.mg_info.as_mut() else {
                return false;
            };
            if i.slowmode_seconds == seconds {
                return false;
            }
            i.slowmode_seconds = seconds;
            true
        });
        if diff {
            self.group_changed(k.peer(), F::Slowmode.into());
        }
    }
    pub fn grow_channel_slowmode_last_message(&self, k: ChannelKey, when: i32, now: i32) {
        let when = when.min(now);
        let diff = self.edit_channel(k, |c| {
            let Some(i) = c.mg_info.as_mut() else {
                return false;
            };
            if i.slowmode_last_message <= now && i.slowmode_last_message >= when {
                return false;
            }
            i.slowmode_last_message = when;
            true
        });
        if diff {
            self.group_changed(k.peer(), F::Slowmode.into());
        }
    }
    pub fn set_channel_boosts_unrestrict(&self, k: ChannelKey, applied: i32, unrestrict: i32) {
        let event = self.edit_channel(k, |c| {
            let was = c.unrestricted_by_boosts();
            let i = c.mg_info.as_mut()?;
            if i.boosts_applied == applied && i.boosts_unrestrict == unrestrict {
                return None;
            }
            i.boosts_applied = applied;
            i.boosts_unrestrict = unrestrict;
            let now = unrestrict > 0 && applied >= unrestrict;
            (was != now).then(|| (i.boosts_changes.clone(), now))
        });
        if let Some((stream, now)) = event {
            stream.fire(now);
            self.group_changed(k.peer(), F::Rights | F::Slowmode);
        }
    }
}

impl Peers {
    pub fn set_channel_access_hash(&self, k: ChannelKey, hash: u64) {
        self.edit_channel(k, |c| c.access_hash = hash);
    }
    pub fn set_channel_invite_link(&self, k: ChannelKey, link: &str) {
        self.edit_channel(k, |c| c.invite_link = link.into());
    }
    pub fn set_channel_available_min_id(&self, k: ChannelKey, id: MsgId) {
        self.edit_channel(k, |c| c.available_min_id = id);
    }
    pub fn update_channel_level_hint(&self, k: ChannelKey, level: i32) {
        self.edit_channel(k, |c| c.level_hint = level);
    }
    pub fn update_channel_subscription_until(&self, k: ChannelKey, until: i32) {
        self.edit_channel(k, |c| c.subscription_until_date = until);
    }
    pub fn set_channel_guard_bot_id(&self, k: ChannelKey, user: UserId) {
        self.edit_channel(k, |c| c.guard_bot_id = user);
    }
    pub fn set_channel_unavailable_reasons_list(
        &self,
        k: ChannelKey,
        reasons: Vec<UnavailableReason>,
    ) {
        self.edit_channel(k, |c| c.unavailable_reasons = reasons);
    }
    pub fn set_channel_stars_per_message(&self, k: ChannelKey, stars: i32) {
        let diff = self.edit_channel(k, |c| {
            let d = c.stars_per_message != stars;
            c.stars_per_message = stars;
            d
        });
        if diff {
            self.group_changed(k.peer(), F::StarsPerMessage.into());
        }
        self.emit_group(GroupEffect::CheckTrustedPay(k.peer()));
    }
    pub fn set_channel_peer_gifts_count(&self, k: ChannelKey, count: i32) {
        let diff = self.edit_channel(k, |c| {
            let d = c.peer_gifts_count != count;
            c.peer_gifts_count = count;
            d
        });
        if diff {
            self.group_changed(k.peer(), F::PeerGifts.into());
        }
    }
    pub fn set_channel_invite_peek(&self, k: ChannelKey, hash: &str, expires: i32) {
        self.edit_channel(k, |c| c.invite_peek = Some((hash.into(), expires)));
    }
    pub fn clear_channel_invite_peek(&self, k: ChannelKey) {
        self.edit_channel(k, |c| c.invite_peek = None);
    }
    pub fn channel_private_error_received(&self, k: ChannelKey, window_available: bool, now: i32) {
        let expires = self.read_channel(k, ChannelData::invite_peek_expires);
        if expires != 0 {
            if window_available {
                let hash = self.read_channel(k, |c| c.invite_peek_hash().to_owned());
                self.clear_channel_invite_peek(k);
                self.emit_group(GroupEffect::PrivateInviteCheck {
                    peer: k.peer(),
                    hash,
                });
            } else {
                self.edit_channel(k, |c| c.invite_peek.as_mut().unwrap().1 = now);
            }
        } else {
            self.mark_channel_forbidden(k);
        }
    }
    pub fn mark_channel_forbidden(&self, k: ChannelKey) {
        let name = self.resolved_group_name(k.peer());
        let c = self.read_channel(k, |c| {
            tg_scheme::mtp_channel_forbidden(
                if c.is_megagroup() {
                    tg_scheme::ChannelForbiddenData::F_MEGAGROUP
                } else {
                    tg_scheme::ChannelForbiddenData::F_BROADCAST
                },
                peer_to_channel(c.peer.id()).bare,
                c.access_hash,
                name.as_bytes().into(),
                None,
            )
        });
        self.emit_group(GroupEffect::ProcessForbidden(c));
    }
    pub fn set_channel_location(&self, k: ChannelKey, data: &tg_scheme::ChannelLocation) {
        let diff = self.edit_channel(k, |c| {
            let Some(i) = c.mg_info.as_mut() else {
                return false;
            };
            let old = i.location.clone();
            let old_some = i.get_location().is_some();
            let location = match data {
                tg_scheme::ChannelLocation::ChannelLocation(d) => match &d.geo_point {
                    tg_scheme::GeoPoint::GeoPoint(p) => ChannelLocation {
                        address: String::from_utf8_lossy(&d.address).into(),
                        point: super::data_location::LocationPoint::from_mtp(p),
                    },
                    _ => ChannelLocation::default(),
                },
                _ => ChannelLocation::default(),
            };
            i.set_location(location);
            old_some != i.get_location().is_some() || (old_some && old != i.location)
        });
        if diff {
            self.group_changed(k.peer(), F::ChannelLocation.into());
        }
    }
    pub fn set_channel_discussion_link(&self, k: ChannelKey, link: Option<ChannelKey>) {
        if let Some(link) = link {
            self.check_scope(link.peer());
        }
        let diff = self.edit_channel(k, |c| {
            let d = c.discussion_link != link || !c.discussion_link_known;
            c.discussion_link = link;
            c.discussion_link_known = true;
            d
        });
        if diff {
            self.emit_group(GroupEffect::HistoryFlagsChanged {
                peer: k.peer(),
                diff: 0,
                which: 0,
            });
            self.group_changed(k.peer(), F::DiscussionLink.into());
        }
    }
    pub fn set_channel_monoforum_link(&self, k: ChannelKey, link: Option<ChannelKey>) {
        if let Some(link) = link {
            self.check_scope(link.peer());
        }
        let old = self.read_channel(k, ChannelData::monoforum_link);
        if let Some(old) = old {
            if self.read_channel(k, ChannelData::is_broadcast) {
                self.set_channel_monoforum_link(old, link.map(|_| k));
            } else if self.read_channel(k, ChannelData::is_monoforum) {
                if link.is_none() && !self.read_channel(k, ChannelData::monoforum_disabled) {
                    self.add_channel_flags(k, ChannelDataFlag::MonoforumDisabled.into());
                } else if link.is_some() && self.read_channel(k, ChannelData::monoforum_disabled) {
                    self.remove_channel_flags(k, ChannelDataFlag::MonoforumDisabled.into());
                }
            }
            return;
        }
        let Some(link) = link else { return };
        self.edit_channel(k, |c| c.monoforum_link = Some(link));
        self.set_channel_monoforum_link(link, Some(k));
        self.group_changed(k.peer(), F::MonoforumLink.into());
        if self.read_channel(k, ChannelData::is_megagroup)
            && self.read_channel(link, ChannelData::can_access_monoforum)
        {
            self.add_channel_flags(k, ChannelDataFlag::MonoforumAdmin.into());
        }
    }
    pub fn set_channel_linked_community_id(&self, k: ChannelKey, id: ChannelId) {
        let diff = self.edit_channel(k, |c| {
            let d = c.linked_community_id != id;
            c.linked_community_id = id;
            d
        });
        if diff {
            self.emit_group(GroupEffect::LinkedCommunityChanged(k.peer()));
        }
    }
    pub fn ensure_channel_community_info(&self, k: ChannelKey) -> ChannelKey {
        let new = self.edit_channel(k, |c| {
            let new = c.community_owner.is_none();
            c.community_owner = Some(k);
            new
        });
        if new {
            self.emit_group(GroupEffect::EnsureCommunity(k.peer()));
        }
        k
    }
    pub fn set_channel_view_as_messages(&self, k: ChannelKey, enabled: bool) {
        if self.read_channel(k, ChannelData::view_forum_as_messages) != enabled {
            let old = self.read_channel(k, ChannelData::flags);
            self.set_channel_flags(
                k,
                (old & !ChannelDataFlags::from(ChannelDataFlag::ViewAsMessages))
                    | if enabled {
                        ChannelDataFlag::ViewAsMessages.into()
                    } else {
                        ChannelDataFlags::empty()
                    },
            );
        }
    }
    pub fn set_channel_stories_state(&self, k: ChannelKey, state: StoriesState) {
        assert_ne!(state, StoriesState::Unknown);
        use ChannelDataFlag as G;
        let flags = self.read_channel(k, |c| c.flags.clone());
        let was = flags.current();
        match state {
            StoriesState::None => {
                flags.remove(G::HasActiveStories | G::HasUnreadStories | G::HasActiveVideoStream)
            }
            StoriesState::HasRead => flags.set(
                (was & !(G::HasUnreadStories | G::HasActiveVideoStream)) | G::HasActiveStories,
            ),
            StoriesState::HasUnread => flags.set(
                (was & !ChannelDataFlags::from(G::HasActiveVideoStream))
                    | G::HasActiveStories
                    | G::HasUnreadStories,
            ),
            StoriesState::HasVideoStream => flags.set(
                (was & !ChannelDataFlags::from(G::HasUnreadStories))
                    | G::HasActiveStories
                    | G::HasActiveVideoStream,
            ),
            StoriesState::Unknown => unreachable!(),
        };
        if flags.current() != was {
            self.emit_group(GroupEffect::UpdateChatListEntryPostponed(k.peer()));
            self.group_changed(k.peer(), F::StoriesState.into());
        }
    }
    pub fn set_chat_bot_commands(&self, k: ChatKey, list: &[BotCommands]) {
        if self.edit_chat(k, |c| c.bot_commands.update(list)) {
            self.emit_group(GroupEffect::BotCommandsChanged(k.peer()));
        }
    }
    pub fn set_channel_bot_commands(&self, k: ChannelKey, list: &[BotCommands]) {
        if self.edit_channel(k, |c| {
            c.mg_info.as_mut().is_some_and(|i| i.set_bot_commands(list))
        }) {
            self.emit_group(GroupEffect::BotCommandsChanged(k.peer()));
        }
    }
    pub fn set_chat_allowed_reactions(&self, k: ChatKey, value: AllowedReactions) {
        if self.read_chat(k, |c| c.allowed_reactions == value) {
            return;
        }
        let was = self.read_chat(k, |c| c.allowed_reactions.enabled());
        self.edit_chat(k, |c| c.allowed_reactions = value);
        let now = self.read_chat(k, |c| c.allowed_reactions.enabled());
        if was != now {
            self.emit_group(GroupEffect::ReactionsEnabled {
                peer: k.peer(),
                enabled: now,
            });
        }
        self.group_changed(k.peer(), F::Reactions.into());
    }
    pub fn set_channel_allowed_reactions(&self, k: ChannelKey, value: AllowedReactions) {
        if self.read_channel(k, |c| c.allowed_reactions == value) {
            return;
        }
        if value.paid_enabled {
            self.emit_group(GroupEffect::LoadPaidReactionShownPeer);
        }
        let was = self.read_channel(k, |c| c.allowed_reactions.enabled());
        self.edit_channel(k, |c| c.allowed_reactions = value);
        let now = self.read_channel(k, |c| c.allowed_reactions.enabled());
        if was != now {
            self.emit_group(GroupEffect::ReactionsEnabled {
                peer: k.peer(),
                enabled: now,
            });
        }
        self.group_changed(k.peer(), F::Reactions.into());
    }
    pub fn set_group_photo(
        &self,
        k: PeerKey,
        photo: &tg_scheme::ChatPhoto,
        input: &tg_scheme::InputPeer,
    ) {
        assert!(peer_is_chat(k.id()) || peer_is_channel(k.id()));
        self.update(k, |p| match photo {
            tg_scheme::ChatPhoto::ChatPhoto(d) => {
                let location = super::data_image_location::StorageFileLocation::peer_photo(
                    d.dc_id,
                    UserId::new(0),
                    input,
                    d.photo_id,
                    false,
                );
                p.set_userpic_checked(
                    d.photo_id,
                    super::data_image_location::ImageLocation::new(
                        super::data_image_location::DownloadLocation::Storage(location),
                        160,
                        160,
                    ),
                    d.is_has_video(),
                )
            }
            _ => p.clear_userpic(),
        });
    }
    pub fn set_group_call_join_as(&self, k: PeerKey, as_peer: PeerId) {
        if let Ok(chat) = ChatKey::try_from(k) {
            self.edit_chat(chat, |c| c.call_default_join_as = as_peer);
        } else {
            self.edit_channel(ChannelKey::try_from(k).expect("group peer"), |c| {
                c.call_default_join_as = as_peer
            });
        }
    }
    pub fn set_group_call(
        &self,
        k: PeerKey,
        input: Option<&tg_scheme::InputGroupCall>,
        schedule_date: i32,
        rtmp: bool,
    ) {
        if let Ok(chat) = ChatKey::try_from(k) {
            if self
                .read_chat(chat, |c| c.migrated_to)
                .is_some_and(|to| self.read_channel(to, ChannelData::am_in))
            {
                return;
            }
        }
        let id = match input {
            Some(tg_scheme::InputGroupCall::InputGroupCall(d)) => d.id,
            _ => 0,
        };
        let old = if let Ok(chat) = ChatKey::try_from(k) {
            self.read_chat(chat, ChatData::group_call)
        } else {
            self.read_channel(
                ChannelKey::try_from(k).expect("group peer"),
                ChannelData::group_call,
            )
        };
        if old.map_or(0, GroupCallKey::id) == id {
            return;
        }
        let new = (id != 0).then(|| GroupCallKey::from_scoped_id(k.scope(), id));
        if let Ok(chat) = ChatKey::try_from(k) {
            self.edit_chat(chat, |c| c.call = new);
        } else {
            self.edit_channel(ChannelKey::try_from(k).unwrap(), |c| c.call = new);
        }
        self.emit_group(GroupEffect::GroupCallChanged {
            peer: k,
            old,
            new,
            input: input.cloned(),
            schedule_date,
            rtmp,
        });
        self.group_changed(k, F::GroupCall.into());
        if let Ok(chat) = ChatKey::try_from(k) {
            if new.is_some() {
                self.add_chat_flags(chat, ChatDataFlag::CallActive.into());
            } else {
                self.remove_chat_flags(chat, ChatDataFlag::CallActive | ChatDataFlag::CallNotEmpty);
            }
        } else {
            let channel = ChannelKey::try_from(k).unwrap();
            if new.is_some() {
                self.add_channel_flags(channel, ChannelDataFlag::CallActive.into());
            } else {
                self.remove_channel_flags(
                    channel,
                    ChannelDataFlag::CallActive | ChannelDataFlag::CallNotEmpty,
                );
            }
        }
    }
}
impl Peers {
    pub fn migrate_group_call(&self, to: ChannelKey, from: PeerKey, call: GroupCallKey) {
        self.check_scope(from);
        assert_eq!(to.scope(), call.scope());
        assert!(self.read_channel(to, ChannelData::group_call).is_none());
        self.edit_channel(to, |c| c.call = Some(call));
        self.emit_group(GroupEffect::MigrateCall {
            from,
            to: to.peer(),
            call,
        });
        self.group_changed(to.peer(), F::GroupCall.into());
        self.add_channel_flags(to, ChannelDataFlag::CallActive.into());
    }
    pub fn clear_group_call(&self, k: PeerKey) {
        let old = if let Ok(chat) = ChatKey::try_from(k) {
            self.read_chat(chat, ChatData::group_call)
        } else {
            self.read_channel(ChannelKey::try_from(k).unwrap(), ChannelData::group_call)
        };
        let Some(call) = old else { return };
        let target = ChatKey::try_from(k)
            .ok()
            .and_then(|chat| self.read_chat(chat, ChatData::get_migrate_to_channel))
            .filter(|to| {
                self.read_channel(*to, ChannelData::am_in)
                    && self.read_channel(*to, ChannelData::group_call).is_none()
            });
        if let Ok(chat) = ChatKey::try_from(k) {
            self.edit_chat(chat, |c| c.call = None);
        } else {
            self.edit_channel(ChannelKey::try_from(k).unwrap(), |c| c.call = None);
        }
        if let Some(to) = target {
            self.migrate_group_call(to, k, call);
        } else {
            self.emit_group(GroupEffect::GroupCallChanged {
                peer: k,
                old: Some(call),
                new: None,
                input: None,
                schedule_date: 0,
                rtmp: false,
            });
        }
        self.group_changed(k, F::GroupCall.into());
        if let Ok(chat) = ChatKey::try_from(k) {
            self.remove_chat_flags(chat, ChatDataFlag::CallActive | ChatDataFlag::CallNotEmpty);
        } else {
            self.remove_channel_flags(
                ChannelKey::try_from(k).unwrap(),
                ChannelDataFlag::CallActive | ChannelDataFlag::CallNotEmpty,
            );
        }
    }
    pub fn apply_channel_edit_member_rank(
        &self,
        k: ChannelKey,
        user: UserKey,
        rank: &str,
        self_id: UserId,
    ) {
        self.check_scope(user.peer());
        let change = self.edit_channel(k, |c| {
            let i = c.mg_info.as_mut()?;
            let mut change = super::data_channel_admins::ChannelMemberRankChanges::new();
            change.feed(i, peer_to_user(user.peer().id()), rank);
            change.finish(i, self_id)
        });
        if let Some(users) = change {
            self.emit_group(GroupEffect::ApplyGroupAdminChanges {
                peer: k.peer(),
                users,
            });
        }
    }
    pub fn apply_channel_edit_admin(
        &self,
        k: ChannelKey,
        user: UserParticipantFields,
        old: ChatAdminRightsInfo,
        new: ChatAdminRightsInfo,
        rank: &str,
        ctx: &GroupContext<'_>,
    ) {
        self.check_scope(user.key.peer());
        let has_info = self.read_channel(k, |c| c.mg_info.is_some());
        if has_info {
            let missing = self.read_channel(k, |c| {
                !c.mg_info
                    .as_ref()
                    .unwrap()
                    .last_participants
                    .contains(&user.key)
            });
            if missing {
                self.edit_channel(k, |c| {
                    c.mg_info
                        .as_mut()
                        .unwrap()
                        .last_participants
                        .push_front(user.key)
                });
                let count = self.read_channel(k, ChannelData::members_count);
                self.set_channel_members_count(k, count.wrapping_add(1));
                if user.is_bot {
                    self.edit_channel(k, |c| {
                        let i = c.mg_info.as_mut().unwrap();
                        if i.bots.insert(user.key) && i.bot_status == BotStatus::NoBots {
                            i.bot_status = BotStatus::HasBots;
                        }
                    });
                }
            }
            let removed = self.edit_channel(k, |c| {
                c.mg_info
                    .as_mut()
                    .unwrap()
                    .last_restricted
                    .remove(&user.key)
                    .is_some()
            });
            if removed {
                let n = self.read_channel(k, ChannelData::restricted_count);
                if n > 0 {
                    self.set_channel_restricted_count(k, n - 1);
                }
            }
            let previous = self.read_channel(k, |c| {
                c.mg_info
                    .as_ref()
                    .unwrap()
                    .last_admins
                    .contains_key(&user.key)
            });
            if new.flags.any() {
                self.edit_channel(k, |c| {
                    c.mg_info.as_mut().unwrap().last_admins.insert(
                        user.key,
                        Admin {
                            rights: new,
                            can_edit: true,
                        },
                    );
                });
                if !previous {
                    let n = self.read_channel(k, ChannelData::admins_count);
                    self.set_channel_admins_count(k, n.wrapping_add(1));
                }
            } else {
                if previous {
                    self.edit_channel(k, |c| {
                        c.mg_info.as_mut().unwrap().last_admins.remove(&user.key);
                    });
                    let n = self.read_channel(k, ChannelData::admins_count);
                    if n > 0 {
                        self.set_channel_admins_count(k, n - 1);
                    }
                }
            }
            let changed = self.edit_channel(k, |c| {
                let i = c.mg_info.as_mut().unwrap();
                let mut change = super::data_channel_admins::ChannelAdminChanges::new(i);
                if new.flags.any() {
                    change.add(i, peer_to_user(user.key.peer().id()), rank);
                } else {
                    change.remove(i, peer_to_user(user.key.peer().id()));
                }
                change.finish(i, peer_to_user(ctx.self_user.peer().id()))
            });
            if let Some(users) = changed {
                self.emit_group(GroupEffect::ApplyGroupAdminChanges {
                    peer: k.peer(),
                    users,
                });
            }
        }
        if old.flags.any() && new.flags.is_empty() {
            let n = self.read_channel(k, ChannelData::admins_count);
            if n > 1 {
                self.set_channel_admins_count(k, n - 1);
            }
            if !self.read_channel(k, ChannelData::is_megagroup) && user.is_bot {
                let n = self.read_channel(k, ChannelData::members_count);
                if n > 1 {
                    self.set_channel_members_count(k, n - 1);
                }
            }
        } else if old.flags.is_empty() && new.flags.any() {
            let n = self.read_channel(k, ChannelData::admins_count);
            self.set_channel_admins_count(k, n.wrapping_add(1));
            self.request_group_full_forced(k.peer());
        }
        self.group_changed(k.peer(), F::Admins.into());
    }
    pub fn request_group_full_forced(&self, k: PeerKey) {
        self.dispatch(vec![PeerEffect::RequestFullPeer(k)]);
        if let Ok(channel) = ChannelKey::try_from(k) {
            if self.read_channel(channel, |c| !c.am_creator() && c.inviter.is_zero()) {
                self.dispatch(vec![PeerEffect::RequestSelfParticipant(k)]);
            }
        }
    }
    pub fn apply_channel_edit_banned(
        &self,
        k: ChannelKey,
        participant: PeerKey,
        _old: ChatRestrictionsInfo,
        new: ChatRestrictionsInfo,
        ctx: &GroupContext<'_>,
    ) {
        self.check_scope(participant);
        let mut flags = PeerFlags::from(F::BannedUsers);
        let kicked = (new.flags & ChatRestriction::ViewMessages).any();
        let restricted = !kicked && new.flags.any();
        let user = UserKey::try_from(participant).ok();
        let has_info = self.read_channel(k, |c| c.mg_info.is_some());
        if let (true, Some(user)) = (has_info, user) {
            let removed = self.edit_channel(k, |c| {
                c.mg_info
                    .as_mut()
                    .unwrap()
                    .last_admins
                    .remove(&user)
                    .is_some()
            });
            if removed {
                let n = self.read_channel(k, ChannelData::admins_count);
                if n > 1 {
                    self.set_channel_admins_count(k, n - 1);
                } else {
                    flags |= F::Admins;
                }
            }
            if restricted {
                let previous = self.edit_channel(k, |c| {
                    c.mg_info
                        .as_mut()
                        .unwrap()
                        .last_restricted
                        .insert(user, Restricted { rights: new })
                        .is_some()
                });
                if !previous {
                    let n = self.read_channel(k, ChannelData::restricted_count);
                    self.set_channel_restricted_count(k, n.wrapping_add(1));
                }
            } else {
                let removed = self.edit_channel(k, |c| {
                    c.mg_info
                        .as_mut()
                        .unwrap()
                        .last_restricted
                        .remove(&user)
                        .is_some()
                });
                if removed {
                    let n = self.read_channel(k, ChannelData::restricted_count);
                    if n > 0 {
                        self.set_channel_restricted_count(k, n - 1);
                    }
                }
                if kicked {
                    let removed = self.edit_channel(k, |c| {
                        let i = c.mg_info.as_mut().unwrap();
                        if let Some(index) = i.last_participants.iter().position(|u| *u == user) {
                            i.last_participants.remove(index);
                            true
                        } else {
                            i.last_participants_status |=
                                MegagroupInfo::LAST_PARTICIPANTS_COUNT_OUTDATED;
                            false
                        }
                    });
                    if removed {
                        let n = self.read_channel(k, ChannelData::members_count);
                        if n > 1 {
                            self.set_channel_members_count(k, n - 1);
                        } else {
                            self.edit_channel(k, |c| {
                                let i = c.mg_info.as_mut().unwrap();
                                i.last_participants_status |=
                                    MegagroupInfo::LAST_PARTICIPANTS_COUNT_OUTDATED;
                                i.last_participants_count = 0;
                            });
                        }
                    }
                    flags |= F::Members;
                    self.emit_group(GroupEffect::RemoveMegagroupParticipant {
                        peer: k.peer(),
                        user,
                    });
                    let n = self.read_channel(k, ChannelData::kicked_count);
                    self.set_channel_kicked_count(k, n.wrapping_add(1));
                    self.edit_channel(k, |c| {
                        let i = c.mg_info.as_mut().unwrap();
                        if i.bots.remove(&user)
                            && i.bots.is_empty()
                            && i.bot_status == BotStatus::HasBots
                        {
                            i.bot_status = BotStatus::NoBots;
                        }
                    });
                }
            }
            let changed = self.edit_channel(k, |c| {
                let i = c.mg_info.as_mut().unwrap();
                let mut change = super::data_channel_admins::ChannelAdminChanges::new(i);
                change.remove(i, peer_to_user(user.peer().id()));
                change.finish(i, peer_to_user(ctx.self_user.peer().id()))
            });
            if let Some(users) = changed {
                self.emit_group(GroupEffect::ApplyGroupAdminChanges {
                    peer: k.peer(),
                    users,
                });
            }
        } else if !has_info && kicked {
            let n = self.read_channel(k, ChannelData::kicked_count);
            self.set_channel_kicked_count(k, n.wrapping_add(1));
        }
        self.group_changed(k.peer(), flags);
    }
}

impl Peers {
    pub fn apply_group_theme(&self, k: PeerKey, token: &str, ctx: &GroupContext<'_>) {
        let old = self.read(k, |p| p.theme_token().to_owned());
        if old == token {
            return;
        }
        let (same, known) = (ctx.theme_facts)(&old, token);
        self.update(k, |p| p.set_theme_token(token, same, known));
    }
}

impl Peers {
    pub fn resolved_group_name(&self, key: PeerKey) -> String {
        if let Ok(chat) = ChatKey::try_from(key) {
            if let Some(to) = self
                .read_chat(chat, ChatData::get_migrate_to_channel)
                .filter(|k| self.read_channel(*k, ChannelData::am_in))
            {
                return self.resolved_group_name(to.peer());
            }
        } else if let Ok(channel) = ChannelKey::try_from(key) {
            if let Some(to) = self.read_channel(channel, |c| {
                if c.is_megagroup() && c.is_monoforum() {
                    c.monoforum_link()
                } else {
                    None
                }
            }) {
                return self.resolved_group_name(to.peer());
            }
        }
        self.read(key, |p| p.raw_name().to_owned())
    }
}
