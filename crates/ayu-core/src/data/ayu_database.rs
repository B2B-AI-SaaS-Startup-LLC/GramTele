//! sqlite_orm заменён rusqlite. Все операции одного владельца сериализованы mutex.
use super::entities::*;
use rusqlite::{params, params_from_iter, Connection, ToSql};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
pub type SchemaEntry = (String, String, String, Option<String>);
#[derive(Clone)]
pub struct AyuDatabase {
    connection: Arc<Mutex<Connection>>,
    path: PathBuf,
}
impl AyuDatabase {
    pub fn initialize(path: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let path = path.as_ref().to_owned();
        let open = || -> rusqlite::Result<Connection> {
            let c = Connection::open(&path)?;
            c.busy_timeout(Duration::ZERO)?;
            Ok(c)
        };
        let db = Self {
            connection: Arc::new(Mutex::new(open()?)),
            path: path.clone(),
        };
        if db
            .sync_schema()
            .and_then(|()| db.run_migrations())
            .and_then(|()| db.sync_schema())
            .is_err()
        {
            drop(db);
            let now = tg_base::unixtime::now() as i64;
            Self::move_current_database(&path, now);
            let db = Self {
                connection: Arc::new(Mutex::new(open()?)),
                path: path.clone(),
            };
            db.sync_schema()?;
            db.connection
                .lock()
                .unwrap()
                .execute("INSERT INTO SchemaVersion(version) VALUES(0)", [])?;
            return Ok(db);
        }
        Ok(db)
    }
    fn sync_schema(&self) -> rusqlite::Result<()> {
        let c = self.connection.lock().unwrap();
        let desired = Connection::open_in_memory()?;
        desired.execute_batch(include_str!("schema.sql"))?;
        let statements = include_str!("schema.sql")
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        for sql in &statements {
            let name = sql.split('"').nth(1).unwrap();
            if sql.starts_with("CREATE INDEX") {
                continue;
            }
            let actual = column_info(&c, name)?;
            let expected = column_info(&desired, name)?;
            if actual.is_empty() {
                c.execute_batch(sql)?;
                continue;
            }
            // sqlite_orm intentionally ignores declared SQL type and column order.
            let unequal = expected.iter().any(|want| {
                actual
                    .iter()
                    .find(|got| got.name == want.name)
                    .is_some_and(|got| {
                        got.not_null != want.not_null
                            || got.has_default != want.has_default
                            || got.pk != want.pk
                            || got.generated != want.generated
                    })
            });
            let missing = expected
                .iter()
                .filter(|want| !actual.iter().any(|got| got.name == want.name))
                .collect::<Vec<_>>();
            let extra = actual
                .iter()
                .filter(|got| !expected.iter().any(|want| got.name == want.name))
                .collect::<Vec<_>>();
            let loses_rows = missing.iter().any(|col| col.not_null && !col.has_default);
            if loses_rows {
                c.execute_batch(&format!("DROP TABLE \"{name}\";{sql}"))?;
            } else if unequal || (!missing.is_empty() && !extra.is_empty()) {
                let mut backup = format!("{name}_backup");
                let mut suffix = 1;
                while !column_info(&c, &backup)?.is_empty() {
                    backup = format!("{name}_backup{suffix}");
                    suffix += 1;
                }
                c.execute_batch(&sql.replacen(
                    &format!("\"{name}\""),
                    &format!("\"{backup}\""),
                    1,
                ))?;
                let columns = expected
                    .iter()
                    .filter(|want| actual.iter().any(|got| got.name == want.name))
                    .map(|col| format!("\"{}\"", col.name))
                    .collect::<Vec<_>>()
                    .join(",");
                c.execute_batch(&format!("INSERT INTO \"{backup}\" ({columns}) SELECT {columns} FROM \"{name}\";DROP TABLE \"{name}\";ALTER TABLE \"{backup}\" RENAME TO \"{name}\";"))?;
            } else {
                for col in extra {
                    c.execute_batch(&format!(
                        "ALTER TABLE \"{name}\" DROP COLUMN \"{}\"",
                        col.name
                    ))?;
                }
                for col in missing {
                    let definition = sql
                        .split_once('(')
                        .unwrap()
                        .1
                        .trim_end_matches(')')
                        .split(", ")
                        .find(|s| s.starts_with(&format!("\"{}\"", col.name)))
                        .unwrap();
                    c.execute_batch(&format!("ALTER TABLE \"{name}\" ADD COLUMN {definition}"))?;
                }
            }
        }
        for sql in statements
            .into_iter()
            .filter(|s| s.starts_with("CREATE INDEX"))
        {
            c.execute_batch(&sql.replacen("CREATE INDEX", "CREATE INDEX IF NOT EXISTS", 1))?;
        }
        Ok(())
    }
    fn run_migrations(&self) -> rusqlite::Result<()> {
        let c = self.connection.lock().unwrap();
        let version = match c.query_row(
            "SELECT CAST(version AS INTEGER) FROM SchemaVersion WHERE id=1",
            [],
            |r| Ok(r.get::<_, i64>(0)? as i32),
        ) {
            Ok(version) => version,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                c.execute("INSERT INTO SchemaVersion(version) VALUES(0)", [])?;
                0
            }
            Err(error) => return Err(error),
        };
        if version < 1 {
            c.execute_batch("BEGIN; DROP TABLE IF EXISTS RegexFilter; UPDATE SchemaVersion SET version=1 WHERE id=1; COMMIT;")?;
        }
        Ok(())
    }
    pub fn move_current_database(path: &Path, unix_time: i64) {
        if let Some(parent) = path.parent() {
            for suffix in ["", "-shm", "-wal"] {
                let from = parent.join(format!("ayudata.db{suffix}"));
                let to = parent.join(format!("ayudata_{unix_time}.db{suffix}"));
                if !to.exists() {
                    let _ = std::fs::rename(from, to);
                }
            }
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Служебный типизированный доступ также покрывает таблицы без публичного API C++.
    pub fn insert<T: Record>(&self, value: &T) -> rusqlite::Result<i64> {
        let c = self.connection.lock().unwrap();
        let tx = c.unchecked_transaction()?;
        let start = usize::from(T::AUTO_ID || T::TABLE == "SchemaVersion");
        let columns = T::COLUMNS[start..]
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(",");
        let values = value.values();
        let placeholders = vec!["?"; values.len() - start].join(",");
        tx.execute(
            &format!(
                "INSERT INTO \"{}\" ({columns}) VALUES ({placeholders})",
                T::TABLE
            ),
            params_from_iter(values[start..].iter()),
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }
    pub fn all<T: Record>(&self) -> Vec<T> {
        self.select::<T>("", &[]).unwrap_or_default()
    }
    fn select<T: Record>(&self, tail: &str, p: &[&dyn ToSql]) -> rusqlite::Result<Vec<T>> {
        let c = self.connection.lock().unwrap();
        let cols = T::COLUMNS
            .iter()
            .zip(T::KINDS)
            .map(|(name, kind)| {
                let ty = match kind {
                    SqlKind::I32
                    | SqlKind::I64
                    | SqlKind::Bool
                    | SqlKind::NullableI32
                    | SqlKind::NullableI64 => "INTEGER",
                    SqlKind::Text => "TEXT",
                    SqlKind::Blob => "BLOB",
                };
                format!("CAST(\"{name}\" AS {ty})")
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut q = c.prepare(&format!("SELECT {cols} FROM \"{}\" {tail}", T::TABLE))?;
        let result = q.query_map(p, T::from_row)?.collect();
        result
    }
    fn execute(&self, sql: &str, p: &[&dyn ToSql]) {
        let result = self.connection.lock().unwrap().execute(sql, p);
        if let Err(error) = result {
            tg_base::log!("AyuDatabase SQL failed: {error}");
        }
    }
    fn exists(&self, sql: &str, p: &[&dyn ToSql]) -> bool {
        self.connection
            .lock()
            .unwrap()
            .query_row(sql, p, |_| Ok(true))
            .unwrap_or(false)
    }
    pub fn add_edited_message(&self, m: &EditedMessage) {
        if let Err(error) = self.insert(m) {
            tg_base::log!("Failed to save edited message for some reason: {error}");
        }
    }
    pub fn add_deleted_message(&self, m: &DeletedMessage) {
        if let Err(error) = self.insert(m) {
            tg_base::log!("Failed to save edited message for some reason: {error}");
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn get_edited_messages(
        &self,
        user: i64,
        dialog: i64,
        message: i64,
        min: i64,
        max: i64,
        limit: i32,
    ) -> Vec<EditedMessage> {
        self.select("WHERE userId=? AND dialogId=? AND messageId=? AND (fakeId>? OR ?=0) AND (fakeId<? OR ?=0) ORDER BY fakeId DESC LIMIT ?",&[&user,&dialog,&message,&min,&min,&max,&max,&limit]).unwrap_or_default()
    }
    pub fn has_revisions(&self, user: i64, dialog: i64, message: i64) -> bool {
        self.exists(
            "SELECT 1 FROM EditedMessage WHERE userId=? AND dialogId=? AND messageId=? LIMIT 1",
            &[&user, &dialog, &message],
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn get_deleted_messages(
        &self,
        user: i64,
        dialog: i64,
        topic: i64,
        min: i64,
        max: i64,
        limit: i32,
        search: &str,
    ) -> Vec<DeletedMessage> {
        let escaped = search
            .chars()
            .flat_map(|c| {
                if "%_\\".contains(c) {
                    vec!['\\', c]
                } else {
                    vec![c]
                }
            })
            .collect::<String>();
        let pattern = format!("%{escaped}%");
        self.select("WHERE userId=? AND dialogId=? AND (topicId=? OR ?=0) AND (messageId>? OR ?=0) AND (messageId<? OR ?=0) AND (?='' OR text LIKE ? ESCAPE '\\') ORDER BY messageId DESC LIMIT ?",&[&user,&dialog,&topic,&topic,&min,&min,&max,&max,&search,&pattern,&limit]).unwrap_or_default()
    }
    pub fn has_deleted_messages(&self, user: i64, dialog: i64, topic: i64) -> bool {
        self.exists("SELECT 1 FROM DeletedMessage WHERE userId=? AND dialogId=? AND (topicId=? OR ?=0) LIMIT 1",&[&user,&dialog,&topic,&topic])
    }
    pub fn remove_deleted_message(&self, user: i64, dialog: i64, message: i64) {
        self.execute(
            "DELETE FROM DeletedMessage WHERE userId=? AND dialogId=? AND messageId=?",
            &[&user, &dialog, &message],
        );
    }
    pub fn clear_deleted_messages(&self, user: i64, dialog: i64, topic: i64) {
        self.execute(
            "DELETE FROM DeletedMessage WHERE userId=? AND dialogId=? AND (topicId=? OR ?=0)",
            &[&user, &dialog, &topic, &topic],
        );
    }
    pub fn get_all_regex_filters(&self) -> Vec<RegexFilter> {
        self.all()
    }
    pub fn get_all_filters_exclusions(&self) -> Vec<RegexFilterGlobalExclusion> {
        self.all()
    }
    pub fn get_by_id(&self, id: &[u8]) -> RegexFilter {
        self.select("WHERE id=?", &[&id])
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default()
    }
    pub fn get_shared(&self) -> Vec<RegexFilter> {
        self.select("WHERE dialogId IS NULL", &[])
            .unwrap_or_default()
    }
    pub fn get_by_dialog_id(&self, id: i64) -> Vec<RegexFilter> {
        self.select("WHERE dialogId=?", &[&id]).unwrap_or_default()
    }
    pub fn get_excluded_by_dialog_id(&self, id: i64) -> Vec<RegexFilter> {
        self.select(
            "WHERE id IN (SELECT filterId FROM RegexFilterGlobalExclusion WHERE dialogId=?)",
            &[&id],
        )
        .unwrap_or_default()
    }
    pub fn get_count(&self) -> i32 {
        self.connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM RegexFilter", [], |r| {
                Ok(r.get::<_, i64>(0)? as i32)
            })
            .unwrap_or(0)
    }
    pub fn add_regex_filter(&self, f: &RegexFilter) {
        self.execute("REPLACE INTO RegexFilter(id,text,enabled,reversed,caseInsensitive,dialogId) VALUES(?,?,?,?,?,?)",&[&f.id,&f.text,&f.enabled,&f.reversed,&f.case_insensitive,&f.dialog_id]);
    }
    pub fn add_regex_exclusion(&self, e: &RegexFilterGlobalExclusion) {
        let _ = self.insert(e);
    }
    pub fn update_regex_filter(&self, f: &RegexFilter) {
        self.execute("UPDATE RegexFilter SET text=?,enabled=?,reversed=?,caseInsensitive=?,dialogId=? WHERE id=?",&[&f.text,&f.enabled,&f.reversed,&f.case_insensitive,&f.dialog_id,&f.id]);
    }
    pub fn delete_filter(&self, id: &[u8]) {
        self.execute("DELETE FROM RegexFilter WHERE id=?", &[&id]);
    }
    pub fn delete_exclusions_by_filter_id(&self, id: &[u8]) {
        self.execute(
            "DELETE FROM RegexFilterGlobalExclusion WHERE filterId=?",
            &[&id],
        );
    }
    pub fn delete_exclusion(&self, dialog: i64, id: &[u8]) {
        self.execute(
            "DELETE FROM RegexFilterGlobalExclusion WHERE dialogId=? AND filterId=?",
            &[&dialog, &id],
        );
    }
    pub fn delete_all_filters(&self) {
        self.execute("DELETE FROM RegexFilter", &[]);
    }
    pub fn delete_all_exclusions(&self) {
        self.execute("DELETE FROM RegexFilterGlobalExclusion", &[]);
    }
    pub fn has_filters(&self) -> bool {
        self.exists("SELECT 1 FROM RegexFilter LIMIT 1", &[])
    }
    pub fn has_per_dialog_filters(&self) -> bool {
        self.exists(
            "SELECT 1 FROM RegexFilter WHERE dialogId IS NOT NULL LIMIT 1",
            &[],
        ) || self.exists("SELECT 1 FROM RegexFilterGlobalExclusion LIMIT 1", &[])
    }
    pub fn schema(&self) -> rusqlite::Result<Vec<SchemaEntry>> {
        let c = self.connection.lock().unwrap();
        let mut q = c.prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY name")?;
        let result = q
            .query_map(params![], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect();
        result
    }
}

struct Column {
    name: String,
    not_null: bool,
    has_default: bool,
    pk: i32,
    generated: bool,
}
fn column_info(c: &Connection, table: &str) -> rusqlite::Result<Vec<Column>> {
    let mut q = c.prepare(&format!("PRAGMA table_xinfo(\"{table}\")"))?;
    let rows = q.query_map([], |r| {
        Ok(Column {
            name: r.get(1)?,
            not_null: r.get(3)?,
            has_default: r
                .get::<_, Option<String>>(4)?
                .is_some_and(|v| !v.is_empty()),
            pk: r.get(5)?,
            generated: r.get::<_, i32>(6)? != 0,
        })
    })?;
    rows.collect()
}
