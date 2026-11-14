use super::super::{
    data_msg_id::{FullMsgId, FullStoryId, MsgId},
    data_peer_id::{peer_from_channel, peer_from_chat, peer_from_user},
};
use super::*;
use tg_rpl::Lifetime;
type Log = Rc<RefCell<Vec<String>>>;
fn observe<U: Update>(
    producer: Producer<U>,
    tag: &'static str,
    index: usize,
    a: U::Key,
    log: &Log,
    life: &Lifetime,
) {
    let log = log.clone();
    producer.on_next(
        move |u| {
            log.borrow_mut().push(format!(
                "{tag},{index},{},{}",
                if u.key() == a { 1 } else { 2 },
                u.bits()
            ))
        },
        life,
    );
}
fn native_scenario<U: Update>(name: &str, a: U::Key, b: U::Key, log: &Log) {
    log.borrow_mut().push(format!("manager,{name}"));
    let manager = Rc::new(Manager::<U>::new());
    let life = Lifetime::new();
    for i in 0..U::COUNT {
        observe(manager.realtime_updates(1 << i), "r", i, a, log, &life);
    }
    observe(manager.updates(u64::MAX), "p", 0, a, log, &life);
    observe(manager.updates_for(a, 2), "f", 0, a, log, &life);
    observe(manager.flags_value(a, 4), "v", 0, a, log, &life);
    manager.updated(b, 2, false);
    manager.updated(a, 1, false);
    manager.updated(a, 2, false);
    manager.updated(a, 2, false);
    manager.send_notifications();
    manager.updated(a, 4, false);
    manager.updated(a, 8, true);
    manager.updated(a, 16, false);
    manager.drop_scheduled(a);
    manager.send_notifications();
    manager.updated(a, 0, false);
    manager.send_notifications();
    let weak = Rc::downgrade(&manager);
    manager.realtime_updates(2).on_next(
        move |u| {
            if u.bits() == 2 {
                let m = weak.upgrade().unwrap();
                m.send_notifications();
                m.updated(b, 4, false);
            }
        },
        &life,
    );
    manager.updated(a, 1, false);
    manager.updated(a, 2, false);
    manager.send_notifications();
    let weak = Rc::downgrade(&manager);
    let nested = Cell::new(false);
    manager.updates(8).on_next(
        move |_| {
            if !nested.replace(true) {
                let m = weak.upgrade().unwrap();
                m.updated(b, 16, false);
                m.send_notifications();
            }
        },
        &life,
    );
    manager.updated(a, 8, false);
    manager.updated(b, 1, false);
    manager.send_notifications();
    manager.send_notifications();
}
#[test]
fn native_all_manager_sequences_and_changes_flush_order() {
    let scope = SessionScope::new();
    let a = scope.peer_key(peer_from_user(1u64));
    let b = scope.peer_key(peer_from_user(2u64));
    let log = Log::default();
    native_scenario::<PeerUpdate>("peer", a, b, &log);
    native_scenario::<HistoryUpdate>("history", a, b, &log);
    let ta = TopicKey {
        history: a,
        root_id: MsgId::new(1),
    };
    let tb = TopicKey {
        history: b,
        root_id: MsgId::new(1),
    };
    native_scenario::<TopicUpdate>("topic", ta, tb, &log);
    let sa = SublistKey::new(a, a);
    let sb = SublistKey::new(a, b);
    native_scenario::<SublistUpdate>("sublist", sa, sb, &log);
    let mut messages = MessageKeys::new(&scope);
    let ma = messages.insert(FullMsgId::new(a.id(), MsgId::new(1)));
    let mb = messages.insert(FullMsgId::new(b.id(), MsgId::new(1)));
    native_scenario::<MessageUpdate>("message", ma, mb, &log);
    native_scenario::<EntryUpdate>("entry", EntryKey::History(a), EntryKey::History(b), &log);
    let sta = scope.story_key(FullStoryId {
        peer: a.id(),
        story: 1,
    });
    let stb = scope.story_key(FullStoryId {
        peer: b.id(),
        story: 1,
    });
    native_scenario::<StoryUpdate>("story", sta, stb, &log);
    log.borrow_mut().push("changes,order".into());
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    macro_rules! order {
        ($method:ident,$flags:ty,$tag:literal) => {{
            let log = log.clone();
            c.$method(<$flags>::from_raw(1))
                .on_next(move |_| log.borrow_mut().push($tag.into()), &life);
        }};
    }
    order!(peer_updates, PeerFlags, "peer");
    order!(history_updates, HistoryFlags, "history");
    order!(message_updates, MessageFlags, "message");
    order!(entry_updates, EntryFlags, "entry");
    order!(topic_updates, TopicFlags, "topic");
    order!(sublist_updates, SublistFlags, "sublist");
    order!(story_updates, StoryFlags, "story");
    c.story_updated(sta, StoryFlags::from_raw(1));
    c.sublist_updated(sa, SublistFlags::from_raw(1));
    c.topic_updated(ta, TopicFlags::from_raw(1));
    c.entry_updated(EntryKey::History(a), EntryFlags::from_raw(1));
    c.message_updated(ma, MessageFlags::from_raw(1));
    c.history_updated(a, HistoryFlags::from_raw(1));
    c.peer_updated(a, PeerFlags::from_raw(1));
    c.send_notifications();
    c.send_notifications();
    queue.process_all();
    let actual = log.borrow().join("\n") + "\n";
    assert_eq!(
        actual,
        include_str!("../../tests/fixtures/data-changes/managers.csv")
    );
}
#[test]
fn native_peer_shift_all_58_bits() {
    let scope = SessionScope::new();
    let a = scope.peer_key(peer_from_user(1u64));
    let manager = Manager::<PeerUpdate>::new();
    let life = Lifetime::new();
    let log = Log::default();
    for i in 0..58 {
        let log = log.clone();
        manager.realtime_updates(1 << i).on_next(
            move |u| log.borrow_mut().push(format!("r,{i},{}", u.flags.value())),
            &life,
        );
    }
    let copy = log.clone();
    manager.updates(u64::MAX).on_next(
        move |u| copy.borrow_mut().push(format!("p,{}", u.flags.value())),
        &life,
    );
    for bit in 0..58 {
        log.borrow_mut().push(format!("bit,{bit}"));
        manager.updated(a, 1 << bit, false);
        manager.send_notifications();
    }
    for bits in [
        1u64 | (1 << 32) | (1 << 57),
        (1u64 << 25) | (1 << 57),
        u64::MAX,
    ] {
        log.borrow_mut().push(format!("mixed,{bits}"));
        manager.updated(a, bits, false);
        manager.send_notifications();
    }
    assert_eq!(
        log.borrow().join("\n") + "\n",
        include_str!("../../tests/fixtures/data-changes/peer-shifts.csv")
    );
}
#[test]
fn mixed_high_flags_realtime_gets_full_input_and_postponed_union() {
    let s = SessionScope::new();
    let a = s.peer_key(peer_from_user(1u64));
    let c = Changes::with_queue(&s, MainQueue::new());
    let life = Lifetime::new();
    let rt = Rc::new(RefCell::new(Vec::new()));
    for flag in [
        PeerFlag::Name,
        PeerFlag::EmojiStatus,
        PeerFlag::MainProfileTab,
    ] {
        let rt = rt.clone();
        c.realtime_peer_updates(flag).on_next(
            move |u| rt.borrow_mut().push((flag as u64, u.flags.value())),
            &life,
        );
    }
    let p = Rc::new(RefCell::new(Vec::new()));
    let copy = p.clone();
    c.peer_updates(PeerFlags::from_raw(u64::MAX))
        .on_next(move |u| copy.borrow_mut().push(u.flags.value()), &life);
    let bits = 1u64 | 1 << 32 | 1 << 57;
    c.peer_updated(a, PeerFlags::from_raw(bits));
    assert_eq!(*rt.borrow(), vec![(1, bits), (1 << 32, bits)]);
    c.peer_updated(a, PeerFlag::MainProfileTab);
    c.send_notifications();
    assert_eq!(*p.borrow(), vec![bits]);
}
#[test]
fn scope_keys_message_reuse_and_typed_views() {
    let s = SessionScope::new();
    let other = SessionScope::new();
    assert_ne!(s.id(), other.id());
    assert!(s.id().get() > 0);
    let p = s.peer_key(peer_from_user(42u64));
    assert_ne!(p, other.peer_key(p.id()));
    let user = UserKey::try_from(p).unwrap();
    assert_eq!(user.peer(), p);
    assert_eq!(user.scope(), s.id());
    assert!(ChatKey::try_from(p).is_err());
    assert!(ChannelKey::try_from(p).is_err());
    let chat = s.peer_key(peer_from_chat(42u64));
    let channel = s.peer_key(peer_from_channel(42u64));
    assert_eq!(ChatKey::try_from(chat).unwrap().peer(), chat);
    assert_eq!(ChannelKey::try_from(channel).unwrap().peer(), channel);
    for key in [
        s.document_key(7).scope(),
        s.photo_key(7).scope(),
        s.folder_key(-1).scope(),
    ] {
        assert_eq!(key, s.id());
    }
    assert_eq!(s.document_key(7).id(), 7);
    assert_eq!(s.photo_key(7).id(), 7);
    assert_eq!(s.folder_key(-1).id(), -1);
    let sub = SublistKey::new(p, chat);
    assert_eq!(sub.history(), p);
    assert_eq!(sub.peer(), chat);
    let topic = TopicKey {
        history: p,
        root_id: MsgId::new(-1),
    };
    for e in [
        EntryKey::History(p),
        EntryKey::Folder(s.folder_key(1)),
        EntryKey::ForumTopic(topic),
        EntryKey::SavedSublist(sub),
    ] {
        assert_eq!(e.scope(), s.id());
    }
    let story = s.story_key(FullStoryId {
        peer: p.id(),
        story: 1,
    });
    assert_eq!(story.id().story, 1);
    assert_eq!(story.scope(), s.id());
    let id = FullMsgId::new(p.id(), MsgId::new(12));
    let mut keys = MessageKeys::new(&s);
    let a = keys.insert(id);
    assert_eq!(a.id(), id);
    assert_eq!(keys.insert(id), a);
    assert!(keys.contains(a));
    assert!(keys.remove(a));
    assert!(!keys.remove(a));
    let b = keys.insert(id);
    assert_ne!(a, b);
    assert!(b.generation() > a.generation());
    assert!(!keys.remove(a));
    assert!(keys.contains(b));
    let foreign = MessageKeys::new(&other).insert(id);
    assert!(!keys.remove(foreign));
    drop(s);
    assert_eq!(keys.get(id), None);
    assert!(!keys.contains(b));
}
#[test]
#[should_panic(expected = "foreign SessionScope key")]
fn changes_reject_foreign_scope() {
    let a = SessionScope::new();
    let b = SessionScope::new();
    Changes::with_queue(&a, MainQueue::new())
        .peer_updated(b.peer_key(peer_from_user(1u64)), PeerFlag::Name);
}
#[test]
#[should_panic(expected = "MsgId(0)")]
fn zero_message_not_registered() {
    let s = SessionScope::new();
    MessageKeys::new(&s).insert(FullMsgId::default());
}
#[test]
#[should_panic(expected = "assertion")]
fn sublist_cannot_join_accounts() {
    let a = SessionScope::new();
    let b = SessionScope::new();
    SublistKey::new(
        a.peer_key(peer_from_user(1u64)),
        b.peer_key(peer_from_user(1u64)),
    );
}
#[test]
fn count_bit_boundaries() {
    assert_eq!(count_bit(1), 0);
    assert_eq!(count_bit(1 << 63), 63);
    assert_eq!(count_bit(PeerFlag::LAST_USED_BIT as u64), 57);
}
#[test]
#[should_panic(expected = "exactly one bit")]
fn count_bit_zero() {
    count_bit(0);
}
#[test]
#[should_panic(expected = "exactly one bit")]
fn count_bit_multiple() {
    count_bit(3);
}

