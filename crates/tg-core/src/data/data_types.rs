//! Scalar contracts needed by scoped stores, from data/data_types.h.
//! Remaining declarations are tracked for stage4 package 16, not substituted.
pub type FolderId = i32;
pub type PhotoId = u64;
pub type DocumentId = u64;

#[derive(Clone, Debug, Default)]
pub struct EmojiStatusId {
    pub document_id: DocumentId,
    pub collectible: Option<std::rc::Rc<super::data_emoji_statuses::EmojiStatusCollectible>>,
}
impl EmojiStatusId {
    pub fn is_present(&self) -> bool {
        self.document_id != 0 || self.collectible.is_some()
    }
}
impl PartialEq for EmojiStatusId {
    fn eq(&self, b: &Self) -> bool {
        self.document_id == b.document_id
            && match (&self.collectible, &b.collectible) {
                (None, None) => true,
                (Some(a), Some(b)) => std::rc::Rc::ptr_eq(a, b),
                _ => false,
            }
    }
}
impl Eq for EmojiStatusId {}

pub type CallId = u64;
