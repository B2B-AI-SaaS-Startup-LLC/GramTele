use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use tg_core::data::{
    data_changes::{Changes, PeerFlag},
    data_channel::*,
    data_channel_admins::*,
    data_chat::*,
    data_chat_participant_status::*,
    data_group_model::*,
    data_message_reaction_id::*,
    data_peer::*,
    data_peer_bot_commands::*,
    data_peer_id::*,
    data_session::{NameIndex, Peers},
    data_store_keys::*,
    data_user_names::*,
};
use tg_rpl::Lifetime;
use tg_runtime::MainQueue;
use tg_scheme as tl;
fn setup() -> (SessionScope, MainQueue, Rc<Peers>, Changes) {
    let scope = SessionScope::new();
    let queue = MainQueue::new();
    let changes = Changes::with_queue(&scope, queue.clone());
    let store = Rc::new(Peers::new(&scope, changes.clone()));
    (scope, queue, store, changes)
}
fn theme_facts(_old: &str, _new: &str) -> (bool, bool) {
    (true, false)
}
fn unhex(s: &str) -> String {
    String::from_utf8(
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect(),
    )
    .unwrap()
}
#[test]
fn native_rights_flags_normalization_zalgo_and_forever() {
    let (_scope, _q, p, _c) = setup();
    let k = p.ensure_channel(ChannelId::new(1));
    for l in include_str!("fixtures/data-chat-channel/pure.csv").lines() {
        let a: Vec<_> = l.split(',').collect();
        match a[0] {
            "rights" => {
                let raw = a[1].parse::<u32>().unwrap();
                let admin = ChatAdminRightsInfo::from_mtp(&tl::mtp_chat_admin_rights(
                    tl::tl::Flags::from_raw(raw as u64),
                ));
                let banned = ChatRestrictionsInfo::from_mtp(&tl::mtp_chat_banned_rights(
                    tl::tl::Flags::from_raw(raw as u64),
                    -1,
                ));
                assert_eq!(admin.flags.value().to_string(), a[2], "{l}");
                let tl::ChatAdminRights::ChatAdminRights(d) = admin_rights_to_mtp(admin);
                assert_eq!(d.flags.value().to_string(), a[3], "{l}");
                assert_eq!(banned.flags.value().to_string(), a[4], "{l}");
                let tl::ChatBannedRights::ChatBannedRights(d) = restrictions_to_mtp(banned);
                assert_eq!(d.flags.value().to_string(), a[5], "{l}");
                assert_eq!(d.until_date, -1);
            }
            "send" => assert_eq!(
                [
                    all_send_restrictions().value(),
                    files_send_restrictions().value(),
                    tabbed_panel_send_restrictions().value()
                ],
                [
                    a[1].parse::<i32>().unwrap(),
                    a[2].parse::<i32>().unwrap(),
                    a[3].parse::<i32>().unwrap()
                ]
            ),
            "zalgo" => assert_eq!(
                tg_core::ayu::utils::telegram_helpers::filter_zalgo(&unhex(a[1])),
                unhex(a[2]),
                "{l}"
            ),
            "mn" => {
                let start = a[1].parse::<u32>().unwrap();
                let end = a[2].parse::<u32>().unwrap();
                for ch in [start, end] {
                    assert!(tg_core::ayu::utils::telegram_helpers::is_mn(
                        char::from_u32(ch).unwrap()
                    ));
                }
            }
            "normalize" => {
                p.set_channel_flags(k, ChannelDataFlags::from_raw(a[1].parse().unwrap()));
                assert_eq!(
                    p.read_channel(k, |c| c.flags().value()).to_string(),
                    a[2],
                    "{l}"
                );
            }
            "forever" => assert_eq!(
                ChannelData::is_restricted_forever(a[1].parse().unwrap()) as u8,
                a[2].parse::<u8>().unwrap()
            ),
            _ => panic!("unknown {l}"),
        }
    }
}
#[test]
fn real_records_defaults_scope_and_common_base() {
    let (s, _q, p, _c) = setup();
    let chat = p.ensure_chat(ChatId::new(10));
    let chan = p.ensure_channel(ChannelId::new(11));
    assert_eq!(chat.peer(), p.ensure_peer(peer_from_chat(10u64)));
    p.read_chat(chat, |c| {
        assert_eq!(c.key(), chat);
        assert_eq!(c.peer.session(), s.id());
        assert_eq!(c.count, 0);
        assert!(c.am_in());
        assert!(c.no_participant_info());
        assert!(!c.peer.is_loaded());
        assert_eq!(c.input_chat(), 10);
    });
    p.read_channel(chan, |c| {
        assert!(c.is_forbidden());
        assert!(!c.am_in());
        assert_eq!(c.members_count(), 1);
        assert!(!c.members_count_known());
        assert_eq!(c.admins_count(), 1);
        assert!(c.mg_info.is_none());
        assert_eq!(c.peer.key(), chan.peer());
    });
    p.update(chan.peer(), |b| {
        b.set_loaded_status(LoadedStatus::Normal);
        PeerMutation::quiet(())
    });
    assert_eq!(p.peer_loaded(chan.peer().id()), Some(chan.peer()));
}
#[test]
#[should_panic(expected = "foreign SessionScope key")]
fn groups_reject_foreign_scope() {
    let (a, _q, p, _) = setup();
    let b = SessionScope::new();
    let _ = a;
    p.read_channel(
        ChannelKey::try_from(b.peer_key(peer_from_channel(1u64))).unwrap(),
        |_| (),
    );
}
#[test]
fn flags_callbacks_run_after_borrow_and_before_followup() {
    let (_s, _q, p, _c) = setup();
    let k = p.ensure_chat(ChatId::new(1));
    let life = Lifetime::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let o = log.clone();
    let p2 = p.clone();
    p.group_effects().on_next(
        move |e| {
            if matches!(e, GroupEffect::UpdateChatListEntry(_)) {
                assert!((p2.read_chat(k, ChatData::flags) & ChatDataFlag::CallNotEmpty).any());
                o.borrow_mut().push("history");
            }
        },
        &life,
    );
    let p2 = p.clone();
    let o = log.clone();
    p.read_chat(k, ChatData::flags_value).on_next(
        move |c| {
            if (c.diff & ChatDataFlag::CallNotEmpty).any() {
                o.borrow_mut().push("client");
                p2.edit_chat(k, |c| c.count = 2);
            }
        },
        &life,
    );
    log.borrow_mut().clear(); // flagsValue emits its initial all-essential diff.
    p.set_chat_flags(k, ChatDataFlag::CallNotEmpty.into());
    assert_eq!(*log.borrow(), vec!["history", "client"]);
    assert_eq!(p.read_chat(k, |c| c.count), 2);
}
#[test]
fn chat_permissions_versions_and_partial_participant_invalidation() {
    let (_s, _q, p, c) = setup();
    let k = p.ensure_chat(ChatId::new(1));
    p.set_chat_default_restrictions(
        k,
        ChatRestriction::ChangeInfo | ChatRestriction::AddParticipants,
    );
    p.read_chat(k, |c| {
        assert!(!c.can_edit_information());
        assert!(!c.can_add_members());
        assert!(!c.anyone_can_add_members());
    });
    p.set_chat_admin_rights(
        k,
        ChatAdminRight::ChangeInfo | ChatAdminRight::InviteByLinkOrAdd,
    );
    p.read_chat(k, |c| {
        assert!(c.can_edit_information());
        assert!(c.can_add_members());
        assert!(c.can_have_invite_link());
    });
    p.set_chat_pending_requests(k, 3, vec![UserId::new(2)]);
    p.set_chat_admin_rights(k, ChatAdminRights::empty());
    assert_eq!(p.read_chat(k, ChatData::pending_requests_count), 0);
    p.set_chat_version(k, 3);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    let life = Lifetime::new();
    c.peer_updates(PeerFlag::Members)
        .on_next(move |u| out.borrow_mut().push(u.flags), &life);
    assert_eq!(p.apply_chat_update_version(k, 2), UpdateStatus::TooOld);
    assert_eq!(p.apply_chat_update_version(k, 6), UpdateStatus::Skipped);
    assert_eq!(p.read_chat(k, ChatData::version), 3);
    c.send_notifications();
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(p.apply_chat_update_version(k, 4), UpdateStatus::Good);
    assert_eq!(p.read_chat(k, ChatData::version), 4);
}
#[test]
fn channel_rights_public_boosts_hidden_members_and_subtype_projections() {
    let (s, _q, p, _c) = setup();
    let k = p.ensure_channel(ChannelId::new(1));
    let me = UserKey::try_from(s.peer_key(peer_from_user(2u64))).unwrap();
    p.set_channel_flags(
        k,
        ChannelDataFlag::Megagroup
            | ChannelDataFlag::CanViewParticipants
            | ChannelDataFlag::ParticipantsHidden
            | ChannelDataFlag::Username,
    );
    p.set_channel_default_restrictions(
        k,
        ChatRestriction::ChangeInfo | ChatRestriction::PinMessages | ChatRestriction::SendOther,
    );
    p.read_channel(k, |c| {
        assert!(!c.can_view_members());
        assert!(!c.can_edit_information());
        assert!(!c.can_pin_messages());
        assert!(c.can_view_admins());
        assert!(!c.can_edit_permissions());
        assert_eq!(
            c.peer.can_pin_messages(&c.peer_fields(), false),
            c.can_pin_messages()
        );
    });
    p.set_channel_boosts_unrestrict(k, 2, 2);
    p.read_channel(k, |c| {
        assert!(c.unrestricted_by_boosts());
        assert!(c.can_edit_information());
        assert!(c.can_pin_messages());
        assert_eq!(
            c.peer.can_pin_messages(&c.peer_fields(), false),
            c.can_pin_messages()
        );
    });
    p.set_channel_admin_rights(k, ChatAdminRight::BanUsers | ChatAdminRight::AddAdmins, me);
    p.read_channel(k, |c| {
        assert!(c.can_view_members());
        assert!(c.can_edit_permissions());
        assert!(c.can_add_admins());
        assert!(!c.can_edit_username());
    });
    p.add_channel_flags(k, ChannelDataFlag::Gigagroup.into());
    assert!(!p.read_channel(k, ChannelData::can_edit_permissions));
    p.add_channel_flags(k, ChannelDataFlag::MonoforumDisabled.into());
    assert!(!p.read_channel(k, ChannelData::can_pin_messages));
}
#[test]
fn flags_forum_monoforum_membership_migration_and_retained_links() {
    let (_s, _q, p, _c) = setup();
    let chat = p.ensure_chat(ChatId::new(1));
    let mono = p.ensure_channel(ChannelId::new(2));
    let broad = p.ensure_channel(ChannelId::new(3));
    p.set_channel_flags(mono, ChannelDataFlag::Forum.into());
    assert!(p.read_channel(mono, |c| c.is_megagroup() && c.forum().is_some()));
    p.set_channel_flags(broad, ChannelDataFlag::Broadcast | ChannelDataFlag::Creator);
    p.set_channel_flags(mono, ChannelDataFlag::Monoforum.into());
    p.set_channel_monoforum_link(broad, Some(mono));
    p.read_channel(mono, |c| {
        assert!(c.am_monoforum_admin());
        assert!(c.monoforum().is_some());
        assert!(!c.is_forum());
    });
    p.set_channel_monoforum_link(broad, None);
    p.read_channel(mono, |c| {
        assert!(c.monoforum_disabled());
        assert_eq!(c.monoforum_link(), Some(broad));
    });
    assert_eq!(
        p.read_channel(broad, ChannelData::monoforum_link),
        Some(mono)
    );
    p.apply_migration(chat, mono);
    assert_eq!(
        p.read_chat(chat, ChatData::get_migrate_to_channel),
        Some(mono)
    );
    assert_eq!(
        p.read_channel(mono, ChannelData::get_migrate_from_chat),
        Some(chat)
    );
}
#[test]
fn slowmode_clock_correction_and_boosts_reentrant_delivery() {
    let (_s, _q, p, c) = setup();
    let k = p.ensure_channel(ChannelId::new(1));
    p.set_channel_flags(
        k,
        ChannelDataFlag::Megagroup | ChannelDataFlag::SlowmodeEnabled,
    );
    p.set_channel_slowmode_seconds(k, 30);
    p.grow_channel_slowmode_last_message(k, 200, 100);
    assert_eq!(p.read_channel(k, ChannelData::slowmode_last_message), 100);
    p.grow_channel_slowmode_last_message(k, 80, 90);
    assert_eq!(p.read_channel(k, ChannelData::slowmode_last_message), 80);
    let life = Lifetime::new();
    let out = Rc::new(RefCell::new(Vec::new()));
    let a = out.clone();
    let pp = p.clone();
    p.read_channel(k, ChannelData::unrestricted_by_boosts_value)
        .on_next(
            move |b| {
                a.borrow_mut().push(b);
                if b {
                    pp.set_channel_members_count(k, 5);
                }
            },
            &life,
        );
    let updates = Rc::new(Cell::new(0));
    let u = updates.clone();
    c.peer_updates(PeerFlag::Rights)
        .on_next(move |_| u.set(u.get() + 1), &life);
    p.set_channel_boosts_unrestrict(k, 1, 2);
    p.set_channel_boosts_unrestrict(k, 2, 2);
    p.set_channel_boosts_unrestrict(k, 3, 2);
    assert_eq!(*out.borrow(), vec![false, true]);
    c.send_notifications();
    assert_eq!(updates.get(), 1);
    assert_eq!(p.read_channel(k, ChannelData::slowmode_last_message), 0);
}
fn context<'a>(
    me: UserKey,
    users: &'a dyn Fn(UserId) -> Option<UserParticipantFields>,
) -> GroupContext<'a> {
    GroupContext {
        self_user: me,
        now: 100,
        now_millis: 1000,
        filter_zalgo: true,
        theme_facts: &theme_facts,
        user_loaded: users,
    }
}
#[test]
fn edited_admin_original_double_counts_bans_and_stale_emplace() {
    let (s, _q, p, _) = setup();
    let k = p.ensure_channel(ChannelId::new(1));
    let me = UserKey::try_from(p.ensure_peer(peer_from_user(2u64))).unwrap();
    let user = UserParticipantFields {
        key: UserKey::try_from(p.ensure_peer(peer_from_user(3u64))).unwrap(),
        is_self: false,
        is_bot: true,
        bot_info_inited: false,
    };
    let ctx = context(me, &|_| None);
    p.set_channel_flags(k, ChannelDataFlag::Megagroup.into());
    let rights = ChatAdminRightsInfo {
        flags: ChatAdminRight::ChangeInfo.into(),
    };
    p.apply_channel_edit_admin(
        k,
        user,
        ChatAdminRightsInfo::default(),
        rights,
        "rank",
        &ctx,
    );
    p.read_channel(k, |c| {
        assert_eq!(c.members_count(), 2);
        assert_eq!(c.admins_count(), 3);
        let i = c.mg_info.as_ref().unwrap();
        assert!(i.bots.contains(&user.key));
        assert!(i.admins.contains(&UserId::new(3)));
        assert_eq!(i.member_ranks[&UserId::new(3)], "rank");
    });
    p.apply_channel_edit_admin(k, user, rights, ChatAdminRightsInfo::default(), "", &ctx);
    assert_eq!(p.read_channel(k, ChannelData::admins_count), 1);
    p.set_channel_admin_rights(k, ChatAdminRight::ChangeInfo.into(), me);
    p.set_channel_admin_rights(k, ChatAdminRight::BanUsers.into(), me);
    p.read_channel(k, |c| {
        assert_eq!(
            c.admin_rights(),
            ChatAdminRights::from(ChatAdminRight::BanUsers)
        );
        assert_eq!(
            c.mg_info.as_ref().unwrap().last_admins[&me].rights.flags,
            ChatAdminRights::from(ChatAdminRight::ChangeInfo)
        );
    });
    let first = ChatRestrictionsInfo {
        flags: ChatRestriction::SendOther.into(),
        until: 500,
    };
    p.set_channel_restrictions(k, first, me);
    p.set_channel_restrictions(
        k,
        ChatRestrictionsInfo {
            flags: ChatRestriction::SendPhotos.into(),
            until: 600,
        },
        me,
    );
    p.read_channel(k, |c| {
        assert_eq!(c.restricted_until(), 600);
        assert_eq!(
            c.mg_info.as_ref().unwrap().last_restricted[&me].rights,
            first
        );
        assert!(!c.has_restrictions_at(600));
    });
    p.apply_channel_edit_banned(
        k,
        user.key.peer(),
        ChatRestrictionsInfo::default(),
        ChannelData::kicked_restricted_rights(true),
        &ctx,
    );
    p.read_channel(k, |c| {
        let i = c.mg_info.as_ref().unwrap();
        assert!(!i.bots.contains(&user.key));
        assert!(!i.last_participants.contains(&user.key));
        assert_eq!(c.kicked_count(), 1);
        assert!(i.member_ranks.contains_key(&UserId::new(3)));
    });
    let _ = s;
}
#[test]
fn admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self() {
    let (s, _q, p, _) = setup();
    let me = UserKey::try_from(s.peer_key(peer_from_user(1u64))).unwrap();
    let other = UserKey::try_from(s.peer_key(peer_from_user(2u64))).unwrap();
    let mut info = MegagroupInfo::default();
    let mut batch = ChannelAdminChanges::new(&info);
    batch.add(&mut info, UserId::new(1), "self");
    assert!(batch.finish(&info, UserId::new(1)).is_none());
    let mut batch = ChannelAdminChanges::new(&info);
    batch.add(&mut info, UserId::new(2), "other");
    batch.remove(&mut info, UserId::new(2));
    assert_eq!(info.member_ranks[&UserId::new(2)], "other");
    assert_eq!(
        batch.finish(&info, UserId::new(1)).unwrap(),
        BTreeSet::from([UserId::new(2)])
    );
    info.creator = Some(me);
    let batch = ChannelAdminChanges::new(&info);
    info.creator = Some(other);
    assert_eq!(
        batch.finish(&info, UserId::new(1)).unwrap(),
        BTreeSet::from([UserId::new(1), UserId::new(2)])
    );
    let mut ranks = ChannelMemberRankChanges::new();
    ranks.feed(&mut info, UserId::new(2), "creator");
    ranks.feed(&mut info, UserId::new(3), "normal member");
    assert_eq!(
        ranks.finish(&info, UserId::new(1)).unwrap(),
        BTreeSet::from([UserId::new(2)])
    );
    drop(p);
}
#[test]
fn chat_tl_versions_missing_users_forbidden_and_bot_participants() {
    let (_s, _q, p, _) = setup();
    let k = p.ensure_chat(ChatId::new(1));
    let me = UserKey::try_from(p.ensure_peer(peer_from_user(2u64))).unwrap();
    let bot = UserKey::try_from(p.ensure_peer(peer_from_user(3u64))).unwrap();
    let users = BTreeMap::from([
        (
            UserId::new(2),
            UserParticipantFields {
                key: me,
                is_self: true,
                is_bot: false,
                bot_info_inited: true,
            },
        ),
        (
            UserId::new(3),
            UserParticipantFields {
                key: bot,
                is_self: false,
                is_bot: true,
                bot_info_inited: false,
            },
        ),
    ]);
    let lookup = |id| users.get(&id).copied();
    let ctx = context(me, &lookup);
    let participants = tl::mtp_chat_participants(
        1,
        vec![
            tl::mtp_chat_participant_creator(tl::tl::Flags::empty(), 2, None),
            tl::mtp_chat_participant(tl::tl::Flags::empty(), 3, 2, 1, None),
        ],
        4,
    );
    p.apply_chat_participants(k, &participants, &ctx);
    p.read_chat(k, |c| {
        assert_eq!(c.version(), 4);
        assert_eq!(c.count, 2);
        assert_eq!(c.creator, UserId::new(2));
        assert!(c.invited_by_me.contains(&bot));
        assert_eq!(c.bot_status, BotStatus::HasBots);
    });
    let tl::Update::UpdateChatParticipantDelete(update) =
        tl::mtp_update_chat_participant_delete(1, 3, 5)
    else {
        unreachable!()
    };
    p.apply_chat_participant_delete(k, &update, &ctx);
    assert_eq!(p.read_chat(k, |c| c.bot_status), BotStatus::NoBots);
    let tl::Update::UpdateChatParticipantAdd(update) =
        tl::mtp_update_chat_participant_add(1, 999, 2, 1, 6)
    else {
        unreachable!()
    };
    p.apply_chat_participant_add(k, &update, &ctx);
    p.read_chat(k, |c| {
        assert_eq!(c.count, 2);
        assert!(c.participants.is_empty());
        assert_eq!(c.version(), 6);
    });
    p.apply_chat_participants(
        k,
        &tl::mtp_chat_participants_forbidden(tl::tl::Flags::empty(), 1, None),
        &ctx,
    );
    assert_eq!(p.read_chat(k, |c| c.count), -1);
}
#[test]
fn names_zalgo_usernames_source_update_order() {
    let (_s, _q, p, _) = setup();
    let chat = p.ensure_chat(ChatId::new(1));
    let channel = p.ensure_channel(ChannelId::new(2));
    p.set_chat_name(chat, "a\u{301}\u{302}\u{303}", true, || {
        NameIndex::default()
    });
    assert_eq!(
        p.read(chat.peer(), |c| c.raw_name().to_owned()),
        "a\u{2060}\u{2060}\u{2060}"
    );
    p.set_channel_name(channel, "Chan", "first", true, NameIndex::default);
    p.read_channel(channel, |c| {
        assert_eq!(c.peer.raw_name(), "Chan");
        assert_eq!(c.username(), "first");
        assert!((c.flags() & ChannelDataFlag::Username).any());
    });
    p.set_channel_usernames(
        channel,
        &vec![Username {
            username: "alias".into(),
            active: true,
            editable: false,
        }],
    );
    p.read_channel(channel, |c| {
        assert_eq!(c.username(), "alias");
        assert_eq!(c.editable_username(), "");
    });
}
#[test]
fn group_call_keys_migrate_and_clear_without_groupcall_standin() {
    let (_s, _q, p, _) = setup();
    let chat = p.ensure_chat(ChatId::new(1));
    let channel = p.ensure_channel(ChannelId::new(2));
    p.set_group_call(
        chat.peer(),
        Some(&tl::mtp_input_group_call(100, 7)),
        0,
        false,
    );
    let call = p.read_chat(chat, ChatData::group_call).unwrap();
    assert_eq!(call.id(), 100);
    p.set_channel_flags(channel, ChannelDataFlag::Megagroup.into());
    p.apply_migration(chat, channel);
    p.clear_group_call(chat.peer());
    assert!(p.read_chat(chat, ChatData::group_call).is_none());
    assert_eq!(p.read_channel(channel, ChannelData::group_call), Some(call));
    p.clear_group_call(channel.peer());
    assert!(p.read_channel(channel, ChannelData::group_call).is_none());
    assert!((p.read_channel(channel, ChannelData::flags) & ChannelDataFlag::CallActive).is_empty());
}

