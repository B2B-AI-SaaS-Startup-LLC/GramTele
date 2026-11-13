//! Port of `lang/lang_text_entity.{h,cpp}`: phrases as `TextWithEntities`.
use super::lang_tag::{self, TAG_REPLACEMENT_SIZE};
use super::lang_values::LangString;
use tg_ui::text::text_entity::{EntityInText, TextWithEntities};
use tg_ui::text::utf16;

impl LangString for TextWithEntities {
    fn start_replacements(lang_string: String) -> Self {
        TextWithEntities::simple(lang_string)
    }

    /// `ReplaceTag<TextWithEntities>::Call`.
    fn replace_tag(self, tag: u16, replacement: &Self) -> Self {
        match lang_tag::find_tag_replacement_position(&self.text, tag) {
            Some(position) => replace(self, replacement, position),
            None => self,
        }
    }
}

/// `ReplaceTag<TextWithEntities>::Replace`; `start` is a byte index of the
/// marker in `original.text`.
pub fn replace(
    original: TextWithEntities,
    replacement: &TextWithEntities,
    start: usize,
) -> TextWithEntities {
    let mut result = TextWithEntities {
        text: lang_tag::replace_at(&original.text, &replacement.text, start),
        entities: Vec::new(),
    };
    let original_entities_count = original.entities.len();
    let replacement_entities_count = replacement.entities.len();
    if original_entities_count == 0 && replacement_entities_count == 0 {
        return result;
    }
    // From here on everything is in UTF-16 units, as in the original.
    let start = utf16::offset(&original.text, start);
    let replacement_size = utf16::len(&replacement.text);
    let result_size = utf16::len(&result.text);
    result
        .entities
        .reserve(original_entities_count + replacement_entities_count);

    let replacement_end = start + replacement_size;
    let mut replacement_entity = replacement.entities.iter().peekable();
    let mut add_replacement_entities_until =
        |entities: &mut Vec<EntityInText>, until_position: i32| {
            while let Some(entity) = replacement_entity.peek() {
                let new_offset = start + entity.offset();
                if new_offset >= until_position {
                    return;
                }
                let new_end = new_offset + entity.length();
                let new_offset = new_offset.clamp(start, replacement_end);
                let new_end = new_end.clamp(start, replacement_end);
                let new_length = new_end - new_offset;
                if new_length != 0 {
                    entities.push(EntityInText::with_data(
                        entity.type_(),
                        new_offset,
                        new_length,
                        entity.data(),
                    ));
                }
                replacement_entity.next();
            }
        };

    for entity in &original.entities {
        // Transform the entity by the replacement.
        let mut offset = entity.offset();
        let mut end = offset + entity.length();
        if offset > start {
            offset = offset + replacement_size - TAG_REPLACEMENT_SIZE;
        }
        if end > start {
            end = end + replacement_size - TAG_REPLACEMENT_SIZE;
        }
        // `std::clamp(value, 0, size)`; the size is never negative.
        let offset = offset.clamp(0, result_size);
        let end = end.clamp(0, result_size);

        // Add all replacement entities that start before the current original entity.
        add_replacement_entities_until(&mut result.entities, offset);

        // Add a modified original entity.
        let length = end - offset;
        if length != 0 {
            result.entities.push(EntityInText::with_data(
                entity.type_(),
                offset,
                length,
                entity.data(),
            ));
        }
    }
    // Add the remaining replacement entities.
    add_replacement_entities_until(&mut result.entities, result_size);
    result
}

/// `tr::upper`, `tr::marked`, `tr::rich`, `tr::bold`, … from
/// `namespace tr::details`. Each one is used both as the projector of a
/// phrase call and as a wrapper of a single replacement value.
///
/// The C++ objects are overloaded for `QString` and `TextWithEntities`
/// arguments; here the `QString` form keeps the name and the
/// `TextWithEntities` form has the `_rich` suffix.
pub mod projections {
    use tg_ui::text::text_entity::{EntityType, TextWithEntities};
    use tg_ui::text::text_utilities as text;

    /// `tr::upper` for a `QString`: `QString::toUpper`.
    pub fn upper(value: String) -> String {
        value.to_uppercase()
    }

    /// `tr::upper` for a `TextWithEntities`.
    pub fn upper_rich(value: TextWithEntities) -> TextWithEntities {
        TextWithEntities {
            text: value.text.to_uppercase(),
            entities: value.entities,
        }
    }

    /// `tr::marked` for a `QString` (and `tr::marked()` for an empty one).
    pub fn marked(value: String) -> TextWithEntities {
        TextWithEntities::simple(value)
    }

