//! Форматы строк SQLite оригинала; postAuthor/replyMarkupSerialized не сохраняются ORM.
use rusqlite::{
    types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef},
    ToSql,
};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SqlText(pub Vec<u8>);
impl From<&str> for SqlText {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().to_vec())
    }
}
impl From<String> for SqlText {
    fn from(s: String) -> Self {
        Self(s.into_bytes())
    }
}
impl ToSql for SqlText {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(&self.0)))
    }
}
impl FromSql for SqlText {
    fn column_result(v: ValueRef<'_>) -> FromSqlResult<Self> {
        let bytes = match v {
            ValueRef::Null => &[][..],
            _ => v.as_bytes()?,
        };
        Ok(Self(
            bytes.split(|b| *b == 0).next().unwrap_or_default().to_vec(),
        ))
    }
}
#[derive(Clone, Copy)]
pub enum SqlKind {
    I32,
    I64,
    Bool,
    Text,
    Blob,
    NullableI32,
    NullableI64,
}
pub trait Record: Sized {
    const KINDS: &'static [SqlKind];
    const TABLE: &'static str;
    const COLUMNS: &'static [&'static str];
    const AUTO_ID: bool = false;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self>;
    fn values(&self) -> Vec<&dyn ToSql>;
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AyuMessageBase {
    pub fake_id: i64,
    pub user_id: i64,
    pub dialog_id: i64,
    pub grouped_id: i64,
    pub peer_id: i64,
    pub from_id: i64,
    pub topic_id: i64,
    pub message_id: i32,
    pub date: i32,
    pub flags: i32,
    pub edit_date: i32,
    pub views: i32,
    pub fwd_flags: i32,
    pub fwd_from_id: i64,
    pub fwd_name: SqlText,
    pub fwd_date: i32,
    pub fwd_post_author: SqlText,
    pub post_author: SqlText,
    pub reply_flags: i32,
    pub reply_message_id: i32,
    pub reply_peer_id: i64,
    pub reply_top_id: i32,
    pub reply_forum_topic: bool,
    pub reply_serialized: Vec<u8>,
    pub reply_markup_serialized: Vec<u8>,
    pub entity_create_date: i32,
    pub text: SqlText,
    pub text_entities: Vec<u8>,
    pub media_path: SqlText,
    pub hq_thumb_path: SqlText,
    pub document_type: i32,
    pub document_serialized: Vec<u8>,
    pub thumbs_serialized: Vec<u8>,
    pub document_attributes_serialized: Vec<u8>,
    pub mime_type: SqlText,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeletedDialog {
    pub fake_id: i64,
    pub user_id: i64,
    pub dialog_id: i64,
    pub peer_id: i64,
    pub folder_id: Option<i32>,
    pub top_message: i32,
    pub last_message_date: i32,
    pub flags: i32,
    pub entity_create_date: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegexFilter {
    pub id: Vec<u8>,
    pub text: SqlText,
    pub enabled: bool,
    pub reversed: bool,
    pub case_insensitive: bool,
    pub dialog_id: Option<i64>,
}
#[derive(Clone, Debug, Default)]
pub struct RegexFilterGlobalExclusion {
    pub fake_id: i64,
    pub dialog_id: i64,
    pub filter_id: Vec<u8>,
}
impl PartialEq for RegexFilterGlobalExclusion {
    fn eq(&self, o: &Self) -> bool {
        self.dialog_id == o.dialog_id && self.filter_id == o.filter_id
    }
}
impl Eq for RegexFilterGlobalExclusion {}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpyMessageRead {
    pub fake_id: i64,
    pub user_id: i64,
    pub dialog_id: i64,
    pub message_id: i32,
    pub entity_create_date: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpyMessageContentsRead {
    pub fake_id: i64,
    pub user_id: i64,
    pub dialog_id: i64,
    pub message_id: i32,
    pub entity_create_date: i32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeletedMessage(pub AyuMessageBase);
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditedMessage(pub AyuMessageBase);
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchemaVersion {
    pub id: i32,
    pub version: i32,
}
impl Record for DeletedDialog {
    const KINDS: &'static [SqlKind] = &[
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::NullableI32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
    ];
    const TABLE: &'static str = "DeletedDialog";
    const COLUMNS: &'static [&'static str] = &[
        "fakeId",
        "userId",
        "dialogId",
        "peerId",
        "folderId",
        "topMessage",
        "lastMessageDate",
        "flags",
        "entityCreateDate",
    ];
    const AUTO_ID: bool = true;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            fake_id: row.get(0)?,
            user_id: row.get(1)?,
            dialog_id: row.get(2)?,
            peer_id: row.get(3)?,
            folder_id: row.get::<_, Option<i64>>(4)?.map(|n| n as i32),
            top_message: read_i32(row, 5)?,
            last_message_date: read_i32(row, 6)?,
            flags: read_i32(row, 7)?,
            entity_create_date: read_i32(row, 8)?,
        })
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.fake_id,
            &self.user_id,
            &self.dialog_id,
            &self.peer_id,
            &self.folder_id,
            &self.top_message,
            &self.last_message_date,
            &self.flags,
            &self.entity_create_date,
        ]
    }
}
impl Record for RegexFilter {
    const KINDS: &'static [SqlKind] = &[
        SqlKind::Blob,
        SqlKind::Text,
        SqlKind::Bool,
        SqlKind::Bool,
        SqlKind::Bool,
        SqlKind::NullableI64,
    ];
    const TABLE: &'static str = "RegexFilter";
    const COLUMNS: &'static [&'static str] = &[
        "id",
        "text",
        "enabled",
        "reversed",
        "caseInsensitive",
        "dialogId",
    ];
    const AUTO_ID: bool = false;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get::<_, Option<Vec<u8>>>(0)?.unwrap_or_default(),
            text: row.get(1)?,
            enabled: (read_i32(row, 2)? != 0),
            reversed: (read_i32(row, 3)? != 0),
            case_insensitive: (read_i32(row, 4)? != 0),
            dialog_id: row.get(5)?,
        })
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.id,
            &self.text,
            &self.enabled,
            &self.reversed,
            &self.case_insensitive,
            &self.dialog_id,
        ]
    }
}
impl Record for RegexFilterGlobalExclusion {
    const KINDS: &'static [SqlKind] = &[SqlKind::I64, SqlKind::I64, SqlKind::Blob];
    const TABLE: &'static str = "RegexFilterGlobalExclusion";
    const COLUMNS: &'static [&'static str] = &["fakeId", "dialogId", "filterId"];
    const AUTO_ID: bool = true;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            fake_id: row.get(0)?,
            dialog_id: row.get(1)?,
            filter_id: row.get::<_, Option<Vec<u8>>>(2)?.unwrap_or_default(),
        })
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![&self.fake_id, &self.dialog_id, &self.filter_id]
    }
}
impl Record for SpyMessageRead {
    const KINDS: &'static [SqlKind] = &[
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I32,
        SqlKind::I32,
    ];
    const TABLE: &'static str = "SpyMessageRead";
    const COLUMNS: &'static [&'static str] = &[
        "fakeId",
        "userId",
        "dialogId",
        "messageId",
        "entityCreateDate",
    ];
    const AUTO_ID: bool = true;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            fake_id: row.get(0)?,
            user_id: row.get(1)?,
            dialog_id: row.get(2)?,
            message_id: read_i32(row, 3)?,
            entity_create_date: read_i32(row, 4)?,
        })
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.fake_id,
            &self.user_id,
            &self.dialog_id,
            &self.message_id,
            &self.entity_create_date,
        ]
    }
}
impl Record for SpyMessageContentsRead {
    const KINDS: &'static [SqlKind] = &[
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I32,
        SqlKind::I32,
    ];
    const TABLE: &'static str = "SpyMessageContentsRead";
    const COLUMNS: &'static [&'static str] = &[
        "fakeId",
        "userId",
        "dialogId",
        "messageId",
        "entityCreateDate",
    ];
    const AUTO_ID: bool = true;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            fake_id: row.get(0)?,
            user_id: row.get(1)?,
            dialog_id: row.get(2)?,
            message_id: read_i32(row, 3)?,
            entity_create_date: read_i32(row, 4)?,
        })
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.fake_id,
            &self.user_id,
            &self.dialog_id,
            &self.message_id,
            &self.entity_create_date,
        ]
    }
}
impl Record for SchemaVersion {
    const KINDS: &'static [SqlKind] = &[SqlKind::I32, SqlKind::I32];
    const TABLE: &'static str = "SchemaVersion";
    const COLUMNS: &'static [&'static str] = &["id", "version"];
    const AUTO_ID: bool = false;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: read_i32(row, 0)?,
            version: read_i32(row, 1)?,
        })
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![&self.id, &self.version]
    }
}
impl Record for DeletedMessage {
    const KINDS: &'static [SqlKind] = &[
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I64,
        SqlKind::Text,
        SqlKind::I32,
        SqlKind::Text,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I64,
        SqlKind::I32,
        SqlKind::Bool,
        SqlKind::Blob,
        SqlKind::I32,
        SqlKind::Text,
        SqlKind::Blob,
        SqlKind::Text,
        SqlKind::Text,
        SqlKind::I32,
        SqlKind::Blob,
        SqlKind::Blob,
        SqlKind::Blob,
        SqlKind::Text,
    ];
    const TABLE: &'static str = "DeletedMessage";
    const COLUMNS: &'static [&'static str] = &[
        "fakeId",
        "userId",
        "dialogId",
        "groupedId",
        "peerId",
        "fromId",
        "topicId",
        "messageId",
        "date",
        "flags",
        "editDate",
        "views",
        "fwdFlags",
        "fwdFromId",
        "fwdName",
        "fwdDate",
        "fwdPostAuthor",
        "replyFlags",
        "replyMessageId",
        "replyPeerId",
        "replyTopId",
        "replyForumTopic",
        "replySerialized",
        "entityCreateDate",
        "text",
        "textEntities",
        "mediaPath",
        "hqThumbPath",
        "documentType",
        "documentSerialized",
        "thumbsSerialized",
        "documentAttributesSerialized",
        "mimeType",
    ];
    const AUTO_ID: bool = true;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self(AyuMessageBase {
            fake_id: row.get(0)?,
            user_id: row.get(1)?,
            dialog_id: row.get(2)?,
            grouped_id: row.get(3)?,
            peer_id: row.get(4)?,
            from_id: row.get(5)?,
            topic_id: row.get(6)?,
            message_id: read_i32(row, 7)?,
            date: read_i32(row, 8)?,
            flags: read_i32(row, 9)?,
            edit_date: read_i32(row, 10)?,
            views: read_i32(row, 11)?,
            fwd_flags: read_i32(row, 12)?,
            fwd_from_id: row.get(13)?,
            fwd_name: row.get(14)?,
            fwd_date: read_i32(row, 15)?,
            fwd_post_author: row.get(16)?,
            reply_flags: read_i32(row, 17)?,
            reply_message_id: read_i32(row, 18)?,
            reply_peer_id: row.get(19)?,
            reply_top_id: read_i32(row, 20)?,
            reply_forum_topic: (read_i32(row, 21)? != 0),
            reply_serialized: row.get::<_, Option<Vec<u8>>>(22)?.unwrap_or_default(),
            entity_create_date: read_i32(row, 23)?,
            text: row.get(24)?,
            text_entities: row.get::<_, Option<Vec<u8>>>(25)?.unwrap_or_default(),
            media_path: row.get(26)?,
            hq_thumb_path: row.get(27)?,
            document_type: read_i32(row, 28)?,
            document_serialized: row.get::<_, Option<Vec<u8>>>(29)?.unwrap_or_default(),
            thumbs_serialized: row.get::<_, Option<Vec<u8>>>(30)?.unwrap_or_default(),
            document_attributes_serialized: row.get::<_, Option<Vec<u8>>>(31)?.unwrap_or_default(),
            mime_type: row.get(32)?,
            ..Default::default()
        }))
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.0.fake_id,
            &self.0.user_id,
            &self.0.dialog_id,
            &self.0.grouped_id,
            &self.0.peer_id,
            &self.0.from_id,
            &self.0.topic_id,
            &self.0.message_id,
            &self.0.date,
            &self.0.flags,
            &self.0.edit_date,
            &self.0.views,
            &self.0.fwd_flags,
            &self.0.fwd_from_id,
            &self.0.fwd_name,
            &self.0.fwd_date,
            &self.0.fwd_post_author,
            &self.0.reply_flags,
            &self.0.reply_message_id,
            &self.0.reply_peer_id,
            &self.0.reply_top_id,
            &self.0.reply_forum_topic,
            &self.0.reply_serialized,
            &self.0.entity_create_date,
            &self.0.text,
            &self.0.text_entities,
            &self.0.media_path,
            &self.0.hq_thumb_path,
            &self.0.document_type,
            &self.0.document_serialized,
            &self.0.thumbs_serialized,
            &self.0.document_attributes_serialized,
            &self.0.mime_type,
        ]
    }
}
impl Record for EditedMessage {
    const KINDS: &'static [SqlKind] = &[
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I64,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I64,
        SqlKind::Text,
        SqlKind::I32,
        SqlKind::Text,
        SqlKind::I32,
        SqlKind::I32,
        SqlKind::I64,
        SqlKind::I32,
        SqlKind::Bool,
        SqlKind::Blob,
        SqlKind::I32,
        SqlKind::Text,
        SqlKind::Blob,
        SqlKind::Text,
        SqlKind::Text,
        SqlKind::I32,
        SqlKind::Blob,
        SqlKind::Blob,
        SqlKind::Blob,
        SqlKind::Text,
    ];
    const TABLE: &'static str = "EditedMessage";
    const COLUMNS: &'static [&'static str] = &[
        "fakeId",
        "userId",
        "dialogId",
        "groupedId",
        "peerId",
        "fromId",
        "topicId",
        "messageId",
        "date",
        "flags",
        "editDate",
        "views",
        "fwdFlags",
        "fwdFromId",
        "fwdName",
        "fwdDate",
        "fwdPostAuthor",
        "replyFlags",
        "replyMessageId",
        "replyPeerId",
        "replyTopId",
        "replyForumTopic",
        "replySerialized",
        "entityCreateDate",
        "text",
        "textEntities",
        "mediaPath",
        "hqThumbPath",
        "documentType",
        "documentSerialized",
        "thumbsSerialized",
        "documentAttributesSerialized",
        "mimeType",
    ];
    const AUTO_ID: bool = true;
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self(AyuMessageBase {
            fake_id: row.get(0)?,
            user_id: row.get(1)?,
            dialog_id: row.get(2)?,
            grouped_id: row.get(3)?,
            peer_id: row.get(4)?,
            from_id: row.get(5)?,
            topic_id: row.get(6)?,
            message_id: read_i32(row, 7)?,
            date: read_i32(row, 8)?,
            flags: read_i32(row, 9)?,
            edit_date: read_i32(row, 10)?,
            views: read_i32(row, 11)?,
            fwd_flags: read_i32(row, 12)?,
            fwd_from_id: row.get(13)?,
            fwd_name: row.get(14)?,
            fwd_date: read_i32(row, 15)?,
            fwd_post_author: row.get(16)?,
            reply_flags: read_i32(row, 17)?,
            reply_message_id: read_i32(row, 18)?,
            reply_peer_id: row.get(19)?,
            reply_top_id: read_i32(row, 20)?,
            reply_forum_topic: (read_i32(row, 21)? != 0),
            reply_serialized: row.get::<_, Option<Vec<u8>>>(22)?.unwrap_or_default(),
            entity_create_date: read_i32(row, 23)?,
            text: row.get(24)?,
            text_entities: row.get::<_, Option<Vec<u8>>>(25)?.unwrap_or_default(),
            media_path: row.get(26)?,
            hq_thumb_path: row.get(27)?,
            document_type: read_i32(row, 28)?,
            document_serialized: row.get::<_, Option<Vec<u8>>>(29)?.unwrap_or_default(),
            thumbs_serialized: row.get::<_, Option<Vec<u8>>>(30)?.unwrap_or_default(),
            document_attributes_serialized: row.get::<_, Option<Vec<u8>>>(31)?.unwrap_or_default(),
            mime_type: row.get(32)?,
            ..Default::default()
        }))
    }
    fn values(&self) -> Vec<&dyn ToSql> {
        vec![
            &self.0.fake_id,
            &self.0.user_id,
            &self.0.dialog_id,
            &self.0.grouped_id,
            &self.0.peer_id,
            &self.0.from_id,
            &self.0.topic_id,
            &self.0.message_id,
            &self.0.date,
            &self.0.flags,
            &self.0.edit_date,
            &self.0.views,
            &self.0.fwd_flags,
            &self.0.fwd_from_id,
            &self.0.fwd_name,
            &self.0.fwd_date,
            &self.0.fwd_post_author,
            &self.0.reply_flags,
            &self.0.reply_message_id,
            &self.0.reply_peer_id,
            &self.0.reply_top_id,
            &self.0.reply_forum_topic,
            &self.0.reply_serialized,
            &self.0.entity_create_date,
            &self.0.text,
            &self.0.text_entities,
            &self.0.media_path,
            &self.0.hq_thumb_path,
            &self.0.document_type,
            &self.0.document_serialized,
            &self.0.thumbs_serialized,
            &self.0.document_attributes_serialized,
            &self.0.mime_type,
        ]
    }
}
impl RegexFilter {
    pub fn to_json(&self) -> serde_json::Value {
        let end = self
            .id
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(self.id.len());
        let mut j = serde_json::json!({"id":String::from_utf8_lossy(&self.id[..end]),"text":String::from_utf8_lossy(&self.text.0),"enabled":self.enabled,"reversed":self.reversed,"caseInsensitive":self.case_insensitive});
        if let Some(id) = self.dialog_id {
            j["dialogId"] = id.into();
        }
        j
    }
}

fn read_i32(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<i32> {
    Ok(row.get::<_, i64>(index)? as i32)
}
