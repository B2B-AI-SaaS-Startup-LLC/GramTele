//! Expectations transcribed from addTextSerialize / DumpToTextType in the
//! original generator. The small core below only supplies integer printing;
//! the MTProto primitive / gzip dumper is a separate port.
use tg_scheme::dump_to_text::{dump_to_text_type, DumpCore};
use tg_scheme::tl::{self, Reader, TypeId};
use tg_scheme::*;

struct Integers;
impl DumpCore for Integers {
    fn dump_core(
        &self,
        to: &mut String,
        from: &mut Reader<'_>,
        cons: TypeId,
        _level: u32,
        _vcons: TypeId,
    ) -> bool {
        if cons != tl::ID_INT {
            return false;
        }
        match from.get() {
            Ok(value) => {
                to.push_str(&(value as i32).to_string());
                true
            }
            Err(_) => false,
        }
    }
}

fn dump(words: &[i32]) -> (bool, String, usize) {
    let mut reader = Reader::new(words);
    let mut text = String::new();
    let ok = dump_to_text_type(&mut text, &mut reader, 0, 0, 0, &Integers);
    (ok, text, reader.position())
}

#[test]
fn dataless_constructor() {
    let words = tl::serialize(&mtp_input_peer_self());
    assert_eq!(dump(&words), (true, "{ inputPeerSelf }".into(), 1));
}

#[test]
fn integer_field_and_trailing_separator() {
    let words = tl::serialize(&MessagesReceivedMessages::new(9));
    assert_eq!(
        dump(&words),
        (
            true,
            "{ messages_receivedMessages\n  max_id: 9,\n}".into(),
            2
        )
    );
}

#[test]
fn nested_generic_request_indentation() {
    let words = tl::serialize(&InvokeWithLayer::new(229, HelpGetConfig::new()));
    assert_eq!(
        dump(&words),
        (
            true,
            "{ invokeWithLayer\n  layer: 229,\n  query: { help_getConfig },\n}".into(),
            3
        )
    );
}

#[test]
fn absent_conditionals_and_true_flag() {
    for (raw, field) in [(0, ""), (1, "  by_me: YES [ BY BIT 0 IN FIELD flags ],\n")] {
        let words = tl::serialize(&mtp_user_status_recently(tl::Flags::from_raw(raw)));
        assert_eq!(
            dump(&words),
            (
                true,
                format!("{{ userStatusRecently\n  flags: {raw},\n{field}}}"),
                2
            )
        );
    }
}

#[test]
fn unknown_and_truncated_input() {
    assert_eq!(dump(&[]), (false, "[ERROR] (insufficient data)".into(), 0));
    assert_eq!(
        dump(&[0x1234_5678]),
        (
            false,
            "[ERROR] (could not decode type)(ERROR_SCHEME_BAD_CONS:0x12345678)".into(),
            1
        )
    );
    let words = tl::serialize(&MessagesReceivedMessages::new(9));
    let (ok, text, consumed) = dump(&words[..1]);
    assert!(!ok);
    assert_eq!(text, "{ messages_receivedMessages\n  max_id: [ERROR] (could not decode type)(ERROR_SCHEME_BAD_CONS:0xa8509bda)");
    assert_eq!(consumed, 1);
}
