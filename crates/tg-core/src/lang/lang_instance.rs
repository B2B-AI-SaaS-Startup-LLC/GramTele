//! Port of `lang/lang_instance.{h,cpp}`: the current language pack.
//!
//! Structure differences from the original:
//! * The base pack is a second `Instance` with a `_derived` pointer there.
//!   Here it is a [`PackState`] inside the main instance: a base pack never
//!   has values or a base of its own, it only overrides the values of the
//!   pack it belongs to. A nested base in serialized data (never written by
//!   the client, and a crash in the original as soon as it has values) is
//!   ignored.
//! * `Core::App().langpack()` is the instance of the main thread:
//!   [`instance`].
//! * `Local::writeLangPack()` is a hook, [`Instance::set_write_handler`]:
//!   local storage is not ported yet.
//! * `Platform::SystemLanguage()` / `QLocale::system().uiLanguages()` come
//!   from a provider, [`set_system_language_provider`] (platform layer).
//! * A null and an empty `QString` are the same `String`; an empty string
//!   field is written as a null one.
use super::lang_file_parser::FileParser;
use super::lang_keys::{default_language_id, language_id_or_default};
use super::lang_tag::{self, TEXT_COMMAND, TEXT_COMMAND_LANG_TAG};
use super::{
    get_key_index, get_original_value, get_tag_index, is_tag_replaced, lt_user, tr, KEYS_COUNT,
    PLURAL_PARTS, TAGS_COUNT,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::rc::Rc;
use tg_base::qt_stream::{Reader, Writer};
use tg_base::qthelp_regex;
use tg_rpl::{EventStream, Producer};
use tg_scheme::{LangPackDifferenceData, LangPackString};
use tg_ui::text::utf16;

/// `kChoosingStickerReplacement`.
pub const CHOOSING_STICKER_REPLACEMENT: &str = "oo";

const SERIALIZE_VERSION_TAG: &str = "#new";
const SERIALIZE_VERSION: i32 = 1;
const CLOUD_LANG_PACK_NAME: &str = "tdesktop";
const CUSTOM_LANGUAGE: &str = "#custom";
const LANG_VALUES_LIMIT: i32 = 20000;

/// `Language`. Equality compares only `id` and `name`, as in the original.
#[derive(Clone, Debug, Default)]
pub struct Language {
    pub id: String,
    pub plural_id: String,
    pub base_id: String,
    pub name: String,
    pub native_name: String,
}

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.name == other.name
    }
}

impl Eq for Language {}

/// `CloudLangPackName`.
pub fn cloud_lang_pack_name() -> String {
    CLOUD_LANG_PACK_NAME.to_owned()
}

/// `CustomLanguageId`.
pub fn custom_language_id() -> String {
    CUSTOM_LANGUAGE.to_owned()
}

/// `DefaultLanguage`.
pub fn default_language() -> Language {
    Language {
        id: "en".into(),
        plural_id: String::new(),
        base_id: String::new(),
        name: "English".into(),
        native_name: "English".into(),
    }
}

/// `Pack`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pack {
    None,
    Current,
    Base,
}

/// `ValueParser`: replaces `{tag}` in a pack value with the internal marker.
/// `None` (after logging) when the value is malformed.
fn parse_value(key: &[u8], key_index: u16, value: &[u8]) -> Option<String> {
    let log_error = |text: &str, tag: &[u8]| {
        let mut logged_key = String::from_utf8_lossy(key).into_owned();
        if !tag.is_empty() {
            logged_key.push(':');
            logged_key.push_str(&String::from_utf8_lossy(tag));
        }
        tg_base::log!("Lang Error: {text} (key '{logged_key}')");
    };
    let is_tag_char = |ch: u8| ch.is_ascii_alphanumeric() || ch == b'_';

    let mut result = String::with_capacity(value.len());
    let mut tags_used = BTreeSet::new();
    let mut begin = 0;
    let mut ch = 0;
    while ch != value.len() {
        if value[ch] == b'{' {
            if ch > begin {
                result.push_str(&String::from_utf8_lossy(&value[begin..ch]));
            }

            ch += 1;
            let tag_start = ch;
            while ch != value.len() && is_tag_char(value[ch]) {
                ch += 1;
            }
            if ch == tag_start {
                log_error("Expected tag name", b"");
                return None;
            }
            let tag = &value[tag_start..ch];
            if ch == value.len() || value[ch] != b'}' {
                log_error("Expected '}' after tag name", tag);
                return None;
            }
            let tag_index = get_tag_index(tag);
            if tag_index == TAGS_COUNT {
                log_error("Unknown tag", tag);
                return None;
            }
            if !is_tag_replaced(key_index, tag_index) {
                log_error("Unexpected tag", tag);
                return None;
            }
            if !tags_used.insert(tag_index) {
                log_error("Repeated tag", tag);
                return None;
            }

            result.push(TEXT_COMMAND);
            result.push(TEXT_COMMAND_LANG_TAG);
            result.push(
                char::from_u32(0x0020 + u32::from(tag_index))
                    .unwrap_or(char::REPLACEMENT_CHARACTER),
            );
            result.push(TEXT_COMMAND);

            begin = ch + 1;
        }
        ch += 1;
    }
    if value.len() > begin {
        result.push_str(&String::from_utf8_lossy(&value[begin..]));
    }
    Some(result)
}

/// `PrepareTestValue`: every character except spaces and tag markers is
/// replaced by `filler`, and one more `filler` goes in front.
fn prepare_test_value(current: &str, filler: char) -> String {
    let mut result = String::with_capacity(current.len() + 1);
    result.push(filler);
    let mut in_command = false;
    for ch in current.chars() {
        let new_in_command = if ch == TEXT_COMMAND {
            !in_command
        } else {
            in_command
        };
        // `QChar::isSpace` is asked per UTF-16 unit: never true for a
        // non-BMP character, which takes two fillers.
        if in_command || new_in_command || (ch.len_utf16() == 1 && is_qt_space(ch)) {
            result.push(ch);
        } else {
            for _ in 0..ch.len_utf16() {
                result.push(filler);
            }
        }
        in_command = new_in_command;
    }
    result
}

/// `QChar::isSpace`: `\t`..`\r`, U+0085, U+00A0 and the separator category.
fn is_qt_space(ch: char) -> bool {
    matches!(ch, '\u{9}'..='\u{d}' | '\u{85}' | '\u{a0}')
        || matches!(
            ch,
            ' ' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
        )
}

/// `PluralCodeForCustom`: the language code from a file name such as
/// `lang_ru_RU.strings` or `lang-de.strings`.
fn plural_code_for_custom(absolute_path: &str, relative_path: &str) -> String {
    let path = if !absolute_path.is_empty() {
        absolute_path
    } else {
        relative_path
    };
    let name = path.rsplit('/').next().unwrap_or(path);
    let pattern = "_([a-z]{2,3}_[A-Z]{2,3}|\\-[a-z]{2,3})?\\.";
    let regex = qthelp_regex::compile(pattern, qthelp_regex::RegExOptions::NONE);
    if let Some(captures) = regex.as_ref().and_then(|regex| regex.captures(name)) {
        return captures
            .get(1)
            .map_or(String::new(), |found| found.as_str().to_owned());
    }
    default_language_id()
}