#[test]
fn peer_public_api_filters_initial_realtime_and_flush() {
    let scope = SessionScope::new();
    let pa = scope.peer_key(peer_from_user(1u64));
    let pb = scope.peer_key(peer_from_user(2u64));
    let (a, b) = (pa, pb);
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let (first, second) = (1, 2);
    let log = Log::default();
    observe(
        c.peer_updates(PeerFlags::from_raw(first | second)),
        "p",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.peer_updates_for(a, PeerFlags::from_raw(first)),
        "f",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.peer_flags_value(a, PeerFlags::from_raw(first)),
        "v",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.realtime_peer_updates(PeerFlag::Name),
        "r",
        0,
        a,
        &log,
        &life,
    );
    c.peer_updated(b, PeerFlags::from_raw(first));
    c.peer_updated(a, PeerFlags::from_raw(first));
    c.peer_updated(a, PeerFlags::from_raw(first));
    c.peer_updated(a, PeerFlags::from_raw(second));
    assert_eq!(
        *log.borrow(),
        vec![
            format!("v,0,1,{first}"),
            format!("r,0,2,{first}"),
            format!("r,0,1,{first}"),
            format!("r,0,1,{first}")
        ]
    );
    assert_eq!(queue.process(), 1);
    assert_eq!(
        &log.borrow()[4..],
        &[
            format!("p,0,1,{}", first | second),
            format!("f,0,1,{}", first | second),
            format!("v,0,1,{}", first | second),
            format!("p,0,2,{first}")
        ]
    );
    assert_eq!(queue.process_all(), 0);
}

