//! Port of the value types of `ui/text/text_entity.h`.
//!
//! Entity offsets and lengths are in UTF-16 code units, exactly as in the
//! original and on the wire; see [`super::utf16`]. Parsing (links, mentions,
//! markdown), `PrepareForSending`, tag (de)serialization and the mime helpers
//! of `text_entity.cpp` are not ported yet (stage 5).
use super::utf16;

/// `EntityType`. The numeric values are part of the local storage format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum EntityType {
    #[default]
    Invalid = 0,
    Url,
    CustomUrl,
    Email,
    Hashtag,
    Cashtag,
    Mention,
    MentionName,
    CustomEmoji,
    BotCommand,
    MediaTimestamp,
    /// Senders in chat list, attachments in chat list, etc.
    Colorized,
    Phone,
    BankCard,
    Bold,
    Semibold,
    Italic,
    Underline,
    StrikeOut,
    /// Inline.
    Code,
    /// Block.
    Pre,
    Blockquote,
    Spoiler,
    Subscript,
    Superscript,
    Marked,
    FormattedDate,
}

/// `FormattedDateFlag` values, combined in [`FormattedDateFlags`].
pub mod formatted_date_flag {
    pub const RELATIVE: u8 = 0x01;
    pub const SHORT_TIME: u8 = 0x02;
    pub const LONG_TIME: u8 = 0x04;
    pub const SHORT_DATE: u8 = 0x08;
    pub const LONG_DATE: u8 = 0x10;
    pub const DAY_OF_WEEK: u8 = 0x20;
}

/// `FormattedDateFlags` (`base::flags<FormattedDateFlag>`), the raw value.
pub type FormattedDateFlags = u8;

/// `SerializeFormattedDateData`.
pub fn serialize_formatted_date_data(date: i32, flags: FormattedDateFlags) -> String {
    format!("{date}:{flags}")
}

/// `DeserializeFormattedDateData`. Like `QString::toInt` / `toUInt`, a part
/// that does not parse gives zero; the flags value is truncated to 8 bits.
pub fn deserialize_formatted_date_data(data: &str) -> (i32, FormattedDateFlags) {
    let parts: Vec<&str> = data.split(':').collect();
    if parts.len() != 2 {
        return (0, 0);
    }
    let date = parts[0].trim().parse::<i32>().unwrap_or(0);
    let flags = parts[1].trim().parse::<u32>().unwrap_or(0) as u8;
    (date, flags)
}

/// `EntityLinkShown`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum EntityLinkShown {
    #[default]
    Full,
    Partial,
}

/// `EntityLinkData`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityLinkData {
    pub text: String,
    pub data: String,
    pub type_: EntityType,
    pub shown: EntityLinkShown,
}

/// `EntitiesInText`.
pub type EntitiesInText = Vec<EntityInText>;

/// `EntityInText`. Field order matches the original, so the derived ordering
/// equals the defaulted `operator<=>`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityInText {
    type_: EntityType,
    local: bool,
    offset: i32,
    length: i32,
    data: String,
}

impl EntityInText {
    pub fn new(type_: EntityType, offset: i32, length: i32) -> Self {
        Self::with_data(type_, offset, length, String::new())
    }

    pub fn with_data(type_: EntityType, offset: i32, length: i32, data: impl Into<String>) -> Self {
        Self {
            type_,
            local: false,
            offset,
            length,
            data: data.into(),
        }
    }

    pub fn type_(&self) -> EntityType {
        self.type_
    }

    pub fn offset(&self) -> i32 {
        self.offset
    }

    pub fn length(&self) -> i32 {
        self.length
    }

    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn valid_for_text(&self, text_length: i32) -> bool {
        self.offset >= 0
            && self.length > 0
            && self.offset <= text_length
            && self.length <= text_length - self.offset
    }

    pub fn set_local(&mut self) {
        self.local = true;
    }

    pub fn is_local(&self) -> bool {
        self.local
    }

    pub fn extend_to_left(&mut self, extent: i32) {
        self.offset -= extent;
        self.length += extent;
    }

    pub fn shrink_from_right(&mut self, shrink: i32) {
        self.length -= shrink;
    }

    pub fn shift_left(&mut self, shift: i32) {
        self.offset -= shift;
        if self.offset < 0 {
            self.length += self.offset;
            self.offset = 0;
            if self.length < 0 {
                self.length = 0;
            }
        }
    }

    pub fn shift_right(&mut self, shift: i32) {
        self.offset += shift;
    }

