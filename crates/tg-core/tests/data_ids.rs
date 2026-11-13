use std::{
    collections::{BTreeSet, HashMap},
    fmt::Write,
};
use tg_base::qt_stream::{Reader as QtReader, Writer as QtWriter};
use tg_core::data::{data_msg_id::*, data_peer_id::*};
use tg_scheme::{mtp_peer_channel, mtp_peer_chat, mtp_peer_user, Peer};

fn hex(data: &[u8]) -> String {
    data.iter().map(|v| format!("{v:02x}")).collect()
}
fn unhex(data: &str) -> Vec<u8> {
    (0..data.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&data[i..i + 2], 16).unwrap())
        .collect()
}
fn wire(peer: &Peer) -> String {
    hex(&tg_tl::primes_to_bytes(&tg_tl::serialize(peer)))
}
fn boolean(value: bool) -> u8 {
    u8::from(value)
}
fn optional(value: Option<i64>) -> String {
    value.map_or_else(|| "-".into(), |v| v.to_string())
}

fn peer_case(kind: u8, bits: u64, expected: &str) -> String {
    let mut input = "-".to_string();
    let id = match kind {
        0 => PeerId::new(bits),
        1 => peer_from_user(UserId::new(bits)),
        2 => peer_from_chat(ChatId::new(bits)),
        3 => peer_from_channel(ChannelId::new(bits)),
        4 => PeerId::from(FakeChatId::new(bits)),
        5 => deserialize_peer_id(bits),
        6..=8 => {
            let native_wire = unhex(expected.split_whitespace().last().unwrap());
            let peer: Peer =
                tg_tl::deserialize_exact(&tg_tl::bytes_to_primes(&native_wire)).unwrap();
            let built = match kind {
                6 => mtp_peer_user(bits),
                7 => mtp_peer_chat(bits),
                _ => mtp_peer_channel(bits),
            };
            assert_eq!(wire(&peer), wire(&built));
            input = wire(&peer);
            peer_from_mtp(&peer)
        }
        9 => peer_from_chat(bits as u32 as i32),
        10 => peer_from_channel(bits as u32 as i32),
        _ => unreachable!(),
    };
    let (serialized, stored) = if id.value & (UserId::RESERVED_BIT << 48) == 0 {
        let value = serialize_peer_id(id);
        let mut data = QtWriter::default();
        data.u64(value);
        let mut reader = QtReader::new(&data.0);
        assert_eq!(deserialize_peer_id(reader.u64().unwrap()), id);
        assert!(reader.at_end());
        (value.to_string(), hex(&data.0))
    } else {
        ("-".into(), "-".into())
    };
    format!(
        "P {} {} {} {} {} {} {} {} {} {} {serialized} {stored} {} {input}",
        id.value,
        boolean(!id.is_zero()),
        boolean(peer_is_user(id)),
        boolean(peer_is_chat(id)),
        boolean(peer_is_channel(id)),
        peer_to_user(id).bare,
        peer_to_chat(id).bare,
        peer_to_channel(id).bare,
        id.to::<127>().bare,
        peer_to_bare_mtp_int(id),
        wire(&peer_to_mtp(id))
    )
}

