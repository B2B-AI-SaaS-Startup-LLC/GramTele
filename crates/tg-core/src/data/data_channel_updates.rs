//! Original channel/community full update model phases; external owners are effects.
use super::{
    data_changes::PeerFlag as F, data_channel::*, data_chat_participant_status::*,
    data_group_model::*, data_msg_id::MsgId, data_peer::*, data_peer_bot_commands::*,
    data_peer_id::*, data_session::Peers, data_store_keys::*, data_types::StickerSetIdentifier,
};
use tg_scheme as tl;
impl Peers {
    pub fn apply_channel_default_banned_rights(
        &self,
        k: ChannelKey,
        u: &tl::UpdateChatDefaultBannedRightsData,
    ) {
        self.set_channel_default_restrictions(
            k,
            ChatRestrictionsInfo::from_mtp(&u.default_banned_rights).flags,
        );
    }
    pub fn apply_channel_full(
        &self,
        k: ChannelKey,
        u: &tl::ChannelFullData,
        ctx: &GroupContext<'_>,
    ) {
        if self.read_channel(k, ChannelData::is_megagroup) {
            let suggest = u
                .vpending_suggestions()
                .is_some_and(|v| v.iter().any(|s| s == b"convert_to_gigagroup"));
            self.emit_group(GroupEffect::SuggestGigagroup {
                peer: k.peer(),
                suggest,
            });
        }
        self.set_channel_available_min_id(
            k,
            MsgId::new(u.vavailable_min_id().unwrap_or_default() as i64),
        );
        let (old_view_admins, old_view_members, old_edit_stickers) = self.read_channel(k, |c| {
            (
                c.can_view_admins(),
                c.can_view_members(),
                c.can_edit_stickers(),
            )
        });
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
        self.set_channel_stars_per_message(
            k,
            u.vsend_paid_messages_stars().unwrap_or_default() as i32,
        );
        self.set_channel_guard_bot_id(k, UserId::new(u.vguard_bot_id().unwrap_or_default()));
        use ChannelDataFlag as G;
        let has_stars = self.read_channel(k, |c| c.stars_per_message() != 0);
        let mask = G::CanSetUsername
            | G::CanViewParticipants
            | G::CanSetStickers
            | G::PreHistoryHidden
            | G::AntiSpam
            | G::Location
            | G::ParticipantsHidden
            | G::CanGetStatistics
            | G::ViewAsMessages
            | G::CanViewRevenue
            | G::PaidMediaAllowed
            | G::CanViewCreditsRevenue
            | G::StargiftsAvailable
            | G::PaidMessagesAvailable
            | G::StarsPerMessageKnown
            | G::HasWelcomeMessages
            | if has_stars {
                G::HasStarsPerMessage.into()
            } else {
                ChannelDataFlags::empty()
            };
        let mut flags = self.read_channel(k, ChannelData::flags) & !mask;
        if u.is_can_set_username() {
            flags |= G::CanSetUsername;
        }
        if u.is_can_view_participants() {
            flags |= G::CanViewParticipants;
        }
        if u.is_can_set_stickers() {
            flags |= G::CanSetStickers;
        }
        if u.is_hidden_prehistory() {
            flags |= G::PreHistoryHidden;
        }
        if u.is_antispam() {
            flags |= G::AntiSpam;
        }
        if u.is_participants_hidden() {
            flags |= G::ParticipantsHidden;
        }
        if u.is_can_view_stats() {
            flags |= G::CanGetStatistics;
        }
        if u.is_view_forum_as_messages() {
            flags |= G::ViewAsMessages;
        }
        if u.is_paid_media_allowed() {
            flags |= G::PaidMediaAllowed;
        }
        if u.is_can_view_revenue() {
            flags |= G::CanViewRevenue;
        }
        if u.is_can_view_stars_revenue() {
            flags |= G::CanViewCreditsRevenue;
        }
        if u.is_stargifts_available() {
            flags |= G::StargiftsAvailable;
        }
        if u.is_paid_messages_available() {
            flags |= G::PaidMessagesAvailable;
        }
        if u.is_has_welcome_messages() {
            flags |= G::HasWelcomeMessages;
        }
        if u.vlocation().is_some() {
            flags |= G::Location;
        }
        if has_stars {
            flags |= G::HasStarsPerMessage;
        }
        flags |= G::StarsPerMessageKnown;
        self.set_channel_flags(k, flags);
        self.emit_group(GroupEffect::ProcessPhoto {
            peer: k.peer(),
            photo: u.chat_photo.clone(),
        });
        if let Some(old) = u.vmigrated_from_chat_id() {
            self.add_channel_flags(k, G::Megagroup.into());
            let chat = self.ensure_chat(ChatId::new(old));
            self.apply_migration(chat, k);
        }
        self.update(k.peer(), |p| {
            p.set_about(&String::from_utf8_lossy(&u.about))
        });
        self.set_channel_members_count(k, u.vparticipants_count().unwrap_or_default());
        self.set_channel_admins_count(k, u.vadmins_count().unwrap_or_default());
        self.set_channel_restricted_count(k, u.vbanned_count().unwrap_or_default());
        self.set_channel_kicked_count(k, u.vkicked_count().unwrap_or_default());
        self.set_channel_slowmode_seconds(k, u.vslowmode_seconds().unwrap_or_default());
        self.set_channel_peer_gifts_count(k, u.vstargifts_count().unwrap_or_default());
        self.update(k.peer(), |p| {
            p.set_main_profile_tab(parse_profile_tab(u.vmain_tab()))
        });
        if let Some(next) = u.vslowmode_next_send_date() {
            let seconds = self.read_channel(k, ChannelData::slowmode_seconds);
            self.grow_channel_slowmode_last_message(k, next.wrapping_sub(seconds), ctx.now);
        }
        self.emit_group(GroupEffect::SetPermanentInvite {
            peer: k.peer(),
            invite: u.vexported_invite().cloned(),
        });
        self.set_channel_location(
            k,
            u.vlocation().unwrap_or(&tl::mtp_channel_location_empty()),
        );
        let linked = u
            .vlinked_chat_id()
            .and_then(|id| self.peer_loaded(peer_from_channel(id)))
            .map(|key| ChannelKey::try_from(key).unwrap());
        self.set_channel_discussion_link(k, linked);
        self.emit_group(GroupEffect::ChannelDialogFields {
            peer: k.peer(),
            available_min: u.vavailable_min_id(),
            folder: u.vfolder_id().unwrap_or_default(),
            pts: u.pts,
            unread: u.unread_count,
            inbox: u.read_inbox_max_id,
            outbox: u.read_outbox_max_id,
        });
        if let Some(message) = u.vpinned_msg_id() {
            self.emit_group(GroupEffect::SetTopPinned {
                peer: k.peer(),
                message,
            });
        }
        if self.read_channel(k, ChannelData::is_megagroup) {
            let commands = u
                .bot_info
                .iter()
                .map(bot_commands_from_tl)
                .collect::<Vec<_>>();
            self.set_channel_bot_commands(k, &commands);
            let sticker = u
                .vstickerset()
                .map(|s| {
                    let tl::StickerSet::StickerSet(d) = s;
                    StickerSetIdentifier {
                        id: d.id,
                        access_hash: d.access_hash,
                        short_name: String::new(),
                    }
                })
                .unwrap_or_default();
            let old_id = self.read_channel(k, |c| c.mg_info.as_ref().unwrap().sticker_set.id);
            let changed = old_edit_stickers != self.read_channel(k, ChannelData::can_edit_stickers)
                || old_id != sticker.id;
            if old_id != sticker.id {
                self.edit_channel(k, |c| c.mg_info.as_mut().unwrap().sticker_set = sticker);
            }
            if changed {
                self.group_changed(k.peer(), F::StickersSet.into());
            }
            let emoji = u
                .vemojiset()
                .map(|s| {
                    let tl::StickerSet::StickerSet(d) = s;
                    StickerSetIdentifier {
                        id: d.id,
                        access_hash: d.access_hash,
                        short_name: String::new(),
                    }
                })
                .unwrap_or_default();
            if self.read_channel(k, |c| c.mg_info.as_ref().unwrap().emoji_set.id) != emoji.id {
                self.edit_channel(k, |c| c.mg_info.as_mut().unwrap().emoji_set = emoji);
                self.group_changed(k.peer(), F::EmojiSet.into());
            }
            self.set_channel_boosts_unrestrict(
                k,
                u.vboosts_applied().unwrap_or_default(),
                u.vboosts_unrestrict().unwrap_or_default(),
            );
        }
        self.apply_group_theme(
            k.peer(),
            &String::from_utf8_lossy(u.vtheme_emoticon().map_or(&[], Vec::as_slice)),
            ctx,
        );
        self.update(k.peer(), |p| {
            p.set_translation_disabled(u.is_translations_disabled())
        });
        let max_count = u.vreactions_limit().unwrap_or_default();
        let paid_enabled = u.is_paid_reactions_available();
        self.set_channel_allowed_reactions(
            k,
            u.vavailable_reactions().map_or(
                AllowedReactions {
                    max_count,
                    paid_enabled,
                    ..Default::default()
                },
                |a| parse_allowed_reactions(a, max_count, paid_enabled),
            ),
        );
        self.emit_group(GroupEffect::BotVerification {
            peer: k.peer(),
            verification: u.vbot_verification().cloned(),
        });
        self.emit_group(GroupEffect::ApplyStories {
            peer: k.peer(),
            stories: u.vstories().cloned(),
        });
        self.update(k.peer(), |p| {
            p.full_updated(ctx.now_millis);
            PeerMutation::quiet(())
        });
        self.set_channel_pending_requests(
            k,
            u.vrequests_pending().unwrap_or_default(),
            u.vrecent_requesters()
                .map_or_else(Vec::new, |v| v.iter().copied().map(UserId::new).collect()),
        );
        if old_view_admins != self.read_channel(k, ChannelData::can_view_admins)
            || old_view_members != self.read_channel(k, ChannelData::can_view_members)
        {
            self.group_changed(k.peer(), F::Rights.into());
        }
        self.emit_group(GroupEffect::ApplyNotify {
            peer: k.peer(),
            settings: u.notify_settings.clone(),
        });
        if let Some(dc) = u.vstats_dc() {
            self.emit_group(GroupEffect::StatsDc { peer: k.peer(), dc });
        }
        self.emit_group(GroupEffect::DefaultSendAs {
            peer: k.peer(),
            send_as: u.vdefault_send_as().map_or(PeerId::new(0), peer_from_mtp),
        });
        self.emit_group(GroupEffect::Wallpaper {
            peer: k.peer(),
            paper: u.vwallpaper().cloned(),
        });
        if (self.read_channel(k, ChannelData::flags)
            & (G::CanViewRevenue | G::CanViewCreditsRevenue))
            .any()
        {
            self.emit_group(GroupEffect::LoadRevenue {
                peer: k.peer(),
                timeout_ms: 60000,
            });
        }
        self.emit_group(GroupEffect::SendHistoryChangeNotifications);
    }
    pub fn apply_community_full(
        &self,
        k: ChannelKey,
        u: &tl::CommunityFullData,
        ctx: &GroupContext<'_>,
    ) {
        self.emit_group(GroupEffect::ProcessPhoto {
            peer: k.peer(),
            photo: u.chat_photo.clone(),
        });
        self.update(k.peer(), |p| {
            p.set_about(&String::from_utf8_lossy(&u.about))
        });
        self.set_channel_admins_count(k, u.vadmins_count().unwrap_or(1));
        self.set_channel_pending_requests(
            k,
            u.vpeer_link_requests_pending().unwrap_or_default(),
            Vec::new(),
        );
        if self.read_channel(k, ChannelData::community_info).is_some() {
            self.emit_group(GroupEffect::CommunityLinkedPeers {
                peer: k.peer(),
                peers: u.linked_peers.clone(),
            });
        }
        self.update(k.peer(), |p| {
            p.full_updated(ctx.now_millis);
            PeerMutation::quiet(())
        });
    }
}