    pub fn update_text_end(&mut self, text_end: i32) {
        if self.offset > text_end {
            self.offset = text_end;
            self.length = 0;
        } else if self.offset + self.length > text_end {
            self.length = text_end - self.offset;
        }
    }

    /// `EntityInText::FirstMonospaceOffset`: the smallest offset among the
    /// valid `Pre` / `Code` entities, `text_length` when there are none.
    pub fn first_monospace_offset(entities: &[EntityInText], text_length: i32) -> i32 {
        entities
            .iter()
            .filter(|entity| {
                entity.valid_for_text(text_length)
                    && matches!(entity.type_, EntityType::Pre | EntityType::Code)
            })
            .map(|entity| entity.offset)
            .min()
            .unwrap_or(text_length)
    }

    /// `explicit operator bool`.
    pub fn is_valid(&self) -> bool {
        self.type_ != EntityType::Invalid
    }
}

/// `TextWithEntities`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextWithEntities {
    pub text: String,
    pub entities: EntitiesInText,
}

impl TextWithEntities {
    /// `TextWithEntities{ text }` / `TextWithEntities::Simple`.
    pub fn simple(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            entities: Vec::new(),
        }
    }

    pub fn empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn reserve(&mut self, size: usize, entities_count: usize) {
        self.text.reserve(size);
        self.entities.reserve(entities_count);
    }

    /// Both `append(TextWithEntities&&)` and `append(const TextWithEntities&)`.
    pub fn append(&mut self, other: TextWithEntities) -> &mut Self {
        let shift = utf16::len(&self.text);
        self.text.push_str(&other.text);
        self.entities.reserve(other.entities.len());
        for mut entity in other.entities {
            entity.shift_right(shift);
            self.entities.push(entity);
        }
        self
    }

    /// `append(const QString&)`, `append(QLatin1String)`.
    pub fn append_str(&mut self, other: &str) -> &mut Self {
        self.text.push_str(other);
        self
    }

    /// `append(QChar)`.
    pub fn append_char(&mut self, other: char) -> &mut Self {
        self.text.push(other);
        self
    }
}

impl From<String> for TextWithEntities {
    fn from(text: String) -> Self {
        Self::simple(text)
    }
}

impl From<&str> for TextWithEntities {
    fn from(text: &str) -> Self {
        Self::simple(text)
    }
}

/// `TextForMimeDataTag`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextForMimeDataTag {
    pub offset: i32,
    pub length: i32,
    pub id: String,
}

/// `TextForMimeData`. `WithExpandedLinks` lives in `text_entity.cpp` and is
/// not ported yet.
#[derive(Clone, Debug, Default)]
pub struct TextForMimeData {
    pub expanded: String,
    pub rich: TextWithEntities,
    pub tags: Vec<TextForMimeDataTag>,
}

impl TextForMimeData {
    pub fn empty(&self) -> bool {
        self.expanded.is_empty()
    }

    pub fn reserve(&mut self, size: usize, entities_count: usize) {
        self.expanded.reserve(size);
        self.rich.reserve(size, entities_count);
        self.tags.reserve(entities_count);
    }

    pub fn append(&mut self, other: TextForMimeData) -> &mut Self {
        let shift = utf16::len(&self.rich.text);
        self.expanded.push_str(&other.expanded);
        self.rich.append(other.rich);
        self.tags.reserve(other.tags.len());
        for mut tag in other.tags {
            tag.offset += shift;
            self.tags.push(tag);
        }
        self
    }

    pub fn append_rich(&mut self, other: TextWithEntities) -> &mut Self {
        self.expanded.push_str(&other.text);
        self.rich.append(other);
        self
    }

    pub fn append_str(&mut self, other: &str) -> &mut Self {
        self.expanded.push_str(other);
        self.rich.append_str(other);
        self
    }

    pub fn append_char(&mut self, other: char) -> &mut Self {
        self.expanded.push(other);
        self.rich.append_char(other);
        self
    }

    pub fn rich(rich: TextWithEntities) -> Self {
        Self {
            expanded: rich.text.clone(),
            rich,
            tags: Vec::new(),
        }
    }

    pub fn simple(simple: impl Into<String>) -> Self {
        let simple = simple.into();
        Self {
            expanded: simple.clone(),
            rich: TextWithEntities::simple(simple),
            tags: Vec::new(),
        }
    }
}