fn message_case(bare: i64, peer: u64, story: StoryId, session: u64, other: i64) -> String {
    let id = MsgId::new(bare);
    let full = FullMsgId::new(PeerId::new(peer), id);
    let story_id = FullStoryId {
        peer: PeerId::new(peer),
        story,
    };
    let global = GlobalMsgId {
        item_id: full,
        session_unique_id: session,
    };
    let neg = bare.checked_neg().map(|_| (-id).bare);
    let sum = bare
        .checked_add(other)
        .map(|_| (id + MsgId::new(other)).bare);
    let difference = bare
        .checked_sub(other)
        .map(|_| (id - MsgId::new(other)).bare);
    let increment = bare.checked_add(1).map(|_| {
        let mut v = id;
        let result = v.increment();
        assert_eq!(result, v);
        result.bare
    });
    let post_increment = bare.checked_add(1).map(|after| {
        let mut v = id;
        let old = v.post_increment();
        assert_eq!(v.bare, after);
        old.bare
    });
    let decrement = bare.checked_sub(1).map(|_| {
        let mut v = id;
        let result = v.decrement();
        assert_eq!(result, v);
        result.bare
    });
    let post_decrement = bare.checked_sub(1).map(|after| {
        let mut v = id;
        let old = v.post_decrement();
        assert_eq!(v.bare, after);
        old.bare
    });
    format!(
        "M {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
        boolean(!id.is_zero()),
        boolean(!id),
        boolean(is_client_msg_id(id)),
        boolean(is_story_msg_id(id)),
        boolean(is_server_msg_id(id)),
        optional(is_client_msg_id(id).then(|| client_msg_index(id) as i64)),
        optional(is_story_msg_id(id).then(|| story_id_from_msg_id(id) as i64)),
        optional(neg),
        optional(sum),
        optional(difference),
        optional(increment),
        optional(post_increment),
        optional(decrement),
        optional(post_decrement),
        boolean(!full.is_zero()),
        boolean(!full),
        boolean(story_id.valid()),
        boolean(!global.is_zero()),
        boolean(!global),
        boolean(id < MsgId::new(other)),
        boolean(id == MsgId::new(other)),
        boolean(full < FullMsgId::new(PeerId::default(), MsgId::new(other))),
        boolean(MsgRange::new(id, MsgId::new(other)) == MsgRange::new(MsgId::new(other), id))
    )
}

fn constants() -> String {
    let values = [
        START_CLIENT_MSG_ID.bare,
        CLIENT_MSG_IDS,
        END_CLIENT_MSG_ID.bare,
        START_STORY_MSG_ID.bare,
        SERVER_MAX_STORY_ID as i64,
        STORY_MSG_IDS,
        END_STORY_MSG_ID.bare,
        SERVER_MAX_MSG_ID.bare,
        SCHEDULED_MAX_MSG_ID.bare,
        SHORTCUT_MAX_MSG_ID.bare,
        WELCOME_MAX_MSG_ID.bare,
        SHOW_AT_UNREAD_MSG_ID.bare,
        SPECIAL_MSG_ID_SHIFT,
        SHOW_AT_THE_END_MSG_ID.bare,
        SWITCH_AT_TOP_MSG_ID.bare,
        SHOW_AND_START_BOT_MSG_ID.bare,
        SHOW_AND_MAYBE_START_BOT_MSG_ID.bare,
        SHOW_FOR_CHOOSE_MESSAGES_MSG_ID.bare,
        SEARCH_QUERY_OFFSET_HINT as i64,
    ];
    let mut result = "C".to_string();
    for value in values {
        write!(result, " {value}").unwrap()
    }
    result
}

#[test]
fn original_strong_ids_serialization_tl_and_message_operations() {
    let input = include_str!("fixtures/data-ids/cases.txt")
        .lines()
        .collect::<Vec<_>>();
    let expected = include_str!("fixtures/data-ids/native.txt")
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(input.len(), expected.len());
    for (index, (line, expected)) in input.iter().zip(expected).enumerate() {
        let words = line.split_whitespace().collect::<Vec<_>>();
        let actual = match words[0] {
            "C" => constants(),
            "P" => peer_case(
                words[1].parse().unwrap(),
                words[2].parse().unwrap(),
                expected,
            ),
            "M" => message_case(
                words[1].parse().unwrap(),
                words[2].parse().unwrap(),
                words[3].parse().unwrap(),
                words[4].parse().unwrap(),
                words[5].parse().unwrap(),
            ),
            "I" => {
                let index = words[1].parse().unwrap();
                let id = story_id_to_msg_id(index);
                format!(
                    "I {} {} {}",
                    client_msg_by_index(index).bare,
                    id.bare,
                    boolean(is_story_msg_id(id))
                )
            }
            _ => unreachable!(),
        };
        assert_eq!(actual, expected, "native operation {index}: {line}");
    }
}

