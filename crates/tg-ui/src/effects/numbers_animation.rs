//! Port of the value type of `ui/effects/numbers_animation.h`. The
//! animation itself (`NumbersAnimation`, `LabelWithNumbers`) is stage 5.

/// `Ui::StringWithNumbers`: a text and the UTF-16 range of the number in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringWithNumbers {
    pub text: String,
    pub offset: i32,
    pub length: i32,
}

impl Default for StringWithNumbers {
    fn default() -> Self {
        Self {
            text: String::new(),
            offset: -1,
            length: 0,
        }
    }
}

impl StringWithNumbers {
    /// `StringWithNumbers::FromString`.
    pub fn from_string(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }
}