pub const TEXT_PARSE_MULTILINE: i32 = 0x001;
pub const TEXT_PARSE_LINKS: i32 = 0x002;
pub const TEXT_PARSE_MENTIONS: i32 = 0x004;
pub const TEXT_PARSE_HASHTAGS: i32 = 0x008;
pub const TEXT_PARSE_BOT_COMMANDS: i32 = 0x010;
pub const TEXT_PARSE_MARKDOWN: i32 = 0x020;
pub const TEXT_PARSE_COLORIZED: i32 = 0x040;

/// `TextWithTags::Tag`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextTag {
    pub offset: i32,
    pub length: i32,
    pub id: String,
}

/// `TextWithTags`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextWithTags {
    pub text: String,
    pub tags: Vec<TextTag>,
}

impl TextWithTags {
    pub fn empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold(offset: i32, length: i32) -> EntityInText {
        EntityInText::new(EntityType::Bold, offset, length)
    }

    #[test]
    fn entity_validity_and_shifts() {
        assert!(bold(0, 3).valid_for_text(3));
        assert!(!bold(0, 4).valid_for_text(3));
        assert!(!bold(-1, 2).valid_for_text(3));
        assert!(!bold(1, 0).valid_for_text(3));
        assert!(!bold(3, 0).valid_for_text(3));

        let mut entity = bold(2, 5);
        entity.shift_left(4);
        assert_eq!((entity.offset(), entity.length()), (0, 3));
        entity.shift_left(10);
        assert_eq!((entity.offset(), entity.length()), (0, 0));

        let mut entity = bold(2, 5);
        entity.extend_to_left(2);
        assert_eq!((entity.offset(), entity.length()), (0, 7));
        entity.shrink_from_right(3);
        assert_eq!(entity.length(), 4);
        entity.shift_right(6);
        assert_eq!(entity.offset(), 6);
        entity.update_text_end(8);
        assert_eq!((entity.offset(), entity.length()), (6, 2));
        entity.update_text_end(3);
        assert_eq!((entity.offset(), entity.length()), (3, 0));
    }

    #[test]
    fn ordering_follows_field_order_of_the_original() {
        let a = bold(5, 1);
        let b = EntityInText::new(EntityType::Italic, 0, 1);
        assert!(a < b);
        let mut local = bold(0, 1);
        local.set_local();
        assert!(bold(9, 9) < local);
        assert!(bold(1, 9) < bold(2, 1));
        assert!(!EntityInText::default().is_valid());
    }

    #[test]
    fn append_shifts_entities_by_utf16_length() {
        let mut text = TextWithEntities::simple("😀a");
        text.entities.push(bold(0, 2));
        let mut other = TextWithEntities::simple("bc");
        other.entities.push(bold(1, 1));
        text.append(other).append_str("!").append_char('?');
        assert_eq!(text.text, "😀abc!?");
        assert_eq!(text.entities, vec![bold(0, 2), bold(4, 1)]);
    }

    #[test]
    fn first_monospace_offset() {
        let entities = vec![
            bold(0, 2),
            EntityInText::new(EntityType::Pre, 7, 2),
            EntityInText::new(EntityType::Code, 4, 1),
        ];
        assert_eq!(EntityInText::first_monospace_offset(&entities, 20), 4);
        assert_eq!(EntityInText::first_monospace_offset(&entities[..1], 20), 20);
        // Entities that do not fit the text are ignored.
        assert_eq!(EntityInText::first_monospace_offset(&entities, 8), 4);
        assert_eq!(EntityInText::first_monospace_offset(&entities[..2], 8), 8);
    }

    #[test]
    fn formatted_date_data() {
        assert_eq!(
            serialize_formatted_date_data(1700000000, 0x21),
            "1700000000:33"
        );
        assert_eq!(
            deserialize_formatted_date_data("1700000000:33"),
            (1700000000, 0x21)
        );
        assert_eq!(deserialize_formatted_date_data("12"), (0, 0));
        assert_eq!(deserialize_formatted_date_data("1:2:3"), (0, 0));
        assert_eq!(deserialize_formatted_date_data("x:300"), (0, 44));
    }

    #[test]
    fn mime_data_append_shifts_tags() {
        let mut data = TextForMimeData::simple("ab");
        let mut other = TextForMimeData::rich(TextWithEntities::simple("cd"));
        other.tags.push(TextForMimeDataTag {
            offset: 1,
            length: 1,
            id: "x".into(),
        });
        data.append(other);
        assert_eq!(data.expanded, "abcd");
        assert_eq!(data.tags[0].offset, 3);
    }
}
