//! Извлечённые значения HistoryItem. Привязка к объектам ядра — этап 4.
use super::{ayu_database::AyuDatabase, entities::*};
/// Поля извлекает вызывающий код, без фиктивных HistoryItem/PeerData в порте.
#[derive(Default)]
pub struct MessageValues {
    pub user_id: u64,
    pub dialog_id: i64,
    pub grouped_id: i64,
    pub peer_id: u64,
    pub from_id: u64,
    pub topic_id: Option<i32>,
    pub message_id: i32,
    pub date: i32,
    pub flags: i32,
    pub edit_date: Option<i32>,
    pub views: i32,
    pub post_author: SqlText,
    pub text: SqlText,
    pub text_entities: Vec<u8>,
}
pub fn map(v: MessageValues, now: i32) -> AyuMessageBase {
    const CHAT_TYPE_MASK: u64 = (1_u64 << 48) - 1;
    AyuMessageBase {
        user_id: (v.user_id & CHAT_TYPE_MASK) as i64,
        dialog_id: v.dialog_id,
        grouped_id: v.grouped_id,
        peer_id: (v.peer_id & CHAT_TYPE_MASK) as i64,
        from_id: (v.from_id & CHAT_TYPE_MASK) as i64,
        topic_id: v.topic_id.unwrap_or(0) as i64,
        message_id: v.message_id,
        date: v.date,
        flags: v.flags,
        edit_date: v.edit_date.unwrap_or(now),
        views: v.views,
        post_author: v.post_author,
        entity_create_date: now,
        text: v.text,
        text_entities: v.text_entities,
        media_path: "/".into(),
        ..Default::default()
    }
}
pub fn add_edited_message(db: &AyuDatabase, v: MessageValues, now: i32) {
    let message = map(v, now);
    if !message.text.0.is_empty() {
        db.add_edited_message(&EditedMessage(message));
    }
}
pub fn add_deleted_message(db: &AyuDatabase, v: MessageValues, now: i32) {
    let message = map(v, now);
    if !message.text.0.is_empty() {
        db.add_deleted_message(&DeletedMessage(message));
    }
}
pub fn convert_to_base<T: Into<AyuMessageBase>>(messages: Vec<T>) -> Vec<AyuMessageBase> {
    messages.into_iter().map(Into::into).collect()
}
impl From<EditedMessage> for AyuMessageBase {
    fn from(m: EditedMessage) -> Self {
        m.0
    }
}
impl From<DeletedMessage> for AyuMessageBase {
    fn from(m: DeletedMessage) -> Self {
        m.0
    }
}
pub fn get_edited_messages(
    db: &AyuDatabase,
    user: i64,
    dialog: i64,
    message: i64,
    min: i64,
    max: i64,
    limit: i32,
) -> Vec<AyuMessageBase> {
    convert_to_base(db.get_edited_messages(user, dialog, message, min, max, limit))
}
pub fn has_revisions(db: &AyuDatabase, user: i64, dialog: i64, message: i64) -> bool {
    db.has_revisions(user, dialog, message)
}
#[allow(clippy::too_many_arguments)]
pub fn get_deleted_messages(
    db: &AyuDatabase,
    user: i64,
    dialog: i64,
    topic: i64,
    min: i64,
    max: i64,
    limit: i32,
    search: &str,
) -> Vec<AyuMessageBase> {
    convert_to_base(db.get_deleted_messages(user, dialog, topic, min, max, limit, search))
}
pub fn has_deleted_messages(db: &AyuDatabase, user: i64, dialog: i64, topic: i64) -> bool {
    db.has_deleted_messages(user, dialog, topic)
}
pub fn remove_deleted_message(db: &AyuDatabase, user: i64, dialog: i64, message: i64) {
    db.remove_deleted_message(user, dialog, message);
}
pub fn clear_deleted_messages(db: &AyuDatabase, user: i64, dialog: i64, topic: i64) {
    db.clear_deleted_messages(user, dialog, topic);
}
