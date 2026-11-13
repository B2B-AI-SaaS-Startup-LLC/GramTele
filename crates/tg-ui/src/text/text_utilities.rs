//! Port of the value helpers of `ui/text/text_utilities.{h,cpp}`.
//!
//! Not ported yet (stage 5): `IconEmoji` / `TryMakeSimpleEmoji` (need
//! `style::IconEmoji` and the custom emoji painter) and `Words` (needs the
//! Unicode `\W` class of `QRegularExpression`).
use super::text_entity::{EntityInText, EntityType, TextWithEntities};
use super::utf16;

/// The default `url` argument of `Ui::Text::Link`.
pub const INTERNAL_ACTION: &str = "internal:action";

fn with_single_entity(text: &str, type_: EntityType, data: &str) -> TextWithEntities {
    TextWithEntities {
        text: text.to_owned(),
        entities: vec![EntityInText::with_data(type_, 0, utf16::len(text), data)],
    }
}

/// `"internal:index" + QChar(index)`.
fn index_link(index: i32) -> String {
    let mut result = String::from("internal:index");
    result.push(char::from_u32(index as u32).unwrap_or(char::REPLACEMENT_CHARACTER));
    result
}

fn colorized_entity_data(index: i32, background_index: i32) -> String {
    if index == 0 && background_index == 0 {
        return String::new();
    }
    let unit = |value: i32| char::from_u32(value as u32).unwrap_or(char::REPLACEMENT_CHARACTER);
    let mut result = String::new();
    result.push(unit(index));
    if background_index != 0 {
        result.push(unit(background_index));
    }
    result
}

pub fn bold(text: &str) -> TextWithEntities {
    with_single_entity(text, EntityType::Bold, "")
}

pub fn semibold(text: &str) -> TextWithEntities {
    with_single_entity(text, EntityType::Semibold, "")
}

pub fn italic(text: &str) -> TextWithEntities {
    with_single_entity(text, EntityType::Italic, "")
}

pub fn underline(text: &str) -> TextWithEntities {
    with_single_entity(text, EntityType::Underline, "")
}

pub fn strike_out(text: &str) -> TextWithEntities {
    with_single_entity(text, EntityType::StrikeOut, "")
}

pub fn code(text: &str) -> TextWithEntities {
    with_single_entity(text, EntityType::Code, "")
}

/// `Link(const QString &text, const QString &url)`.
pub fn link(text: &str, url: &str) -> TextWithEntities {
    with_single_entity(text, EntityType::CustomUrl, url)
}

/// `Link(const QString &text, int index)`.
pub fn link_index(text: &str, index: i32) -> TextWithEntities {
    link(text, &index_link(index))
}

/// `Link(TextWithEntities text, const QString &url)`.
pub fn link_rich(text: TextWithEntities, url: &str) -> TextWithEntities {
    wrapped(text, EntityType::CustomUrl, url)
}

/// `Link(TextWithEntities text, int index)`.
pub fn link_rich_index(text: TextWithEntities, index: i32) -> TextWithEntities {
    link_rich(text, &index_link(index))
}

/// `Colorized(const QString &text, int index, int backgroundIndex)`.
pub fn colorized(text: &str, index: i32, background_index: i32) -> TextWithEntities {
    with_single_entity(
        text,
        EntityType::Colorized,
        &colorized_entity_data(index, background_index),
    )
}

/// `Colorized(TextWithEntities text, int index, int backgroundIndex)`.
pub fn colorized_rich(
    text: TextWithEntities,
    index: i32,
    background_index: i32,
) -> TextWithEntities {
    wrapped(
        text,
        EntityType::Colorized,
        &colorized_entity_data(index, background_index),
    )
}

/// `Wrapped`: the new entity covers the whole text and goes first.
pub fn wrapped(mut text: TextWithEntities, type_: EntityType, data: &str) -> TextWithEntities {
    let length = utf16::len(&text.text);
    text.entities
        .insert(0, EntityInText::with_data(type_, 0, length, data));
    text
}