#[test]
fn history_public_api_filters_initial_realtime_and_flush() {
    let scope = SessionScope::new();
    let pa = scope.peer_key(peer_from_user(1u64));
    let pb = scope.peer_key(peer_from_user(2u64));
    let (a, b) = (pa, pb);
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let (first, second) = (1, 2);
    let log = Log::default();
    observe(
        c.history_updates(HistoryFlags::from_raw(first | second)),
        "p",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.history_updates_for(a, HistoryFlags::from_raw(first)),
        "f",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.history_flags_value(a, HistoryFlags::from_raw(first)),
        "v",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.realtime_history_updates(HistoryFlag::IsPinned),
        "r",
        0,
        a,
        &log,
        &life,
    );
    c.history_updated(b, HistoryFlags::from_raw(first));
    c.history_updated(a, HistoryFlags::from_raw(first));
    c.history_updated(a, HistoryFlags::from_raw(first));
    c.history_updated(a, HistoryFlags::from_raw(second));
    assert_eq!(
        *log.borrow(),
        vec![
            format!("v,0,1,{first}"),
            format!("r,0,2,{first}"),
            format!("r,0,1,{first}"),
            format!("r,0,1,{first}")
        ]
    );
    assert_eq!(queue.process(), 1);
    assert_eq!(
        &log.borrow()[4..],
        &[
            format!("p,0,1,{}", first | second),
            format!("f,0,1,{}", first | second),
            format!("v,0,1,{}", first | second),
            format!("p,0,2,{first}")
        ]
    );
    assert_eq!(queue.process_all(), 0);
}

#[test]
fn topic_public_api_filters_initial_realtime_and_flush() {
    let scope = SessionScope::new();
    let pa = scope.peer_key(peer_from_user(1u64));
    let pb = scope.peer_key(peer_from_user(2u64));
    let (a, b) = (
        TopicKey {
            history: pa,
            root_id: MsgId::new(1),
        },
        TopicKey {
            history: pb,
            root_id: MsgId::new(1),
        },
    );
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let (first, second) = (2, 4);
    let log = Log::default();
    observe(
        c.topic_updates(TopicFlags::from_raw(first | second)),
        "p",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.topic_updates_for(a, TopicFlags::from_raw(first)),
        "f",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.topic_flags_value(a, TopicFlags::from_raw(first)),
        "v",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.realtime_topic_updates(TopicFlag::UnreadView),
        "r",
        0,
        a,
        &log,
        &life,
    );
    c.topic_updated(b, TopicFlags::from_raw(first));
    c.topic_updated(a, TopicFlags::from_raw(first));
    c.topic_updated(a, TopicFlags::from_raw(first));
    c.topic_updated(a, TopicFlags::from_raw(second));
    assert_eq!(
        *log.borrow(),
        vec![
            format!("v,0,1,{first}"),
            format!("r,0,2,{first}"),
            format!("r,0,1,{first}"),
            format!("r,0,1,{first}")
        ]
    );
    assert_eq!(queue.process(), 1);
    assert_eq!(
        &log.borrow()[4..],
        &[
            format!("p,0,1,{}", first | second),
            format!("f,0,1,{}", first | second),
            format!("v,0,1,{}", first | second),
            format!("p,0,2,{first}")
        ]
    );
    assert_eq!(queue.process_all(), 0);

    let destroyed = Log::default();
    observe(
        c.topic_updates(TopicFlag::Destroyed),
        "d",
        0,
        a,
        &destroyed,
        &life,
    );
    c.topic_updated(a, TopicFlags::from_raw(first));
    c.topic_updated(a, TopicFlag::Destroyed);
    assert_eq!(*destroyed.borrow(), vec![format!("d,0,1,{}", first | 2048)]);
    assert_eq!(queue.process_all(), 1);
    c.topic_updated(b, TopicFlag::Destroyed);
    assert_eq!(queue.process_all(), 0);

    let before = log.borrow().len();
    c.topic_updated(a, TopicFlags::from_raw(first));
    c.topic_removed(a);
    queue.process_all();
    assert_eq!(log.borrow().len(), before + 1); // realtime only
}

#[test]
fn sublist_public_api_filters_initial_realtime_and_flush() {
    let scope = SessionScope::new();
    let pa = scope.peer_key(peer_from_user(1u64));
    let pb = scope.peer_key(peer_from_user(2u64));
    let (a, b) = (SublistKey::new(pa, pa), SublistKey::new(pa, pb));
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let (first, second) = (2, 4);
    let log = Log::default();
    observe(
        c.sublist_updates(SublistFlags::from_raw(first | second)),
        "p",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.sublist_updates_for(a, SublistFlags::from_raw(first)),
        "f",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.sublist_flags_value(a, SublistFlags::from_raw(first)),
        "v",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.realtime_sublist_updates(SublistFlag::UnreadView),
        "r",
        0,
        a,
        &log,
        &life,
    );
    c.sublist_updated(b, SublistFlags::from_raw(first));
    c.sublist_updated(a, SublistFlags::from_raw(first));
    c.sublist_updated(a, SublistFlags::from_raw(first));
    c.sublist_updated(a, SublistFlags::from_raw(second));
    assert_eq!(
        *log.borrow(),
        vec![
            format!("v,0,1,{first}"),
            format!("r,0,2,{first}"),
            format!("r,0,1,{first}"),
            format!("r,0,1,{first}")
        ]
    );
    assert_eq!(queue.process(), 1);
    assert_eq!(
        &log.borrow()[4..],
        &[
            format!("p,0,1,{}", first | second),
            format!("f,0,1,{}", first | second),
            format!("v,0,1,{}", first | second),
            format!("p,0,2,{first}")
        ]
    );
    assert_eq!(queue.process_all(), 0);

    let destroyed = Log::default();
    observe(
        c.sublist_updates(SublistFlag::Destroyed),
        "d",
        0,
        a,
        &destroyed,
        &life,
    );
    c.sublist_updated(a, SublistFlags::from_raw(first));
    c.sublist_updated(a, SublistFlag::Destroyed);
    assert_eq!(*destroyed.borrow(), vec![format!("d,0,1,{}", first | 16)]);
    assert_eq!(queue.process_all(), 1);
    c.sublist_updated(b, SublistFlag::Destroyed);
    assert_eq!(queue.process_all(), 0);

    let before = log.borrow().len();
    c.sublist_updated(a, SublistFlags::from_raw(first));
    c.sublist_removed(a);
    queue.process_all();
    assert_eq!(log.borrow().len(), before + 1); // realtime only
}

