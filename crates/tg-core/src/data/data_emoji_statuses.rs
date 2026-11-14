//! Value-only portion of data_emoji_statuses.h; service/automatic timers are effects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EmojiStatusCollectible {
    pub id: u64,
    pub document_id: u64,
    pub title: String,
    pub slug: String,
    pub pattern_document_id: u64,
    /// QColor as serialized RGBA, None for the invalid default QColor.
    pub center_color: Option<u32>,
    pub edge_color: Option<u32>,
    pub pattern_color: Option<u32>,
    pub text_color: Option<u32>,
}
impl EmojiStatusCollectible {
    pub fn is_present(&self) -> bool {
        self.id != 0
    }
}
