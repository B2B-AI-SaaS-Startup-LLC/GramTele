//! `SourceFiles/lang`: language values, plural forms, cloud language packs.
//!
//! The generated part (`lang_auto.h/.cpp` in the original) is produced by
//! `tg-codegen` from `lang.strings` in `build.rs` and included as
//! [`generated`]. Phrases are used as in the original, see [`lang_values`]:
//!
//! ```ignore
//! use tg_core::lang::{tr, lt_count, lt_user};
//! let title = tr::lng_settings_title.now();
//! let text = tr::lng_action_user_joined.tag(lt_from, &name).now();
//! let files = tr::lng_files_selected.count(count).now();
//! ```
pub mod lang_cloud_manager;
pub mod lang_file_parser;
pub mod lang_hardcoded;
pub mod lang_instance;
pub mod lang_keys;
pub mod lang_numbers_animation;
pub mod lang_tag;
pub mod lang_text_entity;
pub mod lang_translator;
pub mod lang_values;

use std::collections::HashMap;
use std::sync::OnceLock;

/// One slot of the key table: a plain key or one plural form of a key.
#[derive(Debug)]
pub struct Entry {
    /// The key; plural forms are `<base>__plural<0..=5>`.
    pub key: &'static str,
    pub index: u16,
    /// The default value in UTF-16, with tag markers already in place.
    pub value: &'static [u16],
    pub tags: &'static [u16],
    /// The key without the plural suffix, empty for a plain key.
    pub plural_base: &'static str,
}

/// `tr::phrase<Tags...>`: a key of `lang.strings`. `base` is the index of
/// the value (of the first of six plural forms when `plural`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phrase {
    pub base: u16,
    pub tags: &'static [u16],
    pub plural: bool,
}

impl Phrase {
    pub const fn new(base: u16, tags: &'static [u16], plural: bool) -> Self {
        Self { base, tags, plural }
    }
}

/// `lang_auto.h` / `lang_auto.cpp`.
#[allow(non_upper_case_globals)]
pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/lang_auto.rs"));
}

pub use generated::*;

/// `namespace tr`: the phrases plus the projections of `lang_text_entity.h`.
pub mod tr {
    pub use super::generated::phrases::*;
    pub use super::lang_text_entity::projections::*;
}

/// `kKeysCount`.
pub const KEYS_COUNT: u16 = generated::KEY_COUNT as u16;
/// `kTagsCount`.
pub const TAGS_COUNT: u16 = generated::TAGS.len() as u16;
/// `kPluralCount`.
pub const PLURAL_COUNT: u16 = 6;

/// `kPluralParts`: the suffix after `#` of each plural form.
pub const PLURAL_PARTS: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

struct Tables {
    /// Plain keys and `<base>#<part>` plural keys, as `GetKeyIndex` accepts.
    keys: HashMap<String, u16>,
    tags: HashMap<&'static str, u16>,
    by_index: Vec<Option<&'static Entry>>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut keys = HashMap::with_capacity(generated::ENTRIES.len());
        let mut by_index = vec![None; generated::KEY_COUNT];
        for entry in generated::ENTRIES {
            by_index[usize::from(entry.index)] = Some(entry);
            if entry.plural_base.is_empty() {
                keys.insert(entry.key.to_owned(), entry.index);
            } else if let Some(part) = entry
                .key
                .strip_prefix(entry.plural_base)
                .and_then(|suffix| suffix.strip_prefix("__plural"))
                .and_then(|shift| shift.parse::<usize>().ok())
                .and_then(|shift| PLURAL_PARTS.get(shift))
            {
                keys.insert(format!("{}#{part}", entry.plural_base), entry.index);
            }
        }
        let tags = generated::TAGS
            .iter()
            .enumerate()
            .map(|(index, tag)| (*tag, index as u16))
            .collect();
        Tables {
            keys,
            tags,
            by_index,
        }
    })
}

/// `GetTagIndex`; `TAGS_COUNT` when there is no such tag.
pub fn get_tag_index(tag: &[u8]) -> u16 {
    std::str::from_utf8(tag)
        .ok()
        .and_then(|tag| tables().tags.get(tag).copied())
        .unwrap_or(TAGS_COUNT)
}

/// `GetKeyIndex`; `KEYS_COUNT` when there is no such key. Plural forms are
/// addressed as `<key>#zero` … `<key>#other`.
pub fn get_key_index(key: &[u8]) -> u16 {
    std::str::from_utf8(key)
        .ok()
        .and_then(|key| tables().keys.get(key).copied())
        .unwrap_or(KEYS_COUNT)
}

/// `IsTagReplaced`.
pub fn is_tag_replaced(key: u16, tag: u16) -> bool {
    tables()
        .by_index
        .get(usize::from(key))
        .copied()
        .flatten()
        .is_some_and(|entry| entry.tags.contains(&tag))
}

/// `GetOriginalValue`. Slots that are kept for removed keys are empty.
pub fn get_original_value(key: u16) -> String {
    tg_base::expects!(key < KEYS_COUNT);
    tables().by_index[usize::from(key)]
        .map(|entry| String::from_utf16_lossy(entry.value))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_lookup_follows_the_generated_tables_of_the_original() {
        assert_eq!(
            get_key_index(b"lng_language_name"),
            tr::lng_language_name.base
        );
        assert_eq!(get_key_index(b"no_such_key"), KEYS_COUNT);
        assert_eq!(get_key_index(b"\xff"), KEYS_COUNT);
        let base = tr::lng_filter_unread_chats.base;
        const { assert!(tr::lng_filter_unread_chats.plural) };
        for (shift, part) in PLURAL_PARTS.iter().enumerate() {
            let key = format!("lng_filter_unread_chats#{part}");
            assert_eq!(get_key_index(key.as_bytes()), base + shift as u16);
        }
        // Neither the bare plural key nor the internal slot name is a key.
        assert_eq!(get_key_index(b"lng_filter_unread_chats"), KEYS_COUNT);
        assert_eq!(
            get_key_index(b"lng_filter_unread_chats__plural1"),
            KEYS_COUNT
        );
        assert_eq!(get_key_index(b"lng_filter_unread_chats#three"), KEYS_COUNT);
        assert_eq!(get_key_index(b"lng_language_name#one"), KEYS_COUNT);
    }

    #[test]
    fn tags_and_original_values() {
        assert_eq!(get_tag_index(b"count"), lt_count);
        assert_eq!(get_tag_index(b"user"), lt_user);
        assert_eq!(get_tag_index(b"no_such_tag"), TAGS_COUNT);
        assert_eq!(get_original_value(tr::lng_language_name.base), "English");
        let base = tr::lng_filter_unread_chats.base;
        assert!(is_tag_replaced(base, lt_count));
        assert!(is_tag_replaced(base + 5, lt_count));
        assert!(!is_tag_replaced(base, lt_user));
        assert!(!is_tag_replaced(tr::lng_language_name.base, lt_count));
        assert!(!is_tag_replaced(KEYS_COUNT, lt_count));
        // Every slot is addressable; unused ones are empty.
        for key in 0..KEYS_COUNT {
            let _ = get_original_value(key);
        }
    }
}
