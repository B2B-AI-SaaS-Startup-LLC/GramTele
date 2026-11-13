//! Port of `lang/lang_numbers_animation.{h,cpp}`: a phrase as a text with
//! the position of its number, for the animated counters.
use super::lang_tag;
use super::lang_values::LangString;
use tg_ui::effects::numbers_animation::StringWithNumbers;
use tg_ui::text::utf16;

impl LangString for StringWithNumbers {
    fn start_replacements(lang_string: String) -> Self {
        StringWithNumbers::from_string(lang_string)
    }

    /// `ReplaceTag<Ui::StringWithNumbers>::Call`: the last replaced tag is
    /// the number. As in the original, a missing tag resets the offset.
    fn replace_tag(mut self, tag: u16, replacement: &Self) -> Self {
        let Some(position) = lang_tag::find_tag_replacement_position(&self.text, tag) else {
            self.offset = -1;
            return self;
        };
        self.offset = utf16::offset(&self.text, position);
        self.text = lang_tag::replace_at(&self.text, &replacement.text, position);
        self.length = utf16::len(&replacement.text);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::lang_instance::{instance, Language};
    use crate::lang::lang_values::PhraseExt;
    use crate::lang::tr;

    #[test]
    fn the_count_position_is_reported_in_utf16_units() {
        let marker = "\u{10}\u{20}\u{21}\u{10}";
        let original = StringWithNumbers::from_string(format!("😀 {marker} x"));
        let replaced = original.replace_tag(1, &StringWithNumbers::from_string("123"));
        assert_eq!(replaced.text, "😀 123 x");
        assert_eq!((replaced.offset, replaced.length), (3, 3));

        let missing = replaced
            .clone()
            .replace_tag(2, &StringWithNumbers::from_string("9"));
        assert_eq!(missing.text, replaced.text);
        assert_eq!((missing.offset, missing.length), (-1, 3));
    }

    #[test]
    fn plural_phrase_with_numbers() {
        instance().switch_to_id(&Language {
            id: "en".into(),
            ..Default::default()
        });
        let text = tr::lng_filter_unread_chats
            .project(StringWithNumbers::from_string)
            .count(25)
            .now();
        assert_eq!(text.length, 2);
        assert_eq!(utf16::mid(&text.text, text.offset, text.length), "25");
    }
}
