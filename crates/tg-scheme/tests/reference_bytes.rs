//! Reference byte sequences written by hand from the TL rules
//! (https://core.telegram.org/mtproto/serialize) and the constructor ids in
//! the scheme files — not produced by the code under test. Each case checks
//! the serialized bytes and that reading them back gives the same bytes.

use tg_scheme::tl::{self, Deserialize, Error, Function, Reader, Serialize};
use tg_scheme::*;

fn bytes_of<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    tl::primes_to_bytes(&tl::serialize(value))
}

fn le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// Checks `value` serializes to `expected` and that `T` reads `expected`
/// back to the same bytes, consuming everything.
fn check<T: Serialize + Deserialize>(value: &T, expected: &[u8]) {
    assert_eq!(bytes_of(value), expected, "serialized bytes");
    let primes = tl::bytes_to_primes(expected);
    let read: T = tl::deserialize_exact(&primes).expect("read back");
    assert_eq!(bytes_of(&read), expected, "re-serialized bytes");
}

fn read_err<T: Deserialize>(bytes: &[u8]) -> Error {
    match T::read(&mut Reader::new(&tl::bytes_to_primes(bytes))) {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

// --- primitives ---

#[test]
fn int() {
    check(&0x0102_0304i32, &[0x04, 0x03, 0x02, 0x01]);
    check(&-1i32, &[0xff, 0xff, 0xff, 0xff]);
    check(&i32::MIN, &[0x00, 0x00, 0x00, 0x80]);
}

#[test]
fn long() {
    check(
        &0x1122_3344_5566_7788u64,
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );
    check(&u64::MAX, &[0xff; 8]);
}

#[test]
fn double() {
    // 1.0 = 0x3FF0000000000000, -2.5 = 0xC004000000000000.
    check(&1.0f64, &[0, 0, 0, 0, 0, 0, 0xf0, 0x3f]);
    check(&-2.5f64, &[0, 0, 0, 0, 0, 0, 0x04, 0xc0]);
}

#[test]
fn int128_int256() {
    let v = tl::Int128 {
        l: 0x0807_0605_0403_0201,
        h: 0x100f_0e0d_0c0b_0a09,
    };
    check(&v, &(1u8..=16).collect::<Vec<_>>());
    let w = tl::Int256 {
        l: v,
        h: tl::Int128 {
            l: 0x1817_1615_1413_1211,
            h: 0x201f_1e1d_1c1b_1a19,
        },
    };
    check(&w, &(1u8..=32).collect::<Vec<_>>());
    assert_eq!(read_err::<tl::Int128>(&[0; 12]), Error::UnexpectedEnd);
    assert_eq!(read_err::<tl::Int256>(&[0; 28]), Error::UnexpectedEnd);
}

#[test]
fn bool_boxed() {
    check(&true, &[0xb5, 0x75, 0x72, 0x99]);
    check(&false, &[0x37, 0x97, 0x79, 0xbc]);
    assert_eq!(
        read_err::<bool>(&[1, 2, 3, 4]),
        Error::UnexpectedConstructor { id: 0x0403_0201 }
    );
}

// --- strings and bytes ---

#[test]
fn short_strings() {
    check(&b"".to_vec(), &[0, 0, 0, 0]);
    check(&b"a".to_vec(), &[1, b'a', 0, 0]);
    check(&b"ab".to_vec(), &[2, b'a', b'b', 0]);
    check(&b"abc".to_vec(), &[3, b'a', b'b', b'c']);
    check(&b"abcd".to_vec(), &[4, b'a', b'b', b'c', b'd', 0, 0, 0]);
    check(
        &b"abcdefg".to_vec(),
        &[7, b'a', b'b', b'c', b'd', b'e', b'f', b'g'],
    );
}

fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 + 1) as u8).collect()
}

#[test]
fn string_253_is_last_short_form() {
    // 1 length byte + 253 bytes = 254, padded with 2 zero bytes to 256.
    let data = payload(253);
    let expected = cat(&[&[253], &data, &[0, 0]]);
    assert_eq!(expected.len(), 256);
    check(&data, &expected);
}

#[test]
fn string_254_is_first_long_form() {
    // 0xFE, 24-bit length, data, padded to 4: 4 + 254 + 2.
    let data = payload(254);
    let expected = cat(&[&[254, 254, 0, 0], &data, &[0, 0]]);
    check(&data, &expected);
}

