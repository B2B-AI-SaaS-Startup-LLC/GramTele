use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::Rc,
};
use tg_core::data::{
    data_birthday::*, data_changes::Changes, data_channel::ChannelDataFlag as G,
    data_chat::ChatDataFlag as C, data_chat_participant_status::*, data_flags::Flags,
    data_image_location::*, data_lastseen_status::*, data_peer::*, data_peer_common::*,
    data_peer_fields::*, data_peer_id::*, data_peer_pinned::*, data_session::*, data_store_keys::*,
    data_types::EmojiStatusId, data_user::UserDataFlag as U, data_user_names::*,
    notify::data_peer_notify_settings::*,
};
use tg_rpl::Lifetime;
use tg_runtime::MainQueue;
fn num(s: &str) -> i32 {
    s.parse().unwrap()
}
fn boolean(v: bool) -> String {
    (v as u8).to_string()
}
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
#[test]
fn native_pure_functions_reference() {
    let mut expected_flags = Vec::new();
    for row in include_str!("fixtures/data-peer/pure.csv").lines() {
        let c: Vec<_> = row.split(',').collect();
        match c[0] {
            "b" => {
                let b = Birthday::new(num(c[1]), num(c[2]), num(c[3]));
                assert_eq!(
                    [
                        b.serialize(),
                        b.valid() as i32,
                        b.day(),
                        b.month(),
                        b.year()
                    ],
                    [num(c[4]), num(c[5]), num(c[6]), num(c[7]), num(c[8])],
                    "{row}"
                );
                assert_eq!(Birthday::from_serialized(b.serialize()), b);
            }
            "age" => {
                let now = CalendarDate {
                    year: num(c[1]),
                    month: num(c[2]),
                    day: num(c[3]),
                };
                let b = Birthday::from_serialized(num(c[4]));
                assert_eq!(
                    (birthday_age(b, now), is_birthday_today(b, now) as i32),
                    (num(c[5]), num(c[6])),
                    "{row}"
                );
            }
            "l" => {
                let l = LastseenStatus::from_serialized(c[1].parse().unwrap());
                let values = vec![
                    l.serialize().to_string(),
                    boolean(l.is_hidden()),
                    boolean(l.is_recently()),
                    boolean(l.is_within_week()),
                    boolean(l.is_within_month()),
                    boolean(l.is_long_ago()),
                    boolean(l.is_hidden_by_me()),
                    boolean(l.is_online(0)),
                    boolean(l.is_online(-1)),
                    boolean(l.is_local_online_value()),
                    l.online_till().to_string(),
                ];
                assert_eq!(values, c[2..], "{row}");
            }
            "o" => assert_eq!(
                LastseenStatus::online_till_value(num(c[1]), num(c[2]) != 0, num(c[3]) != 0)
                    .serialize()
                    .to_string(),
                c[4],
                "{row}"
            ),
            "legacy" => assert_eq!(
                LastseenStatus::from_legacy(num(c[1]))
                    .serialize()
                    .to_string(),
                c[2],
                "{row}"
            ),
            "reg" => assert_eq!(parse_registration_date(c[1]), num(c[2]), "{row}"),
            "p" => {
                let s = SessionScope::new();
                let id = PeerId::new(c[1].parse().unwrap());
                assert_eq!(
                    [
                        peer_is_user(id) as i32,
                        peer_is_chat(id) as i32,
                        peer_is_channel(id) as i32,
                        (peer_is_user(id) && id.value.is_multiple_of(1000)) as i32
                    ],
                    [num(c[2]), num(c[3]), num(c[4]), num(c[5])],
                    "{row}"
                );
                if peer_is_user(id) || peer_is_chat(id) || peer_is_channel(id) {
                    let p = PeerData::new(&s, id);
                    assert_eq!(p.is_service_user() as i32, num(c[5]));
                }
            }
            "f" => expected_flags.push((num(c[1]), num(c[2]))),
            "names" => {
                let mut n = UsernamesInfo::new();
                n.set_usernames(&[
                    Username {
                        username: "first".into(),
                        active: true,
                        editable: false,
                    },
                    Username {
                        username: "edit".into(),
                        active: true,
                        editable: true,
                    },
                ]);
                n.set_username("");
                n.set_username("appended");
                assert_eq!(
                    (n.username(), n.editable_username(), n.usernames().len()),
                    (c[1], c[2], c[3].parse().unwrap())
                );
            }
            "search" => {
                let text = String::from_utf8(unhex(c[1])).unwrap();
                assert_eq!(
                    tg_core::core::utils::translit_rus_eng(&text).as_bytes(),
                    unhex(c[2]),
                    "{row}"
                );
                assert_eq!(
                    tg_core::core::utils::rus_keyboard_layout_switch(&text).as_bytes(),
                    unhex(c[3]),
                    "{row}"
                );
            }
            "fake" => assert_eq!(
                fake_peer_id_for_just_name(&String::from_utf8(unhex(c[1])).unwrap()).value,
                c[2].parse::<u64>().unwrap(),
                "{row}"
            ),
            "profile" => {
                let tab = match num(c[1]) {
                    1 => ProfileTab::Posts,
                    2 => ProfileTab::Gifts,
                    3 => ProfileTab::Media,
                    4 => ProfileTab::Files,
                    5 => ProfileTab::Music,
                    6 => ProfileTab::Voice,
                    7 => ProfileTab::Links,
                    8 => ProfileTab::Gifs,
                    _ => panic!(),
                };
                let mtp = profile_tab_to_mtp(tab);
                assert_eq!(mtp.type_id(), c[2].parse::<u32>().unwrap());
                assert_eq!(parse_profile_tab(Some(&mtp)) as i32, num(c[3]));
            }
            "color" => {
                let color = tg_scheme::mtp_peer_color(
                    tg_scheme::PeerColorData::F_COLOR
                        | tg_scheme::PeerColorData::F_BACKGROUND_EMOJI_ID,
                    Some(num(c[1])),
                    Some(42),
                );
                assert_eq!(color_index_from_color(Some(&color)), Some(num(c[2]) as u8));
                assert_eq!(
                    background_emoji_id_from_color(Some(&color)),
                    c[3].parse::<u64>().unwrap()
                );
            }
            "flag" => {
                let value = match (c[1], c[2]) {
                    ("UserDataFlag", "Contact") => U::Contact as u64,
                    ("UserDataFlag", "MutualContact") => U::MutualContact as u64,
                    ("UserDataFlag", "Deleted") => U::Deleted as u64,
                    ("UserDataFlag", "Verified") => U::Verified as u64,
                    ("UserDataFlag", "Scam") => U::Scam as u64,
                    ("UserDataFlag", "Fake") => U::Fake as u64,
                    ("UserDataFlag", "BotInlineGeo") => U::BotInlineGeo as u64,
                    ("UserDataFlag", "Blocked") => U::Blocked as u64,
                    ("UserDataFlag", "HasPhoneCalls") => U::HasPhoneCalls as u64,
                    ("UserDataFlag", "PhoneCallsPrivate") => U::PhoneCallsPrivate as u64,
                    ("UserDataFlag", "Support") => U::Support as u64,
                    ("UserDataFlag", "CanPinMessages") => U::CanPinMessages as u64,
                    ("UserDataFlag", "DiscardMinPhoto") => U::DiscardMinPhoto as u64,
                    ("UserDataFlag", "Self") => U::SelfUser as u64,
                    ("UserDataFlag", "Premium") => U::Premium as u64,
                    ("UserDataFlag", "UnofficialSecurityRisk") => U::UnofficialSecurityRisk as u64,
                    ("UserDataFlag", "VoiceMessagesForbidden") => U::VoiceMessagesForbidden as u64,
                    ("UserDataFlag", "PersonalPhoto") => U::PersonalPhoto as u64,
                    ("UserDataFlag", "StoriesHidden") => U::StoriesHidden as u64,
                    ("UserDataFlag", "HasActiveStories") => U::HasActiveStories as u64,
                    ("UserDataFlag", "HasUnreadStories") => U::HasUnreadStories as u64,
                    ("UserDataFlag", "RequiresPremiumToWrite") => U::RequiresPremiumToWrite as u64,
                    ("UserDataFlag", "HasRequirePremiumToWrite") => {
                        U::HasRequirePremiumToWrite as u64
                    }
                    ("UserDataFlag", "HasStarsPerMessage") => U::HasStarsPerMessage as u64,
                    ("UserDataFlag", "MessageMoneyRestrictionsKnown") => {
                        U::MessageMoneyRestrictionsKnown as u64
                    }
                    ("UserDataFlag", "ReadDatesPrivate") => U::ReadDatesPrivate as u64,
                    ("UserDataFlag", "StoriesCorrespondent") => U::StoriesCorrespondent as u64,
                    ("UserDataFlag", "Forum") => U::Forum as u64,
                    ("UserDataFlag", "HasActiveVideoStream") => U::HasActiveVideoStream as u64,
                    ("UserDataFlag", "NoForwardsMyEnabled") => U::NoForwardsMyEnabled as u64,
                    ("UserDataFlag", "NoForwardsPeerEnabled") => U::NoForwardsPeerEnabled as u64,
                    ("ChatDataFlag", "Left") => C::Left as u64,
                    ("ChatDataFlag", "Creator") => C::Creator as u64,
                    ("ChatDataFlag", "Deactivated") => C::Deactivated as u64,
                    ("ChatDataFlag", "Forbidden") => C::Forbidden as u64,
                    ("ChatDataFlag", "CallActive") => C::CallActive as u64,
                    ("ChatDataFlag", "CallNotEmpty") => C::CallNotEmpty as u64,
                    ("ChatDataFlag", "CanSetUsername") => C::CanSetUsername as u64,
                    ("ChatDataFlag", "NoForwards") => C::NoForwards as u64,
                    ("ChatDataFlag", "HasWelcomeMessages") => C::HasWelcomeMessages as u64,
                    ("ChatDataFlag", "AyuNoForwards") => C::AyuNoForwards as u64,
                    ("ChannelDataFlag", "Left") => G::Left as u64,
                    ("ChannelDataFlag", "Creator") => G::Creator as u64,
                    ("ChannelDataFlag", "Forbidden") => G::Forbidden as u64,
                    ("ChannelDataFlag", "CallActive") => G::CallActive as u64,
                    ("ChannelDataFlag", "CallNotEmpty") => G::CallNotEmpty as u64,
                    ("ChannelDataFlag", "Signatures") => G::Signatures as u64,
                    ("ChannelDataFlag", "Verified") => G::Verified as u64,
                    ("ChannelDataFlag", "Scam") => G::Scam as u64,
                    ("ChannelDataFlag", "Fake") => G::Fake as u64,
                    ("ChannelDataFlag", "Megagroup") => G::Megagroup as u64,
                    ("ChannelDataFlag", "Broadcast") => G::Broadcast as u64,
                    ("ChannelDataFlag", "Gigagroup") => G::Gigagroup as u64,
                    ("ChannelDataFlag", "Username") => G::Username as u64,
                    ("ChannelDataFlag", "Location") => G::Location as u64,
                    ("ChannelDataFlag", "CanSetUsername") => G::CanSetUsername as u64,
                    ("ChannelDataFlag", "CanSetStickers") => G::CanSetStickers as u64,
                    ("ChannelDataFlag", "PreHistoryHidden") => G::PreHistoryHidden as u64,
                    ("ChannelDataFlag", "CanViewParticipants") => G::CanViewParticipants as u64,
                    ("ChannelDataFlag", "HasLink") => G::HasLink as u64,
                    ("ChannelDataFlag", "SlowmodeEnabled") => G::SlowmodeEnabled as u64,
                    ("ChannelDataFlag", "NoForwards") => G::NoForwards as u64,
                    ("ChannelDataFlag", "JoinToWrite") => G::JoinToWrite as u64,
                    ("ChannelDataFlag", "RequestToJoin") => G::RequestToJoin as u64,
                    ("ChannelDataFlag", "Forum") => G::Forum as u64,
                    ("ChannelDataFlag", "AntiSpam") => G::AntiSpam as u64,
                    ("ChannelDataFlag", "ParticipantsHidden") => G::ParticipantsHidden as u64,
                    ("ChannelDataFlag", "StoriesHidden") => G::StoriesHidden as u64,
                    ("ChannelDataFlag", "HasActiveStories") => G::HasActiveStories as u64,
                    ("ChannelDataFlag", "HasUnreadStories") => G::HasUnreadStories as u64,
                    ("ChannelDataFlag", "CanGetStatistics") => G::CanGetStatistics as u64,
                    ("ChannelDataFlag", "ViewAsMessages") => G::ViewAsMessages as u64,
                    ("ChannelDataFlag", "SimilarExpanded") => G::SimilarExpanded as u64,
                    ("ChannelDataFlag", "CanViewRevenue") => G::CanViewRevenue as u64,
                    ("ChannelDataFlag", "PaidMediaAllowed") => G::PaidMediaAllowed as u64,
                    ("ChannelDataFlag", "CanViewCreditsRevenue") => G::CanViewCreditsRevenue as u64,
                    ("ChannelDataFlag", "SignatureProfiles") => G::SignatureProfiles as u64,
                    ("ChannelDataFlag", "StargiftsAvailable") => G::StargiftsAvailable as u64,
                    ("ChannelDataFlag", "PaidMessagesAvailable") => G::PaidMessagesAvailable as u64,
                    ("ChannelDataFlag", "AutoTranslation") => G::AutoTranslation as u64,
                    ("ChannelDataFlag", "Monoforum") => G::Monoforum as u64,
                    ("ChannelDataFlag", "MonoforumAdmin") => G::MonoforumAdmin as u64,
                    ("ChannelDataFlag", "MonoforumDisabled") => G::MonoforumDisabled as u64,
                    ("ChannelDataFlag", "ForumTabs") => G::ForumTabs as u64,
                    ("ChannelDataFlag", "HasStarsPerMessage") => G::HasStarsPerMessage as u64,
                    ("ChannelDataFlag", "StarsPerMessageKnown") => G::StarsPerMessageKnown as u64,
                    ("ChannelDataFlag", "HasActiveVideoStream") => G::HasActiveVideoStream as u64,
                    ("ChannelDataFlag", "Community") => G::Community as u64,
                    ("ChannelDataFlag", "CommunityCollapsed") => G::CommunityCollapsed as u64,
                    ("ChannelDataFlag", "HasWelcomeMessages") => G::HasWelcomeMessages as u64,
                    ("ChannelDataFlag", "AyuNoForwards") => G::AyuNoForwards as u64,
                    ("ChatRestriction", "ViewMessages") => ChatRestriction::ViewMessages as u64,
                    ("ChatRestriction", "SendStickers") => ChatRestriction::SendStickers as u64,
                    ("ChatRestriction", "SendGifs") => ChatRestriction::SendGifs as u64,
                    ("ChatRestriction", "SendGames") => ChatRestriction::SendGames as u64,
                    ("ChatRestriction", "SendInline") => ChatRestriction::SendInline as u64,
                    ("ChatRestriction", "SendPolls") => ChatRestriction::SendPolls as u64,
                    ("ChatRestriction", "SendPhotos") => ChatRestriction::SendPhotos as u64,
                    ("ChatRestriction", "SendVideos") => ChatRestriction::SendVideos as u64,
                    ("ChatRestriction", "SendVideoMessages") => {
                        ChatRestriction::SendVideoMessages as u64
                    }
                    ("ChatRestriction", "SendMusic") => ChatRestriction::SendMusic as u64,
                    ("ChatRestriction", "SendVoiceMessages") => {
                        ChatRestriction::SendVoiceMessages as u64
                    }
                    ("ChatRestriction", "SendFiles") => ChatRestriction::SendFiles as u64,
                    ("ChatRestriction", "SendOther") => ChatRestriction::SendOther as u64,
                    ("ChatRestriction", "EmbedLinks") => ChatRestriction::EmbedLinks as u64,
                    ("ChatRestriction", "ChangeInfo") => ChatRestriction::ChangeInfo as u64,
                    ("ChatRestriction", "AddParticipants") => {
                        ChatRestriction::AddParticipants as u64
                    }
                    ("ChatRestriction", "PinMessages") => ChatRestriction::PinMessages as u64,
                    ("ChatRestriction", "CreateTopics") => ChatRestriction::CreateTopics as u64,
                    ("ChatRestriction", "EditRank") => ChatRestriction::EditRank as u64,
                    ("ChatRestriction", "SendReactions") => ChatRestriction::SendReactions as u64,
                    ("ChatRestriction", "ManageLinkedPeers") => {
                        ChatRestriction::ManageLinkedPeers as u64
                    }
                    ("PeerBarSetting", "ReportSpam") => PeerBarSetting::ReportSpam as u64,
                    ("PeerBarSetting", "AddContact") => PeerBarSetting::AddContact as u64,
                    ("PeerBarSetting", "BlockContact") => PeerBarSetting::BlockContact as u64,
                    ("PeerBarSetting", "ShareContact") => PeerBarSetting::ShareContact as u64,
                    ("PeerBarSetting", "NeedContactsException") => {
                        PeerBarSetting::NeedContactsException as u64
                    }
                    ("PeerBarSetting", "AutoArchived") => PeerBarSetting::AutoArchived as u64,
                    ("PeerBarSetting", "RequestChat") => PeerBarSetting::RequestChat as u64,
                    ("PeerBarSetting", "RequestChatIsBroadcast") => {
                        PeerBarSetting::RequestChatIsBroadcast as u64
                    }
                    ("PeerBarSetting", "HasBusinessBot") => PeerBarSetting::HasBusinessBot as u64,
                    ("PeerBarSetting", "BusinessBotPaused") => {
                        PeerBarSetting::BusinessBotPaused as u64
                    }
                    ("PeerBarSetting", "BusinessBotCanReply") => {
                        PeerBarSetting::BusinessBotCanReply as u64
                    }
                    ("PeerBarSetting", "Unknown") => PeerBarSetting::Unknown as u64,
                    _ => panic!("unknown flag row {row}"),
                };
                assert_eq!(value, c[3].parse::<u64>().unwrap());
            }
            "normalization" => {} // Actual Qt samples are consumed by the strict fixture service below.
            "restriction" => {
                let r = match c[2] {
                    "1" => RestrictionCheckResult::Allowed,
                    _ => {
                        if c[3] == "1" {
                            RestrictionCheckResult::WithEveryone
                        } else {
                            RestrictionCheckResult::Explicit
                        }
                    }
                };
                assert_eq!(
                    [
                        r.restricted() as i32,
                        r.is_allowed() as i32,
                        r.is_with_everyone() as i32,
                        r.is_explicit() as i32
                    ],
                    [num(c[1]), num(c[2]), num(c[3]), num(c[4])]
                );
            }
            "image" => {
                let a = StorageFileLocation {
                    dc_id: 1,
                    kind: StorageFileType::PeerPhoto,
                    id: 42,
                    volume_id: 7,
                    size_letter: b'a',
                    ..Default::default()
                };
                let mut b = a.clone();
                b.access_hash = 99;
                b.file_reference = b"changed".to_vec();
                let ia = ImageLocation::new(DownloadLocation::Storage(a.clone()), 160, 160);
                let ib = ImageLocation::new(DownloadLocation::Storage(b.clone()), 320, 320);
                assert_eq!(
                    [
                        a.valid() as i32,
                        (a == b) as i32,
                        (ia == ib) as i32,
                        ia.width,
                        ib.height
                    ],
                    [num(c[1]), num(c[2]), num(c[3]), num(c[4]), num(c[5])]
                );
            }
            "imageDifferent" => assert_eq!(num(c[1]), 0),
            "freeze" => assert_eq!(
                is_freeze_appeal_username(&String::from_utf8(unhex(c[1])).unwrap()) as i32,
                num(c[2])
            ),
            _ => panic!("unknown fixture: {row}"),
        }
    }
    let flags = Flags::<PeerBarSetting>::with_essential(
        PeerBarSettings::empty(),
        PeerBarSettings::from_raw(3),
    );
    let life = Lifetime::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let copy = log.clone();
    flags.value().on_next(
        move |c| copy.borrow_mut().push((c.diff.value(), c.value.value())),
        &life,
    );
    for raw in [4, 5, 5, 7, 6, 2, 0] {
        flags.set(PeerBarSettings::from_raw(raw));
    }
    flags.add(PeerBarSetting::ReportSpam.into());
    flags.add(PeerBarSetting::ReportSpam.into());
    flags.remove(PeerBarSetting::ReportSpam.into());
    assert_eq!(*log.borrow(), expected_flags);
}
#[test]
fn flags_reentrant_essential_initial_snapshot_and_lifetime() {
    let flags = Flags::<PeerBarSetting>::default();
    let initial = flags.value();
    flags.add(PeerBarSetting::BlockContact.into());
    let l = Lifetime::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let copy = log.clone();
    initial.on_next(move |c| copy.borrow_mut().push(c.value.value()), &l);
    assert_eq!(*log.borrow(), vec![0]);
    let alias = flags.clone();
    let copy = log.clone();
    flags.changes().on_next(
        move |c| {
            copy.borrow_mut().push(c.value.value());
            if c.value.value() == 5 {
                alias.remove(PeerBarSetting::ReportSpam.into());
            }
        },
        &l,
    );
    flags.add(PeerBarSetting::ReportSpam.into());
    assert_eq!(*log.borrow(), vec![0, 5, 5, 4, 4]);
    assert_eq!(flags.current().value(), 4);
    l.destroy();
    flags.set(PeerBarSettings::empty());
    assert_eq!(log.borrow().len(), 5);
}
#[test]
fn lastseen_order_hidden_bits_and_serialized_validation() {
    assert!(LastseenStatus::recently(false) < LastseenStatus::recently(true));
    assert!(LastseenStatus::long_ago(true) < LastseenStatus::recently(false));
    assert!(LastseenStatus::recently(false).is_hidden());
    assert!(LastseenStatus::within_week(true).is_within_week());
    assert!(LastseenStatus::within_month(false).is_within_month());
    assert!(LastseenStatus::long_ago(false).is_long_ago());
    let a = LastseenStatus::online_till_value(i32::MAX, false, false);
    assert_ne!(a.serialize(), 0);
    assert_eq!(
        LastseenStatus::from_serialized(a.serialize()),
        LastseenStatus::default()
    );
    let local = LastseenStatus::online_till_value(LIFE_START_DATE + 10, true, true);
    assert!(local.is_local_online_value());
    assert!(local.is_online(LIFE_START_DATE + 9));
    assert!(!local.is_online(LIFE_START_DATE + 10));
    assert!(local.is_hidden_by_me());
}
#[test]
fn birthday_minute_producer_virtual_calendar_and_leap_bug() {
    use std::sync::Arc;
    use tg_base::timer::{ManualClock, TimerHost};
    let clock = ManualClock::new(0);
    let host = TimerHost::new(Arc::new(clock.clone()));
    let now = Rc::new(Cell::new(CalendarDate {
        year: 2024,
        month: 2,
        day: 28,
    }));
    let copy = now.clone();
    let log = Rc::new(RefCell::new(Vec::new()));
    let out = log.clone();
    let l = Lifetime::new();
    is_birthday_today_value(Birthday::new(29, 2, 2000), &host, move || copy.get())
        .on_next(move |v| out.borrow_mut().push(v), &l);
    clock.set(60_000);
    host.process();
    now.set(CalendarDate {
        year: 2024,
        month: 2,
        day: 29,
    });
    clock.set(120_000);
    host.process();
    clock.set(180_000);
    host.process();
    now.set(CalendarDate {
        year: 2024,
        month: 3,
        day: 1,
    });
    clock.set(240_000);
    host.process();
    assert_eq!(*log.borrow(), vec![false, true, false]);
    l.destroy();
    clock.set(300_000);
    host.process();
    assert_eq!(log.borrow().len(), 3);
    assert_eq!(
        birthday_age(
            Birthday::new(29, 2, 2000),
            CalendarDate {
                year: 2023,
                month: 1,
                day: 1
            }
        ),
        23
    );
    assert_eq!(birthday_cake(), "🎂");
    assert!(!Birthday::new(1, 1, 1874).valid());
}
fn make_store() -> (SessionScope, MainQueue, Rc<Peers>, Changes) {
    let s = SessionScope::new();
    let q = MainQueue::new();
    let c = Changes::with_queue(&s, q.clone());
    let p = Rc::new(Peers::new(&s, c.clone()));
    (s, q, p, c)
}
#[test]
fn peer_store_scope_load_states_and_reentrant_changes() {
    let (s, q, peers, c) = make_store();
    let id = peer_from_user(42u64);
    let key = peers.ensure_peer(id);
    assert_eq!(peers.ensure_peer(id), key);
    assert_eq!(peers.peer_loaded(id), None);
    for state in [
        LoadedStatus::Not,
        LoadedStatus::Minimal,
        LoadedStatus::Normal,
        LoadedStatus::Full,
        LoadedStatus::Not,
    ] {
        peers.update(key, |p| {
            p.set_loaded_status(state);
            PeerMutation::quiet(())
        });
        assert_eq!(
            peers.peer_loaded(id),
            matches!(state, LoadedStatus::Normal | LoadedStatus::Full).then_some(key)
        );
        peers.read(key, |p| {
            assert_eq!(p.session(), s.id());
            assert_eq!(p.owner(), s.id());
            assert_eq!(p.is_minimal_loaded(), state != LoadedStatus::Not);
            assert_eq!(p.is_full_loaded(), state == LoadedStatus::Full);
        });
    }
    let weak = Rc::downgrade(&peers);
    let l = Lifetime::new();
    c.realtime_peer_updates(tg_core::data::data_changes::PeerFlag::About)
        .on_next(
            move |u| {
                let p = weak.upgrade().unwrap();
                assert_eq!(p.read(u.peer, |p| p.about().to_owned()), "new");
                p.update(u.peer, |p| p.set_messages_ttl(7));
            },
            &l,
        );
    assert!(peers.update(key, |p| p.set_about("new")));
    assert!(!peers.update(key, |p| p.set_about("new")));
    assert_eq!(peers.read(key, PeerData::messages_ttl), 7);
    q.process_all();
}
#[test]
#[should_panic(expected = "foreign SessionScope")]
fn peer_store_rejects_cross_account() {
    let (s, _, p, _) = make_store();
    let other = SessionScope::new();
    let key = other.peer_key(peer_from_user(1u64));
    let _keep = s;
    p.read(key, PeerData::id);
}
#[test]
#[should_panic(expected = "unsupported PeerId")]
fn peer_store_rejects_unknown_tag() {
    let (_s, _, p, _) = make_store();
    p.ensure_peer(PeerId::new(3u64 << 48));
}
#[test]
fn common_colors_emoji_identity_ttl_block_and_theme_effects() {
    let s = SessionScope::new();
    let mut p = PeerData::new(&s, peer_from_user(42u64));
    assert_eq!(p.color_index(), 0);
    assert!(p.change_color_index(65));
    assert_eq!(p.color_index(), 1);
    assert!(!p.change_color_index(1));
    assert!(p.clear_color_index());
    assert!(!p.clear_color_index());
    assert_eq!(p.color_index(), 0);
    assert!(p.change_color_profile_index(255));
    assert_eq!(p.color_profile_index(), Some(63));
    assert!(!p.change_color_profile_index(63));
    assert!(p.clear_color_profile_index());
    assert!(!p.clear_color_profile_index());
    assert!(p.change_background_emoji_id(4));
    assert!(!p.change_background_emoji_id(4));
    assert_eq!(p.background_emoji_id(), 4);
    assert!(p.change_profile_background_emoji_id(7));
    assert!(!p.change_profile_background_emoji_id(7));
    assert_eq!(p.profile_background_emoji_id(), 7);
    let collectible = Rc::new(tg_core::data::data_emoji_statuses::EmojiStatusCollectible {
        id: 1,
        ..Default::default()
    });
    assert!(collectible.is_present());
    let a = EmojiStatusId {
        document_id: 2,
        collectible: Some(collectible.clone()),
    };
    let b = EmojiStatusId {
        document_id: 2,
        collectible: Some(Rc::new((*collectible).clone())),
    };
    assert_ne!(a, b);
    assert_eq!(a, a.clone());
    assert!(a.is_present());
    assert_eq!(p.set_emoji_status(a.clone(), 5).effects.len(), 2);
    assert_eq!(p.set_emoji_status(a, 6).effects.len(), 1);
    assert_eq!(p.set_messages_ttl(12).effects.len(), 1);
    assert_eq!(p.messages_ttl(), 12);
    assert!(p.set_messages_ttl(12).effects.is_empty());
    assert_eq!(p.set_is_blocked(true).effects.len(), 3);
    assert!(p.is_blocked());
    assert_eq!(p.block_status(), BlockStatus::Blocked);
    assert!(p.set_is_blocked(true).effects.is_empty());
    assert_eq!(p.set_is_blocked(false).effects.len(), 3);
    assert_eq!(p.set_translation_disabled(false).effects.len(), 1);
    assert_eq!(p.translation_flag(), TranslationFlag::Enabled);
    assert_eq!(p.save_translation_disabled(false).effects.len(), 1);
    assert_eq!(p.set_theme_token("red", true, false).effects.len(), 0);
    assert_eq!(p.theme_token(), "red");
    assert_eq!(p.set_theme_token("gift:abc", true, false).effects.len(), 2);
    assert_eq!(p.set_main_profile_tab(ProfileTab::Gifts).effects.len(), 1);
    assert_eq!(p.main_profile_tab(), ProfileTab::Gifts);
    assert!(p.set_main_profile_tab(ProfileTab::Gifts).effects.is_empty());
    assert!(!p.check_trusted_pay_for_message(1, true, true));
    assert!(p.check_trusted_pay_for_message(0, true, true));
    assert!(!p.check_trusted_pay_for_message(0, true, true));
}
fn user_view<'a>(key: PeerKey, flags: &'a Flags<U>, names: &'a UsernamesInfo) -> PeerFields<'a> {
    PeerFields::User(UserPeerFields {
        forum: None,
        key: UserKey::try_from(key).unwrap(),
        user_creates_topics: false,
        flags,
        usernames: names,
        first_name: "First",
        last_name: "Last",
        name_or_phone: "Phone",
        bot_info_present: false,
        effective_premium: false,
        stars_per_message: 12,
        stars_rating: StarsRating {
            level: 1,
            stars: 2,
            this_level_stars: 3,
            next_level_stars: 4,
        },
        peer_gifts_count: 7,
    })
}
fn channel_view<'a>(key: PeerKey, flags: &'a Flags<G>, names: &'a UsernamesInfo) -> PeerFields<'a> {
    PeerFields::Channel(ChannelPeerFields {
        migrated_from: None,
        monoforum_link: None,
        forum: None,
        monoforum: None,
        group_call: None,
        group_call_default_join_as: PeerId::new(0),
        key: ChannelKey::try_from(key).unwrap(),
        can_post_messages: false,
        can_edit_messages: false,
        can_pin_messages: false,
        can_post_stories: false,
        can_edit_stories: false,
        can_delete_stories: false,
        can_delete: false,
        members_count_known: false,
        flags,
        usernames: names,
        admin_rights: ChatAdminRights::empty(),
        default_restrictions: ChatRestrictions::empty(),
        restrictions: ChatRestrictions::empty(),
        boosts_unrestrict: 0,
        boosts_applied: 0,
        slowmode_seconds: 0,
        slowmode_last_message: 0,
        has_megagroup_info: false,
        inviter: UserId::new(0),
        stars_per_message: 8,
        peer_gifts_count: 9,
    })
}
#[test]
fn user_predicates_read_real_flag_projections_and_ayu_policy() {
    let s = SessionScope::new();
    let p = PeerData::new(&s, peer_from_user(42u64));
    let flags = Flags::<U>::default();
    let mut names = UsernamesInfo::new();
    names.set_username("SpAmBoT");
    let mut f = user_view(p.key(), &flags, &names);
    assert!(p.is_user());
    assert!(!p.is_chat());
    assert!(!p.is_channel());
    assert_eq!(f.key(), p.key());
    assert_eq!(p.as_user(), Some(UserKey::try_from(p.key()).unwrap()));
    assert_eq!(p.as_chat(), None);
    assert_eq!(p.as_channel(), None);
    assert!(!p.is_bot(&f));
    assert!(!p.is_self(&f));
    assert!(p.is_freeze_appeal_chat(&f));
    assert_eq!(p.username(&f), "SpAmBoT");
    assert_eq!(p.editable_username(&f), "SpAmBoT");
    assert!(p.is_username_editable(&f, "SpAmBoT"));
    assert_eq!(p.usernames(&f).len(), 1);
    assert_eq!(p.short_name(&f), "First");
    assert_eq!(p.top_bar_name_text(&f), "Phone");
    assert_eq!(
        p.am_restricted(&f, ChatRestriction::PinMessages, false),
        RestrictionCheckResult::Explicit
    );
    assert!(!p.can_pin_messages(&f, false));
    flags.add(
        U::SelfUser
            | U::Verified
            | U::Scam
            | U::Fake
            | U::VoiceMessagesForbidden
            | U::StoriesHidden
            | U::HasActiveStories
            | U::HasUnreadStories
            | U::HasActiveVideoStream
            | U::NoForwardsMyEnabled
            | U::RequiresPremiumToWrite,
    );
    assert!(p.is_self(&f));
    assert!(p.is_verified(&f));
    assert!(p.is_scam(&f));
    assert!(p.is_fake(&f));
    assert!(p.has_stories_hidden(&f));
    assert!(p.has_active_stories(&f));
    assert!(p.has_unread_stories(&f));
    assert!(p.has_active_video_stream(&f));
    assert!(p.is_ayu_no_forwards(&f));
    assert!(p.allows_forwarding(&f)); // pinned AyuGram explicitly returns true
    assert!(p.shared_media_info(&f, true));
    assert!(p.saved_sublists_info(&f, true));
    assert!(!p.saved_sublists_info(&f, false));
    assert!(p.can_manage_group_call(&f));
    assert!(!p.can_manage_ranks(&f));
    assert!(!p.can_manage_welcome_messages(&f));
    assert!(!p.am_anonymous(&f));
    assert!(!p.slowmode_applied(&f));
    assert_eq!(p.slowmode_seconds_left(&f, 100), 0);
    assert_eq!(p.stars_per_message(&f), 12);
    assert_eq!(p.stars_per_message_checked(&f), 12);
    assert!(p.stars_rating(&f).is_present());
    assert_eq!(p.peer_gifts_count(&f), 7);
    assert_eq!(
        p.am_restricted(&f, ChatRestriction::SendVoiceMessages, false),
        RestrictionCheckResult::Explicit
    );
    assert_eq!(
        p.am_restricted(&f, ChatRestriction::SendPolls, false),
        RestrictionCheckResult::Allowed
    );
    flags.remove(U::SelfUser.into());
    assert_eq!(
        p.am_restricted(&f, ChatRestriction::SendPolls, false),
        RestrictionCheckResult::Explicit
    );
    assert_eq!(
        p.am_restricted(&f, ChatRestriction::SendPolls, true),
        RestrictionCheckResult::Allowed
    );
    if let PeerFields::User(v) = &mut f {
        v.bot_info_present = true;
        v.effective_premium = true;
        v.user_creates_topics = true;
    }
    flags.add(U::Forum | U::CanPinMessages);
    assert!(p.is_premium(&f));
    assert!(p.is_bot(&f));
    assert!(p.is_forum(&f));
    assert!(p.is_bot_user_creates_topics(&f));
    assert!(!p.is_bot_creates_topics(&f));
    assert!(p.can_manage_topics(&f));
    assert!(p.can_create_topics(&f, true));
    assert!(p.display_as_forum(&f, || false));
    assert!(p.as_bot(&f).is_some());
    assert!(p.can_create_polls(&f, true, true, |_, _| panic!("user branch")));
    assert!(p.can_create_todo_lists(&f, true, false, |_, _| false));
    assert!(!p.can_revoke_full_history(&f, true, i32::MAX));
    flags.add(U::Support.into());
    assert!(p.can_revoke_full_history(&f, true, i32::MAX));
    assert!(!p.can_create_polls(&f, true, true, |_, _| true));
    flags.add(U::SelfUser.into());
    assert!(p.can_post_stories(&f));
    assert!(p.can_edit_stories(&f));
    assert!(p.can_delete_stories(&f));
    assert!(p.can_manage_gifts(&f));
    assert!(p.can_transfer_gifts(&f));
    assert!(p.can_edit_messages_indefinitely(&f));
    assert!(matches!(
        p.set_stories_hidden(true).effects[0],
        PeerEffect::SetStoriesHidden { hidden: true, .. }
    ));
    assert!(matches!(
        p.set_stories_state(StoriesState::HasUnread).effects[0],
        PeerEffect::SetStoriesState {
            state: StoriesState::HasUnread,
            ..
        }
    ));
}
#[test]
fn chat_and_channel_restrictions_admin_public_boost_priority() {
    let s = SessionScope::new();
    let chat = PeerData::new(&s, peer_from_chat(5u64));
    let cf = Flags::<C>::default();
    let mut f = PeerFields::Chat(ChatPeerFields {
        migrated_to: None,
        group_call: None,
        group_call_default_join_as: PeerId::new(0),
        key: ChatKey::try_from(chat.key()).unwrap(),
        flags: &cf,
        admin_rights: ChatAdminRights::empty(),
        default_restrictions: ChatRestriction::SendPolls | ChatRestriction::PinMessages,
    });
    assert_eq!(f.admin_rights().value(), 0);
    assert_eq!(
        chat.am_restricted(&f, ChatRestriction::SendPolls, false),
        RestrictionCheckResult::WithEveryone
    );
    assert!(!chat.can_pin_messages(&f, false));
    cf.add(C::Creator.into());
    assert!(f.is_creator());
    assert!(chat.can_pin_messages(&f, false));
    assert!(chat.can_manage_group_call(&f));
    assert!(chat.can_manage_ranks(&f));
    assert!(chat.can_manage_welcome_messages(&f));
    assert!(chat.can_revoke_full_history(&f, false, 0));
    assert!(!chat.is_bot(&f));
    assert_eq!(chat.stars_per_message(&f), 0);
    assert_eq!(chat.peer_gifts_count(&f), 0);
    assert!(!chat.stars_rating(&f).is_present());
    assert_eq!(chat.usernames(&f).len(), 0);
    assert!(!chat.can_edit_messages_indefinitely(&f));
    assert!(!chat.can_create_topics(&f, false));
    assert!(chat.can_create_polls(&f, true, true, |r, b| r == ChatRestriction::SendPolls && b));
    cf.remove(C::Creator.into());
    if let PeerFields::Chat(v) = &mut f {
        v.admin_rights = ChatAdminRight::ChangeInfo.into();
    }
    assert_eq!(
        chat.am_restricted(&f, ChatRestriction::SendPolls, false),
        RestrictionCheckResult::Allowed
    );
    assert_eq!(
        chat.am_restricted(&f, ChatRestriction::PinMessages, false),
        RestrictionCheckResult::WithEveryone
    );
    cf.add(C::NoForwards | C::AyuNoForwards);
    assert!(chat.is_ayu_no_forwards(&f));
    assert!(!chat.allows_forwarding(&f));
    let channel = PeerData::new(&s, peer_from_channel(7u64));
    let gf = Flags::<G>::new(G::Megagroup | G::Username | G::SlowmodeEnabled);
    let names = UsernamesInfo::new();
    let mut f = channel_view(channel.key(), &gf, &names);
    assert_eq!(
        channel.am_restricted(&f, ChatRestriction::PinMessages, false),
        RestrictionCheckResult::WithEveryone
    );
    assert!(channel.slowmode_applied(&f));
    assert!(channel.as_megagroup(&f).is_some());
    assert!(channel.as_broadcast(&f).is_none());
    if let PeerFields::Channel(v) = &mut f {
        v.has_megagroup_info = true;
        v.boosts_unrestrict = 2;
        v.boosts_applied = 2;
        v.restrictions = ChatRestriction::PinMessages.into();
        v.slowmode_seconds = 30;
        v.slowmode_last_message = 100;
    }
    assert_eq!(
        channel.am_restricted(&f, ChatRestriction::PinMessages, false),
        RestrictionCheckResult::Explicit
    );
    assert_eq!(channel.slowmode_seconds_left(&f, 101), 0); // boosts suppress last message
    if let PeerFields::Channel(v) = &mut f {
        v.boosts_applied = 0;
    }
    assert_eq!(channel.slowmode_seconds_left(&f, 110), 20);
    assert_eq!(channel.slowmode_seconds_left(&f, 200), 0);
    assert_eq!(channel.slowmode_seconds_left(&f, 90), 40);
    gf.add(G::Creator.into());
    assert_eq!(
        channel.am_restricted(&f, ChatRestriction::PinMessages, false),
        RestrictionCheckResult::Allowed
    );
    assert!(!channel.slowmode_applied(&f));
    assert!(channel.can_manage_group_call(&f));
    gf.add(G::MonoforumDisabled.into());
    assert_eq!(
        channel.am_restricted(&f, ChatRestriction::PinMessages, false),
        RestrictionCheckResult::WithEveryone
    );
    gf.remove(G::MonoforumDisabled.into());
    gf.add(
        G::Monoforum
            | G::Community
            | G::Verified
            | G::Scam
            | G::Fake
            | G::Gigagroup
            | G::AutoTranslation
            | G::MonoforumAdmin
            | G::StoriesHidden
            | G::HasActiveStories
            | G::HasUnreadStories
            | G::HasActiveVideoStream
            | G::ViewAsMessages,
    );
    assert!(
        channel.is_verified(&f)
            && channel.is_scam(&f)
            && channel.is_fake(&f)
            && channel.is_gigagroup(&f)
            && channel.auto_translation(&f)
            && channel.am_monoforum_admin(&f)
            && channel.view_forum_as_messages(&f)
    );
    assert!(
        channel.has_stories_hidden(&f)
            && channel.has_active_stories(&f)
            && channel.has_unread_stories(&f)
            && channel.has_active_video_stream(&f)
    );
    assert!(channel.as_monoforum(&f).is_some());
    assert!(!channel.can_manage_group_call(&f));
    assert!(!channel.can_manage_ranks(&f));
    assert!(channel.can_manage_welcome_messages(&f));
    assert_eq!(channel.stars_per_message_checked(&f), 0);
    assert_eq!(channel.peer_gifts_count(&f), 9);
    assert!(!channel.can_create_polls(&f, true, true, |_, _| panic!()));
    assert!(!channel.can_create_todo_lists(&f, true, true, |_, _| panic!()));
    gf.remove(G::Monoforum | G::Megagroup);
    gf.add(G::Broadcast.into());
    assert!(channel.is_broadcast(&f));
    assert!(channel.as_broadcast(&f).is_some());
    assert!(channel.am_anonymous(&f));
    gf.add(G::SignatureProfiles.into());
    assert!(!channel.am_anonymous(&f));
}
#[test]
fn full_update_timeout_and_creator_slowmode_producer_difference() {
    let s = SessionScope::new();
    let mut p = PeerData::new(&s, peer_from_channel(7u64));
    let flags = Flags::<G>::new(G::Creator | G::SlowmodeEnabled);
    let names = UsernamesInfo::new();
    let f = channel_view(p.key(), &flags, &names);
    assert!(!p.was_full_updated());
    assert_eq!(p.update_full(0, &f).effects.len(), 1);
    p.full_updated(10);
    assert!(p.is_full_loaded());
    assert!(p.was_full_updated());
    assert!(p.update_full(5010, &f).effects.is_empty());
    assert_eq!(p.update_full(5011, &f).effects.len(), 1);
    flags.remove(G::Creator.into());
    assert_eq!(p.update_full_forced(&f).effects.len(), 2);
    flags.add(G::Creator.into());
    assert!(!p.slowmode_applied(&f));
    let rights = Flags::<ChatAdminRight>::default();
    let l = Lifetime::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let copy = log.clone();
    p.slowmode_applied_value(Some((rights.value().map(|c| c.value.any()), flags.value())))
        .on_next(move |v| copy.borrow_mut().push(v), &l);
    assert_eq!(*log.borrow(), vec![true]);
    rights.add(ChatAdminRight::ManageCall.into());
    flags.remove(G::SlowmodeEnabled.into());
    rights.remove(ChatAdminRight::ManageCall.into());
    flags.add(G::SlowmodeEnabled.into());
    assert_eq!(*log.borrow(), vec![true, false, false, false, true]);
    let user = PeerData::new(&s, peer_from_user(8u64));
    user.slowmode_applied_value(None)
        .on_next(|v| assert!(!v), &l);
}
#[test]
fn chat_name_old_letters_first_update_suppression_and_wrap16() {
    let (_s, q, peers, c) = make_store();
    let key = peers.ensure_peer(peer_from_chat(10u64));
    let l = Lifetime::new();
    let names = Rc::new(RefCell::new(Vec::new()));
    let copy = names.clone();
    let weak = Rc::downgrade(&peers);
    c.realtime_name_updates_for(key).on_next(
        move |u| {
            let p = weak.upgrade().unwrap();
            let name = p.read(key, |p| p.raw_name().to_owned());
            copy.borrow_mut().push((name, u.old_first_letters));
        },
        &l,
    );
    let fill = || {
        NameIndex::from_words(
            [peers.read(key, |p| p.raw_name().encode_utf16().collect::<Vec<_>>())],
        )
    };
    peers.set_name(key, "alpha", fill);
    assert!(names.borrow().is_empty());
    assert_eq!(peers.read(key, PeerData::name_version), 2);
    peers.set_name(key, "beta", fill);
    assert_eq!(
        *names.borrow(),
        vec![("beta".into(), BTreeSet::from([b'a' as u16]))]
    );
    peers.set_name(key, "beta", || panic!("same name skips index"));
    assert_eq!(peers.read(key, PeerData::name_version), 3);
    l.destroy();
    // Start from version3, make 65533 more distinct changes => wraps to zero.
    for i in 0..65_533 {
        peers.set_name(key, if i % 2 == 0 { "a" } else { "b" }, || {
            NameIndex::default()
        });
    }
    assert_eq!(peers.read(key, PeerData::name_version), 0);
    peers.set_name(key, "a", NameIndex::default);
    assert_eq!(peers.read(key, PeerData::name_version), 1);
    peers.set_name(key, "a", NameIndex::default);
    assert_eq!(peers.read(key, PeerData::name_version), 2);
    q.process_all();
}
#[test]
fn channel_name_flags_callback_can_reenter_and_fill_current_name() {
    let (_s, _q, p, c) = make_store();
    let key = p.ensure_peer(peer_from_channel(11u64));
    let names = Rc::new(RefCell::new(UsernamesInfo::new()));
    let flags = Flags::<G>::default();
    let l = Lifetime::new();
    let get = || names.borrow().editable_username().to_owned();
    let active = || names.borrow().username().to_owned();
    let set = |s: &str| names.borrow_mut().set_username(s);
    let set_flag = |on: bool| {
        if on {
            flags.add(G::Username.into())
        } else {
            flags.remove(G::Username.into())
        }
    };
    p.update_name_delayed(
        key,
        "old",
        "",
        "",
        NameFields::Channel {
            editable_username: &get,
            username: &active,
            set_username: &set,
            set_username_flag: &set_flag,
        },
        || NameIndex::from_words(["old".encode_utf16().collect()]),
    );
    let weak = Rc::downgrade(&p);
    let n = names.clone();
    let f = flags.clone();
    flags.changes().on_next(
        move |_| {
            let p = weak.upgrade().unwrap();
            let get = || n.borrow().editable_username().to_owned();
            let active = || n.borrow().username().to_owned();
            let set = |s: &str| n.borrow_mut().set_username(s);
            let sf = |on: bool| {
                if on {
                    f.add(G::Username.into())
                } else {
                    f.remove(G::Username.into())
                }
            };
            p.update_name_delayed(
                key,
                "inner",
                "",
                "inneruser",
                NameFields::Channel {
                    editable_username: &get,
                    username: &active,
                    set_username: &set,
                    set_username_flag: &sf,
                },
                || NameIndex::from_words([p.read(key, |p| p.raw_name().encode_utf16().collect())]),
            );
        },
        &l,
    );
    let log = Rc::new(RefCell::new(Vec::new()));
    let copy = log.clone();
    let weak = Rc::downgrade(&p);
    c.realtime_name_updates_for(key).on_next(
        move |u| {
            let p = weak.upgrade().unwrap();
            assert_eq!(p.read(key, |p| p.raw_name().to_owned()), "inner");
            copy.borrow_mut().push(u.old_first_letters);
        },
        &l,
    );
    p.update_name_delayed(
        key,
        "outer",
        "",
        "outeruser",
        NameFields::Channel {
            editable_username: &get,
            username: &active,
            set_username: &set,
            set_username_flag: &set_flag,
        },
        || NameIndex::from_words([p.read(key, |p| p.raw_name().encode_utf16().collect())]),
    );
    assert_eq!(names.borrow().username(), "inneruser");
    assert_eq!(
        *log.borrow(),
        vec![BTreeSet::from([b'o' as u16]), BTreeSet::from([b'o' as u16])]
    );
    assert_eq!(
        p.read(key, |p| p.name_words().clone()),
        BTreeSet::from(["inner".encode_utf16().collect::<Vec<_>>()])
    );
}
#[test]
fn user_name_phone_username_and_source_index_edge_rules() {
    let (_s, _q, p, _c) = make_store();
    let key = p.ensure_peer(peer_from_user(9u64));
    let n = RefCell::new(UsernamesInfo::new());
    let phone = RefCell::new(String::new());
    let get = || n.borrow().editable_username().to_owned();
    let gp = || phone.borrow().clone();
    let set = |s: &str| n.borrow_mut().set_username(s);
    let sp = |s: &str| *phone.borrow_mut() = s.into();
    p.update_name_delayed(
        key,
        "name",
        "+123",
        "Name",
        NameFields::User {
            editable_username: &get,
            name_or_phone: &gp,
            set_username: &set,
            set_name_or_phone: &sp,
        },
        NameIndex::default,
    );
    assert_eq!(phone.borrow().as_str(), "+123");
    assert_eq!(n.borrow().username(), "Name");
    p.update_name_delayed(
        key,
        "name",
        "+123",
        "Name",
        NameFields::User {
            editable_username: &get,
            name_or_phone: &gp,
            set_username: &set,
            set_name_or_phone: &sp,
        },
        || panic!("unchanged"),
    );
    let mut n = UsernamesInfo::new();
    n.set_usernames(&[
        Username {
            username: "first".into(),
            active: true,
            editable: false,
        },
        Username {
            username: "edit".into(),
            active: true,
            editable: true,
        },
    ]);
    n.set_username("");
    n.set_username("append");
    assert_eq!(n.editable_username(), "first");
    assert!(n.is_editable("first"));
    n.set_usernames(&[
        Username {
            username: "new".into(),
            active: true,
            editable: false,
        },
        Username {
            username: "inactive".into(),
            active: false,
            editable: true,
        },
    ]);
    assert_eq!(n.editable_username(), "new");
}
#[test]
fn bar_unknown_initial_suppression_reentrant_hide_links_and_details() {
    let (_s, _q, p, c) = make_store();
    let key = p.ensure_peer(peer_from_user(12u64));
    let log = Rc::new(RefCell::new(Vec::new()));
    let l = Lifetime::new();
    let copy = log.clone();
    p.read(key, PeerData::bar_settings_value)
        .on_next(move |v| copy.borrow_mut().push(v.value.value()), &l);
    assert!(log.borrow().is_empty());
    assert!(p.read(key, PeerData::hide_links));
    let weak = Rc::downgrade(&p);
    let effects = Rc::new(RefCell::new(Vec::new()));
    let out = effects.clone();
    p.effects().on_next(
        move |e| {
            if let PeerEffect::ScheduleHiddenLinksRefresh(k) = e {
                let p = weak.upgrade().unwrap();
                assert!(!p.read(k, PeerData::hide_links));
                out.borrow_mut().push(k);
            }
        },
        &l,
    );
    p.set_bar_settings(key, PeerBarSettings::empty());
    assert_eq!(*log.borrow(), vec![0]);
    assert_eq!(*effects.borrow(), vec![key]);
    let l2 = Lifetime::new();
    let init = Rc::new(Cell::new(0));
    let copy = init.clone();
    p.read(key, PeerData::bar_settings_value)
        .on_next(move |_| copy.set(copy.get() + 1), &l2);
    assert_eq!(init.get(), 1);
    let updates = Rc::new(Cell::new(0));
    let out = updates.clone();
    c.realtime_peer_updates(tg_core::data::data_changes::PeerFlag::PaysPerMessage)
        .on_next(move |_| out.set(out.get() + 1), &l);
    let data = tg_scheme::PeerSettingsData {
        flags: tg_scheme::PeerSettingsData::F_PHONE_COUNTRY
            | tg_scheme::PeerSettingsData::F_REGISTRATION_MONTH
            | tg_scheme::PeerSettingsData::F_NAME_CHANGE_DATE
            | tg_scheme::PeerSettingsData::F_PHOTO_CHANGE_DATE
            | tg_scheme::PeerSettingsData::F_REQUEST_CHAT_TITLE
            | tg_scheme::PeerSettingsData::F_CHARGE_PAID_MESSAGE_STARS,
        phone_country: Some(b"RU".to_vec()),
        registration_month: Some(b"11.2026".to_vec()),
        name_change_date: Some(42),
        photo_change_date: Some(43),
        request_chat_title: Some(b"request".to_vec()),
        request_chat_date: Some(44),
        charge_paid_message_stars: Some(12),
        ..Default::default()
    };
    p.set_bar_settings_from_mtp(key, &data);
    p.read(key, |p| {
        assert_eq!(p.phone_country_code(), "RU");
        assert_eq!(p.registration_month(), 11);
        assert_eq!(p.registration_year(), 2026);
        assert_eq!(p.name_change_date(), 42);
        assert_eq!(p.photo_change_date(), 43);
        assert_eq!(p.request_chat_title(), "request");
        assert_eq!(p.request_chat_date(), 44);
        assert_eq!(p.pays_per_message(), 12);
        assert_eq!(p.business_bot(), None);
        assert_eq!(p.business_bot_manage_url(), "");
    });
    p.update(key, PeerData::clear_pays_per_message);
    assert_eq!(p.read(key, PeerData::pays_per_message), 0);
    assert_eq!(p.read(key, |p| p.phone_country_code().to_owned()), "RU");
    p.clear_business_bot(key);
    assert_eq!(p.read(key, |p| p.phone_country_code().to_owned()), "RU");
    let invalid = tg_scheme::PeerSettingsData {
        phone_country: Some(b"not present without flags".to_vec()),
        ..Default::default()
    };
    p.set_bar_settings_from_mtp(key, &invalid);
    assert_eq!(p.read(key, |p| p.phone_country_code().to_owned()), "");
    // High-only PaysPerMessage (bit37) has no realtime callback in pinned Changes.
    assert_eq!(updates.get(), 0);
}
#[test]
fn unavailable_reason_preload_list_assignment_and_notification_order() {
    let (_s, _q, p, c) = make_store();
    let key = p.ensure_peer(peer_from_user(13u64));
    let reasons = RefCell::new(Vec::<UnavailableReason>::new());
    let l = Lifetime::new();
    let log = Rc::new(RefCell::new(Vec::new()));
    let out = log.clone();
    p.effects().on_next(
        move |e| {
            if matches!(e, PeerEffect::PreloadSensitiveContent) {
                out.borrow_mut().push("preload");
            }
        },
        &l,
    );
    let actual = Rc::new(RefCell::new(Vec::<UnavailableReason>::new()));
    let snapshot = actual.clone();
    let out = log.clone();
    c.realtime_peer_updates(tg_core::data::data_changes::PeerFlag::UnavailableReason)
        .on_next(
            move |_| {
                assert_eq!(snapshot.borrow()[0].reason, "other");
                out.borrow_mut().push("updated");
            },
            &l,
        );
    p.set_unavailable_reasons(
        key,
        vec![
            UnavailableReason::sensitive_reason(),
            UnavailableReason {
                reason: "other".into(),
                text: "text".into(),
            },
        ],
        || reasons.borrow().clone(),
        |v| {
            *actual.borrow_mut() = v.clone();
            *reasons.borrow_mut() = v;
            log.borrow_mut().push("assign");
        },
    );
    assert_eq!(*log.borrow(), vec!["preload", "assign", "updated"]);
    assert!(p.read(key, PeerData::has_sensitive_content));
    assert!(UnavailableReason::sensitive_reason().sensitive());
    assert!(UnavailableReason::ignore_sensitive_mark());
    assert_eq!(UnavailableReason::compute(&reasons.borrow()), "");
    assert!(UnavailableReason::extract(None).is_empty());
    assert!(p.read(key, |peer| peer.unavailable_reasons().is_empty()));
}
#[test]
#[should_panic(expected = "PeerData::setUnavailableReasonsList.")]
fn base_unavailable_list_setter_keeps_original_unexpected() {
    let scope = SessionScope::new();
    let mut peer = PeerData::new(&scope, peer_from_chat(1u64));
    peer.set_unavailable_reasons_list(Vec::new());
}
#[test]
fn userpic_location_identity_excludes_dimensions_hash_and_reference() {
    let s = SessionScope::new();
    let mut p = PeerData::new(&s, peer_from_user(42u64));
    assert!(p.userpic_photo_unknown());
    assert_eq!(p.userpic_photo_id(), 0);
    assert_eq!(p.userpic_photo_key(), None);
    assert!(!p.has_userpic());
    let a = StorageFileLocation {
        dc_id: 2,
        kind: StorageFileType::PeerPhoto,
        id: 42,
        volume_id: 7,
        size_letter: b'a',
        access_hash: 1,
        file_reference: vec![1],
        ..Default::default()
    };
    let mut b = a.clone();
    b.access_hash = 99;
    b.file_reference = vec![2];
    b.in_message_id = 123;
    assert_eq!(a, b);
    assert!(a.valid());
    assert!(!a.is_legacy());
    let image = ImageLocation::new(DownloadLocation::Storage(a.clone()), 160, 160);
    let mut other = ImageLocation::new(DownloadLocation::Storage(b), 1, 2);
    assert_eq!(image, other);
    other.set_size(300, 400);
    assert_eq!(image, other);
    assert!(image.valid());
    assert_eq!(
        p.set_userpic_checked(7, image.clone(), true).effects.len(),
        2
    );
    assert_eq!(p.userpic_photo_key(), Some(s.photo_key(7)));
    assert!(p.userpic_has_video());
    assert!(p.has_userpic());
    assert_eq!(p.userpic_location().width, 160);
    assert!(p.set_userpic_checked(7, other, true).effects.is_empty());
    assert_eq!(p.userpic_location().width, 160);
    assert_eq!(p.clear_userpic().effects.len(), 3);
    assert!(!p.has_userpic());
    assert!(!p.userpic_photo_unknown());
    assert_eq!(p.userpic_photo_id(), 0);
    assert_eq!(
        p.set_userpic_photo_id(Some(s.photo_key(10))).effects.len(),
        1
    );
    assert!(p
        .set_userpic_photo_id(Some(s.photo_key(10)))
        .effects
        .is_empty());
    assert_eq!(p.set_userpic_photo_id(None).effects.len(), 1);
    for kind in [
        StorageFileType::Encrypted,
        StorageFileType::Secure,
        StorageFileType::Document,
        StorageFileType::Photo,
        StorageFileType::Takeout,
        StorageFileType::StickerSetThumb,
        StorageFileType::GroupCallStream,
    ] {
        let a = StorageFileLocation { kind, ..a.clone() };
        assert!(a.valid());
        let mut b = a.clone();
        b.access_hash += 1;
        assert_eq!(a, b);
    }
    assert_eq!(
        StorageFileLocation::default(),
        StorageFileLocation {
            kind: StorageFileType::Document,
            dc_id: 3,
            ..Default::default()
        }
    );
    assert!(StorageFileLocation {
        dc_id: 1,
        kind: StorageFileType::Legacy,
        volume_id: 1,
        local_id: 1,
        ..Default::default()
    }
    .valid());
    assert!(ImageLocation::new(DownloadLocation::Geo(GeoPointLocation::default()), 0, 0).valid());
    assert!(!ImageLocation::new(
        DownloadLocation::Web {
            access_hash: 0,
            url: vec![]
        },
        0,
        0
    )
    .valid());
    assert!(ImageLocation::new(DownloadLocation::Url("x".into()), 0, 0).valid());
    assert!(ImageLocation::new(DownloadLocation::AudioAlbumThumb { document_id: 1 }, 0, 0).valid());
    assert!(ImageLocation::new(DownloadLocation::InMemory(vec![1]), 0, 0).valid());
    let flags = Flags::<U>::new(U::SelfUser.into());
    let names = UsernamesInfo::new();
    let fields = user_view(p.key(), &flags, &names);
    let input = tg_scheme::mtp_input_peer_self();
    let result = p.update_userpic(30, 65538, false, &fields, &input);
    assert!(!result.effects.is_empty());
    let DownloadLocation::Storage(file) = &p.userpic_location().file else {
        panic!()
    };
    assert_eq!(file.dc_id, 2);
    assert_eq!(file.id, 42);
    assert_eq!(file.volume_id, 30);
    assert_eq!(file.size_letter, b'a');
}
#[test]
fn pinned_existing_storage_migrated_priority_and_effect_order() {
    use tg_core::data::data_msg_id::{MsgId, SERVER_MAX_MSG_ID};
    use tg_core::storage::storage_shared_media::*;
    use tg_core::storage::storage_sparse_ids_list::MsgRange;
    let s = SessionScope::new();
    let a = s.peer_key(peer_from_channel(2u64));
    let old = s.peer_key(peer_from_chat(1u64));
    let mut store = SharedMedia::default();
    let actions = set_top_pinned_message_id(a, MsgId::new(10), Some(MsgId::new(10)), MsgId::new(4));
    assert!(actions.is_empty());
    let actions = set_top_pinned_message_id(a, MsgId::new(10), None, MsgId::new(4));
    assert!(matches!(actions[0], PinnedEffect::ClearHiddenPinned { .. }));
    assert!(matches!(actions[1], PinnedEffect::SaveSettingsDelayed));
    assert!(matches!(actions[2], PinnedEffect::AddExisting(_)));
    assert!(matches!(
        actions[3],
        PinnedEffect::SetHistoryHasPinned { value: true, .. }
    ));
    for a in actions {
        if let PinnedEffect::AddExisting(q) = a {
            store.add_existing(q);
        }
    }
    assert_eq!(
        resolve_top_pinned_id(&store, a.id(), MsgId::new(0), PeerId::new(0), None).msg,
        MsgId::new(10)
    );
    assert_eq!(
        resolve_min_pinned_id(&store, a.id(), MsgId::new(0), PeerId::new(0), None).msg,
        MsgId::new(0)
    );
    store.add_slice(SharedMediaAddSlice {
        peer_id: old.id().value,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        message_ids: vec![1, 2],
        no_skip_range: MsgRange {
            from: 0,
            till: SERVER_MAX_MSG_ID.bare,
        },
        type_: SharedMediaType::Pinned,
        count: Some(2),
    });
    assert_eq!(
        resolve_min_pinned_id(
            &store,
            a.id(),
            MsgId::new(0),
            PeerId::new(0),
            Some(old.id())
        )
        .peer,
        old.id()
    );
    assert_eq!(
        resolve_top_pinned_id(
            &store,
            a.id(),
            MsgId::new(0),
            PeerId::new(0),
            Some(old.id())
        )
        .peer,
        a.id()
    );
    store.add_slice(SharedMediaAddSlice {
        peer_id: a.id().value,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        message_ids: vec![],
        no_skip_range: MsgRange {
            from: 0,
            till: SERVER_MAX_MSG_ID.bare,
        },
        type_: SharedMediaType::Pinned,
        count: Some(0),
    }); // addSlice does not remove known items; use a fresh empty peer
    let empty = peer_from_channel(3u64);
    store.add_slice(SharedMediaAddSlice {
        peer_id: empty.value,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        message_ids: vec![],
        no_skip_range: MsgRange {
            from: 0,
            till: SERVER_MAX_MSG_ID.bare,
        },
        type_: SharedMediaType::Pinned,
        count: Some(0),
    });
    assert_eq!(
        resolve_top_pinned_id(&store, empty, MsgId::new(0), PeerId::new(0), Some(old.id())).msg,
        MsgId::new(2)
    );
    assert!(
        resolve_top_pinned_id(&store, empty, MsgId::new(1), PeerId::new(0), Some(old.id()))
            .is_zero()
    );
}
#[test]
fn notify_unknown_default_partial_changes_sound_and_tl_flags() {
    let mut n = PeerNotifySettings::new();
    assert!(n.settings_unknown());
    assert_eq!(n.mute_until(), None);
    assert_eq!(n.silent_posts(), None);
    assert_eq!(n.sound(), None);
    assert!(n.reset_to_default());
    assert!(!n.reset_to_default());
    assert!(!n.settings_unknown());
    assert!(!n.change_local(MuteValue::default(), None, None, None, 100));
    assert!(n.change_local(
        MuteValue {
            period: 30,
            ..Default::default()
        },
        Some(false),
        Some(NotifySound {
            title: "local".into(),
            data: "file".into(),
            ..Default::default()
        }),
        Some(false),
        100
    ));
    assert_eq!(n.mute_until(), Some(130));
    assert_eq!(n.silent_posts(), Some(false));
    assert_eq!(n.sound().unwrap().title, "local");
    let value = n.serialize();
    let tg_scheme::InputPeerNotifySettings::InputPeerNotifySettings(d) = value;
    assert_eq!(d.vmute_until(), Some(130));
    assert_eq!(d.vsilent(), Some(false));
    assert_eq!(d.vstories_muted(), Some(false));
    assert!(d.vshow_previews().is_none());
    assert!(matches!(
        d.vsound(),
        Some(tg_scheme::NotificationSound::NotificationSoundLocal(_))
    ));
    assert!(n.change_local(
        MuteValue {
            forever: true,
            ..Default::default()
        },
        None,
        Some(NotifySound {
            id: 5,
            ..Default::default()
        }),
        None,
        100
    ));
    assert_eq!(n.mute_until(), Some(i32::MAX));
    let tg_scheme::InputPeerNotifySettings::InputPeerNotifySettings(d) = n.serialize();
    assert!(
        matches!(d.vsound(),Some(tg_scheme::NotificationSound::NotificationSoundRingtone(v)) if v.id==5)
    );
    assert_eq!(
        MuteValue {
            period: 100,
            ..Default::default()
        }
        .until(i32::MAX),
        i32::MAX
    );
    assert_eq!(
        MuteValue {
            unmute: true,
            ..Default::default()
        }
        .until(1),
        0
    );
    assert_eq!(MuteValue::default().until(1), -1);
    assert!(MuteValue {
        period: -1,
        ..Default::default()
    }
    .is_present());
    let raw = tg_scheme::mtp_peer_notify_settings(
        tg_scheme::PeerNotifySettingsData::F_OTHER_SOUND
            | tg_scheme::PeerNotifySettingsData::F_SHOW_PREVIEWS,
        Some(false),
        None,
        None,
        None,
        None,
        Some(tg_scheme::mtp_notification_sound_none()),
        None,
        None,
        None,
        None,
        None,
    );
    assert!(n.change(&raw));
    assert_eq!(n.mute_until(), None);
    assert!(n.sound().unwrap().none);
    assert!(!n.change(&raw));
    let tg_scheme::InputPeerNotifySettings::InputPeerNotifySettings(d) = n.serialize();
    assert_eq!(d.vshow_previews(), Some(false));
    assert!(matches!(
        d.vsound(),
        Some(tg_scheme::NotificationSound::NotificationSoundNone(_))
    ));
    let raw = tg_scheme::mtp_peer_notify_settings(
        tg_scheme::tl::Flags::empty(),
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
    );
    assert!(n.change(&raw));
    assert!(!n.change(&raw));
    let tg_scheme::InputPeerNotifySettings::InputPeerNotifySettings(d) = n.serialize();
    assert!(d.flags.is_empty());
}
#[test]
fn peer_constants_restriction_values_and_rating_presence() {
    let s = SessionScope::new();
    for id in [333000u64, 777000] {
        assert!(PeerData::new(&s, peer_from_user(id)).is_notifications_user());
    }
    assert!(PeerData::new(&s, peer_from_user(2666000u64)).is_saved_hidden_author());
    assert!(PeerData::new(&s, peer_from_user(489000u64)).is_verify_codes());
    assert!(PeerData::new(&s, peer_from_user(1271266957u64)).is_replies_chat(true));
    assert!(PeerData::new(&s, peer_from_user(708513u64)).is_replies_chat(false));
    assert!(RestrictionCheckResult::Allowed.is_allowed());
    assert!(!RestrictionCheckResult::Allowed.restricted());
    assert!(RestrictionCheckResult::Explicit.is_explicit());
    assert!(RestrictionCheckResult::WithEveryone.is_with_everyone());
    assert!(!StarsRating::default().is_present());
    assert!(StarsRating {
        this_level_stars: 1,
        ..Default::default()
    }
    .is_present());
    assert!(!StarsRatingPending {
        value: StarsRating {
            level: 1,
            ..Default::default()
        },
        date: 0
    }
    .is_present());
    assert!(StarsRatingPending {
        value: StarsRating {
            level: 1,
            ..Default::default()
        },
        date: 1
    }
    .is_present());
    assert_eq!(registration_year(201299), 0);
    assert_eq!(registration_month(202613), 0);
    assert_eq!(registration_year(209912), 2099);
    assert!(is_freeze_appeal_username("ſpambot"));
    assert!(!is_freeze_appeal_username("spam\0bot"));
}
#[test]
#[should_panic(expected = "foreign peer projection")]
fn projected_fields_cannot_cross_scopes() {
    let a = SessionScope::new();
    let b = SessionScope::new();
    let p = PeerData::new(&a, peer_from_user(1u64));
    let flags = Flags::<U>::default();
    let names = UsernamesInfo::new();
    let f = user_view(b.peer_key(p.id()), &flags, &names);
    p.is_self(&f);
}
#[test]
fn color_profile_tabs_and_bot_button_value_helpers() {
    let (_s, _q, p, _c) = make_store();
    let key = p.ensure_peer(peer_from_user(14u64));
    assert_eq!(parse_profile_tab(None), ProfileTab::None);
    assert_eq!(color_index_from_color(None), None);
    assert_eq!(background_emoji_id_from_color(None), 0);
    let color = tg_scheme::mtp_peer_color(
        tg_scheme::PeerColorData::F_COLOR | tg_scheme::PeerColorData::F_BACKGROUND_EMOJI_ID,
        Some(-1),
        Some(42),
    );
    assert_eq!(color_index_from_color(Some(&color)), Some(255));
    assert_eq!(background_emoji_id_from_color(Some(&color)), 42);
    assert!(p.change_color(key, Some(&color), |_| {
        assert_eq!(p.read(key, PeerData::color_index), 63);
        assert_eq!(p.read(key, PeerData::background_emoji_id), 42);
        false
    }));
    assert!(!p.change_color(key, Some(&color), |_| false));
    assert!(p.change_color(key, Some(&color), |_| true));
    assert!(p.change_color_profile(key, Some(&color), |_| false));
    assert_eq!(p.read(key, PeerData::color_profile_index), Some(63));
    assert_eq!(p.read(key, PeerData::profile_background_emoji_id), 42);
    assert!(p.change_color_profile(key, None, |_| false));
    assert!(p.change_color(key, None, |_| false));
    let ignored = tg_scheme::mtp_peer_color(tg_scheme::tl::Flags::empty(), Some(7), Some(99));
    assert_eq!(color_index_from_color(Some(&ignored)), None);
    assert_eq!(background_emoji_id_from_color(Some(&ignored)), 0);
    for tab in [
        ProfileTab::Posts,
        ProfileTab::Gifts,
        ProfileTab::Media,
        ProfileTab::Files,
        ProfileTab::Music,
        ProfileTab::Voice,
        ProfileTab::Links,
        ProfileTab::Gifs,
    ] {
        assert_eq!(parse_profile_tab(Some(&profile_tab_to_mtp(tab))), tab);
    }
    let (mut text, mut url) = ("old".into(), "old".into());
    assert!(apply_bot_menu_button(&mut text, &mut url, None));
    assert!(!apply_bot_menu_button(&mut text, &mut url, None));
    let button =
        tg_scheme::mtp_bot_menu_button(b"Open".to_vec(), b"https://example.invalid".to_vec());
    assert!(apply_bot_menu_button(&mut text, &mut url, Some(&button)));
    assert_eq!(text, "Open");
    assert!(!apply_bot_menu_button(&mut text, &mut url, Some(&button)));
}
#[test]
#[should_panic(expected = "ProfileTabToMTP")]
fn no_none_profile_tab_encoding() {
    profile_tab_to_mtp(ProfileTab::None);
}
#[test]
fn premium_photo_check_reads_state_after_realtime_callback() {
    let (_s, _q, p, c) = make_store();
    let key = p.ensure_peer(peer_from_user(15u64));
    let l = Lifetime::new();
    let once = Cell::new(false);
    let weak = Rc::downgrade(&p);
    p.set_userpic_checked(key, 7, ImageLocation::default(), false, || true, || 0);
    c.realtime_peer_updates(tg_core::data::data_changes::PeerFlag::Photo)
        .on_next(
            move |_| {
                if !once.replace(true) {
                    let p = weak.upgrade().unwrap();
                    p.update(key, |p| {
                        p.set_userpic(PeerData::UNKNOWN_PHOTO_ID, ImageLocation::default(), false)
                    });
                }
            },
            &l,
        );
    let requests = Rc::new(Cell::new(0));
    let out = requests.clone();
    let weak = Rc::downgrade(&p);
    p.effects().on_next(
        move |e| {
            if let PeerEffect::RequestFullPeer(k) = e {
                assert!(weak
                    .upgrade()
                    .unwrap()
                    .read(k, PeerData::userpic_photo_unknown));
                out.set(out.get() + 1);
            }
        },
        &l,
    );
    p.set_userpic_checked(key, 0, ImageLocation::default(), false, || true, || 0);
    assert_eq!(requests.get(), 1);
}
#[test]
fn migrated_hide_links_and_name_resolution_projections() {
    let (_s, _q, p, _c) = make_store();
    let old = p.ensure_peer(peer_from_chat(18u64));
    let target = p.ensure_peer(peer_from_channel(19u64));
    p.set_bar_settings(old, PeerBarSettings::empty());
    assert!(p.hide_links_with_migration(old, &|k| if k == old { Some(target) } else { None }));
    let log = Rc::new(RefCell::new(Vec::new()));
    let out = log.clone();
    let l = Lifetime::new();
    p.effects().on_next(
        move |e| {
            if let PeerEffect::ScheduleHiddenLinksRefresh(k) = e {
                out.borrow_mut().push(k);
            }
        },
        &l,
    );
    p.set_bar_settings_with_migration(
        target,
        PeerBarSettings::empty(),
        |_| None,
        |k| if k == target { Some(old) } else { None },
    );
    assert_eq!(*log.borrow(), vec![target, old]);
    assert!(!p.hide_links_with_migration(old, &|k| if k == old { Some(target) } else { None }));
    let flags = Flags::<C>::default();
    let f = PeerFields::Chat(ChatPeerFields {
        migrated_to: None,
        group_call: None,
        group_call_default_join_as: PeerId::new(0),
        key: ChatKey::try_from(old).unwrap(),
        flags: &flags,
        admin_rights: ChatAdminRights::empty(),
        default_restrictions: ChatRestrictions::empty(),
    });
    p.read(old, |peer| {
        assert_eq!(peer.name(Some("migrated"), Some("broadcast")), "migrated");
        assert_eq!(peer.short_name_with_links(&f, Some("short"), None), "short");
        assert_eq!(
            peer.top_bar_name_text_with_migration(&f, Some("top")),
            "top"
        );
    });
}
#[test]
#[should_panic(expected = "index out of bounds")]
fn source_stale_editable_index_panics_instead_of_cpp_undefined_access() {
    let mut n = UsernamesInfo::new();
    n.set_usernames(&[
        Username {
            username: "one".into(),
            active: true,
            editable: false,
        },
        Username {
            username: "edit".into(),
            active: true,
            editable: true,
        },
    ]);
    n.set_usernames(&[
        Username {
            username: "one".into(),
            active: true,
            editable: false,
        },
        Username {
            username: "inactive".into(),
            active: false,
            editable: true,
        },
    ]);
    assert!(!n.is_editable("inactive"));
    n.editable_username();
}
#[test]
fn migration_forum_monoforum_and_group_call_keys_are_real_projections() {
    use tg_core::data::data_msg_id::MsgId;
    let (s, _q, store, _c) = make_store();
    let old = store.ensure_peer(peer_from_chat(20u64));
    let channel = store.ensure_peer(peer_from_channel(21u64));
    let broadcast = store.ensure_peer(peer_from_channel(22u64));
    let cf = Flags::<C>::default();
    let f = PeerFields::Chat(ChatPeerFields {
        key: ChatKey::try_from(old).unwrap(),
        flags: &cf,
        admin_rights: ChatAdminRights::empty(),
        default_restrictions: ChatRestrictions::empty(),
        migrated_to: Some(ChannelKey::try_from(channel).unwrap()),
        group_call: Some(s.group_call_key(55)),
        group_call_default_join_as: old.id(),
    });
    store.read(old, |p| {
        assert_eq!(p.migrate_to(&f, |_| false), None);
        assert_eq!(p.as_chat_not_migrated(&f, |_| false), p.as_chat());
        assert_eq!(p.migrate_to(&f, |_| true).unwrap().peer(), channel);
        assert_eq!(p.as_chat_not_migrated(&f, |_| true), None);
        assert_eq!(
            p.as_channel_or_migrated(&f, |_| true).unwrap().peer(),
            channel
        );
        assert_eq!(p.migrate_to_or_me(&f, |_| true), channel);
        assert_eq!(p.group_call(&f).unwrap().id(), 55);
        assert_eq!(p.group_call(&f).unwrap().scope(), s.id());
        assert_eq!(p.group_call_default_join_as(&f), old.id());
        assert_eq!(p.forum(&f), None);
    });
    let gf = Flags::<G>::new(G::Megagroup | G::Monoforum | G::Forum | G::ForumTabs);
    let names = UsernamesInfo::new();
    let mut f = channel_view(channel, &gf, &names);
    if let PeerFields::Channel(v) = &mut f {
        v.migrated_from = Some(ChatKey::try_from(old).unwrap());
        v.monoforum_link = Some(ChannelKey::try_from(broadcast).unwrap());
        v.forum = Some(channel);
        v.monoforum = Some(channel);
        v.group_call = Some(s.group_call_key(56));
        v.group_call_default_join_as = broadcast.id();
    }
    store.read(channel, |p| {
        assert_eq!(p.migrate_from(&f).unwrap().peer(), old);
        assert_eq!(p.monoforum_broadcast(&f).unwrap().peer(), broadcast);
        assert_eq!(p.userpic_painting_peer(&f), broadcast);
        assert_eq!(p.monoforum(&f), Some(channel));
        assert!(p.use_subsection_tabs(&f));
        assert!(p.display_subsection_tabs(&f, || false));
        assert_eq!(p.group_call(&f).unwrap().id(), 56);
        assert_eq!(p.group_call_default_join_as(&f), broadcast.id());
        assert!(p
            .forum_topic_for(&f, MsgId::new(0), |_, _| panic!())
            .is_none());
        let topic = p
            .forum_topic_for(&f, MsgId::new(1), |owner, root_id| {
                Some(TopicKey {
                    history: owner,
                    root_id,
                })
            })
            .unwrap();
        assert_eq!(topic.history, channel);
        let called = Cell::new(0);
        p.process_topics(&f, &[], |owner, topics| {
            assert_eq!(owner, channel);
            assert!(topics.is_empty());
            called.set(called.get() + 1);
        });
        assert_eq!(called.get(), 1);
        assert!(p.can_export_chat_history(&f, true, true, 0, || true, None));
        assert!(!p.can_export_chat_history(&f, true, false, 5, || panic!(), None));
    });
    gf.add(G::Left.into());
    assert_eq!(store.read(channel, |p| p.migrate_from(&f)), None);
    gf.remove(G::Left.into());
    gf.add(G::Broadcast.into());
    assert_eq!(
        store
            .read(channel, |p| p.broadcast_monoforum(&f))
            .unwrap()
            .peer(),
        broadcast
    );
    let sub = peer_from_user(77u64);
    let loaded = store
        .monoforum_sublist_for(channel, sub, Some(channel), |history, peer| {
            Some(SublistKey::new(history, peer))
        })
        .unwrap();
    assert_eq!(loaded.history(), channel);
    assert_eq!(loaded.peer(), store.ensure_peer(sub));
    assert!(store
        .monoforum_sublist_for(channel, PeerId::new(0), Some(channel), |_, _| panic!())
        .is_none());
}
#[test]
fn fill_names_requires_real_text_boundary_and_uses_only_active_first_username() {
    use tg_core::data::data_peer_search::*;
    struct RestrictedText;
    impl NameTextServices for RestrictedText {
        fn remove_accents(&self, v: &str) -> String {
            assert!(v.is_ascii());
            v.into()
        }
        fn prepare_search_words(&self, v: &str) -> Vec<Vec<u16>> {
            assert!(v.chars().all(|c| c.is_ascii_alphanumeric()
                || c.is_whitespace()
                || ('А'..='я').contains(&c)
                || matches!(c, 'І' | 'і' | 'Ї' | 'ї' | 'Є' | 'є')));
            v.split_whitespace()
                .map(|s| s.to_lowercase().encode_utf16().collect())
                .collect()
        }
    }
    let s = SessionScope::new();
    let p = PeerData::new(&s, peer_from_user(42u64));
    let flags = Flags::<U>::new(U::SelfUser.into());
    let mut names = UsernamesInfo::new();
    names.set_usernames(&[
        Username {
            username: "user".into(),
            active: true,
            editable: true,
        },
        Username {
            username: "second".into(),
            active: true,
            editable: false,
        },
    ]);
    let mut f = user_view(p.key(), &flags, &names);
    if let PeerFields::User(v) = &mut f {
        v.name_or_phone = "Alice";
    }
    let index = fill_names(
        &p,
        &f,
        "Alice",
        true,
        NameSearchLabels {
            saved_messages: "Saved messages",
            replies: "Replies",
            verification_codes: "Verification Codes",
        },
        &RestrictedText,
    );
    assert!(index
        .words
        .contains(&"alice".encode_utf16().collect::<Vec<_>>()));
    assert!(index
        .words
        .contains(&"user".encode_utf16().collect::<Vec<_>>()));
    assert!(!index
        .words
        .contains(&"second".encode_utf16().collect::<Vec<_>>()));
    assert!(index
        .words
        .contains(&"saved".encode_utf16().collect::<Vec<_>>()));
    assert!(index
        .words
        .contains(&"messages".encode_utf16().collect::<Vec<_>>()));
    assert!(index.first_letters.contains(&(b'a' as u16)));
}
#[test]
fn native_qt_normalization_samples_preserve_utf16_index_values() {
    // A fixture service is intentionally limited to recorded inputs; no production
    // fallback or imitation of the not-yet-ported TextUtilities is installed.
    use tg_core::data::data_peer_search::NameTextServices;
    struct RecordedQt;
    impl NameTextServices for RecordedQt {
        fn remove_accents(&self, input: &str) -> String {
            for line in include_str!("fixtures/data-peer/pure.csv").lines() {
                let c: Vec<_> = line.split(',').collect();
                if c[0] == "normalization" && unhex(c[1]) == input.as_bytes() {
                    return String::from_utf8(unhex(c[2])).unwrap();
                }
            }
            panic!("unrecorded Qt input")
        }
        fn prepare_search_words(&self, input: &str) -> Vec<Vec<u16>> {
            for line in include_str!("fixtures/data-peer/pure.csv").lines() {
                let c: Vec<_> = line.split(',').collect();
                if c[0] == "normalization" && unhex(c[1]) == input.as_bytes() {
                    return c[3]
                        .split(';')
                        .filter(|s| !s.is_empty())
                        .map(|s| {
                            String::from_utf8(unhex(s))
                                .unwrap()
                                .encode_utf16()
                                .collect()
                        })
                        .collect();
                }
            }
            panic!("unrecorded Qt input")
        }
    }
    let qt = RecordedQt;
    assert_eq!(qt.remove_accents("École 東京"), "ecole 東京");
    assert_eq!(qt.remove_accents("Сохранённое"), "Сохраненное");
    let index = NameIndex::from_words(qt.prepare_search_words("a\u{301} 😀"));
    assert_eq!(index.first_letters, BTreeSet::from([b'a' as u16, 0xd83d]));
    assert_eq!(qt.prepare_search_words("abc@User-Name_1").len(), 4);
}
#[test]
fn folder_notify_and_bar_detail_clear_keep_original_drop_conditions() {
    let s = SessionScope::new();
    let mut p = PeerData::new(&s, peer_from_user(32u64));
    assert!(p.notify().settings_unknown());
    assert!(p.notify_mut().reset_to_default());
    assert!(!p.notify().settings_unknown());
    assert!(p.check_folder(0, true, Some(1)).effects.is_empty());
    assert!(p.check_folder(2, false, Some(1)).effects.is_empty());
    assert!(p.check_folder(2, true, None).effects.is_empty());
    assert_eq!(p.check_folder(2, true, Some(1)).effects.len(), 1);
    p.apply_bar_details(Some(PeerBarDetails {
        registration_date: 202611,
        pays_per_message: 5,
        ..Default::default()
    }));
    assert_eq!(p.registration_year(), 2026);
    assert_eq!(p.clear_pays_per_message().effects.len(), 1);
    assert_eq!(p.registration_year(), 0); // date-only details are not kept by the original condition
}
