//! Original ApplyChatUpdate overloads. Owners outside this model receive typed effects.
use super::{
    data_changes::PeerFlag as F, data_chat::*, data_chat_participant_status::*,
    data_group_model::*, data_peer::*, data_peer_bot_commands::*, data_peer_id::*,
    data_session::Peers, data_store_keys::*,
};
use tg_scheme as tl;
impl Peers {
    fn loaded_chat_user(&self, ctx: &GroupContext<'_>, id: u64) -> Option<UserParticipantFields> {
        let u = (ctx.user_loaded)(UserId::new(id))?;
        self.check_scope(u.key.peer());
        assert_eq!(peer_to_user(u.key.peer().id()).bare, id);
        Some(u)
    }
    pub fn apply_chat_participant_add(
        &self,
        k: ChatKey,
        u: &tl::UpdateChatParticipantAddData,
        ctx: &GroupContext<'_>,
    ) {
        if self.apply_chat_update_version(k, u.version) != UpdateStatus::Good
            || self.read_chat(k, |c| c.count < 0)
        {
            return;
        }
        let user = self.loaded_chat_user(ctx, u.user_id);
        if user.is_none()
            || self.read_chat(k, |c| {
                !c.participants.is_empty() && c.participants.contains(&user.unwrap().key)
            })
        {
            self.invalidate_chat_participants(k);
            self.edit_chat(k, |c| c.count = c.count.wrapping_add(1));
            return;
        }
        let user = user.unwrap();
        if self.read_chat(k, |c| c.participants.is_empty()) {
            self.edit_chat(k, |c| {
                if c.count > 0 {
                    c.count = c.count.wrapping_add(1);
                }
                c.bot_status = BotStatus::Unknown;
            });
        } else {
            self.edit_chat(k, |c| {
                c.participants.insert(user.key);
                if UserId::new(u.inviter_id) == peer_to_user(ctx.self_user.peer().id()) {
                    c.invited_by_me.insert(user.key);
                } else {
                    c.invited_by_me.remove(&user.key);
                }
                c.count = c.count.wrapping_add(1);
                if user.is_bot {
                    c.bot_status = BotStatus::HasBots;
                }
            });
            if user.is_bot && !user.bot_info_inited {
                self.dispatch(vec![PeerEffect::RequestFullPeer(user.key.peer())]);
            }
        }
        self.group_changed(k.peer(), F::Members.into());
    }
    pub fn apply_chat_participant_delete(
        &self,
        k: ChatKey,
        u: &tl::UpdateChatParticipantDeleteData,
        ctx: &GroupContext<'_>,
    ) {
        if self.apply_chat_update_version(k, u.version) != UpdateStatus::Good
            || self.read_chat(k, |c| c.count <= 0)
        {
            return;
        }
        let user = self.loaded_chat_user(ctx, u.user_id);
        if user.is_none()
            || self.read_chat(k, |c| {
                !c.participants.is_empty() && !c.participants.contains(&user.unwrap().key)
            })
        {
            self.invalidate_chat_participants(k);
            self.edit_chat(k, |c| c.count = c.count.wrapping_sub(1));
            return;
        }
        let user = user.unwrap();
        if self.read_chat(k, |c| c.participants.is_empty()) {
            self.edit_chat(k, |c| {
                if c.count > 0 {
                    c.count -= 1;
                }
                c.bot_status = BotStatus::Unknown;
            });
        } else {
            self.edit_chat(k, |c| {
                c.participants.remove(&user.key);
                c.count -= 1;
                c.invited_by_me.remove(&user.key);
                c.admins.remove(&user.key);
            });
            if user.is_self {
                self.set_chat_admin_rights(k, ChatAdminRights::empty());
            }
            self.emit_group(GroupEffect::ClearKeyboardIfFrom {
                peer: k.peer(),
                user: user.key.peer(),
            });
            if self.read_chat(k, |c| c.bot_status == BotStatus::HasBots) && user.is_bot {
                self.refresh_chat_bot_status(k, &|key| {
                    (ctx.user_loaded)(peer_to_user(key.peer().id()))
                        .expect("actual participant fields required")
                        .is_bot
                });
            }
        }
        self.group_changed(k.peer(), F::Members.into());
    }
    pub fn apply_chat_participant_admin(
        &self,
        k: ChatKey,
        u: &tl::UpdateChatParticipantAdminData,
        ctx: &GroupContext<'_>,
    ) {
        if self.apply_chat_update_version(k, u.version) != UpdateStatus::Good {
            return;
        }
        let Some(user) = self.loaded_chat_user(ctx, u.user_id) else {
            self.invalidate_chat_participants(k);
            return;
        };
        let is_admin = u.is_admin;
        if user.is_self {
            let rights = if is_admin {
                self.read_chat(k, |c| c.default_admin_rights(user.key, user.is_self).flags)
            } else {
                ChatAdminRights::empty()
            };
            self.set_chat_admin_rights(k, rights);
        }
        if is_admin {
            if self.read_chat(k, ChatData::no_participant_info) {
                self.dispatch(vec![PeerEffect::RequestFullPeer(k.peer())]);
            } else {
                self.edit_chat(k, |c| {
                    c.admins.insert(user.key);
                });
            }
        } else {
            self.edit_chat(k, |c| {
                c.admins.remove(&user.key);
            });
        }
        self.group_changed(k.peer(), F::Admins.into());
    }
    pub fn apply_chat_participant_rank(
        &self,
        k: ChatKey,
        u: &tl::UpdateChatParticipantRankData,
        ctx: &GroupContext<'_>,
    ) {
        if self.apply_chat_update_version(k, u.version) != UpdateStatus::Good {
            return;
        }
        let id = UserId::new(u.user_id);
        let rank = String::from_utf8_lossy(&u.rank);
        self.edit_chat(k, |c| {
            if rank.is_empty() {
                c.member_ranks.remove(&id);
            } else {
                c.member_ranks.insert(id, rank.into_owned());
            }
        });
        if id != peer_to_user(ctx.self_user.peer().id()) {
            self.emit_group(GroupEffect::ApplyGroupAdminChanges {
                peer: k.peer(),
                users: [id].into_iter().collect(),
            });
        }
        self.group_changed(k.peer(), F::Members.into());
    }
    pub fn apply_chat_default_banned_rights(
        &self,
        k: ChatKey,
        u: &tl::UpdateChatDefaultBannedRightsData,
    ) {
        if self.apply_chat_update_version(k, u.version) == UpdateStatus::Good {
            self.set_chat_default_restrictions(
                k,
                ChatRestrictionsInfo::from_mtp(&u.default_banned_rights).flags,
            );
        }
    }
    pub fn apply_chat_participants(
        &self,
        k: ChatKey,
        u: &tl::ChatParticipants,
        ctx: &GroupContext<'_>,
    ) {
        match u {
            tl::ChatParticipants::ChatParticipantsForbidden(_) => {
                self.edit_chat(k, |c| c.count = -1);
                self.invalidate_chat_participants(k);
            }
            tl::ChatParticipants::ChatParticipants(d) => {
                if self.apply_chat_update_version(k, d.version) == UpdateStatus::TooOld {
                    return;
                }
                self.edit_chat(k, |c| {
                    c.version = d.version;
                    c.count = d.participants.len() as i32;
                    c.participants.clear();
                    c.invited_by_me.clear();
                    c.admins.clear();
                    c.member_ranks.clear();
                });
                self.set_chat_admin_rights(k, ChatAdminRights::empty());
                for p in &d.participants {
                    use tl::ChatParticipant::*;
                    let (id, inviter, rank, creator, admin) = match p {
                        ChatParticipant(d) => (d.user_id, d.inviter_id, d.vrank(), false, false),
                        ChatParticipantAdmin(d) => {
                            (d.user_id, d.inviter_id, d.vrank(), false, true)
                        }
                        ChatParticipantCreator(d) => (d.user_id, 0, d.vrank(), true, false),
                    };
                    let Some(user) = self.loaded_chat_user(ctx, id) else {
                        self.invalidate_chat_participants(k);
                        break;
                    };
                    self.edit_chat(k, |c| {
                        c.participants.insert(user.key);
                        if UserId::new(inviter) == peer_to_user(ctx.self_user.peer().id()) {
                            c.invited_by_me.insert(user.key);
                        }
                        if creator {
                            c.creator = UserId::new(id);
                        }
                        if admin {
                            c.admins.insert(user.key);
                        }
                    });
                    if admin && user.is_self {
                        let rights =
                            self.read_chat(k, |c| c.default_admin_rights(user.key, true).flags);
                        self.set_chat_admin_rights(k, rights);
                    }
                    if let Some(rank) = rank.filter(|r| !r.is_empty()) {
                        self.edit_chat(k, |c| {
                            c.member_ranks
                                .insert(UserId::new(id), String::from_utf8_lossy(rank).into());
                        });
                    }
                }
                if self.read_chat(k, |c| c.participants.is_empty()) {
                    return;
                }
                let users =
                    self.read_chat(k, |c| c.participants.iter().map(|k| k.peer()).collect());
                self.emit_group(GroupEffect::CheckKeyboardParticipants {
                    peer: k.peer(),
                    users,
                });
                self.refresh_chat_bot_status(k, &|key| {
                    (ctx.user_loaded)(peer_to_user(key.peer().id()))
                        .expect("actual participant fields required")
                        .is_bot
                });
                self.group_changed(k.peer(), F::Members | F::Admins);
            }
        }
    }
    pub fn apply_chat_full(&self, k: ChatKey, u: &tl::ChatFullData, ctx: &GroupContext<'_>) {
        self.apply_chat_participants(k, &u.participants, ctx);
        if let Some(call) = u.vcall() {
            self.set_group_call(k.peer(), Some(call), 0, false);
        } else {
            self.clear_group_call(k.peer());
        }
        self.set_group_call_join_as(
            k.peer(),
            u.vgroupcall_default_join_as()
                .map_or(PeerId::new(0), peer_from_mtp),
        );
        self.update(k.peer(), |p| {
            p.set_messages_ttl(u.vttl_period().unwrap_or_default())
        });
        let commands = u
            .vbot_info()
            .map_or_else(Vec::new, |v| v.iter().map(bot_commands_from_tl).collect());
        self.set_chat_bot_commands(k, &commands);
        let mask = ChatDataFlag::CanSetUsername | ChatDataFlag::HasWelcomeMessages;
        let old = self.read_chat(k, ChatData::flags);
        self.set_chat_flags(
            k,
            (old & !mask)
                | if u.is_can_set_username() {
                    ChatDataFlag::CanSetUsername.into()
                } else {
                    ChatDataFlags::empty()
                }
                | if u.is_has_welcome_messages() {
                    ChatDataFlag::HasWelcomeMessages.into()
                } else {
                    ChatDataFlags::empty()
                },
        );
        self.emit_group(GroupEffect::ProcessPhoto {
            peer: k.peer(),
            photo: u
                .vchat_photo()
                .cloned()
                .unwrap_or_else(|| tl::mtp_photo_empty(0)),
        });
        self.emit_group(GroupEffect::SetPermanentInvite {
            peer: k.peer(),
            invite: u.vexported_invite().cloned(),
        });
        if let Some(message) = u.vpinned_msg_id() {
            self.emit_group(GroupEffect::SetTopPinned {
                peer: k.peer(),
                message,
            });
        }
        self.emit_group(GroupEffect::CheckFolder {
            peer: k.peer(),
            folder: u.vfolder_id().unwrap_or_default(),
        });
        self.apply_group_theme(
            k.peer(),
            &String::from_utf8_lossy(u.vtheme_emoticon().map_or(&[], Vec::as_slice)),
            ctx,
        );
        self.update(k.peer(), |p| {
            p.set_translation_disabled(u.is_translations_disabled())
        });
        let max_count = u.vreactions_limit().unwrap_or_default();
        self.set_chat_allowed_reactions(
            k,
            u.vavailable_reactions().map_or(
                AllowedReactions {
                    max_count,
                    ..Default::default()
                },
                |v| parse_allowed_reactions(v, max_count, false),
            ),
        );
        self.update(k.peer(), |p| {
            p.full_updated(ctx.now_millis);
            PeerMutation::quiet(())
        });
        self.update(k.peer(), |p| {
            p.set_about(&String::from_utf8_lossy(&u.about))
        });
        self.set_chat_pending_requests(
            k,
            u.vrequests_pending().unwrap_or_default(),
            u.vrecent_requesters()
                .map_or_else(Vec::new, |v| v.iter().copied().map(UserId::new).collect()),
        );
        self.emit_group(GroupEffect::ApplyNotify {
            peer: k.peer(),
            settings: u.notify_settings.clone(),
        });
    }
}
impl Peers {
    pub fn apply_chat_participants_update(
        &self,
        k: ChatKey,
        u: &tl::UpdateChatParticipantsData,
        ctx: &GroupContext<'_>,
    ) {
        self.apply_chat_participants(k, &u.participants, ctx);
    }
}