#[test]
fn message_public_api_filters_initial_realtime_and_flush() {
    let scope = SessionScope::new();
    let pa = scope.peer_key(peer_from_user(1u64));
    let pb = scope.peer_key(peer_from_user(2u64));
    let mut keys = MessageKeys::new(&scope);
    let a = keys.insert(FullMsgId::new(pa.id(), MsgId::new(1)));
    let b = keys.insert(FullMsgId::new(pb.id(), MsgId::new(1)));
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let (first, second) = (1, 4);
    let log = Log::default();
    observe(
        c.message_updates(MessageFlags::from_raw(first | second)),
        "p",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.message_updates_for(a, MessageFlags::from_raw(first)),
        "f",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.message_flags_value(a, MessageFlags::from_raw(first)),
        "v",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.realtime_message_updates(MessageFlag::Edited),
        "r",
        0,
        a,
        &log,
        &life,
    );
    c.message_updated(b, MessageFlags::from_raw(first));
    c.message_updated(a, MessageFlags::from_raw(first));
    c.message_updated(a, MessageFlags::from_raw(first));
    c.message_updated(a, MessageFlags::from_raw(second));
    assert_eq!(
        *log.borrow(),
        vec![
            format!("v,0,1,{first}"),
            format!("r,0,2,{first}"),
            format!("r,0,1,{first}"),
            format!("r,0,1,{first}")
        ]
    );
    assert_eq!(queue.process(), 1);
    assert_eq!(
        &log.borrow()[4..],
        &[
            format!("p,0,1,{}", first | second),
            format!("f,0,1,{}", first | second),
            format!("v,0,1,{}", first | second),
            format!("p,0,2,{first}")
        ]
    );
    assert_eq!(queue.process_all(), 0);

    let destroyed = Log::default();
    observe(
        c.message_updates(MessageFlag::Destroyed),
        "d",
        0,
        a,
        &destroyed,
        &life,
    );
    c.message_updated(a, MessageFlags::from_raw(first));
    c.message_updated(a, MessageFlag::Destroyed);
    assert_eq!(*destroyed.borrow(), vec![format!("d,0,1,{}", first | 2)]);
    assert_eq!(queue.process_all(), 1);
    c.message_updated(b, MessageFlag::Destroyed);
    assert_eq!(queue.process_all(), 0);
}

#[test]
fn entry_public_api_filters_initial_realtime_and_flush() {
    let scope = SessionScope::new();
    let pa = scope.peer_key(peer_from_user(1u64));
    let pb = scope.peer_key(peer_from_user(2u64));
    let (a, b) = (EntryKey::History(pa), EntryKey::History(pb));
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let (first, second) = (1, 2);
    let log = Log::default();
    observe(
        c.entry_updates(EntryFlags::from_raw(first | second)),
        "p",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.entry_updates_for(a, EntryFlags::from_raw(first)),
        "f",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.entry_flags_value(a, EntryFlags::from_raw(first)),
        "v",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.realtime_entry_updates(EntryFlag::Repaint),
        "r",
        0,
        a,
        &log,
        &life,
    );
    c.entry_updated(b, EntryFlags::from_raw(first));
    c.entry_updated(a, EntryFlags::from_raw(first));
    c.entry_updated(a, EntryFlags::from_raw(first));
    c.entry_updated(a, EntryFlags::from_raw(second));
    assert_eq!(
        *log.borrow(),
        vec![
            format!("v,0,1,{first}"),
            format!("r,0,2,{first}"),
            format!("r,0,1,{first}"),
            format!("r,0,1,{first}")
        ]
    );
    assert_eq!(queue.process(), 1);
    assert_eq!(
        &log.borrow()[4..],
        &[
            format!("p,0,1,{}", first | second),
            format!("f,0,1,{}", first | second),
            format!("v,0,1,{}", first | second),
            format!("p,0,2,{first}")
        ]
    );
    assert_eq!(queue.process_all(), 0);

    let destroyed = Log::default();
    observe(
        c.entry_updates(EntryFlag::Destroyed),
        "d",
        0,
        a,
        &destroyed,
        &life,
    );
    c.entry_updated(a, EntryFlags::from_raw(first));
    c.entry_updated(a, EntryFlag::Destroyed);
    assert_eq!(*destroyed.borrow(), vec![format!("d,0,1,{}", first | 32)]);
    assert_eq!(queue.process_all(), 1);
    c.entry_updated(b, EntryFlag::Destroyed);
    assert_eq!(queue.process_all(), 0);

    let before = log.borrow().len();
    c.entry_updated(a, EntryFlags::from_raw(first));
    c.entry_removed(a);
    queue.process_all();
    assert_eq!(log.borrow().len(), before + 1); // realtime only
}