#[test]
fn string_255_and_256() {
    let data = payload(255);
    check(&data, &cat(&[&[254, 255, 0, 0], &data, &[0]]));
    let data = payload(256);
    check(&data, &cat(&[&[254, 0, 1, 0], &data]));
}

#[test]
fn string_long_lengths() {
    let data = payload(1000);
    check(&data, &cat(&[&[254, 0xe8, 0x03, 0], &data]));
    let data = payload(0x01_2345);
    check(&data, &cat(&[&[254, 0x45, 0x23, 0x01], &data, &[0, 0, 0]]));
}

#[test]
fn string_read_quirks() {
    // A short string written in the long form is accepted, like the original.
    let read: Vec<u8> =
        tl::deserialize_exact(&tl::bytes_to_primes(&[254, 3, 0, 0, b'a', b'b', b'c', 0])).unwrap();
    assert_eq!(read, b"abc");
    // Padding bytes are not checked.
    let read: Vec<u8> = tl::deserialize_exact(&tl::bytes_to_primes(&[1, b'x', 9, 9])).unwrap();
    assert_eq!(read, b"x");
    assert_eq!(read_err::<Vec<u8>>(&[255, 1, 2, 3]), Error::InvalidString);
    assert_eq!(read_err::<Vec<u8>>(&[5, 1, 2, 3]), Error::UnexpectedEnd);
    assert_eq!(
        read_err::<Vec<u8>>(&[254, 9, 0, 0, 1, 2, 3, 4]),
        Error::UnexpectedEnd
    );
    assert_eq!(read_err::<Vec<u8>>(&[]), Error::UnexpectedEnd);
}

#[test]
fn utf8_text() {
    let text = tl::core_types::mtp_string("Привет");
    check(
        &text,
        &[
            12, 0xd0, 0x9f, 0xd1, 0x80, 0xd0, 0xb8, 0xd0, 0xb2, 0xd0, 0xb5, 0xd1, 0x82, 0, 0, 0,
        ],
    );
    assert_eq!(tl::core_types::qs(&text), "Привет");
    assert_eq!(tl::core_types::qs(&[0x61, 0xff]), "a\u{fffd}");
}

// --- vectors ---

const VECTOR: [u8; 4] = [0x15, 0xc4, 0xb5, 0x1c];

#[test]
fn boxed_vectors() {
    check(&Vec::<i32>::new(), &cat(&[&VECTOR, &[0, 0, 0, 0]]));
    check(&vec![1i32, 2], &cat(&[&VECTOR, &le(2), &le(1), &le(2)]));
    check(
        &vec![5u64],
        &cat(&[&VECTOR, &le(1), &[5, 0, 0, 0, 0, 0, 0, 0]]),
    );
    check(
        &vec![b"a".to_vec(), Vec::new()],
        &cat(&[&VECTOR, &le(2), &[1, b'a', 0, 0], &[0, 0, 0, 0]]),
    );
    check(
        &vec![true],
        &cat(&[&VECTOR, &le(1), &[0xb5, 0x75, 0x72, 0x99]]),
    );
}

#[test]
fn bare_vector() {
    check(&tl::BareVec(vec![7i32]), &cat(&[&le(1), &le(7)]));
}

#[test]
fn vector_errors() {
    // Declared count larger than what is left.
    assert_eq!(
        read_err::<Vec<i32>>(&cat(&[&VECTOR, &le(3), &le(1), &le(2)])),
        Error::UnexpectedEnd
    );
    // Huge count is rejected before allocating.
    assert_eq!(
        read_err::<Vec<i32>>(&cat(&[&VECTOR, &le(0xffff_ffff)])),
        Error::UnexpectedEnd
    );
    assert_eq!(
        read_err::<Vec<i32>>(&cat(&[&le(0x1234_5678), &le(0)])),
        Error::UnexpectedConstructor { id: 0x1234_5678 }
    );
}

// --- constructors ---

#[test]
fn constructors_without_fields() {
    check(&mtp_input_peer_empty(), &[0xea, 0x18, 0x3b, 0x7f]);
    check(&mtp_input_peer_self(), &[0xc9, 0x7e, 0xa0, 0x7d]);
    assert_eq!(InputPeerSelfData::ID, 0x7da0_7ec9);
}