    /// `tr::rich`.
    pub fn rich(value: String) -> TextWithEntities {
        text::rich_lang_value(&value)
    }

    /// `tr::bold` for a `QString`.
    pub fn bold(value: String) -> TextWithEntities {
        text::bold(&value)
    }

    /// `tr::bold` for a `TextWithEntities`.
    pub fn bold_rich(value: TextWithEntities) -> TextWithEntities {
        text::wrapped(value, EntityType::Bold, "")
    }

    /// `tr::semibold` for a `QString`.
    pub fn semibold(value: String) -> TextWithEntities {
        text::semibold(&value)
    }

    /// `tr::semibold` for a `TextWithEntities`.
    pub fn semibold_rich(value: TextWithEntities) -> TextWithEntities {
        text::wrapped(value, EntityType::Semibold, "")
    }

    /// `tr::italic` for a `QString`.
    pub fn italic(value: String) -> TextWithEntities {
        text::italic(&value)
    }

    /// `tr::italic` for a `TextWithEntities`.
    pub fn italic_rich(value: TextWithEntities) -> TextWithEntities {
        text::wrapped(value, EntityType::Italic, "")
    }

    /// `tr::underline` for a `QString`.
    pub fn underline(value: String) -> TextWithEntities {
        text::underline(&value)
    }

    /// `tr::underline` for a `TextWithEntities`.
    pub fn underline_rich(value: TextWithEntities) -> TextWithEntities {
        text::wrapped(value, EntityType::Underline, "")
    }

    /// `tr::strikeout` for a `QString`.
    pub fn strikeout(value: String) -> TextWithEntities {
        text::strike_out(&value)
    }

    /// `tr::strikeout` for a `TextWithEntities`.
    pub fn strikeout_rich(value: TextWithEntities) -> TextWithEntities {
        text::wrapped(value, EntityType::StrikeOut, "")
    }

    /// `tr::link(value)`: a link with the default `internal:action` url.
    pub fn link(value: String) -> TextWithEntities {
        text::link(&value, text::INTERNAL_ACTION)
    }

    /// `tr::link(value)` for a `TextWithEntities`.
    pub fn link_rich(value: TextWithEntities) -> TextWithEntities {
        text::link_rich(value, text::INTERNAL_ACTION)
    }

    /// `tr::url(url)`: a projection that makes the text a link to `url`.
    pub fn url(url: impl Into<String>) -> impl Fn(String) -> TextWithEntities + Clone {
        let url = url.into();
        move |value| text::link(&value, &url)
    }

    /// `tr::url(url)` applied to a `TextWithEntities`.
    pub fn url_rich(
        url: impl Into<String>,
    ) -> impl Fn(TextWithEntities) -> TextWithEntities + Clone {
        let url = url.into();
        move |value| text::link_rich(value, &url)
    }

    /// `tr::url(index)`: a link to `internal:index<index>`.
    pub fn url_index(index: i32) -> impl Fn(String) -> TextWithEntities + Clone {
        move |value| text::link_index(&value, index)
    }