#[test]
fn raw_bare_bits_and_zero_kind_semantics_remain_distinct() {
    assert!(peer_is_user(PeerId::default()));
    assert!(PeerId::default().is_zero());
    assert_eq!(PeerId::from(ChatId::default()).value, 1 << 48);
    assert!(!PeerId::from(ChatId::default()).is_zero());
    let typed = UserId::new((1 << 48) | 7);
    let peer = PeerId::from(typed);
    assert!(!peer_is_user(peer));
    assert!(peer_is_chat(peer));
    assert_eq!(peer_to_chat(peer), ChatId::new(7));
    assert_eq!(peer_to_user(peer), UserId::default());
    assert_eq!(peer_from_chat(-1_i32).value, u64::MAX);
    assert_eq!(peer_from_channel(-2_i32).value, u64::MAX - 1);
    let modern = PeerId::new((0xA5 << 56) | (2 << 48) | 99);
    assert_eq!(deserialize_peer_id(serialize_peer_id(modern)), modern);
    assert_eq!(
        peer_to_mtp(PeerId::from(FakeChatId::new(7))).type_id(),
        tg_scheme::PeerUserData::ID
    );
}

#[test]
fn full_keys_preserve_session_peer_and_signed_message_order() {
    let first = FullMsgId::new(peer_from_user(42_u64), MsgId::new(7));
    let second = FullMsgId::new(peer_from_chat(42_u64), MsgId::new(7));
    let mut map = HashMap::new();
    map.insert(first, "user");
    map.insert(second, "chat");
    assert_eq!(map.len(), 2);
    assert_eq!(map[&first], "user");
    assert_eq!(map[&second], "chat");
    let a = GlobalMsgId {
        item_id: first,
        session_unique_id: 1,
    };
    let b = GlobalMsgId {
        item_id: first,
        session_unique_id: 2,
    };
    let mut sessions = HashMap::new();
    sessions.insert(a, 1);
    sessions.insert(b, 2);
    assert_eq!(sessions.len(), 2);
    let ordered = [MsgId::new(7), MsgId::new(-1), MsgId::new(0)]
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ordered.into_iter().map(|v| v.bare).collect::<Vec<_>>(),
        vec![-1, 0, 7]
    );
    assert!(!FullMsgId::new(PeerId::default(), MsgId::new(-1)).is_zero());
    assert!(FullStoryId {
        peer: peer_from_user(1_u64),
        story: -1
    }
    .valid());
}

#[test]
fn reserved_flag_preconditions_and_undefined_signed_overflow_are_rejected() {
    let invalid = PeerId::new(UserId::RESERVED_BIT << 48);
    assert!(std::panic::catch_unwind(|| serialize_peer_id(invalid)).is_err());
    assert!(std::panic::catch_unwind(|| client_msg_index(END_CLIENT_MSG_ID)).is_err());
    assert!(std::panic::catch_unwind(|| client_msg_by_index(-1)).is_err());
    assert!(std::panic::catch_unwind(|| story_id_from_msg_id(END_STORY_MSG_ID)).is_err());
    assert!(std::panic::catch_unwind(|| story_id_to_msg_id(-1)).is_err());
    assert!(!is_story_msg_id(story_id_to_msg_id(SERVER_MAX_STORY_ID)));
    assert!(std::panic::catch_unwind(|| -MsgId::new(i64::MIN)).is_err());
    assert!(std::panic::catch_unwind(|| MsgId::new(i64::MAX) + MsgId::new(1)).is_err());
    assert!(std::panic::catch_unwind(|| MsgId::new(i64::MIN) - MsgId::new(1)).is_err());
    assert!(std::panic::catch_unwind(|| {
        let mut id = MsgId::new(i64::MAX);
        id.increment()
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        let mut id = MsgId::new(i64::MIN);
        id.decrement()
    })
    .is_err());
}