/// `RichLangValue`: `**bold**` and `__italic__` markup of language values.
///
/// As in the original, an opening marker without a closing one is dropped
/// together with the plain text in front of it.
pub fn rich_lang_value(text: &str) -> TextWithEntities {
    let mut result = TextWithEntities::default();
    let mut offset = 0;
    while offset < text.len() {
        let rest = &text[offset..];
        let found = match (rest.find("**"), rest.find("__")) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let Some(found) = found else {
            result.text.push_str(rest);
            break;
        };
        let position = offset + found;
        let from = position + 2;
        let tag = &text[position..from];
        // `text.indexOf(tag, from + 1)`: the body has at least one UTF-16 unit.
        let search_from = text[from..].chars().next().map(|ch| from + ch.len_utf8());
        let till = search_from.and_then(|start| text[start..].find(tag).map(|i| start + i));
        let Some(till) = till else {
            offset = from;
            continue;
        };
        if position > offset {
            result.text.push_str(&text[offset..position]);
        }
        let type_ = if tag == "__" {
            EntityType::Italic
        } else {
            EntityType::Bold
        };
        let body = &text[from..till];
        result.entities.push(EntityInText::new(
            type_,
            utf16::len(&result.text),
            utf16::len(body),
        ));
        result.text.push_str(body);
        offset = till + tag.len();
    }
    result
}

/// `WithEntities`.
pub fn with_entities(text: &str) -> TextWithEntities {
    TextWithEntities::simple(text)
}

/// `SingleCustomEmoji`; an empty `text` becomes `"@"`.
pub fn single_custom_emoji(data: &str, text: &str) -> TextWithEntities {
    let text = if text.is_empty() { "@" } else { text };
    TextWithEntities {
        text: text.to_owned(),
        entities: vec![EntityInText::with_data(
            EntityType::CustomEmoji,
            0,
            utf16::len(text),
            data,
        )],
    }
}

/// `Mid`; `n == -1` means "till the end".
pub fn mid(text: &TextWithEntities, position: i32, n: i32) -> TextWithEntities {
    let n = if n == -1 {
        utf16::len(&text.text) - position
    } else {
        n
    };
    let mid_end = position + n;
    let entities = text
        .entities
        .iter()
        .filter(|entity| {
            // Intersects of ranges.
            let l1 = entity.offset();
            let r1 = entity.offset() + entity.length() - 1;
            let l2 = position;
            let r2 = mid_end - 1;
            !(l1 > r2 || l2 > r1)
        })
        .map(|entity| {
            let start = entity.offset().max(position);
            let end = (entity.offset() + entity.length()).min(mid_end);
            EntityInText::with_data(entity.type_(), start - position, end - start, entity.data())
        })
        .collect();
    TextWithEntities {
        text: utf16::mid(&text.text, position, n),
        entities,
    }
}

/// `Filtered`: keeps only the entities of the given types.
pub fn filtered(text: &TextWithEntities, types: &[EntityType]) -> TextWithEntities {
    TextWithEntities {
        text: text.text.clone(),
        entities: text
            .entities
            .iter()
            .filter(|entity| types.contains(&entity.type_()))
            .cloned()
            .collect(),
    }
}

/// `FixAmpersandInAction`.
pub fn fix_ampersand_in_action(text: &str) -> String {
    text.replace('&', "&&")
}

/// `WrapEmailPattern`: the `*…*` part of a masked email becomes a spoiler.
pub fn wrap_email_pattern(pattern: &str) -> TextWithEntities {
    const HIDDEN: char = '*';
    let mut result = TextWithEntities::simple(pattern);
    if let (Some(from), Some(to)) = (pattern.find(HIDDEN), pattern.rfind(HIDDEN)) {
        let from = utf16::offset(pattern, from);
        let to = utf16::offset(pattern, to);
        result
            .entities
            .push(EntityInText::new(EntityType::Spoiler, from, to - from + 1));
    }
    result
}