    /// `tr::url(index)` applied to a `TextWithEntities`.
    pub fn url_index_rich(index: i32) -> impl Fn(TextWithEntities) -> TextWithEntities + Clone {
        move |value| text::link_rich_index(value, index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::lang_instance::{instance, Language};
    use crate::lang::lang_values::PhraseExt;
    use crate::lang::{lt_first_name, lt_last_name, tr};
    use tg_ui::text::text_entity::EntityType;

    fn marker(tag: u16) -> String {
        format!(
            "\u{10}\u{20}{}\u{10}",
            char::from_u32(0x20 + u32::from(tag)).unwrap()
        )
    }

    fn entity(type_: EntityType, offset: i32, length: i32) -> EntityInText {
        EntityInText::new(type_, offset, length)
    }

    #[test]
    fn replacement_shifts_and_merges_entities() {
        // "ab<tag>cd" with entities before, around and after the marker.
        let original = TextWithEntities {
            text: format!("ab{}cd", marker(1)),
            entities: vec![
                entity(EntityType::Bold, 0, 2),
                entity(EntityType::Italic, 1, 6),
                entity(EntityType::Code, 6, 2),
            ],
        };
        let replacement = TextWithEntities {
            text: "XYZ".into(),
            entities: vec![entity(EntityType::Underline, 1, 2)],
        };
        let result = original.replace_tag(1, &replacement);
        assert_eq!(result.text, "abXYZcd");
        assert_eq!(
            result.entities,
            vec![
                entity(EntityType::Bold, 0, 2),
                entity(EntityType::Italic, 1, 5),
                entity(EntityType::Underline, 3, 2),
                entity(EntityType::Code, 5, 2),
            ]
        );
    }

    #[test]
    fn replacement_without_entities_and_missing_tag() {
        let original = TextWithEntities::simple(format!("a{}", marker(2)));
        let result = original
            .clone()
            .replace_tag(2, &TextWithEntities::simple("😀"));
        assert_eq!(result.text, "a😀");
        assert!(result.entities.is_empty());
        assert_eq!(
            original
                .clone()
                .replace_tag(3, &TextWithEntities::simple("x")),
            original
        );
    }

    #[test]
    fn replacement_entities_are_clamped_to_the_replacement() {
        let original = TextWithEntities::simple(format!("😀{}!", marker(1)));
        let replacement = TextWithEntities {
            text: "xy".into(),
            entities: vec![
                entity(EntityType::Bold, 0, 5),
                entity(EntityType::Italic, 2, 3),
            ],
        };
        let result = original.replace_tag(1, &replacement);
        assert_eq!(result.text, "😀xy!");
        // Offsets are UTF-16 units: the emoji takes two. The entity that
        // starts at the end of the replacement is dropped (zero length).
        assert_eq!(result.entities, vec![entity(EntityType::Bold, 2, 2)]);
    }

    #[test]
    fn an_original_entity_that_covered_only_the_marker_is_dropped_when_empty() {
        let original = TextWithEntities {
            text: format!("a{}b", marker(1)),
            entities: vec![entity(EntityType::Bold, 1, 4)],
        };
        let result = original.replace_tag(1, &TextWithEntities::simple(""));
        assert_eq!(result.text, "ab");
        assert!(result.entities.is_empty());
    }

    #[test]
    fn projected_phrases() {
        instance().switch_to_id(&Language {
            id: "en".into(),
            ..Default::default()
        });
        let name = tr::lng_full_name
            .project(tr::marked)
            .tag(lt_first_name, tr::bold("A".into()))
            .tag(lt_last_name, tr::link("B".into()))
            .now();
        assert_eq!(name.text, "A B");
        assert_eq!(name.entities.len(), 2);
        assert_eq!(name.entities[0], entity(EntityType::Bold, 0, 1));
        assert_eq!(name.entities[1].type_(), EntityType::CustomUrl);
        assert_eq!(name.entities[1].offset(), 2);
        assert_eq!(name.entities[1].data(), "internal:action");

        let title = tr::lng_language_name.project(tr::bold).now();
        assert_eq!(title.entities, vec![entity(EntityType::Bold, 0, 7)]);
        assert_eq!(tr::lng_language_name.project(tr::upper).now(), "ENGLISH");
        let linked = tr::lng_language_name.project(tr::url("https://t.me")).now();
        assert_eq!(linked.entities[0].data(), "https://t.me");
    }

    #[test]
    fn projection_helpers() {
        use projections::*;
        let text = bold("ab".into());
        assert_eq!(
            italic_rich(text.clone()).entities[0].type_(),
            EntityType::Italic
        );
        assert_eq!(bold_rich(text.clone()).entities.len(), 2);
        assert_eq!(
            semibold("a".into()).entities[0].type_(),
            EntityType::Semibold
        );
        assert_eq!(
            semibold_rich(text.clone()).entities[0].type_(),
            EntityType::Semibold
        );
        assert_eq!(italic("a".into()).entities[0].type_(), EntityType::Italic);
        assert_eq!(
            underline("a".into()).entities[0].type_(),
            EntityType::Underline
        );
        assert_eq!(
            underline_rich(text.clone()).entities[0].type_(),
            EntityType::Underline
        );
        assert_eq!(
            strikeout("a".into()).entities[0].type_(),
            EntityType::StrikeOut
        );
        assert_eq!(
            strikeout_rich(text.clone()).entities[0].type_(),
            EntityType::StrikeOut
        );
        assert_eq!(
            link_rich(text.clone()).entities[0].data(),
            "internal:action"
        );
        assert_eq!(url_rich("u")(text.clone()).entities[0].data(), "u");
        assert_eq!(
            url_index(2)("a".into()).entities[0].data(),
            "internal:index\u{2}"
        );
        assert_eq!(
            url_index_rich(2)(text.clone()).entities[0].data(),
            "internal:index\u{2}"
        );
        assert_eq!(upper_rich(text).text, "AB");
        assert_eq!(rich("a **b**".into()).text, "a b");
        assert!(marked(String::new()).empty());
    }
}