#[test]
fn constructor_with_longs() {
    let peer = mtp_input_peer_user(0x0102_0304_0506_0708, 0x1112_1314_1516_1718);
    check(
        &peer,
        &[
            0x4c, 0xa5, 0xe8, 0xdd, 8, 7, 6, 5, 4, 3, 2, 1, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
            0x12, 0x11,
        ],
    );
    assert_eq!(peer.type_id(), 0xdde8_a54c);
    assert_eq!(peer.c_input_peer_user().vuser_id(), 0x0102_0304_0506_0708);
}

#[test]
fn nested_boxed() {
    let peer = mtp_input_peer_user_from_message(mtp_input_peer_self(), 5, 7);
    check(
        &peer,
        &cat(&[
            &le(0xa87b_0a1c),
            &le(0x7da0_7ec9),
            &le(5),
            &[7, 0, 0, 0, 0, 0, 0, 0],
        ]),
    );
}

#[test]
fn mtproto_rpc_error() {
    let error = mtp_rpc_error(400, b"FLOOD".to_vec());
    check(
        &error,
        &[
            0x19, 0xca, 0x44, 0x21, 0x90, 0x01, 0, 0, 5, b'F', b'L', b'O', b'O', b'D', 0, 0,
        ],
    );
}

#[test]
fn bare_constructor_items() {
    // future_salts#ae500895 req_msg_id:long now:int salts:vector<future_salt>
    let salts = mtp_future_salts(
        1,
        2,
        tl::BareVec(vec![FutureSaltData {
            valid_since: 3,
            valid_until: 4,
            salt: 5,
        }]),
    );
    check(
        &salts,
        &cat(&[
            &le(0xae50_0895),
            &[1, 0, 0, 0, 0, 0, 0, 0],
            &le(2),
            &le(1),
            &le(3),
            &le(4),
            &[5, 0, 0, 0, 0, 0, 0, 0],
        ]),
    );
}

#[test]
fn vector_of_vectors() {
    let block = mtp_tls_block_permutation(vec![vec![mtp_tls_block_domain()]]);
    check(
        &block,
        &cat(&[
            &le(TlsBlockPermutationData::ID),
            &VECTOR,
            &le(1),
            &VECTOR,
            &le(1),
            &le(TlsBlockDomainData::ID),
        ]),
    );
}

// --- conditional fields ---

const PHOTO_EXTERNAL: u32 = 0xe5bb_fe1a;

#[test]
fn conditional_all_flag_combinations() {
    // inputMediaPhotoExternal flags:# spoiler:flags.1?true url:string ttl_seconds:flags.0?int
    let url = [3, b'u', b'r', b'l'];
    let cases: [(u64, Vec<u8>); 4] = [
        (0, cat(&[&le(PHOTO_EXTERNAL), &le(0), &url])),
        (1, cat(&[&le(PHOTO_EXTERNAL), &le(1), &url, &le(60)])),
        (2, cat(&[&le(PHOTO_EXTERNAL), &le(2), &url])),
        (3, cat(&[&le(PHOTO_EXTERNAL), &le(3), &url, &le(60)])),
    ];
    for (flags, expected) in cases {
        let media =
            mtp_input_media_photo_external(tl::Flags::from_raw(flags), b"url".to_vec(), Some(60));
        check(&media, &expected);
        let data = media.c_input_media_photo_external();
        assert_eq!(data.is_spoiler(), flags & 2 != 0);
        assert_eq!(data.vttl_seconds(), (flags & 1 != 0).then_some(60));
    }
}

#[test]
fn conditional_flag_decides_not_the_value() {
    // A value without its flag is not written (the original ignores it too).
    let media = mtp_input_media_photo_external(tl::Flags::empty(), b"url".to_vec(), Some(60));
    assert_eq!(
        bytes_of(&media),
        cat(&[&le(PHOTO_EXTERNAL), &le(0), &[3, b'u', b'r', b'l']])
    );
    assert_eq!(media.c_input_media_photo_external().vttl_seconds(), None);
    // A flag without a value writes the default value, like the original's
    // default-constructed placeholder (`MTPint()`).
    let media = mtp_input_media_photo_external(
        InputMediaPhotoExternalData::F_TTL_SECONDS,
        b"url".to_vec(),
        None,
    );
    assert_eq!(
        bytes_of(&media),
        cat(&[&le(PHOTO_EXTERNAL), &le(1), &[3, b'u', b'r', b'l'], &le(0)])
    );
}