/// `ParseKeyValue`.
fn parse_key_value(key: &[u8], value: &[u8], save: impl FnOnce(u16, String)) {
    let index = get_key_index(key);
    if index != KEYS_COUNT {
        if let Some(parsed) = parse_value(key, index, value) {
            save(index, parsed);
        }
    } else if !key.starts_with(b"cloud_") {
        tg_base::debug_log!(
            "Lang Warning: Unknown key '{}'",
            String::from_utf8_lossy(key)
        );
    }
}

/// `HandleString`: `set(key, value)` for every key-value pair of a server
/// string, `reset(key)` for every key a deleted string covers.
fn handle_string(
    string: &LangPackString,
    mut set_callback: impl FnMut(&[u8], &[u8]),
    mut reset_callback: impl FnMut(&[u8]),
) {
    let with_part = |key: &[u8], part: &str| {
        let mut result = key.to_vec();
        result.push(b'#');
        result.extend_from_slice(part.as_bytes());
        result
    };
    match string {
        LangPackString::LangPackString(data) => {
            set_callback(&data.key, &data.value);
        }
        LangPackString::LangPackStringPluralized(data) => {
            let values = [
                data.zero_value.as_deref().unwrap_or_default(),
                data.one_value.as_deref().unwrap_or_default(),
                data.two_value.as_deref().unwrap_or_default(),
                data.few_value.as_deref().unwrap_or_default(),
                data.many_value.as_deref().unwrap_or_default(),
                data.other_value.as_slice(),
            ];
            for (part, value) in PLURAL_PARTS.iter().zip(values) {
                set_callback(&with_part(&data.key, part), value);
            }
        }
        LangPackString::LangPackStringDeleted(data) => {
            reset_callback(&data.key);
            for part in PLURAL_PARTS {
                reset_callback(&with_part(&data.key, part));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ChoosingStickerReplacement {
    support: bool,
    right_index: i32,
    right_index_named: i32,
}

/// What both the current and the base pack have (an `Instance` there).
#[derive(Debug)]
struct PackState {
    id: String,
    plural_id: String,
    name: String,
    native_name: String,
    custom_file_path_absolute: String,
    custom_file_path_relative: String,
    custom_file_content: Vec<u8>,
    version: i32,
    non_default_set: Vec<bool>,
    non_default_values: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl PackState {
    fn new() -> Self {
        Self {
            id: String::new(),
            plural_id: String::new(),
            name: String::new(),
            native_name: String::new(),
            custom_file_path_absolute: String::new(),
            custom_file_path_relative: String::new(),
            custom_file_content: Vec::new(),
            version: 0,
            non_default_set: vec![false; usize::from(KEYS_COUNT)],
            non_default_values: BTreeMap::new(),
        }
    }

    fn is_custom(&self) -> bool {
        self.id == CUSTOM_LANGUAGE || self.id == "#TEST_X" || self.id == "#TEST_0"
    }

    fn get_non_default_value(&self, key: &[u8]) -> String {
        self.non_default_values
            .get(key)
            .map_or(String::new(), |value| {
                String::from_utf8_lossy(value).into_owned()
            })
    }

    fn is_non_default_plural(&self, key: u16) -> bool {
        let key = usize::from(key);
        tg_base::expects!(key + 5 < self.non_default_set.len());
        self.non_default_set[key..key + 6].iter().any(|set| *set)
    }

    /// The plural id `updatePluralRules` settles on.
    fn ensure_plural_id(&mut self) -> &str {
        if self.plural_id.is_empty() {
            self.plural_id = if self.is_custom() {
                plural_code_for_custom(
                    &self.custom_file_path_absolute,
                    &self.custom_file_path_relative,
                )
            } else {
                language_id_or_default(&self.id)
            };
        }
        &self.plural_id
    }

    fn serialize(&self, base: &[u8]) -> Vec<u8> {
        let mut stream = Writer::default();
        let string = |stream: &mut Writer, value: &str| {
            if value.is_empty() {
                stream.nullable_utf16(None);
            } else {
                let units: Vec<u16> = value.encode_utf16().collect();
                stream.nullable_utf16(Some(&units));
            }
        };
        let bytes = |stream: &mut Writer, value: &[u8]| {
            stream.nullable_bytes(if value.is_empty() { None } else { Some(value) });
        };
        string(&mut stream, SERIALIZE_VERSION_TAG);
        stream.i32(SERIALIZE_VERSION);
        string(&mut stream, &self.id);
        string(&mut stream, &self.plural_id);
        string(&mut stream, &self.name);
        string(&mut stream, &self.native_name);
        stream.i32(self.version);
        string(&mut stream, &self.custom_file_path_absolute);
        string(&mut stream, &self.custom_file_path_relative);
        bytes(&mut stream, &self.custom_file_content);
        stream.i32(self.non_default_values.len() as i32);
        for (key, value) in &self.non_default_values {
            // An absent plural form is a null `QByteArray` value in the
            // original (`value_or_empty()`), by far the most common source
            // of an empty value: empty values are written as null.
            stream.bytes(key);
            bytes(&mut stream, value);
        }
        bytes(&mut stream, base);
        stream.0
    }
}

/// The fields of a serialized pack, before they are applied.
struct Serialized {
    id: String,
    plural_id: String,
    name: String,
    native_name: String,
    version: i32,
    custom_file_path_absolute: String,
    custom_file_path_relative: String,
    custom_file_content: Vec<u8>,
    non_default_strings: Vec<(Vec<u8>, Vec<u8>)>,
    base: Vec<u8>,
}

enum ReadResult {
    /// Logged. `base_reset`: the original had already dropped its base pack
    /// when the failure was found, everything else is unchanged.
    Failed {
        base_reset: bool,
    },
    /// The custom file on disk differs from the cached content: reload it.
    CustomFileChanged {
        absolute: String,
        relative: String,
        content: Vec<u8>,
    },
    Ready(Box<Serialized>),
}

fn read_string(stream: &mut Reader<'_>) -> Option<String> {
    let units = stream.nullable_utf16()?.unwrap_or_default();
    Some(String::from_utf16_lossy(&units))
}

fn read_bytes(stream: &mut Reader<'_>) -> Option<Vec<u8>> {
    Some(stream.nullable_bytes()?.unwrap_or_default())
}

/// The reading part of `fillFromSerialized`.
fn read_serialized(data: &[u8]) -> ReadResult {
    const READ_ERROR: &str = "Lang Error: Could not read data from serialized langpack.";
    macro_rules! read {
        ($value:expr) => {
            match $value {
                Some(value) => value,
                None => {
                    tg_base::log!("{READ_ERROR}");
                    return ReadResult::Failed { base_reset: false };
                }
            }
        };
    }
    let mut stream = Reader::new(data);
    let mut id;
    let mut plural_id = String::new();
    let mut name = String::new();
    let mut native_name = String::new();

    // A failed read leaves the tag empty: the legacy branch then fails too.
    let serialize_version_tag = read_string(&mut stream).unwrap_or_default();
    let legacy_format = serialize_version_tag != SERIALIZE_VERSION_TAG;
    if legacy_format {
        id = serialize_version_tag;
    } else {
        let serialize_version = stream.i32().unwrap_or(0);
        if serialize_version != SERIALIZE_VERSION {
            tg_base::log!("Lang Error: Unsupported serialize version.");
            return ReadResult::Failed { base_reset: false };
        }
        id = read!(read_string(&mut stream));
        plural_id = read!(read_string(&mut stream));
        name = read!(read_string(&mut stream));
        native_name = read!(read_string(&mut stream));
    }
    let version = read!(stream.i32());
    let custom_file_path_absolute = read!(read_string(&mut stream));
    let custom_file_path_relative = read!(read_string(&mut stream));
    let custom_file_content = read!(read_bytes(&mut stream));
    let non_default_values_count = read!(stream.i32());
    if non_default_values_count > LANG_VALUES_LIMIT {
        tg_base::log!("Lang Error: Values count limit exceeded: {non_default_values_count}");
        return ReadResult::Failed { base_reset: false };
    }

    if !custom_file_path_absolute.is_empty() {
        id = custom_language_id();
        let current_custom_file_content = FileParser::read_file(
            Path::new(&custom_file_path_absolute),
            Path::new(&custom_file_path_relative),
        );
        if !current_custom_file_content.is_empty()
            && current_custom_file_content != custom_file_content
        {
            return ReadResult::CustomFileChanged {
                absolute: custom_file_path_absolute,
                relative: custom_file_path_relative,
                content: current_custom_file_content,
            };
        }
    }

    let mut non_default_strings = Vec::new();
    for _ in 0..non_default_values_count.max(0) {
        let key = read!(read_bytes(&mut stream));
        let value = read!(read_bytes(&mut stream));
        non_default_strings.push((key, value));
    }

    // From here on `_base` is already reset in the original.
    let failed_after_reset = || {
        tg_base::log!("{READ_ERROR}");
        ReadResult::Failed { base_reset: true }
    };
    let mut base = Vec::new();
    if legacy_format {
        if !stream.at_end() {
            let Some(read) = read_string(&mut stream) else {
                return failed_after_reset();
            };
            plural_id = read;
        } else {
            plural_id = id.clone();
        }
        if !stream.at_end() {
            let Some(read) = read_bytes(&mut stream) else {
                return failed_after_reset();
            };
            base = read;
            if base.is_empty() {
                return failed_after_reset();
            }
        }
    } else {
        // The status is not checked after this read in the original: a
        // missing base is an empty one.
        base = read_bytes(&mut stream).unwrap_or_default();
    }
    ReadResult::Ready(Box::new(Serialized {
        id,
        plural_id,
        name,
        native_name,
        version,
        custom_file_path_absolute,
        custom_file_path_relative,
        custom_file_content,
        non_default_strings,
        base,
    }))
}

struct State {
    me: PackState,
    /// `_values` of the main instance.
    values: Vec<String>,
    /// `_base`.
    base: Option<Box<PackState>>,
    choosing_sticker_replacement: ChoosingStickerReplacement,
    system_language: String,
}

fn prepare_default_values() -> Vec<String> {
    (0..KEYS_COUNT).map(get_original_value).collect()
}

impl State {
    fn new() -> Self {
        Self {
            me: PackState::new(),
            values: prepare_default_values(),
            base: None,
            choosing_sticker_replacement: ChoosingStickerReplacement::default(),
            system_language: String::new(),
        }
    }

    /// `setBaseId`.
    fn set_base_id(&mut self, base_id: &str, plural_id: &str) {
        if base_id.is_empty() {
            self.base = None;
            return;
        }
        // `_base->switchToId({ baseId, pluralId })`: a full reset of the
        // base pack. It has no values of its own, so nothing else changes.
        let mut base = PackState::new();
        base.id = language_id_or_default(base_id);
        base.plural_id = if !plural_id.is_empty() {
            plural_id.to_owned()
        } else {
            base_id.to_owned()
        };
        lang_tag::update_plural_rules(base.ensure_plural_id());
        self.base = Some(Box::new(base));
    }

    /// `reset`, without the `_idChanges` notification.
    fn reset(&mut self, data: &Language) {
        let computed_plural_id = if !data.plural_id.is_empty() {
            data.plural_id.clone()
        } else if !data.base_id.is_empty() {
            data.base_id.clone()
        } else {
            data.id.clone()
        };
        self.set_base_id(&data.base_id, &computed_plural_id);
        self.me.id = language_id_or_default(&data.id);
        self.me.plural_id = computed_plural_id;
        self.me.name = data.name.clone();
        self.me.native_name = data.native_name.clone();

        self.me.custom_file_path_absolute = String::new();
        self.me.custom_file_path_relative = String::new();
        self.me.custom_file_content = Vec::new();
        self.me.version = 0;
        self.me.non_default_values.clear();
        for (index, value) in self.values.iter_mut().enumerate() {
            *value = get_original_value(index as u16);
        }
        self.me.non_default_set.fill(false);
        self.update_choosing_sticker_replacement();
    }

    fn pack(&self, pack: Pack) -> Option<&PackState> {
        match pack {
            Pack::Base => self.base.as_deref(),
            _ => Some(&self.me),
        }
    }

    /// `applyValue` of the current pack (`base == false`) or of the base.
    fn apply_value(&mut self, base: bool, key: &[u8], value: &[u8]) {
        let pack = if base {
            self.base.as_deref_mut().expect("base pack")
        } else {
            &mut self.me
        };
        pack.non_default_values.insert(key.to_vec(), value.to_vec());
        let mut saved = None;
        parse_key_value(key, value, |index, parsed| saved = Some((index, parsed)));
        let Some((index, parsed)) = saved else {
            return;
        };
        pack.non_default_set[usize::from(index)] = true;
        if !base || !self.me.non_default_set[usize::from(index)] {
            self.values[usize::from(index)] = parsed;
        }
        if index == tr::lng_send_action_choose_sticker.base
            || index == tr::lng_user_action_choose_sticker.base
        {
            self.update_choosing_sticker_replacement();
        }
    }

    /// `resetValue` of the current pack (`base == false`) or of the base.
    fn reset_value(&mut self, base: bool, key: &[u8]) {
        let pack = if base {
            self.base.as_deref_mut().expect("base pack")
        } else {
            &mut self.me
        };
        pack.non_default_values.remove(key);

        let key_index = get_key_index(key);
        if key_index == KEYS_COUNT {
            return;
        }
        pack.non_default_set[usize::from(key_index)] = false;
        if !base {
            // The raw text of the base value, without tag parsing: so does
            // the original.
            let from_base = self
                .base
                .as_ref()
                .map_or(String::new(), |base| base.get_non_default_value(key));
            self.values[usize::from(key_index)] = if !from_base.is_empty() {
                from_base
            } else {
                get_original_value(key_index)
            };
        } else if !self.me.non_default_set[usize::from(key_index)] {
            self.values[usize::from(key_index)] = get_original_value(key_index);
        }
        if key_index == tr::lng_send_action_choose_sticker.base
            || key_index == tr::lng_user_action_choose_sticker.base
        {
            self.update_choosing_sticker_replacement();
        }
    }

    /// `updateChoosingStickerReplacement`.
    fn update_choosing_sticker_replacement(&mut self) {
        // A language changing in the runtime is not supported.
        let phrase = &self.values[usize::from(tr::lng_send_action_choose_sticker.base)];
        let first = utf16::index_of(phrase, CHOOSING_STICKER_REPLACEMENT, 0);
        let support = first != -1;
        let phrase_named = lang_tag::replace_tag(
            self.values[usize::from(tr::lng_user_action_choose_sticker.base)].clone(),
            lt_user,
            "",
        );
        let first_named = utf16::index_of(&phrase_named, CHOOSING_STICKER_REPLACEMENT, 0);
        let support_named = first_named != -1;

        self.choosing_sticker_replacement = ChoosingStickerReplacement {
            support: support_named && support,
            right_index: utf16::len(phrase) - first,
            right_index_named: utf16::len(&phrase_named) - first_named,
        };
    }

    /// `updatePluralRules` of the current pack.
    fn update_plural_rules(&mut self) {
        lang_tag::update_plural_rules(self.me.ensure_plural_id());
    }

    /// `loadFromContent`.
    fn load_from_content(&mut self, content: &[u8]) {
        let loader = FileParser::new(content);
        for (key, value) in &loader.pairs {
            self.apply_value(false, key, value);
        }
        if !loader.errors().is_empty() {
            tg_base::log!("Lang load errors: {}", loader.errors());
        } else if !loader.warnings().is_empty() {
            tg_base::log!("Lang load warnings: {}", loader.warnings());
        }
    }

    /// `loadFromCustomContent`.
    fn load_from_custom_content(
        &mut self,
        absolute_path: &str,
        relative_path: &str,
        content: Vec<u8>,
    ) {
        self.me.version = 0;
        self.me.custom_file_path_absolute = absolute_path.to_owned();
        self.me.custom_file_path_relative = relative_path.to_owned();
        self.me.custom_file_content = content.clone();
        self.load_from_content(&content);
    }

    /// `fillFromCustomContent`, without the `_idChanges` notification.
    fn fill_from_custom_content(
        &mut self,
        absolute_path: &str,
        relative_path: &str,
        content: Vec<u8>,
    ) {
        self.set_base_id("", "");
        self.me.id = custom_language_id();
        self.me.plural_id = plural_code_for_custom(absolute_path, relative_path);
        self.me.name = String::new();
        self.me.native_name = String::new();
        self.load_from_custom_content(absolute_path, relative_path, content);
        self.update_choosing_sticker_replacement();
    }

    /// The applying part of `fillFromSerialized` for the base pack.
    fn fill_base(&mut self, data: &[u8]) {
        let serialized = match read_serialized(data) {
            ReadResult::Failed { .. } => {
                // The base instance exists by now, empty.
                self.base = Some(Box::new(PackState::new()));
                return;
            }
            ReadResult::CustomFileChanged {
                absolute,
                relative,
                content,
            } => {
                // `fillFromCustomContent` on the base: its values go to the
                // derived pack through `applyValue`.
                let mut base = Box::new(PackState::new());
                base.id = custom_language_id();
                base.plural_id = plural_code_for_custom(&absolute, &relative);
                base.custom_file_path_absolute = absolute;
                base.custom_file_path_relative = relative;
                base.custom_file_content = content.clone();
                self.base = Some(base);
                let loader = FileParser::new(&content);
                for (key, value) in &loader.pairs {
                    self.apply_value(true, key, value);
                }
                return;
            }
            ReadResult::Ready(serialized) => serialized,
        };
        let mut base = Box::new(PackState::new());
        base.plural_id = if serialized.id == CUSTOM_LANGUAGE {
            plural_code_for_custom(
                &serialized.custom_file_path_absolute,
                &serialized.custom_file_path_relative,
            )
        } else {
            serialized.plural_id
        };
        base.id = serialized.id;
        base.name = serialized.name;
        base.native_name = serialized.native_name;
        base.version = serialized.version;
        base.custom_file_path_absolute = serialized.custom_file_path_absolute;
        base.custom_file_path_relative = serialized.custom_file_path_relative;
        base.custom_file_content = serialized.custom_file_content;
        self.base = Some(base);
        tg_base::log!(
            "Lang Info: Loaded cached, keys: {}",
            serialized.non_default_strings.len()
        );
        for (key, value) in &serialized.non_default_strings {
            self.apply_value(true, key, value);
        }
        let base = self.base.as_deref_mut().expect("base pack");
        lang_tag::update_plural_rules(base.ensure_plural_id());
    }
}

struct Inner {
    state: RefCell<State>,
    id_changes: EventStream<String>,
    updated: EventStream<()>,
    write_handler: RefCell<Option<Rc<dyn Fn()>>>,
}

/// `Lang::Instance`. A handle: clones refer to the same pack.
#[derive(Clone)]
pub struct Instance {
    inner: Rc<Inner>,
}

impl Default for Instance {
    fn default() -> Self {
        Self::new()
    }
}

impl Instance {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(Inner {
                state: RefCell::new(State::new()),
                id_changes: EventStream::new(),
                updated: EventStream::new(),
                write_handler: RefCell::new(None),
            }),
        }
    }

    /// What `Local::writeLangPack()` does; see the module docs.
    pub fn set_write_handler(&self, handler: Option<Rc<dyn Fn()>>) {
        *self.inner.write_handler.borrow_mut() = handler;
    }

    fn write_lang_pack(&self) {
        let handler = self.inner.write_handler.borrow().clone();
        if let Some(handler) = handler {
            handler();
        }
    }

    fn fire_id_changes(&self) {
        let id = self.inner.state.borrow().me.id.clone();
        self.inner.id_changes.fire(id);
    }

    /// `switchToId`.
    pub fn switch_to_id(&self, data: &Language) {
        self.inner.state.borrow_mut().reset(data);
        self.fire_id_changes();
        let test = {
            let mut state = self.inner.state.borrow_mut();
            let test = state.me.id == "#TEST_X" || state.me.id == "#TEST_0";
            if test {
                let filler = state.me.id.chars().nth(5).unwrap_or('X');
                for value in &mut state.values {
                    *value = prepare_test_value(value, filler);
                }
            }
            test
        };
        if test {
            self.inner.updated.fire(());
        }
        self.inner.state.borrow_mut().update_plural_rules();
    }

    /// `switchToCustomFile`.
    pub fn switch_to_custom_file(&self, file_path: &str) {
        if self.load_from_custom_file(file_path) {
            self.write_lang_pack();
            self.inner.updated.fire(());
        }
    }

    /// `loadFromCustomFile`.
    fn load_from_custom_file(&self, file_path: &str) -> bool {
        let absolute_path = std::path::absolute(file_path)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| file_path.to_owned());
        // `QDir().relativeFilePath(filePath)`: relative to the working
        // directory when the path is inside it.
        let relative_path = std::env::current_dir()
            .ok()
            .and_then(|current| {
                Path::new(&absolute_path)
                    .strip_prefix(&current)
                    .ok()
                    .map(|path| path.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| file_path.to_owned());
        let content = FileParser::read_file(Path::new(&absolute_path), Path::new(&relative_path));
        if content.is_empty() {
            return false;
        }
        let language = Language {
            id: custom_language_id(),
            plural_id: plural_code_for_custom(&absolute_path, &relative_path),
            ..Default::default()
        };
        self.inner.state.borrow_mut().reset(&language);
        self.fire_id_changes();
        let mut state = self.inner.state.borrow_mut();
        state.load_from_custom_content(&absolute_path, &relative_path, content);
        state.update_plural_rules();
        true
    }

    /// `systemLangCode`.
    pub fn system_lang_code(&self) -> String {
        let mut state = self.inner.state.borrow_mut();
        if state.system_language.is_empty() {
            state.system_language = system_language();
            if state.system_language.is_empty() {
                state.system_language = default_language_id();
            }
        }
        state.system_language.clone()
    }

    /// `cloudLangCode`.
    pub fn cloud_lang_code(&self, pack: Pack) -> String {
        if self.is_custom() || self.id().is_empty() {
            default_language_id()
        } else {
            self.id_of(pack)
        }
    }

    /// `id()`.
    pub fn id(&self) -> String {
        self.id_of(Pack::Current)
    }

    /// `idChanges`.
    pub fn id_changes(&self) -> Producer<String> {
        self.inner.id_changes.events()
    }

    /// `baseId`.
    pub fn base_id(&self) -> String {
        self.id_of(Pack::Base)
    }

    /// `name`.
    pub fn name(&self) -> String {
        let name = self.inner.state.borrow().me.name.clone();
        if name.is_empty() {
            self.get_value(tr::lng_language_name.base)
        } else {
            name
        }
    }

    /// `nativeName`.
    pub fn native_name(&self) -> String {
        let name = self.inner.state.borrow().me.native_name.clone();
        if name.is_empty() {
            self.get_value(tr::lng_language_name.base)
        } else {
            name
        }
    }

    /// `id(Pack)`.
    pub fn id_of(&self, pack: Pack) -> String {
        self.inner
            .state
            .borrow()
            .pack(pack)
            .map_or(String::new(), |pack| pack.id.clone())
    }

    /// `isCustom`.
    pub fn is_custom(&self) -> bool {
        self.inner.state.borrow().me.is_custom()
    }

    /// `version`.
    pub fn version(&self, pack: Pack) -> i32 {
        self.inner
            .state
            .borrow()
            .pack(pack)
            .map_or(0, |pack| pack.version)
    }

    /// `langPackName`.
    pub fn lang_pack_name(&self) -> String {
        if self.is_custom() {
            String::new()
        } else {
            cloud_lang_pack_name()
        }
    }

    /// `serialize`.
    pub fn serialize(&self) -> Vec<u8> {
        let state = self.inner.state.borrow();
        let base = state
            .base
            .as_ref()
            .map_or(Vec::new(), |base| base.serialize(&[]));
        state.me.serialize(&base)
    }

    /// `fillFromSerialized`. `data_app_version` is not used by the original
    /// either.
    pub fn fill_from_serialized(&self, data: &[u8], _data_app_version: i32) {
        let serialized = match read_serialized(data) {
            ReadResult::Failed { base_reset } => {
                if base_reset {
                    self.inner.state.borrow_mut().base = None;
                }
                return;
            }
            ReadResult::CustomFileChanged {
                absolute,
                relative,
                content,
            } => {
                self.inner
                    .state
                    .borrow_mut()
                    .fill_from_custom_content(&absolute, &relative, content);
                self.fire_id_changes();
                self.write_lang_pack();
                return;
            }
            ReadResult::Ready(serialized) => serialized,
        };
        {
            let mut state = self.inner.state.borrow_mut();
            state.base = None;
            if !serialized.base.is_empty() {
                state.fill_base(&serialized.base);
            }

            state.me.plural_id = if serialized.id == CUSTOM_LANGUAGE {
                plural_code_for_custom(
                    &serialized.custom_file_path_absolute,
                    &serialized.custom_file_path_relative,
                )
            } else {
                serialized.plural_id
            };
            state.me.id = serialized.id;
            state.me.name = serialized.name;
            state.me.native_name = serialized.native_name;
            state.me.version = serialized.version;
            state.me.custom_file_path_absolute = serialized.custom_file_path_absolute;
            state.me.custom_file_path_relative = serialized.custom_file_path_relative;
            state.me.custom_file_content = serialized.custom_file_content;
            tg_base::log!(
                "Lang Info: Loaded cached, keys: {}",
                serialized.non_default_strings.len()
            );
            for (key, value) in &serialized.non_default_strings {
                state.apply_value(false, key, value);
            }
            state.update_plural_rules();
            state.update_choosing_sticker_replacement();
        }
        self.fire_id_changes();
    }

    /// `supportChoosingStickerReplacement`.
    pub fn support_choosing_sticker_replacement(&self) -> bool {
        self.inner
            .state
            .borrow()
            .choosing_sticker_replacement
            .support
    }

    /// `rightIndexChoosingStickerReplacement`.
    pub fn right_index_choosing_sticker_replacement(&self, named: bool) -> i32 {
        let replacement = self.inner.state.borrow().choosing_sticker_replacement;
        if named {
            replacement.right_index_named
        } else {
            replacement.right_index
        }
    }

    /// `applyDifference`.
    pub fn apply_difference(&self, pack: Pack, difference: &LangPackDifferenceData) {
        let base = match pack {
            Pack::Current => false,
            Pack::Base => {
                tg_base::tg_assert!(self.inner.state.borrow().base.is_some());
                true
            }
            Pack::None => tg_base::unexpected!("Pack in Instance::applyDifference."),
        };
        {
            let mut state = self.inner.state.borrow_mut();
            {
                let target = if base {
                    state.base.as_deref_mut().expect("base pack")
                } else {
                    &mut state.me
                };
                tg_base::expects!(
                    language_id_or_default(&target.id).as_bytes()
                        == difference.lang_code.as_slice()
                );
                tg_base::expects!(difference.from_version <= target.version);
                target.version = difference.version;
            }
            for string in &difference.strings {
                // The callbacks need the state one at a time.
                let mut sets = Vec::new();
                let mut resets = Vec::new();
                handle_string(
                    string,
                    |key, value| sets.push((key.to_vec(), value.to_vec())),
                    |key| resets.push(key.to_vec()),
                );
                for (key, value) in sets {
                    state.apply_value(base, &key, &value);
                }
                for key in resets {
                    state.reset_value(base, &key);
                }
            }
        }
        self.inner.updated.fire(());
    }

    /// `ParseStrings`.
    pub fn parse_strings(strings: &[LangPackString]) -> BTreeMap<u16, String> {
        let result = RefCell::new(BTreeMap::new());
        for string in strings {
            handle_string(
                string,
                |key, value| {
                    parse_key_value(key, value, |index, parsed| {
                        result.borrow_mut().insert(index, parsed);
                    });
                },
                |key| {
                    let key_index = get_key_index(key);
                    if key_index != KEYS_COUNT {
                        result.borrow_mut().remove(&key_index);
                    }
                },
            );
        }
        result.into_inner()
    }

    /// `updated`.
    pub fn updated(&self) -> Producer<()> {
        self.inner.updated.events()
    }

    /// `getValue`.
    pub fn get_value(&self, key: u16) -> String {
        let state = self.inner.state.borrow();
        tg_base::expects!(usize::from(key) < state.values.len());
        state.values[usize::from(key)].clone()
    }

    /// `getNonDefaultValue`.
    pub fn get_non_default_value(&self, key: &[u8]) -> String {
        let state = self.inner.state.borrow();
        if state.me.non_default_values.contains_key(key) {
            state.me.get_non_default_value(key)
        } else if let Some(base) = &state.base {
            base.get_non_default_value(key)
        } else {
            String::new()
        }
    }

    /// `isNonDefaultPlural`.
    pub fn is_non_default_plural(&self, key: u16) -> bool {
        let state = self.inner.state.borrow();
        state.me.is_non_default_plural(key)
            || state
                .base
                .as_ref()
                .is_some_and(|base| base.is_non_default_plural(key))
    }

    /// `resetValue`.
    pub fn reset_value(&self, key: &[u8]) {
        self.inner.state.borrow_mut().reset_value(false, key);
    }

    /// `applyValue`.
    pub fn apply_value(&self, key: &[u8], value: &[u8]) {
        self.inner.state.borrow_mut().apply_value(false, key, value);
    }

    /// `updatePluralRules`.
    pub fn update_plural_rules(&self) {
        self.inner.state.borrow_mut().update_plural_rules();
    }
}

type SystemLanguageProvider = Rc<dyn Fn() -> String>;

thread_local! {
    static INSTANCE: Instance = Instance::new();
    static SYSTEM_LANGUAGE: RefCell<Option<SystemLanguageProvider>> = const { RefCell::new(None) };
}

/// Sets what `Platform::SystemLanguage()` (falling back to the first of
/// `QLocale::system().uiLanguages()`) returns on this thread.
pub fn set_system_language_provider(provider: Option<SystemLanguageProvider>) {
    SYSTEM_LANGUAGE.with(|current| *current.borrow_mut() = provider);
}

fn system_language() -> String {
    let provider = SYSTEM_LANGUAGE.with(|current| current.borrow().clone());
    provider.map_or(String::new(), |provider| provider())
}

/// `Lang::GetInstance()`: the language pack of this (the main) thread.
pub fn instance() -> Instance {
    INSTANCE.with(Instance::clone)
}

/// `Lang::Id`.
pub fn id() -> String {
    instance().id()
}

/// `Lang::Updated`.
pub fn updated() -> Producer<()> {
    instance().updated()
}

/// `Lang::GetNonDefaultValue`.
pub fn get_non_default_value(key: &[u8]) -> String {
    instance().get_non_default_value(key)
}

/// `Lang::details::Current`.
pub fn current(key: u16) -> String {
    instance().get_value(key)
}

/// `Lang::details::Value`.
pub fn value(key: u16) -> Producer<String> {
    tg_rpl::single(current(key)).then(updated().map(move |_| current(key)))
}

/// `Lang::details::IsNonDefaultPlural`.
pub fn is_non_default_plural(key_base: u16) -> bool {
    instance().is_non_default_plural(key_base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::Arc;
    use tg_rpl::Lifetime;
    use tg_scheme::{LangPackStringData, LangPackStringDeletedData, LangPackStringPluralizedData};

    fn marker(tag: u16) -> String {
        let mut result = String::new();
        result.push(TEXT_COMMAND);
        result.push(TEXT_COMMAND_LANG_TAG);
        result.push(char::from_u32(0x20 + u32::from(tag)).unwrap());
        result.push(TEXT_COMMAND);
        result
    }

    fn language(id: &str) -> Language {
        Language {
            id: id.into(),
            ..Default::default()
        }
    }

    fn simple(key: &str, value: &str) -> LangPackString {
        LangPackString::LangPackString(Arc::new(LangPackStringData {
            key: key.into(),
            value: value.into(),
        }))
    }

    fn deleted(key: &str) -> LangPackString {
        LangPackString::LangPackStringDeleted(Arc::new(LangPackStringDeletedData {
            key: key.into(),
        }))
    }

    fn pluralized(key: &str, one: Option<&str>, few: Option<&str>, other: &str) -> LangPackString {
        LangPackString::LangPackStringPluralized(Arc::new(LangPackStringPluralizedData {
            flags: Default::default(),
            key: key.into(),
            zero_value: None,
            one_value: one.map(Into::into),
            two_value: None,
            few_value: few.map(Into::into),
            many_value: None,
            other_value: other.into(),
        }))
    }

    fn difference(
        code: &str,
        from: i32,
        version: i32,
        strings: Vec<LangPackString>,
    ) -> LangPackDifferenceData {
        LangPackDifferenceData {
            lang_code: code.into(),
            from_version: from,
            version,
            strings,
        }
    }

    #[test]
    fn value_parser_replaces_tags_and_reports_errors() {
        let key = b"lng_action_user_joined";
        let index = get_key_index(key);
        assert_ne!(index, KEYS_COUNT);
        let tag = tr::lng_action_user_joined.tags[0];
        let name = generated_tag_name(tag);
        let parsed = parse_value(key, index, format!("a {{{name}}} б").as_bytes()).unwrap();
        assert_eq!(parsed, format!("a {} б", marker(tag)));
        assert_eq!(parse_value(key, index, b"plain").unwrap(), "plain");
        assert_eq!(parse_value(key, index, b"").unwrap(), "");
        // Errors.
        assert!(parse_value(key, index, b"a {} b").is_none());
        assert!(parse_value(key, index, format!("{{{name}").as_bytes()).is_none());
        assert!(parse_value(key, index, format!("{{{name} }}").as_bytes()).is_none());
        assert!(parse_value(key, index, b"{no_such_tag}").is_none());
        assert!(parse_value(key, index, b"{count}").is_none());
        assert!(parse_value(key, index, format!("{{{name}}}{{{name}}}").as_bytes()).is_none());
        // A closing brace alone is plain text.
        assert_eq!(parse_value(key, index, b"a } b").unwrap(), "a } b");
    }

    fn generated_tag_name(tag: u16) -> &'static str {
        crate::lang::TAGS[usize::from(tag)]
    }

    #[test]
    fn test_values_keep_spaces_and_markers() {
        let value = format!("ab c{}d\u{a0}😀", marker(7));
        assert_eq!(
            prepare_test_value(&value, 'X'),
            format!("XXX X{}X\u{a0}XX", marker(7))
        );
        assert_eq!(prepare_test_value("", '0'), "0");
    }

    #[test]
    fn plural_code_from_custom_file_name() {
        assert_eq!(plural_code_for_custom("/a/lang_ru_RU.strings", ""), "ru_RU");
        assert_eq!(plural_code_for_custom("", "x/lang_-de.strings"), "-de");
        assert_eq!(
            plural_code_for_custom("/a/lang_fil_PH.strings", "b_-en.x"),
            "fil_PH"
        );
        // The group is optional: `_.` matches with an empty capture.
        assert_eq!(plural_code_for_custom("/a/lang_.strings", ""), "");
        // No match at all: the default language.
        assert_eq!(plural_code_for_custom("/a/lang.strings", ""), "en");
        assert_eq!(plural_code_for_custom("/a_ru_RU.b/lang.strings", ""), "en");
        // The first place where the whole pattern matches wins.
        assert_eq!(plural_code_for_custom("/a/x_ru_RUS_-pt.strings", ""), "-pt");
    }

    #[test]
    fn switch_apply_and_reset_values() {
        let instance = Instance::new();
        let lifetime = Lifetime::new();
        let ids = Rc::new(RefCell::new(Vec::new()));
        let seen = ids.clone();
        instance
            .id_changes()
            .on_next(move |id| seen.borrow_mut().push(id), &lifetime);

        instance.switch_to_id(&language("ru"));
        assert_eq!(instance.id(), "ru");
        assert_eq!(instance.base_id(), "");
        assert_eq!(instance.cloud_lang_code(Pack::Current), "ru");
        assert_eq!(instance.lang_pack_name(), "tdesktop");
        assert_eq!(*ids.borrow(), vec!["ru".to_owned()]);
        assert_eq!(instance.name(), "English");

        let key = tr::lng_language_name.base;
        instance.apply_value(b"lng_language_name", "Русский".as_bytes());
        assert_eq!(instance.get_value(key), "Русский");
        assert_eq!(instance.name(), "Русский");
        assert_eq!(
            instance.get_non_default_value(b"lng_language_name"),
            "Русский"
        );
        // Unknown and cloud keys are remembered but change no value.
        instance.apply_value(b"cloud_x", b"y");
        instance.apply_value(b"what", b"y");
        assert_eq!(instance.get_non_default_value(b"cloud_x"), "y");
        instance.reset_value(b"lng_language_name");
        assert_eq!(instance.get_value(key), "English");
        assert_eq!(instance.get_non_default_value(b"lng_language_name"), "");

        // An empty id means the default language.
        instance.switch_to_id(&language(""));
        assert_eq!(instance.id(), "en");
        assert_eq!(instance.get_non_default_value(b"cloud_x"), "");
    }

    #[test]
    fn plural_forms_become_non_default_only_when_applied() {
        let instance = Instance::new();
        instance.switch_to_id(&language("ru"));
        let base = tr::lng_filter_unread_chats.base;
        assert!(!instance.is_non_default_plural(base));
        instance.apply_value(b"lng_filter_unread_chats#few", b"{count} x");
        assert!(instance.is_non_default_plural(base));
        assert_eq!(
            instance.get_value(base + 3),
            format!("{} x", marker(crate::lang::lt_count))
        );
        instance.reset_value(b"lng_filter_unread_chats#few");
        assert!(!instance.is_non_default_plural(base));
        assert_eq!(instance.get_value(base + 3), get_original_value(base + 3));
    }

    #[test]
    fn base_pack_values_are_overridden_by_the_current_pack() {
        let instance = Instance::new();
        instance.switch_to_id(&Language {
            id: "uk-raw".into(),
            base_id: "ru".into(),
            ..Default::default()
        });
        assert_eq!(instance.id(), "uk-raw");
        assert_eq!(instance.base_id(), "ru");
        assert_eq!(instance.cloud_lang_code(Pack::Base), "ru");
        let key = tr::lng_language_name.base;

        instance.apply_difference(
            Pack::Base,
            &difference("ru", 0, 7, vec![simple("lng_language_name", "base")]),
        );
        assert_eq!(instance.version(Pack::Base), 7);
        assert_eq!(instance.version(Pack::Current), 0);
        assert_eq!(instance.get_value(key), "base");
        assert_eq!(instance.get_non_default_value(b"lng_language_name"), "base");

        instance.apply_difference(
            Pack::Current,
            &difference("uk-raw", 0, 3, vec![simple("lng_language_name", "mine")]),
        );
        assert_eq!(instance.get_value(key), "mine");
        // A new base value does not replace the value of the current pack.
        instance.apply_difference(
            Pack::Base,
            &difference("ru", 7, 8, vec![simple("lng_language_name", "base2")]),
        );
        assert_eq!(instance.get_value(key), "mine");
        // Deleting the current value falls back to the base one.
        instance.apply_difference(
            Pack::Current,
            &difference("uk-raw", 3, 4, vec![deleted("lng_language_name")]),
        );
        assert_eq!(instance.get_value(key), "base2");
        // Deleting the base value falls back to the default.
        instance.apply_difference(
            Pack::Base,
            &difference("ru", 8, 9, vec![deleted("lng_language_name")]),
        );
        assert_eq!(instance.get_value(key), "English");
    }

    #[test]
    fn difference_fires_updated_and_handles_plurals() {
        let instance = Instance::new();
        instance.switch_to_id(&language("ru"));
        let lifetime = Lifetime::new();
        let fired = Rc::new(Cell::new(0));
        let counter = fired.clone();
        instance
            .updated()
            .on_next(move |_| counter.set(counter.get() + 1), &lifetime);
        let base = tr::lng_filter_unread_chats.base;
        instance.apply_difference(
            Pack::Current,
            &difference(
                "ru",
                0,
                5,
                vec![pluralized(
                    "lng_filter_unread_chats",
                    Some("one"),
                    Some("few"),
                    "other",
                )],
            ),
        );
        assert_eq!(fired.get(), 1);
        assert_eq!(instance.version(Pack::Current), 5);
        assert_eq!(instance.get_value(base + 1), "one");
        assert_eq!(instance.get_value(base + 3), "few");
        assert_eq!(instance.get_value(base + 5), "other");
        // Missing forms are applied as empty values.
        assert_eq!(instance.get_value(base), "");
        assert_eq!(instance.get_value(base + 2), "");

        instance.apply_difference(
            Pack::Current,
            &difference("ru", 5, 6, vec![deleted("lng_filter_unread_chats")]),
        );
        assert_eq!(fired.get(), 2);
        for shift in 0..6 {
            assert_eq!(
                instance.get_value(base + shift),
                get_original_value(base + shift)
            );
        }
    }

    #[test]
    fn parse_strings_keeps_the_last_state_of_each_key() {
        let key = tr::lng_language_name.base;
        let parsed = Instance::parse_strings(&[
            simple("lng_language_name", "a"),
            simple("lng_language_name", "b"),
            simple("unknown", "c"),
        ]);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[&key], "b");
        let parsed = Instance::parse_strings(&[
            simple("lng_language_name", "a"),
            deleted("lng_language_name"),
        ]);
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_languages_replace_every_value() {
        let instance = Instance::new();
        let lifetime = Lifetime::new();
        let fired = Rc::new(Cell::new(0));
        let counter = fired.clone();
        instance
            .updated()
            .on_next(move |_| counter.set(counter.get() + 1), &lifetime);
        instance.switch_to_id(&language("#TEST_X"));
        assert!(instance.is_custom());
        assert_eq!(instance.lang_pack_name(), "");
        assert_eq!(instance.cloud_lang_code(Pack::Current), "en");
        assert_eq!(instance.get_value(tr::lng_language_name.base), "XXXXXXXX");
        assert_eq!(fired.get(), 1);
        instance.switch_to_id(&language("#TEST_0"));
        assert_eq!(instance.get_value(tr::lng_language_name.base), "00000000");
    }

    #[test]
    fn serialization_matches_the_original_bytes() {
        let native = include_bytes!("../../tests/fixtures/core-base/lang-pack-native.bin");
        let instance = Instance::new();
        instance.fill_from_serialized(native, 0);
        assert_eq!(instance.id(), "ru");
        assert_eq!(instance.base_id(), "en");
        assert_eq!(instance.version(Pack::Current), 42);
        assert_eq!(instance.name(), "Russian");
        assert_eq!(instance.native_name(), "Русский");
        assert_eq!(instance.get_value(tr::lng_language_name.base), "Язык");
        assert_eq!(instance.get_non_default_value(b"cloud_test"), "a\0b");
        assert_eq!(instance.serialize(), native);
    }

    #[test]
    fn serialization_round_trip_and_errors() {
        let instance = Instance::new();
        instance.switch_to_id(&Language {
            id: "pt-br".into(),
            plural_id: "pt".into(),
            base_id: "es".into(),
            name: "Portuguese".into(),
            native_name: "Português".into(),
        });
        instance.apply_difference(
            Pack::Base,
            &difference("es", 0, 2, vec![simple("lng_language_name", "es")]),
        );
        instance.apply_difference(
            Pack::Current,
            &difference("pt-br", 0, 9, vec![simple("lng_cancel", "Cancelar")]),
        );
        let bytes = instance.serialize();

        let restored = Instance::new();
        restored.fill_from_serialized(&bytes, 0);
        assert_eq!(restored.id(), "pt-br");
        assert_eq!(restored.base_id(), "es");
        assert_eq!(restored.version(Pack::Current), 9);
        assert_eq!(restored.version(Pack::Base), 2);
        assert_eq!(restored.get_value(tr::lng_language_name.base), "es");
        assert_eq!(restored.get_value(tr::lng_cancel.base), "Cancelar");
        assert_eq!(restored.serialize(), bytes);

        // Truncated data and an unknown version leave the pack as it was.
        let untouched = Instance::new();
        untouched.switch_to_id(&language("de"));
        untouched.fill_from_serialized(&bytes[..bytes.len() / 2], 0);
        assert_eq!(untouched.id(), "de");
        let mut bad_version = bytes.clone();
        bad_version[15] = 2;
        untouched.fill_from_serialized(&bad_version, 0);
        assert_eq!(untouched.id(), "de");
        untouched.fill_from_serialized(&[], 0);
        assert_eq!(untouched.id(), "de");
    }

    fn legacy(id: &str, version: i32, values: &[(&str, &str)], tail: &[u8]) -> Vec<u8> {
        let mut stream = Writer::default();
        let units: Vec<u16> = id.encode_utf16().collect();
        stream.nullable_utf16(Some(&units));
        stream.i32(version);
        stream.nullable_utf16(None);
        stream.nullable_utf16(None);
        stream.nullable_bytes(None);
        stream.i32(values.len() as i32);
        for (key, value) in values {
            stream.bytes(key.as_bytes());
            stream.bytes(value.as_bytes());
        }
        stream.0.extend_from_slice(tail);
        stream.0
    }

    #[test]
    fn legacy_format_is_read() {
        let instance = Instance::new();
        instance.fill_from_serialized(&legacy("it", 4, &[("lng_cancel", "Annulla")], &[]), 0);
        assert_eq!(instance.id(), "it");
        assert_eq!(instance.version(Pack::Current), 4);
        assert_eq!(instance.base_id(), "");
        assert_eq!(instance.get_value(tr::lng_cancel.base), "Annulla");

        // With a plural id but an empty base the data is rejected.
        let mut tail = Writer::default();
        tail.nullable_utf16(Some(&"it".encode_utf16().collect::<Vec<_>>()));
        tail.nullable_bytes(Some(&[]));
        let other = Instance::new();
        other.fill_from_serialized(&legacy("it", 4, &[], &tail.0), 0);
        assert_eq!(other.id(), "");

        // A values count over the limit is rejected, a negative one is empty.
        let mut over = legacy("it", 4, &[], &[]);
        let count_at = over.len() - 4;
        over[count_at..].copy_from_slice(&20001i32.to_be_bytes());
        other.fill_from_serialized(&over, 0);
        assert_eq!(other.id(), "");
        over[count_at..].copy_from_slice(&(-5i32).to_be_bytes());
        other.fill_from_serialized(&over, 0);
        assert_eq!(other.id(), "it");
    }

    #[test]
    fn custom_file_is_loaded_and_reloaded_when_changed() {
        let dir = std::env::temp_dir().join(format!("tg-core-lang-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lang_ru_RU.strings");
        std::fs::write(&path, "\"lng_cancel\" = \"Отмена\";\n").unwrap();

        let instance = Instance::new();
        let written = Rc::new(Cell::new(0));
        let counter = written.clone();
        instance.set_write_handler(Some(Rc::new(move || counter.set(counter.get() + 1))));
        instance.switch_to_custom_file(path.to_str().unwrap());
        assert_eq!(written.get(), 1);
        assert_eq!(instance.id(), "#custom");
        assert!(instance.is_custom());
        assert_eq!(instance.get_value(tr::lng_cancel.base), "Отмена");
        let bytes = instance.serialize();

        // The same file: the cached values are used, nothing is rewritten.
        let cached = Instance::new();
        let rewritten = Rc::new(Cell::new(0));
        let counter = rewritten.clone();
        cached.set_write_handler(Some(Rc::new(move || counter.set(counter.get() + 1))));
        cached.fill_from_serialized(&bytes, 0);
        assert_eq!(cached.id(), "#custom");
        assert_eq!(cached.get_value(tr::lng_cancel.base), "Отмена");
        assert_eq!(rewritten.get(), 0);

        // The file changed: it is parsed again and the pack is rewritten.
        std::fs::write(&path, "\"lng_cancel\" = \"Нет\";\n").unwrap();
        cached.fill_from_serialized(&bytes, 0);
        assert_eq!(cached.get_value(tr::lng_cancel.base), "Нет");
        assert_eq!(rewritten.get(), 1);

        // A missing file is not an error for the cached pack.
        std::fs::remove_file(&path).unwrap();
        let offline = Instance::new();
        offline.fill_from_serialized(&bytes, 0);
        assert_eq!(offline.get_value(tr::lng_cancel.base), "Отмена");
        assert!(!Instance::new().load_from_custom_file(path.to_str().unwrap()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn choosing_sticker_replacement_indices() {
        let instance = Instance::new();
        instance.switch_to_id(&language("en"));
        // "choosing a sticker": "oo" is at 2 of 18.
        assert!(instance.support_choosing_sticker_replacement());
        assert_eq!(instance.right_index_choosing_sticker_replacement(false), 16);
        instance.apply_value(b"lng_send_action_choose_sticker", "выбирает".as_bytes());
        assert!(!instance.support_choosing_sticker_replacement());
        // Not found: `size - (-1)`, as in the original.
        assert_eq!(instance.right_index_choosing_sticker_replacement(false), 9);
    }

    #[test]
    fn system_language_and_thread_instance() {
        let instance = Instance::new();
        assert_eq!(instance.system_lang_code(), "en");
        set_system_language_provider(Some(Rc::new(|| "de-DE".to_owned())));
        // Cached after the first request.
        assert_eq!(instance.system_lang_code(), "en");
        assert_eq!(Instance::new().system_lang_code(), "de-DE");
        set_system_language_provider(None);

        super::instance().switch_to_id(&language("en"));
        assert_eq!(id(), "en");
        assert_eq!(current(tr::lng_language_name.base), "English");
        let lifetime = Lifetime::new();
        let values = Rc::new(RefCell::new(Vec::new()));
        let seen = values.clone();
        value(tr::lng_language_name.base).on_next(move |v| seen.borrow_mut().push(v), &lifetime);
        super::instance().apply_difference(
            Pack::Current,
            &difference("en", 0, 1, vec![simple("lng_language_name", "Eng")]),
        );
        assert_eq!(
            *values.borrow(),
            vec!["English".to_owned(), "Eng".to_owned()]
        );
        assert_eq!(get_non_default_value(b"lng_language_name"), "Eng");
        assert!(!is_non_default_plural(tr::lng_filter_unread_chats.base));
    }
}