/// `StripUrlProtocol`.
pub fn strip_url_protocol(link: &str) -> &str {
    link.strip_prefix("https://")
        .or_else(|| link.strip_prefix("http://"))
        .unwrap_or(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(type_: EntityType, offset: i32, length: i32) -> EntityInText {
        EntityInText::new(type_, offset, length)
    }

    #[test]
    fn single_entity_helpers_cover_the_utf16_length() {
        assert_eq!(bold("a😀").entities, vec![entity(EntityType::Bold, 0, 3)]);
        assert_eq!(italic("ab").entities[0].type_(), EntityType::Italic);
        assert_eq!(semibold("ab").entities[0].type_(), EntityType::Semibold);
        assert_eq!(underline("ab").entities[0].type_(), EntityType::Underline);
        assert_eq!(strike_out("ab").entities[0].type_(), EntityType::StrikeOut);
        assert_eq!(code("ab").entities[0].type_(), EntityType::Code);
        let linked = link("ab", INTERNAL_ACTION);
        assert_eq!(linked.entities[0].data(), "internal:action");
        assert_eq!(
            link_index("ab", 3).entities[0].data(),
            "internal:index\u{3}"
        );
        assert_eq!(colorized("ab", 0, 0).entities[0].data(), "");
        assert_eq!(colorized("ab", 2, 0).entities[0].data(), "\u{2}");
        assert_eq!(colorized("ab", 0, 5).entities[0].data(), "\u{0}\u{5}");
    }

    #[test]
    fn wrapped_puts_the_new_entity_first() {
        let text = wrapped(bold("ab"), EntityType::CustomUrl, "u");
        assert_eq!(text.entities[0].type_(), EntityType::CustomUrl);
        assert_eq!(text.entities[1].type_(), EntityType::Bold);
        assert_eq!(
            link_rich_index(bold("x"), 1).entities[0].data(),
            "internal:index\u{1}"
        );
        assert_eq!(
            colorized_rich(bold("x"), 1, 2).entities[0].data(),
            "\u{1}\u{2}"
        );
    }

    #[test]
    fn rich_lang_value_markup() {
        let rich = rich_lang_value("a **b** c __dd__ e");
        assert_eq!(rich.text, "a b c dd e");
        assert_eq!(
            rich.entities,
            vec![
                entity(EntityType::Bold, 2, 1),
                entity(EntityType::Italic, 6, 2)
            ]
        );
        assert_eq!(rich_lang_value("plain").text, "plain");
        assert!(rich_lang_value("").text.is_empty());
        // Offsets are UTF-16 units.
        let rich = rich_lang_value("😀**б**");
        assert_eq!(rich.entities, vec![entity(EntityType::Bold, 2, 1)]);
    }

    #[test]
    fn rich_lang_value_keeps_the_quirks_of_the_original() {
        // No closing marker: the marker and the text before it are dropped.
        assert_eq!(rich_lang_value("a ** b").text, " b");
        // An empty body is not a pair: the search starts one unit later.
        assert_eq!(rich_lang_value("****").text, "");
        let rich = rich_lang_value("**a****");
        assert_eq!(rich.text, "a");
        assert_eq!(rich.entities, vec![entity(EntityType::Bold, 0, 1)]);
        // Different markers do not close each other.
        assert_eq!(rich_lang_value("**a__").text, "");
    }

    #[test]
    fn mid_cuts_entities() {
        let mut text = TextWithEntities::simple("0123456789");
        text.entities = vec![
            entity(EntityType::Bold, 0, 3),
            entity(EntityType::Italic, 2, 5),
            entity(EntityType::Code, 8, 2),
        ];
        let cut = mid(&text, 2, 4);
        assert_eq!(cut.text, "2345");
        assert_eq!(
            cut.entities,
            vec![
                entity(EntityType::Bold, 0, 1),
                entity(EntityType::Italic, 0, 4)
            ]
        );
        let tail = mid(&text, 7, -1);
        assert_eq!(tail.text, "789");
        assert_eq!(tail.entities, vec![entity(EntityType::Code, 1, 2)]);
    }

    #[test]
    fn filtered_and_small_helpers() {
        let mut text = bold("ab");
        text.entities.push(entity(EntityType::Italic, 0, 1));
        assert_eq!(filtered(&text, &[EntityType::Italic]).entities.len(), 1);
        assert_eq!(fix_ampersand_in_action("a&b&"), "a&&b&&");
        assert_eq!(strip_url_protocol("https://t.me/x"), "t.me/x");
        assert_eq!(strip_url_protocol("http://t.me/x"), "t.me/x");
        assert_eq!(strip_url_protocol("tg://x"), "tg://x");
        assert_eq!(single_custom_emoji("d", "").text, "@");
        assert_eq!(single_custom_emoji("d", "😀").entities[0].length(), 2);
        assert_eq!(with_entities("x").text, "x");
    }

    #[test]
    fn email_pattern() {
        let wrapped = wrap_email_pattern("a***b@x.org");
        assert_eq!(wrapped.entities, vec![entity(EntityType::Spoiler, 1, 3)]);
        assert!(wrap_email_pattern("ab@x.org").entities.is_empty());
        assert_eq!(
            wrap_email_pattern("a*b").entities,
            vec![entity(EntityType::Spoiler, 1, 1)]
        );
    }
}