#[test]
fn story_public_api_filters_initial_realtime_and_flush() {
    let scope = SessionScope::new();
    let pa = scope.peer_key(peer_from_user(1u64));
    let pb = scope.peer_key(peer_from_user(2u64));
    let (a, b) = (
        scope.story_key(FullStoryId {
            peer: pa.id(),
            story: 1,
        }),
        scope.story_key(FullStoryId {
            peer: pb.id(),
            story: 1,
        }),
    );
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let (first, second) = (1, 4);
    let log = Log::default();
    observe(
        c.story_updates(StoryFlags::from_raw(first | second)),
        "p",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.story_updates_for(a, StoryFlags::from_raw(first)),
        "f",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.story_flags_value(a, StoryFlags::from_raw(first)),
        "v",
        0,
        a,
        &log,
        &life,
    );
    observe(
        c.realtime_story_updates(StoryFlag::Edited),
        "r",
        0,
        a,
        &log,
        &life,
    );
    c.story_updated(b, StoryFlags::from_raw(first));
    c.story_updated(a, StoryFlags::from_raw(first));
    c.story_updated(a, StoryFlags::from_raw(first));
    c.story_updated(a, StoryFlags::from_raw(second));
    assert_eq!(
        *log.borrow(),
        vec![
            format!("v,0,1,{first}"),
            format!("r,0,2,{first}"),
            format!("r,0,1,{first}"),
            format!("r,0,1,{first}")
        ]
    );
    assert_eq!(queue.process(), 1);
    assert_eq!(
        &log.borrow()[4..],
        &[
            format!("p,0,1,{}", first | second),
            format!("f,0,1,{}", first | second),
            format!("v,0,1,{}", first | second),
            format!("p,0,2,{first}")
        ]
    );
    assert_eq!(queue.process_all(), 0);

    let destroyed = Log::default();
    observe(
        c.story_updates(StoryFlag::Destroyed),
        "d",
        0,
        a,
        &destroyed,
        &life,
    );
    c.story_updated(a, StoryFlags::from_raw(first));
    c.story_updated(a, StoryFlag::Destroyed);
    assert_eq!(*destroyed.borrow(), vec![format!("d,0,1,{}", first | 2)]);
    assert_eq!(queue.process_all(), 1);
    c.story_updated(b, StoryFlag::Destroyed);
    assert_eq!(queue.process_all(), 0);
}
#[test]
fn name_admin_rank_are_immediate_utf16_and_scoped() {
    use super::super::data_chat_participant_status::ChatAdminRight;
    let scope = SessionScope::default();
    let a = scope.peer_key(peer_from_user(1u64));
    let b = scope.peer_key(peer_from_user(2u64));
    let u = UserKey::try_from(b).unwrap();
    let c = Changes::new(&scope);
    assert_eq!(c.session(), scope.id());
    let life = Lifetime::new();
    let names = Rc::new(RefCell::new(Vec::new()));
    let filtered = Rc::new(RefCell::new(Vec::new()));
    let copy = names.clone();
    c.realtime_name_updates()
        .on_next(move |v| copy.borrow_mut().push(v), &life);
    let copy = filtered.clone();
    c.realtime_name_updates_for(a)
        .on_next(move |v| copy.borrow_mut().push(v), &life);
    let letters = BTreeSet::from([0xd800u16, 0x0041, 0x0000]);
    c.name_updated(b, letters.clone());
    c.name_updated(a, letters.clone());
    assert_eq!(names.borrow().len(), 2);
    assert_eq!(*filtered.borrow(), vec![NameUpdate::new(a, letters)]);
    let admins = Rc::new(RefCell::new(Vec::new()));
    let ranks = Rc::new(RefCell::new(Vec::new()));
    let copy = admins.clone();
    c.chat_admin_changes()
        .on_next(move |v| copy.borrow_mut().push(v), &life);
    let copy = ranks.clone();
    c.chat_member_rank_changes()
        .on_next(move |v| copy.borrow_mut().push(v), &life);
    let rights = ChatAdminRight::Anonymous | ChatAdminRight::ManageWelcomeMessages;
    let rank = vec![0xd800, 0, 0xdfff];
    c.chat_admin_changed(a, u, rights, rank.clone());
    c.chat_member_rank_changed(a, u, rank.clone());
    assert_eq!(
        *admins.borrow(),
        vec![ChatAdminChange {
            peer: a,
            user: u,
            rights,
            rank: rank.clone()
        }]
    );
    assert_eq!(
        *ranks.borrow(),
        vec![ChatMemberRankChange {
            peer: a,
            user: u,
            rank
        }]
    );
    assert!(!c.inner.notify.get());
}
#[test]
fn pending_borrows_are_released_cross_category_and_recursive_flush() {
    let scope = SessionScope::new();
    let a = scope.peer_key(peer_from_user(1u64));
    let queue = MainQueue::new();
    let c = Changes::with_queue(&scope, queue.clone());
    let life = Lifetime::new();
    let log = Log::default();
    let weak = Rc::downgrade(&c.inner);
    let copy = log.clone();
    let once = Cell::new(false);
    c.peer_updates(PeerFlags::from_raw(3)).on_next(
        move |v| {
            copy.borrow_mut().push(format!("peer,{}", v.flags.value()));
            if !once.replace(true) {
                let c = Changes {
                    inner: weak.upgrade().unwrap(),
                };
                c.history_updated(a, HistoryFlag::IsPinned);
                c.peer_updated(a, PeerFlag::Username);
                c.send_notifications();
            }
        },
        &life,
    );
    let copy = log.clone();
    c.history_updates(HistoryFlag::IsPinned)
        .on_next(move |_| copy.borrow_mut().push("history".into()), &life);
    c.peer_updated(a, PeerFlag::Name);
    c.send_notifications();
    assert_eq!(*log.borrow(), vec!["peer,1", "peer,2", "history"]);
    assert_eq!(queue.process_all(), 2); // stale jobs both see notify=false
}
#[test]
fn cross_category_updates_join_current_flush_but_earlier_category_waits() {
    let s = SessionScope::new();
    let a = s.peer_key(peer_from_user(1u64));
    let q = MainQueue::new();
    let c = Changes::with_queue(&s, q.clone());
    let l = Lifetime::new();
    let log = Log::default();
    let weak = Rc::downgrade(&c.inner);
    let copy = log.clone();
    c.peer_updates(PeerFlag::Name).on_next(
        move |_| {
            copy.borrow_mut().push("peer".into());
            let c = Changes {
                inner: weak.upgrade().unwrap(),
            };
            c.history_updated(a, HistoryFlag::IsPinned);
            c.peer_updated(a, PeerFlag::Username);
        },
        &l,
    );
    let copy = log.clone();
    c.peer_updates(PeerFlag::Username)
        .on_next(move |_| copy.borrow_mut().push("next-peer".into()), &l);
    let copy = log.clone();
    c.history_updates(HistoryFlag::IsPinned)
        .on_next(move |_| copy.borrow_mut().push("history".into()), &l);
    c.peer_updated(a, PeerFlag::Name);
    assert_eq!(q.process(), 1);
    assert_eq!(*log.borrow(), vec!["peer", "history"]);
    assert_eq!(q.process(), 1);
    assert_eq!(*log.borrow(), vec!["peer", "history", "next-peer"]);
}
#[test]
fn manual_flush_does_not_cancel_old_queued_job() {
    let s = SessionScope::new();
    let a = s.peer_key(peer_from_user(1u64));
    let q = MainQueue::new();
    let c = Changes::with_queue(&s, q.clone());
    let l = Lifetime::new();
    let log = Log::default();
    let copy = log.clone();
    c.peer_updates(PeerFlags::from_raw(3)).on_next(
        move |v| copy.borrow_mut().push(v.flags.value().to_string()),
        &l,
    );
    c.peer_updated(a, PeerFlag::Name);
    c.send_notifications();
    let copy = log.clone();
    q.post_local(move || copy.borrow_mut().push("marker".into()));
    c.peer_updated(a, PeerFlag::Username);
    q.process_all();
    assert_eq!(*log.borrow(), vec!["1", "2", "marker"]);
}
#[test]
fn destroyed_message_is_still_live_during_notifications() {
    let s = SessionScope::new();
    let id = FullMsgId::new(peer_from_user(1u64), MsgId::new(1));
    let keys = Rc::new(RefCell::new(MessageKeys::new(&s)));
    let key = keys.borrow_mut().insert(id);
    let c = Changes::with_queue(&s, MainQueue::new());
    let l = Lifetime::new();
    let copy = keys.clone();
    c.message_updates(MessageFlag::Destroyed)
        .on_next(move |u| assert!(copy.borrow().contains(u.item)), &l);
    c.message_updated(key, MessageFlag::Destroyed);
    assert!(keys.borrow_mut().remove(key));
    let next = keys.borrow_mut().insert(id);
    assert_ne!(next, key);
}
#[test]
fn queued_flush_cancels_with_scope_or_service() {
    let s = SessionScope::new();
    let a = s.peer_key(peer_from_user(1u64));
    let q = MainQueue::new();
    let c = Changes::with_queue(&s, q.clone());
    let l = Lifetime::new();
    let count = Rc::new(Cell::new(0));
    let copy = count.clone();
    c.peer_updates(PeerFlag::Name)
        .on_next(move |_| copy.set(copy.get() + 1), &l);
    c.peer_updated(a, PeerFlag::Name);
    drop(s);
    q.process_all();
    assert_eq!(count.get(), 0);
    let s = SessionScope::new();
    let a = s.peer_key(a.id());
    let c = Changes::with_queue(&s, q.clone());
    let weak = Rc::downgrade(&c.inner);
    c.peer_updated(a, PeerFlag::Name);
    drop(c);
    assert!(weak.upgrade().is_none());
    q.process_all();
}
#[test]
fn flags_value_zero_emits_once_and_unsubscribes() {
    let s = SessionScope::new();
    let a = s.peer_key(peer_from_user(1u64));
    let c = Changes::with_queue(&s, MainQueue::new());
    let log = Log::default();
    let l = Lifetime::new();
    observe(
        c.peer_flags_value(a, PeerFlags::empty()),
        "z",
        0,
        a,
        &log,
        &l,
    );
    assert_eq!(*log.borrow(), vec!["z,0,1,0"]);
    l.destroy();
    c.peer_updated(a, PeerFlag::Name);
    c.send_notifications();
    assert_eq!(log.borrow().len(), 1);
}