fn chat_full_data() -> tl::ChatFullData {
    tl::ChatFullData {
        flags: tl::tl::Flags::empty(),
        id: 0,
        about: Vec::new(),
        participants: tl::mtp_chat_participants(1, Vec::new(), 0),
        chat_photo: None,
        notify_settings: tl::mtp_peer_notify_settings(
            tl::tl::Flags::empty(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        exported_invite: None,
        bot_info: None,
        pinned_msg_id: None,
        folder_id: None,
        call: None,
        ttl_period: None,
        groupcall_default_join_as: None,
        theme_emoticon: None,
        requests_pending: None,
        recent_requesters: None,
        available_reactions: None,
        reactions_limit: None,
    }
}

fn channel_full_data() -> tl::ChannelFullData {
    tl::ChannelFullData {
        flags: tl::tl::Flags::empty(),
        id: 0,
        about: Vec::new(),
        participants_count: None,
        admins_count: None,
        kicked_count: None,
        banned_count: None,
        online_count: None,
        read_inbox_max_id: 0,
        read_outbox_max_id: 0,
        unread_count: 0,
        chat_photo: tl::mtp_photo_empty(0),
        notify_settings: tl::mtp_peer_notify_settings(
            tl::tl::Flags::empty(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        exported_invite: None,
        bot_info: Vec::new(),
        migrated_from_chat_id: None,
        migrated_from_max_id: None,
        pinned_msg_id: None,
        stickerset: None,
        available_min_id: None,
        folder_id: None,
        linked_chat_id: None,
        location: None,
        slowmode_seconds: None,
        slowmode_next_send_date: None,
        stats_dc: None,
        pts: 0,
        call: None,
        ttl_period: None,
        pending_suggestions: None,
        groupcall_default_join_as: None,
        theme_emoticon: None,
        requests_pending: None,
        recent_requesters: None,
        default_send_as: None,
        available_reactions: None,
        reactions_limit: None,
        stories: None,
        wallpaper: None,
        boosts_applied: None,
        boosts_unrestrict: None,
        emojiset: None,
        bot_verification: None,
        stargifts_count: None,
        send_paid_messages_stars: None,
        main_tab: None,
        guard_bot_id: None,
    }
}

fn community_full_data() -> tl::CommunityFullData {
    tl::CommunityFullData {
        flags: tl::tl::Flags::empty(),
        id: 0,
        about: Vec::new(),
        chat_photo: tl::mtp_photo_empty(0),
        linked_peers: Vec::new(),
        admins_count: None,
        kicked_count: None,
        peer_link_requests_pending: None,
    }
}
#[test]
fn tl_full_updates_apply_scalar_state_and_preserve_stale_star_flag() {
    let (_s, _q, p, _) = setup();
    let me = UserKey::try_from(p.ensure_peer(peer_from_user(10u64))).unwrap();
    let lookup = |_| None;
    let ctx = context(me, &lookup);
    let chat = p.ensure_chat(ChatId::new(1));
    let channel = p.ensure_channel(ChannelId::new(2));
    let mut full = chat_full_data();
    full.about = b"about".to_vec();
    full.flags = tl::ChatFullData::F_TTL_PERIOD
        | tl::ChatFullData::F_REQUESTS_PENDING
        | tl::ChatFullData::F_RECENT_REQUESTERS
        | tl::ChatFullData::F_HAS_WELCOME_MESSAGES;
    full.ttl_period = Some(123);
    full.requests_pending = Some(2);
    full.recent_requesters = Some(vec![10, 11]);
    p.apply_chat_full(chat, &full, &ctx);
    p.read_chat(chat, |c| {
        assert_eq!(c.peer.about(), "about");
        assert_eq!(c.peer.messages_ttl(), 123);
        assert!(c.peer.is_full_loaded());
        assert_eq!(c.pending_requests_count(), 2);
        assert!(c.has_welcome_messages());
    });
    p.set_channel_flags(
        channel,
        ChannelDataFlag::Megagroup | ChannelDataFlag::HasStarsPerMessage,
    );
    let mut full = channel_full_data();
    full.about = b"channel about".to_vec();
    full.flags = tl::ChannelFullData::F_SLOWMODE_SECONDS
        | tl::ChannelFullData::F_BOOSTS_APPLIED
        | tl::ChannelFullData::F_BOOSTS_UNRESTRICT;
    full.slowmode_seconds = Some(30);
    full.boosts_applied = Some(2);
    full.boosts_unrestrict = Some(2);
    p.apply_channel_full(channel, &full, &ctx);
    p.read_channel(channel, |c| {
        assert_eq!(c.peer.about(), "channel about");
        assert!(c.peer.is_full_loaded());
        assert_eq!(c.stars_per_message(), 0);
        assert!(c.has_stars_per_message());
        assert!(c.stars_per_message_known());
        assert!(c.unrestricted_by_boosts());
        assert_eq!(c.slowmode_seconds(), 30);
        assert_eq!(c.admins_count(), 0);
        assert!(c.members_count_known());
    });
    let community = community_full_data();
    p.apply_community_full(channel, &community, &ctx);
    assert_eq!(p.read_channel(channel, ChannelData::admins_count), 1);
}
#[test]
fn reaction_and_bot_command_values_and_source_partial_updates() {
    use tg_core::data::data_peer_bot_command::*;
    let mut commands = ChatBotCommands::default();
    let a = BotCommands {
        user_id: UserId::new(1),
        commands: vec![BotCommand {
            command: "x".into(),
            description: "desc".into(),
            ephemeral: true,
        }],
    };
    let b = BotCommands {
        user_id: UserId::new(2),
        commands: a.commands.clone(),
    };
    assert!(commands.update(&[a.clone(), b]));
    assert!(!commands.update(std::slice::from_ref(&a)));
    assert_eq!(commands.0.len(), 2);
    assert!(commands.update(&[]));
    assert!(commands.0.is_empty());
    for value in [
        ReactionId::default(),
        ReactionId::paid_id(),
        ReactionId::Custom(42),
        ReactionId::Emoji("😀".into()),
    ] {
        assert_eq!(reaction_from_mtp(&reaction_to_mtp(&value)), value);
    }
    let reaction = ReactionId::Custom(0);
    assert!(!reaction.empty());
    assert!(reaction_from_mtp(&reaction_to_mtp(&reaction)).empty());
    let r = parse_allowed_reactions(
        &tl::mtp_chat_reactions_all(tl::ChatReactionsAllData::F_ALLOW_CUSTOM),
        4,
        true,
    );
    assert_eq!(r.kind, AllowedReactionsType::All);
    assert!(r.enabled());
    assert_eq!(r.max_count, 4);
    assert_eq!(
        list_of_restrictions(RestrictionsSetOptions::default()).len(),
        15
    );
    assert_eq!(
        list_of_restrictions(RestrictionsSetOptions {
            is_forum: true,
            is_user_specific: true
        })
        .len(),
        16
    );
    let e = SendError {
        monoforum_admin: true,
        ..Default::default()
    };
    assert!(e.is_present());
    assert!(!e.has_value());
    assert!(e.value_or(SendError::default()).is_present());
    let wire_command = tl::mtp_bot_command(
        tl::BotCommandData::F_EPHEMERAL,
        b"wire".to_vec(),
        b"description".to_vec(),
    );
    let wire_info = tl::BotInfo::BotInfo(std::sync::Arc::new(tl::BotInfoData {
        flags: tl::BotInfoData::F_USER_ID | tl::BotInfoData::F_COMMANDS,
        user_id: Some(42),
        description: None,
        description_photo: None,
        description_document: None,
        commands: Some(vec![wire_command]),
        menu_button: None,
        privacy_policy_url: None,
        app_settings: None,
        verifier_settings: None,
    }));
    let parsed = bot_commands_from_tl(&wire_info);
    assert_eq!(parsed.user_id, UserId::new(42));
    assert_eq!(parsed.commands[0].command, "wire");
    assert!(parsed.commands[0].ephemeral);
    let sticker = tg_core::data::data_types::StickerSetIdentifier {
        short_name: "name".into(),
        ..Default::default()
    };
    assert!(!sticker.empty());
}
#[test]
fn channel_rights_callbacks_reenter_before_self_cache_update() {
    let (_s, _q, p, _) = setup();
    let k = p.ensure_channel(ChannelId::new(1));
    let me = UserKey::try_from(p.ensure_peer(peer_from_user(2u64))).unwrap();
    p.set_channel_flags(k, ChannelDataFlag::Megagroup.into());
    let life = Lifetime::new();
    let pp = p.clone();
    let once = Rc::new(Cell::new(false));
    let done = once.clone();
    p.read_channel(k, ChannelData::admin_rights_value).on_next(
        move |c| {
            if c.value == ChatAdminRights::from(ChatAdminRight::ChangeInfo) && !done.replace(true) {
                pp.set_channel_admin_rights(k, ChatAdminRight::BanUsers.into(), me);
            }
        },
        &life,
    );
    p.set_channel_admin_rights(k, ChatAdminRight::ChangeInfo.into(), me);
    p.read_channel(k, |c| {
        assert_eq!(
            c.admin_rights(),
            ChatAdminRights::from(ChatAdminRight::BanUsers)
        );
        assert_eq!(
            c.mg_info.as_ref().unwrap().last_admins[&me].rights.flags,
            ChatAdminRights::from(ChatAdminRight::BanUsers)
        );
    });
}
#[test]
fn scalar_wrappers_predicate_matrix_locations_and_pending_fields() {
    let (_s, _q, p, _) = setup();
    let chat = p.ensure_chat(ChatId::new(1));
    let k = p.ensure_channel(ChannelId::new(2));
    let me = UserKey::try_from(p.ensure_peer(peer_from_user(3u64))).unwrap();
    p.set_chat_flags(chat, ChatDataFlag::Creator | ChatDataFlag::CanSetUsername);
    p.set_chat_invite_link(chat, "invite");
    p.read_chat(chat, |c| {
        assert!(
            c.can_add_admins()
                && c.can_ban_members()
                && c.can_delete_messages()
                && c.can_edit_permissions()
                && c.can_edit_pre_history_hidden()
                && c.can_edit_username()
        );
        assert!(!c.has_admin_rights());
        assert_eq!(c.invite_link(), "invite");
        assert!(c.allows_forwarding());
        assert!(!c.is_ayu_no_forwards());
        assert!(!c.is_deactivated());
        assert!(!c.have_left());
        assert!(c.default_admin_rights(me, true).flags.any());
        assert!(c.admin_rights().is_empty());
        assert!(c.default_restrictions().is_empty());
    });
    p.set_channel_flags(
        k,
        ChannelDataFlag::Creator
            | ChannelDataFlag::Broadcast
            | ChannelDataFlag::CanSetUsername
            | ChannelDataFlag::CanSetStickers
            | ChannelDataFlag::PaidMediaAllowed
            | ChannelDataFlag::PreHistoryHidden,
    );
    p.set_channel_access_hash(k, 7);
    p.set_channel_invite_link(k, "cinvite");
    p.set_channel_pending_requests(k, 3, vec![UserId::new(3)]);
    p.set_channel_peer_gifts_count(k, 5);
    p.set_channel_guard_bot_id(k, UserId::new(3));
    p.update_channel_level_hint(k, 8);
    p.update_channel_subscription_until(k, 99);
    p.read_channel(k, |c| {
        assert_eq!(c.access_hash(), 7);
        assert_eq!(c.invite_link(), "cinvite");
        assert_eq!(c.pending_requests_count(), 3);
        assert_eq!(c.recent_requesters(), [UserId::new(3)]);
        assert_eq!(c.level_hint(), 8);
        assert_eq!(c.subscription_until_date(), 99);
        assert_eq!(c.peer_gifts_count(), 5);
        assert_eq!(c.guard_bot_id(), UserId::new(3));
        assert_eq!(c.guard_bot(|_| Some(me)), Some(me));
        assert_eq!(
            c.common_stars_per_message(|peer| {
                assert_eq!(peer, k.peer());
                17
            }),
            17
        );
        assert!(
            c.can_post_messages()
                && c.can_edit_messages()
                && c.can_delete_messages()
                && c.can_post_stories()
                && c.can_edit_stories()
                && c.can_delete_stories()
        );
        assert!(
            c.can_post_paid_media()
                && c.can_access_monoforum()
                && c.can_have_invite_link()
                && c.can_edit_username()
                && c.can_edit_emoji()
                && c.can_edit_stickers()
                && c.can_delete()
        );
        assert!(c.can_edit_information() && c.can_edit_signatures() && c.can_edit_auto_translate());
        assert!(c.can_add_members() && c.can_add_admins() && c.can_ban_members());
        assert!(c.can_view_admins() && c.can_view_banned());
        assert!(c.hidden_pre_history());
        assert!(c.anyone_can_add_members());
        assert!(c.can_edit_admin(me, false));
        assert!(!c.can_edit_admin(me, true));
        assert!(c.can_restrict_participant(me.peer(), false));
        assert!(matches!(
            c.input_channel(None),
            tl::InputChannel::InputChannel(_)
        ));
    });
    p.set_channel_invite_peek(k, "hash", 20);
    assert_eq!(
        p.read_channel(k, |c| c.invite_peek_hash().to_owned()),
        "hash"
    );
    p.channel_private_error_received(k, false, 10);
    assert_eq!(p.read_channel(k, ChannelData::invite_peek_expires), 10);
    p.channel_private_error_received(k, true, 11);
    assert_eq!(p.read_channel(k, ChannelData::invite_peek_expires), 0);
    let a = ChannelLocation {
        address: String::new(),
        point: tg_core::data::data_location::LocationPoint {
            lat: 1.,
            lon: 2.,
            access_hash: 3,
        },
    };
    assert_eq!(a, ChannelLocation::default());
    let error = SendError::from_args(SendErrorArgs {
        text: "x".into(),
        premium_to_lift: true,
        ..Default::default()
    });
    assert_eq!(&*error, "x");
    assert!(error.premium_to_lift);
    assert!(SendError::new("plain").has_value());
}
#[test]
fn can_send_any_threadless_groups_freeze_and_zero_mask() {
    let (_s, _q, p, _) = setup();
    let chat = p.ensure_chat(ChatId::new(1));
    let channel = p.ensure_channel(ChannelId::new(2));
    let mut ctx = SendContext {
        forwarding: false,
        frozen: false,
        session_premium: false,
        production: true,
        user_inaccessible: false,
    };
    p.read_chat(chat, |c| {
        assert!(can_send_texts(&c.peer, &c.peer_fields(), true, &ctx));
        assert!(can_send_anything(&c.peer, &c.peer_fields(), true, &ctx));
        assert!(!can_send_any_of(
            &c.peer,
            &c.peer_fields(),
            ChatRestrictions::empty(),
            true,
            &ctx
        ));
    });
    p.set_chat_flags(chat, ChatDataFlag::Creator.into());
    p.read_chat(chat, |c| {
        assert!(can_send_any_of(
            &c.peer,
            &c.peer_fields(),
            ChatRestrictions::empty(),
            true,
            &ctx
        ))
    });
    p.set_channel_flags(channel, ChannelDataFlag::Forum.into());
    p.read_channel(channel, |c| {
        assert!(!can_send_texts(&c.peer, &c.peer_fields(), true, &ctx));
        assert!(can_send_texts(&c.peer, &c.peer_fields(), false, &ctx));
    });
    ctx.forwarding = true;
    p.read_channel(channel, |c| {
        assert!(!can_send_anything(&c.peer, &c.peer_fields(), false, &ctx))
    });
    ctx.forwarding = false;
    ctx.frozen = true;
    p.read_chat(chat, |c| {
        assert!(!can_send_texts(&c.peer, &c.peer_fields(), true, &ctx))
    });
}
#[test]
fn photo_stories_location_and_metadata_commands_use_real_record_state() {
    let (_s, _q, p, _) = setup();
    let k = p.ensure_channel(ChannelId::new(2));
    let chat = p.ensure_chat(ChatId::new(1));
    p.set_channel_flags(k, ChannelDataFlag::Megagroup.into());
    let input = tl::mtp_input_peer_channel(2, 7);
    p.set_group_photo(
        k.peer(),
        &tl::mtp_chat_photo(tl::ChatPhotoData::F_HAS_VIDEO, 9, None, 4),
        &input,
    );
    p.read_channel(k, |c| {
        assert_eq!(c.peer.userpic_photo_id(), 9);
        assert!(c.peer.userpic_has_video());
        assert!(c.peer.has_userpic());
    });
    p.update(k.peer(), |peer| peer.set_stories_hidden(true));
    p.update(k.peer(), |peer| {
        peer.set_stories_state(StoriesState::HasVideoStream)
    });
    p.read_channel(k, |c| {
        assert!(c.has_stories_hidden());
        assert!(c.has_active_stories());
        assert!(c.has_active_video_stream());
        assert!(!c.has_unread_stories());
        assert!(c.is_megagroup());
    });
    p.set_channel_location(
        k,
        &tl::mtp_channel_location(
            tl::mtp_geo_point(tl::tl::Flags::empty(), 2., 1., 99, None),
            b"address".to_vec(),
        ),
    );
    p.read_channel(k, |c| {
        let l = c.get_location().unwrap();
        assert_eq!(l.address, "address");
        assert_eq!(l.point.lat, 1.);
        assert_eq!(l.point.lon, 2.);
        assert_eq!(l.point.access_hash, 99);
    });
    p.set_channel_location(k, &tl::mtp_channel_location_empty());
    assert!(p.read_channel(k, |c| c.get_location().is_none()));
    p.set_group_photo(k.peer(), &tl::mtp_chat_photo_empty(), &input);
    assert_eq!(p.read_channel(k, |c| c.peer.userpic_photo_id()), 0);
    let count = Rc::new(Cell::new(0));
    let out = count.clone();
    let life = Lifetime::new();
    let pp = p.clone();
    p.group_effects().on_next(
        move |e| {
            if matches!(e, GroupEffect::BotCommandsChanged(_)) {
                assert!(!pp.read_chat(chat, |c| c.bot_commands().0.is_empty()));
                out.set(out.get() + 1);
            }
        },
        &life,
    );
    let cmd = BotCommands {
        user_id: UserId::new(5),
        commands: vec![tg_core::data::data_peer_bot_command::BotCommand {
            command: "c".into(),
            ..Default::default()
        }],
    };
    p.set_chat_bot_commands(chat, std::slice::from_ref(&cmd));
    p.set_chat_bot_commands(chat, std::slice::from_ref(&cmd));
    assert_eq!(count.get(), 1);
    let me = UserKey::try_from(p.ensure_peer(peer_from_user(5u64))).unwrap();
    p.apply_chat_edit_admin(chat, me, true);
    assert!(p.read_chat(chat, |c| c.admins.contains(&me)));
    p.set_channel_available_min_id(k, tg_core::data::data_msg_id::MsgId::new(4));
    assert_eq!(p.read_channel(k, ChannelData::available_min_id).bare, 4);
}