#[test]
fn conditional_three_fields_all_combinations() {
    // inputMediaDocumentExternal#779600f9 flags:# spoiler:flags.1?true url:string
    //   ttl_seconds:flags.0?int video_cover:flags.2?InputPhoto video_timestamp:flags.3?int
    let photo = cat(&[
        &le(0x3bb3_b94a),
        &[9, 0, 0, 0, 0, 0, 0, 0],
        &[8, 0, 0, 0, 0, 0, 0, 0],
        &[2, 0xaa, 0xbb, 0],
    ]);
    for flags in 0u64..16 {
        let mut expected = cat(&[&le(0x7796_00f9), &le(flags as u32), &[0, 0, 0, 0]]);
        if flags & 1 != 0 {
            expected.extend_from_slice(&le(10));
        }
        if flags & 4 != 0 {
            expected.extend_from_slice(&photo);
        }
        if flags & 8 != 0 {
            expected.extend_from_slice(&le(20));
        }
        let media = mtp_input_media_document_external(
            tl::Flags::from_raw(flags),
            Vec::new(),
            Some(10),
            Some(mtp_input_photo(9, 8, vec![0xaa, 0xbb])),
            Some(20),
        );
        check(&media, &expected);
    }
}

#[test]
fn conditional_bool_fields() {
    // peerNotifySettings#99622c0c flags:# show_previews:flags.0?Bool silent:flags.1?Bool
    //   mute_until:flags.2?int ...
    let settings = mtp_peer_notify_settings(
        PeerNotifySettingsData::F_SHOW_PREVIEWS | PeerNotifySettingsData::F_MUTE_UNTIL,
        Some(true),
        Some(false),
        Some(77),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    check(
        &settings,
        &cat(&[&le(0x9962_2c0c), &le(5), &le(0x9972_75b5), &le(77)]),
    );
    let data = settings.data();
    assert_eq!(data.vshow_previews(), Some(true));
    assert_eq!(data.vsilent(), None);
}

#[test]
fn flags2_is_the_high_word() {
    // user#b1b8cc83 flags:# self:flags.10?true ... flags2:# stories_hidden:flags2.3?true
    //   id:long ... usernames:flags2.0?Vector<Username> ...
    let flags = UserData::F_SELF | UserData::F_STORIES_HIDDEN | UserData::F_USERNAMES;
    assert_eq!(flags.value(), (1 << 10) | (1 << 35) | (1 << 32));
    let user: User = UserData {
        flags,
        id: 1,
        usernames: Some(vec![mtp_username(
            UsernameData::F_ACTIVE,
            b"durov".to_vec(),
        )]),
        ..Default::default()
    }
    .into();
    check(
        &user,
        &cat(&[
            &le(0xb1b8_cc83),
            &le(1 << 10),
            &le((1 << 3) | 1),
            &[1, 0, 0, 0, 0, 0, 0, 0],
            &VECTOR,
            &le(1),
            &le(0xb407_3647),
            &le(2),
            &[5, b'd', b'u', b'r', b'o', b'v', 0, 0],
        ]),
    );
    let data = user.c_user();
    assert!(data.is_self() && data.is_stories_hidden() && !data.is_bot());
    assert_eq!(data.vusernames().map(|v| v.len()), Some(1));
    assert_eq!(data.vfirst_name(), None);
}

// --- methods ---

#[test]
fn method_with_parameters() {
    // messages.getHistory#4423e6c5 peer:InputPeer offset_id:int offset_date:int
    //   add_offset:int limit:int max_id:int min_id:int hash:long = messages.Messages
    let request = MessagesGetHistory::new(mtp_input_peer_self(), 1, 2, 3, 4, 5, 6, 7);
    check(
        &request,
        &cat(&[
            &le(0x4423_e6c5),
            &le(0x7da0_7ec9),
            &le(1),
            &le(2),
            &le(3),
            &le(4),
            &le(5),
            &le(6),
            &[7, 0, 0, 0, 0, 0, 0, 0],
        ]),
    );
    fn response<F: Function<Response = MessagesMessages>>(_: &F) {}
    response(&request);
}

#[test]
fn method_without_parameters_and_generic_wrapper() {
    check(&HelpGetConfig::new(), &le(0xc4f9_186b));
    let wrapped = InvokeWithLayer::new(CURRENT_LAYER, HelpGetConfig::new());
    check(
        &wrapped,
        &cat(&[&le(0xda9b_0d0d), &le(229), &le(0xc4f9_186b)]),
    );
    fn response<F: Function<Response = Config>>(_: &F) {}
    response(&wrapped);
}

#[test]
fn method_returning_vector() {
    fn response<F: Function<Response = Vec<ReceivedNotifyMessage>>>(_: &F) {}
    let request = MessagesReceivedMessages::new(9);
    response(&request);
    check(&request, &cat(&[&le(0x05a9_54c0), &le(9)]));
}

#[test]
fn method_name_collision_exception() {
    // rpc_drop_answer (method) vs RpcDropAnswer (type).
    check(
        &RpcDropAnswerMethod::new(3),
        &cat(&[&le(0x58e4_a740), &[3, 0, 0, 0, 0, 0, 0, 0]]),
    );
    check(&mtp_rpc_answer_unknown(), &le(0x5e2a_d36e));
}

// --- reading errors and quirks ---

#[test]
fn unknown_constructor_is_an_error() {
    assert_eq!(
        read_err::<InputPeer>(&le(0xdead_beef)),
        Error::UnexpectedConstructor { id: 0xdead_beef }
    );
    assert_eq!(
        read_err::<MessagesGetHistory>(&le(0xdead_beef)),
        Error::UnexpectedConstructor { id: 0xdead_beef }
    );
    // Single constructor with data: the id is still checked.
    assert_eq!(
        read_err::<RpcError>(&cat(&[&le(1), &le(1), &[0; 4]])),
        Error::UnexpectedConstructor { id: 1 }
    );
}

#[test]
fn truncated_constructor_is_an_error() {
    let full = bytes_of(&mtp_input_peer_user(1, 2));
    for len in (0..full.len()).step_by(4) {
        assert_eq!(
            read_err::<InputPeer>(&full[..len]),
            Error::UnexpectedEnd,
            "{len}"
        );
    }
}

#[test]
fn single_dataless_constructor_accepts_any_id() {
    // Quirk of the original: `MTPtrue::read` (no data, one constructor) does
    // not look at `cons`.
    let value: True = tl::deserialize_exact(&tl::bytes_to_primes(&le(0x1234_5678))).unwrap();
    assert_eq!(bytes_of(&value), le(0x3fed_d339));
    assert_eq!(read_err::<True>(&[]), Error::UnexpectedEnd);
}

// --- generated API details ---

#[test]
fn defaults() {
    // The default of a multi-constructor type is its first constructor.
    check(&User::default(), &cat(&[&le(0xd3bc_4b7a), &[0; 8]]));
    assert_eq!(InputPeer::default().type_id(), InputPeerEmptyData::ID);
}

#[test]
fn ayugram_noforwards_hook() {
    // message#... noforwards:flags.26?true; restricted is on channel.
    let message = MessageData {
        flags: MessageData::F_NOFORWARDS,
        ..Default::default()
    };
    assert!(!message.is_noforwards());
    assert!(message.is_ayu_noforwards());
    let channel = ChannelData {
        flags: ChannelData::F_RESTRICTED | ChannelData::F_NOFORWARDS,
        ..Default::default()
    };
    assert!(!channel.is_restricted() && channel.is_ayu_restricted());
    assert!(!channel.is_noforwards() && channel.is_ayu_noforwards());
}

#[test]
fn flag_inheritance() {
    let flags: tl::Flags<MessageData> = MessageServiceData::F_OUT.inherit();
    assert_eq!(flags, MessageData::F_OUT);
    let flags: tl::Flags<MessageData> =
        tl::core_types::mtp_cast_flags(UpdateShortMessageData::F_SILENT);
    assert_eq!(flags, MessageData::F_SILENT);
}

#[test]
fn match_on_constructors() {
    let peer = mtp_input_peer_channel(5, 6);
    let id = match &peer {
        InputPeer::InputPeerChannel(data) => data.vchannel_id(),
        InputPeer::InputPeerUser(data) => data.vuser_id(),
        _ => 0,
    };
    assert_eq!(id, 5);
}