#[test]
fn native_all_flag_values_and_last_bits() {
    let mut rows = Vec::<String>::new();
    rows.push(format!("Peer,None,{}", PeerFlag::None as u64));
    rows.push(format!("Peer,Name,{}", PeerFlag::Name as u64));
    rows.push(format!("Peer,Username,{}", PeerFlag::Username as u64));
    rows.push(format!("Peer,Photo,{}", PeerFlag::Photo as u64));
    rows.push(format!("Peer,About,{}", PeerFlag::About as u64));
    rows.push(format!(
        "Peer,Notifications,{}",
        PeerFlag::Notifications as u64
    ));
    rows.push(format!("Peer,Migration,{}", PeerFlag::Migration as u64));
    rows.push(format!(
        "Peer,UnavailableReason,{}",
        PeerFlag::UnavailableReason as u64
    ));
    rows.push(format!(
        "Peer,ChatThemeToken,{}",
        PeerFlag::ChatThemeToken as u64
    ));
    rows.push(format!(
        "Peer,ChatWallPaper,{}",
        PeerFlag::ChatWallPaper as u64
    ));
    rows.push(format!("Peer,IsBlocked,{}", PeerFlag::IsBlocked as u64));
    rows.push(format!("Peer,MessagesTTL,{}", PeerFlag::MessagesTTL as u64));
    rows.push(format!("Peer,FullInfo,{}", PeerFlag::FullInfo as u64));
    rows.push(format!("Peer,Usernames,{}", PeerFlag::Usernames as u64));
    rows.push(format!(
        "Peer,TranslationDisabled,{}",
        PeerFlag::TranslationDisabled as u64
    ));
    rows.push(format!("Peer,Color,{}", PeerFlag::Color as u64));
    rows.push(format!(
        "Peer,ColorProfile,{}",
        PeerFlag::ColorProfile as u64
    ));
    rows.push(format!(
        "Peer,BackgroundEmoji,{}",
        PeerFlag::BackgroundEmoji as u64
    ));
    rows.push(format!(
        "Peer,StoriesState,{}",
        PeerFlag::StoriesState as u64
    ));
    rows.push(format!("Peer,VerifyInfo,{}", PeerFlag::VerifyInfo as u64));
    rows.push(format!(
        "Peer,StarsPerMessage,{}",
        PeerFlag::StarsPerMessage as u64
    ));
    rows.push(format!(
        "Peer,CanShareContact,{}",
        PeerFlag::CanShareContact as u64
    ));
    rows.push(format!("Peer,IsContact,{}", PeerFlag::IsContact as u64));
    rows.push(format!("Peer,PhoneNumber,{}", PeerFlag::PhoneNumber as u64));
    rows.push(format!(
        "Peer,OnlineStatus,{}",
        PeerFlag::OnlineStatus as u64
    ));
    rows.push(format!("Peer,BotCommands,{}", PeerFlag::BotCommands as u64));
    rows.push(format!(
        "Peer,BotCanBeInvited,{}",
        PeerFlag::BotCanBeInvited as u64
    ));
    rows.push(format!(
        "Peer,BotStartToken,{}",
        PeerFlag::BotStartToken as u64
    ));
    rows.push(format!("Peer,CommonChats,{}", PeerFlag::CommonChats as u64));
    rows.push(format!("Peer,PeerGifts,{}", PeerFlag::PeerGifts as u64));
    rows.push(format!("Peer,HasCalls,{}", PeerFlag::HasCalls as u64));
    rows.push(format!("Peer,SupportInfo,{}", PeerFlag::SupportInfo as u64));
    rows.push(format!("Peer,IsBot,{}", PeerFlag::IsBot as u64));
    rows.push(format!("Peer,EmojiStatus,{}", PeerFlag::EmojiStatus as u64));
    rows.push(format!(
        "Peer,BusinessDetails,{}",
        PeerFlag::BusinessDetails as u64
    ));
    rows.push(format!("Peer,Birthday,{}", PeerFlag::Birthday as u64));
    rows.push(format!(
        "Peer,PersonalChannel,{}",
        PeerFlag::PersonalChannel as u64
    ));
    rows.push(format!(
        "Peer,StarRefProgram,{}",
        PeerFlag::StarRefProgram as u64
    ));
    rows.push(format!(
        "Peer,PaysPerMessage,{}",
        PeerFlag::PaysPerMessage as u64
    ));
    rows.push(format!(
        "Peer,GiftSettings,{}",
        PeerFlag::GiftSettings as u64
    ));
    rows.push(format!("Peer,StarsRating,{}", PeerFlag::StarsRating as u64));
    rows.push(format!("Peer,ContactNote,{}", PeerFlag::ContactNote as u64));
    rows.push(format!("Peer,InviteLinks,{}", PeerFlag::InviteLinks as u64));
    rows.push(format!("Peer,Members,{}", PeerFlag::Members as u64));
    rows.push(format!("Peer,Admins,{}", PeerFlag::Admins as u64));
    rows.push(format!("Peer,BannedUsers,{}", PeerFlag::BannedUsers as u64));
    rows.push(format!("Peer,Rights,{}", PeerFlag::Rights as u64));
    rows.push(format!(
        "Peer,PendingRequests,{}",
        PeerFlag::PendingRequests as u64
    ));
    rows.push(format!("Peer,Reactions,{}", PeerFlag::Reactions as u64));
    rows.push(format!("Peer,ChannelAmIn,{}", PeerFlag::ChannelAmIn as u64));
    rows.push(format!("Peer,StickersSet,{}", PeerFlag::StickersSet as u64));
    rows.push(format!("Peer,EmojiSet,{}", PeerFlag::EmojiSet as u64));
    rows.push(format!(
        "Peer,DiscussionLink,{}",
        PeerFlag::DiscussionLink as u64
    ));
    rows.push(format!(
        "Peer,MonoforumLink,{}",
        PeerFlag::MonoforumLink as u64
    ));
    rows.push(format!(
        "Peer,ChannelLocation,{}",
        PeerFlag::ChannelLocation as u64
    ));
    rows.push(format!("Peer,Slowmode,{}", PeerFlag::Slowmode as u64));
    rows.push(format!("Peer,GroupCall,{}", PeerFlag::GroupCall as u64));
    rows.push(format!("Peer,ManagedBot,{}", PeerFlag::ManagedBot as u64));
    rows.push(format!(
        "Peer,MainProfileTab,{}",
        PeerFlag::MainProfileTab as u64
    ));
    rows.push(format!(
        "Peer,LastUsedBit,{}",
        PeerFlag::LAST_USED_BIT as u64
    ));
    rows.push(format!("History,None,{}", HistoryFlag::None as u64));
    rows.push(format!("History,IsPinned,{}", HistoryFlag::IsPinned as u64));
    rows.push(format!(
        "History,UnreadView,{}",
        HistoryFlag::UnreadView as u64
    ));
    rows.push(format!(
        "History,TopPromoted,{}",
        HistoryFlag::TopPromoted as u64
    ));
    rows.push(format!("History,Folder,{}", HistoryFlag::Folder as u64));
    rows.push(format!(
        "History,UnreadMentions,{}",
        HistoryFlag::UnreadMentions as u64
    ));
    rows.push(format!(
        "History,UnreadReactions,{}",
        HistoryFlag::UnreadReactions as u64
    ));
    rows.push(format!(
        "History,ClientSideMessages,{}",
        HistoryFlag::ClientSideMessages as u64
    ));
    rows.push(format!(
        "History,ChatOccupied,{}",
        HistoryFlag::ChatOccupied as u64
    ));
    rows.push(format!(
        "History,MessageSent,{}",
        HistoryFlag::MessageSent as u64
    ));
    rows.push(format!(
        "History,ScheduledSent,{}",
        HistoryFlag::ScheduledSent as u64
    ));
    rows.push(format!(
        "History,OutboxRead,{}",
        HistoryFlag::OutboxRead as u64
    ));
    rows.push(format!(
        "History,BotKeyboard,{}",
        HistoryFlag::BotKeyboard as u64
    ));
    rows.push(format!(
        "History,CloudDraft,{}",
        HistoryFlag::CloudDraft as u64
    ));
    rows.push(format!(
        "History,TranslateFrom,{}",
        HistoryFlag::TranslateFrom as u64
    ));
    rows.push(format!(
        "History,TranslatedTo,{}",
        HistoryFlag::TranslatedTo as u64
    ));
    rows.push(format!(
        "History,UnreadPollVotes,{}",
        HistoryFlag::UnreadPollVotes as u64
    ));
    rows.push(format!(
        "History,StreamedDrafts,{}",
        HistoryFlag::StreamedDrafts as u64
    ));
    rows.push(format!(
        "History,LastUsedBit,{}",
        HistoryFlag::LAST_USED_BIT as u64
    ));
    rows.push(format!("Topic,None,{}", TopicFlag::None as u64));
    rows.push(format!("Topic,UnreadView,{}", TopicFlag::UnreadView as u64));
    rows.push(format!(
        "Topic,UnreadMentions,{}",
        TopicFlag::UnreadMentions as u64
    ));
    rows.push(format!(
        "Topic,UnreadReactions,{}",
        TopicFlag::UnreadReactions as u64
    ));
    rows.push(format!(
        "Topic,Notifications,{}",
        TopicFlag::Notifications as u64
    ));
    rows.push(format!("Topic,Title,{}", TopicFlag::Title as u64));
    rows.push(format!("Topic,IconId,{}", TopicFlag::IconId as u64));
    rows.push(format!("Topic,ColorId,{}", TopicFlag::ColorId as u64));
    rows.push(format!("Topic,CloudDraft,{}", TopicFlag::CloudDraft as u64));
    rows.push(format!("Topic,Closed,{}", TopicFlag::Closed as u64));
    rows.push(format!("Topic,Creator,{}", TopicFlag::Creator as u64));
    rows.push(format!("Topic,Destroyed,{}", TopicFlag::Destroyed as u64));
    rows.push(format!(
        "Topic,UnreadPollVotes,{}",
        TopicFlag::UnreadPollVotes as u64
    ));
    rows.push(format!(
        "Topic,LastUsedBit,{}",
        TopicFlag::LAST_USED_BIT as u64
    ));
    rows.push(format!("Sublist,None,{}", SublistFlag::None as u64));
    rows.push(format!(
        "Sublist,UnreadView,{}",
        SublistFlag::UnreadView as u64
    ));
    rows.push(format!(
        "Sublist,UnreadReactions,{}",
        SublistFlag::UnreadReactions as u64
    ));
    rows.push(format!(
        "Sublist,CloudDraft,{}",
        SublistFlag::CloudDraft as u64
    ));
    rows.push(format!(
        "Sublist,Destroyed,{}",
        SublistFlag::Destroyed as u64
    ));
    rows.push(format!(
        "Sublist,UnreadPollVotes,{}",
        SublistFlag::UnreadPollVotes as u64
    ));
    rows.push(format!(
        "Sublist,LastUsedBit,{}",
        SublistFlag::LAST_USED_BIT as u64
    ));
    rows.push(format!("Message,None,{}", MessageFlag::None as u64));
    rows.push(format!("Message,Edited,{}", MessageFlag::Edited as u64));
    rows.push(format!(
        "Message,Destroyed,{}",
        MessageFlag::Destroyed as u64
    ));
    rows.push(format!(
        "Message,DialogRowRepaint,{}",
        MessageFlag::DialogRowRepaint as u64
    ));
    rows.push(format!(
        "Message,DialogRowRefresh,{}",
        MessageFlag::DialogRowRefresh as u64
    ));
    rows.push(format!("Message,NewAdded,{}", MessageFlag::NewAdded as u64));
    rows.push(format!(
        "Message,ReplyMarkup,{}",
        MessageFlag::ReplyMarkup as u64
    ));
    rows.push(format!(
        "Message,BotCallbackSent,{}",
        MessageFlag::BotCallbackSent as u64
    ));
    rows.push(format!(
        "Message,NewMaybeAdded,{}",
        MessageFlag::NewMaybeAdded as u64
    ));
    rows.push(format!(
        "Message,ReplyToTopAdded,{}",
        MessageFlag::ReplyToTopAdded as u64
    ));
    rows.push(format!(
        "Message,NewUnreadReaction,{}",
        MessageFlag::NewUnreadReaction as u64
    ));
    rows.push(format!(
        "Message,LastUsedBit,{}",
        MessageFlag::LAST_USED_BIT as u64
    ));
    rows.push(format!("Entry,None,{}", EntryFlag::None as u64));
    rows.push(format!("Entry,Repaint,{}", EntryFlag::Repaint as u64));
    rows.push(format!(
        "Entry,HasPinnedMessages,{}",
        EntryFlag::HasPinnedMessages as u64
    ));
    rows.push(format!(
        "Entry,ForwardDraft,{}",
        EntryFlag::ForwardDraft as u64
    ));
    rows.push(format!(
        "Entry,LocalDraftSet,{}",
        EntryFlag::LocalDraftSet as u64
    ));
    rows.push(format!("Entry,Height,{}", EntryFlag::Height as u64));
    rows.push(format!("Entry,Destroyed,{}", EntryFlag::Destroyed as u64));
    rows.push(format!(
        "Entry,LastUsedBit,{}",
        EntryFlag::LAST_USED_BIT as u64
    ));
    rows.push(format!("Story,None,{}", StoryFlag::None as u64));
    rows.push(format!("Story,Edited,{}", StoryFlag::Edited as u64));
    rows.push(format!("Story,Destroyed,{}", StoryFlag::Destroyed as u64));
    rows.push(format!("Story,NewAdded,{}", StoryFlag::NewAdded as u64));
    rows.push(format!(
        "Story,ViewsChanged,{}",
        StoryFlag::ViewsChanged as u64
    ));
    rows.push(format!("Story,MarkRead,{}", StoryFlag::MarkRead as u64));
    rows.push(format!("Story,Reaction,{}", StoryFlag::Reaction as u64));
    rows.push(format!(
        "Story,LastUsedBit,{}",
        StoryFlag::LAST_USED_BIT as u64
    ));
    rows.push(format!(
        "ChatAdminRight,ChangeInfo,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ChangeInfo as i32
    ));
    rows.push(format!(
        "ChatAdminRight,PostMessages,{}",
        super::super::data_chat_participant_status::ChatAdminRight::PostMessages as i32
    ));
    rows.push(format!(
        "ChatAdminRight,EditMessages,{}",
        super::super::data_chat_participant_status::ChatAdminRight::EditMessages as i32
    ));
    rows.push(format!(
        "ChatAdminRight,DeleteMessages,{}",
        super::super::data_chat_participant_status::ChatAdminRight::DeleteMessages as i32
    ));
    rows.push(format!(
        "ChatAdminRight,BanUsers,{}",
        super::super::data_chat_participant_status::ChatAdminRight::BanUsers as i32
    ));
    rows.push(format!(
        "ChatAdminRight,InviteByLinkOrAdd,{}",
        super::super::data_chat_participant_status::ChatAdminRight::InviteByLinkOrAdd as i32
    ));
    rows.push(format!(
        "ChatAdminRight,PinMessages,{}",
        super::super::data_chat_participant_status::ChatAdminRight::PinMessages as i32
    ));
    rows.push(format!(
        "ChatAdminRight,AddAdmins,{}",
        super::super::data_chat_participant_status::ChatAdminRight::AddAdmins as i32
    ));
    rows.push(format!(
        "ChatAdminRight,Anonymous,{}",
        super::super::data_chat_participant_status::ChatAdminRight::Anonymous as i32
    ));
    rows.push(format!(
        "ChatAdminRight,ManageCall,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ManageCall as i32
    ));
    rows.push(format!(
        "ChatAdminRight,Other,{}",
        super::super::data_chat_participant_status::ChatAdminRight::Other as i32
    ));
    rows.push(format!(
        "ChatAdminRight,ManageTopics,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ManageTopics as i32
    ));
    rows.push(format!(
        "ChatAdminRight,PostStories,{}",
        super::super::data_chat_participant_status::ChatAdminRight::PostStories as i32
    ));
    rows.push(format!(
        "ChatAdminRight,EditStories,{}",
        super::super::data_chat_participant_status::ChatAdminRight::EditStories as i32
    ));
    rows.push(format!(
        "ChatAdminRight,DeleteStories,{}",
        super::super::data_chat_participant_status::ChatAdminRight::DeleteStories as i32
    ));
    rows.push(format!(
        "ChatAdminRight,ManageDirect,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ManageDirect as i32
    ));
    rows.push(format!(
        "ChatAdminRight,ManageRanks,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ManageRanks as i32
    ));
    rows.push(format!(
        "ChatAdminRight,ProcessJoinRequests,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ProcessJoinRequests as i32
    ));
    rows.push(format!(
        "ChatAdminRight,ManageLinkedPeers,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ManageLinkedPeers as i32
    ));
    rows.push(format!(
        "ChatAdminRight,ManageWelcomeMessages,{}",
        super::super::data_chat_participant_status::ChatAdminRight::ManageWelcomeMessages as i32
    ));
    assert_eq!(
        rows.join("\n") + "\n",
        include_str!("../../tests/fixtures/data-changes/flags.csv")
    );
}
