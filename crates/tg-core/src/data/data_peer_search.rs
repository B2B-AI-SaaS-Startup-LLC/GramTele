//! PeerData::fillNames composition. The real TextUtilities normalization service
//! is a required boundary; there is deliberately no fallback implementation.
use super::{data_peer::PeerData, data_peer_fields::PeerFields, data_session::NameIndex};
use crate::core::utils::{rus_keyboard_layout_switch, translit_rus_eng};
pub trait NameTextServices {
    fn remove_accents(&self, value: &str) -> String;
    fn prepare_search_words(&self, value: &str) -> Vec<Vec<u16>>;
}
pub struct NameSearchLabels<'a> {
    pub saved_messages: &'a str,
    pub replies: &'a str,
    pub verification_codes: &'a str,
}
pub fn fill_names(
    peer: &PeerData,
    fields: &PeerFields<'_>,
    resolved_name: &str,
    production: bool,
    labels: NameSearchLabels<'_>,
    text: &impl NameTextServices,
) -> NameIndex {
    assert_eq!(peer.key(), fields.key());
    let mut list = Vec::new();
    let append = |list: &mut Vec<String>, value: &str| {
        if !value.is_empty() {
            list.push(text.remove_accents(value));
        }
    };
    append(&mut list, resolved_name);
    if list
        .first()
        .is_some_and(|s| s.chars().any(|c| matches!(c,'а'..='я'|'А'..='Я'|'ё'|'Ё')))
    {
        let translit = translit_rus_eng(&list[0]);
        append(&mut list, &translit);
    }
    match fields {
        PeerFields::User(v) => {
            if v.name_or_phone != resolved_name {
                append(&mut list, v.name_or_phone);
            }
            append(&mut list, v.usernames.username());
            let special = if peer.is_self(fields) {
                Some(("Saved messages", labels.saved_messages))
            } else if peer.is_replies_chat(production) {
                Some(("Replies", labels.replies))
            } else if peer.is_verify_codes() {
                Some(("Verification Codes", labels.verification_codes))
            } else {
                None
            };
            if let Some((english, localized)) = special {
                append(&mut list, english);
                if english != localized {
                    append(&mut list, localized);
                }
            }
        }
        PeerFields::Channel(v) => append(&mut list, v.usernames.username()),
        PeerFields::Chat(_) => {}
    }
    let mut joined = list.join(" ");
    let switched = rus_keyboard_layout_switch(&joined);
    joined.push(' ');
    joined.push_str(&switched);
    NameIndex::from_words(text.prepare_search_words(&joined))
}
