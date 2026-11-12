use ayu_core::{
    ayu_settings::*,
    data::{ayu_database::AyuDatabase, entities::*, messages_storage::*},
};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn temporary(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ayu-data-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn native_default_json_exact() {
    let settings = AyuSettings::default();
    assert_eq!(
        settings.json_bytes().unwrap(),
        include_bytes!("fixtures/settings-default.json")
    );
    assert_eq!(
        AYU_SETTINGS.len() + GHOST_MODE_ACCOUNT_SETTINGS.len() + MESSAGE_SHOT_SETTINGS.len(),
        115
    );
}
#[test]
fn native_all_changed_json_exact() {
    let mut s = AyuSettings::default();
    s.from_json(
        &serde_json::from_slice(include_bytes!("fixtures/settings-changed.json.input")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        s.json_bytes().unwrap(),
        include_bytes!("fixtures/settings-changed.json")
    );
    if let Some(out) = std::env::var_os("AYUGRAM_AYU_RUST_REFERENCE_DIR") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        write_json_file(&out.join("settings.json"), &s).unwrap();
    }
    let dir = temporary("json");
    write_json_file(&dir.join("settings.json"), &s).unwrap();
}
#[test]
fn every_setting_load_and_reactive_change() {
    for specs in [
        AYU_SETTINGS,
        GHOST_MODE_ACCOUNT_SETTINGS,
        MESSAGE_SHOT_SETTINGS,
    ] {
        for spec in specs {
            let s = Section::new(specs);
            let default: Value = serde_json::from_str(spec.default_json).unwrap();
            assert_eq!(s.get(spec.key), Some(default));
            let seen = std::rc::Rc::new(std::cell::Cell::new(0));
            let count = seen.clone();
            let lifetime = tg_rpl::Lifetime::new();
            if let Some(value) = s.value(spec.key) {
                value.on_next(move |_| count.set(count.get() + 1), &lifetime);
                assert_eq!(seen.get(), 1);
            } else {
                assert!(matches!(spec.kind, Kind::Ids));
            }
        }
    }
    let mut s = AyuSettings::default();
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = seen.clone();
    let life = tg_rpl::Lifetime::new();
    s.settings
        .changes("saveForBots")
        .unwrap()
        .on_next(move |_| count.set(count.get() + 1), &life);
    s.set("saveForBots", json!(true)).unwrap();
    s.set("saveForBots", json!(true)).unwrap();
    assert_eq!(seen.get(), 1);
}
#[test]
fn invalid_types_partial_assignment_enums_unknown_and_legacy() {
    let mut s = AyuSettings::default();
    assert!(s
        .from_json(&json!({"saveDeletedMessages":false,"saveMessagesHistory":"bad"}))
        .is_err());
    assert_eq!(s.settings.get("saveDeletedMessages"), Some(json!(false)));
    assert_eq!(s.settings.get("saveMessagesHistory"), Some(json!(true)));
    s.from_json(&json!({"unknown":123,"showPeerId":"wrong","translationProvider":false}))
        .unwrap();
    assert_eq!(s.settings.get("showPeerId"), Some(json!(0)));
    assert_eq!(
        s.settings.get("translationProvider"),
        Some(json!("telegram"))
    );
    assert!(s.to_json().get("unknown").is_none());
    s.from_json(&json!({"messageShotSettings":{"themeType":2,"themeAccentColor":123},"ghostModeSettings":{"-1tail":{"sendWithoutSound":true}}})).unwrap();
    assert_eq!(s.message_shot.get("embeddedThemeType"), Some(json!(2)));
    s.set("useGlobalGhostMode", json!(false)).unwrap();
    assert!(s.ghost(u64::MAX).should_send_without_sound());
    assert!(s
        .from_json(&json!({"messageShotSettings":{"embeddedThemeType":2,"themeType":"bad"}}))
        .is_err());
    let dir = temporary("load");
    let path = dir.join("settings.json");
    std::fs::write(&path, b"{broken").unwrap();
    let mut s = AyuSettings::with_path(&path);
    assert!(s.load(false, None).is_err());
    std::fs::write(&path, b"{\"sendReadMessages\":false} ignored suffix").unwrap();
    s.load(false, None).unwrap();
    assert_eq!(s.ghost(4).get("sendReadMessages"), Some(json!(false)));
    std::fs::write(&path, b"{}").unwrap();
    s.load(true, Some(4)).unwrap();
    assert!(s.ghost(4).ghost_mode_active());
    s.set("wideMultiplier", json!(99.)).unwrap();
    s.set("avatarCorners", json!(-1)).unwrap();
    assert!(s.validate().unwrap());
    assert_eq!(s.settings.get("wideMultiplier"), Some(json!(1.)));
    s.reset().unwrap();
    assert!(s.to_json()["ghostModeSettings"]
        .as_object()
        .unwrap()
        .is_empty());
}
#[test]
fn ghost_locks_reactive_state_accounts_and_themes() {
    let mut s = AyuSettings::default();
    s.set("useGlobalGhostMode", json!(false)).unwrap();
    s.set_ghost(7, "sendReadMessagesLocked", json!(true))
        .unwrap();
    let seen = std::rc::Rc::new(std::cell::RefCell::new(vec![]));
    let output = seen.clone();
    let life = tg_rpl::Lifetime::new();
    s.ghost(7)
        .ghost_mode_active_value()
        .on_next(move |v| output.borrow_mut().push(v), &life);
    assert_eq!(
        s.set_ghost_mode_enabled(7, true, Some(9)).unwrap(),
        vec![Effect::MarkAsOnline(9)]
    );
    assert!(s.ghost(7).ghost_mode_active());
    assert_eq!(s.ghost(7).get("sendReadMessages"), Some(json!(true)));
    assert!(!s.ghost(8).ghost_mode_active());
    s.set_ghost(7, "sendWithoutSound", json!(1)).unwrap();
    assert!(s.ghost(7).should_send_without_sound());
    s.set_ghost(7, "useScheduledMessages", json!(true)).unwrap();
    assert!(s.ghost(7).is_use_scheduled_messages());
    s.set_ghost_mode_enabled(7, false, None).unwrap();
    assert!(!s.ghost(7).should_send_without_sound());
    assert!(seen.borrow().contains(&true));
    assert!(s.set_cloud_theme(1, 2, 3, 4, "🌏").unwrap());
    assert!(!s.set_cloud_theme(1, 2, 3, 4, "🌏").unwrap());
    assert!(s.set_embedded_theme(2, 123).unwrap());
    assert!(s.message_shot.is_cloud_theme_empty());
    assert_eq!(s.message_shot.get("cloudThemeAccountId"), Some(json!(0)));
    assert!(!s.set_embedded_theme(2, 123).unwrap());
    assert!(s.clear_theme().unwrap());
    assert!(!s.clear_theme().unwrap());
    for (k, v, effect) in [
        (
            "materialSwitches",
            json!(false),
            Effect::MaterialSwitches(false),
        ),
        ("avatarCorners", json!(2), Effect::AvatarCorners(2)),
        (
            "hideNotificationBadge",
            json!(true),
            Effect::RefreshUnreadBadge,
        ),
        (
            "translationProvider",
            json!("google"),
            Effect::ResetTranslationCache,
        ),
        ("streamerMode", json!(true), Effect::StreamerMode(true)),
    ] {
        assert!(s.set(k, v).unwrap().contains(&effect));
    }
    assert_eq!(s.add_shadow_ban(4).unwrap(), vec![Effect::RebuildFilters]);
    assert!(s.add_shadow_ban(4).unwrap().is_empty());
    assert_eq!(
        s.remove_shadow_ban(4).unwrap(),
        vec![Effect::RebuildFilters]
    );
    assert!(s.remove_shadow_ban(4).unwrap().is_empty());
}
fn database(label: &str) -> AyuDatabase {
    AyuDatabase::initialize(temporary(label).join("ayudata.db")).unwrap()
}
#[test]
fn native_database_schema_and_all_eight_tables_in_both_directions() {
    let p = temporary("original").join("ayudata.db");
    std::fs::write(&p, include_bytes!("fixtures/original-ayudata.db")).unwrap();
    let original = AyuDatabase::initialize(&p).unwrap();
    let rust = database("written");
    assert_eq!(
        serde_json::to_value(original.schema().unwrap()).unwrap(),
        serde_json::from_slice::<Value>(include_bytes!("fixtures/sqlite-master.json")).unwrap()
    );
    assert_eq!(original.schema().unwrap(), rust.schema().unwrap());
    macro_rules! copy {
        ($t:ty) => {{
            let rows = original.all::<$t>();
            assert!(!rows.is_empty());
            for row in &rows {
                rust.replace(row).unwrap();
            }
            assert_eq!(rows, rust.all::<$t>());
        }};
    }
    copy!(DeletedMessage);
    copy!(EditedMessage);
    copy!(DeletedDialog);
    copy!(RegexFilter);
    copy!(RegexFilterGlobalExclusion);
    copy!(SpyMessageRead);
    copy!(SpyMessageContentsRead);
    assert_eq!(original.all::<SchemaVersion>(), rust.all::<SchemaVersion>());
    let row = &rust.all::<DeletedMessage>()[0].0;
    assert_eq!(row.user_id, i64::MAX);
    assert_eq!(row.reply_serialized, vec![0, 127, 128, 255]);
    assert_eq!(row.text, SqlText::from("Привет 🌏\n"));
    if let Some(out) = std::env::var_os("AYUGRAM_AYU_RUST_REFERENCE_DIR") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(out.join("tdata")).unwrap();
        std::fs::copy(rust.path(), out.join("tdata/ayudata.db")).unwrap();
    }
}
#[test]
fn messages_queries_bounds_order_literal_search_and_mapping() {
    let db = database("messages");
    for id in 1..=4 {
        let v = MessageValues {
            user_id: (2 << 48) | 9,
            dialog_id: -3,
            peer_id: 44,
            from_id: 55,
            topic_id: Some(if id == 4 { 2 } else { 1 }),
            message_id: id,
            date: 100,
            flags: 32,
            text: if id == 2 {
                "100%_\\ literal".into()
            } else {
                "é".into()
            },
            ..Default::default()
        };
        add_deleted_message(&db, v, 123);
    }
    assert_eq!(
        db.get_deleted_messages(9, -3, 0, 0, 0, -1, "")
            .iter()
            .map(|m| m.0.message_id)
            .collect::<Vec<_>>(),
        vec![4, 3, 2, 1]
    );
    assert_eq!(
        db.get_deleted_messages(9, -3, 1, 1, 4, 1, "")[0]
            .0
            .message_id,
        3
    );
    assert_eq!(db.get_deleted_messages(9, -3, 0, 0, 0, 10, "%_\\").len(), 1);
    assert!(has_deleted_messages(&db, 9, -3, 1));
    assert!(!has_deleted_messages(&db, 9, -3, 3));
    remove_deleted_message(&db, 9, -3, 2);
    assert!(get_deleted_messages(&db, 9, -3, 0, 0, 0, 10, "%_").is_empty());
    clear_deleted_messages(&db, 9, -3, 1);
    assert_eq!(db.all::<DeletedMessage>().len(), 1);
    let v = MessageValues {
        user_id: 9,
        dialog_id: -3,
        message_id: 4,
        text: "edited".into(),
        post_author: "author".into(),
        ..Default::default()
    };
    let mapped = map(v, 234);
    assert_eq!(mapped.edit_date, 234);
    assert_eq!(mapped.media_path, SqlText::from("/"));
    assert_eq!(mapped.entity_create_date, 234);
    add_edited_message(&db, MessageValues::default(), 1);
    assert!(db.all::<EditedMessage>().is_empty());
    db.add_edited_message(&EditedMessage(mapped.clone()));
    db.add_edited_message(&EditedMessage(mapped));
    assert!(has_revisions(&db, 9, -3, 4));
    assert_eq!(get_edited_messages(&db, 9, -3, 4, 0, 0, 1)[0].fake_id, 2);
    assert_eq!(get_edited_messages(&db, 9, -3, 4, 1, 3, 10).len(), 1);
    assert!(!has_revisions(&db, 9, -3, 5));
}
#[test]
fn regex_queries_null_blob_replace_update_exclusions_and_equality() {
    let db = database("regex");
    let mut a = RegexFilter {
        id: vec![0, 255],
        text: "тест".into(),
        enabled: true,
        ..Default::default()
    };
    db.add_regex_filter(&a);
    db.add_regex_filter(&a);
    assert_eq!(db.get_count(), 1);
    assert!(db.has_filters());
    assert!(!db.has_per_dialog_filters());
    assert_eq!(db.get_shared(), vec![a.clone()]);
    assert_eq!(db.get_by_id(&a.id), a);
    a.dialog_id = Some(-2);
    a.reversed = true;
    db.update_regex_filter(&a);
    assert_eq!(db.get_by_dialog_id(-2), vec![a.clone()]);
    assert!(db.has_per_dialog_filters());
    db.add_regex_exclusion(&RegexFilterGlobalExclusion {
        fake_id: 777,
        dialog_id: -3,
        filter_id: a.id.clone(),
    });
    assert_eq!(db.get_excluded_by_dialog_id(-3), vec![a.clone()]);
    // C++ materializes filterId through vector<char> before constructing IN.
    let legacy = rusqlite::Connection::open(db.path()).unwrap();
    legacy
        .execute(
            "UPDATE RegexFilterGlobalExclusion SET filterId=CAST(filterId AS TEXT)",
            [],
        )
        .unwrap();
    assert_eq!(db.get_excluded_by_dialog_id(-3), vec![a.clone()]);
    legacy
        .execute(
            "UPDATE RegexFilterGlobalExclusion SET filterId=CAST(filterId AS BLOB)",
            [],
        )
        .unwrap();
    assert_eq!(db.get_all_filters_exclusions()[0].fake_id, 1);
    assert_eq!(db.get_all_regex_filters(), vec![a.clone()]);
    db.delete_filter(&a.id);
    assert!(db.has_per_dialog_filters());
    db.delete_exclusion(-3, &a.id);
    assert!(!db.has_per_dialog_filters());
    db.add_regex_filter(&a);
    db.add_regex_exclusion(&RegexFilterGlobalExclusion {
        dialog_id: -3,
        filter_id: a.id.clone(),
        ..Default::default()
    });
    db.delete_exclusions_by_filter_id(&a.id);
    assert!(db.get_all_filters_exclusions().is_empty());
    db.delete_all_filters();
    db.delete_all_exclusions();
    assert!(!db.has_filters());
    assert_eq!(db.get_by_id(&[9]), RegexFilter::default());
    assert_eq!(
        RegexFilterGlobalExclusion {
            fake_id: 1,
            ..Default::default()
        },
        RegexFilterGlobalExclusion {
            fake_id: 2,
            ..Default::default()
        }
    );
    a.id = b"a\0ignored".to_vec();
    assert_eq!(a.to_json()["id"], "a");
}
#[test]
fn old_v0_schema_drops_regex_and_parallel_writes_are_serialized() {
    let dir = temporary("old");
    let path = dir.join("ayudata.db");
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("CREATE TABLE SchemaVersion(id INTEGER PRIMARY KEY,version INTEGER);INSERT INTO SchemaVersion VALUES(1,0);CREATE TABLE RegexFilter(old TEXT);INSERT INTO RegexFilter VALUES('lost');").unwrap();
    drop(c);
    let db = AyuDatabase::initialize(&path).unwrap();
    assert_eq!(db.all::<SchemaVersion>()[0].version, 1);
    assert!(db.all::<RegexFilter>().is_empty());
    let threads = (0..4)
        .map(|i| {
            let db = db.clone();
            std::thread::spawn(move || {
                for j in 0..20 {
                    db.add_regex_filter(&RegexFilter {
                        id: vec![i, j],
                        ..Default::default()
                    });
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(db.get_count(), 80);
    drop(db);
    AyuDatabase::move_current_database(&path, 123);
    assert!(!path.exists());
    assert!(dir.join("ayudata_123.db").exists());
}
#[test]
fn native_sync_schema_migrations_preservation_and_loss() {
    for name in [
        "extra-column",
        "missing-nullable",
        "missing-required",
        "v0",
        "future-version",
        "missing-version-row",
        "overflow-version",
        "changed-nullability",
        "changed-type-ignored",
    ] {
        let root = temporary(name);
        let path = root.join("ayudata.db");
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        std::fs::copy(fixtures.join(format!("{name}.input.db")), &path).unwrap();
        let port = AyuDatabase::initialize(&path).unwrap();
        let expected_path = temporary("expected").join("ayudata.db");
        std::fs::copy(fixtures.join(format!("{name}.original.db")), &expected_path).unwrap();
        let expected = rusqlite::Connection::open(expected_path).unwrap();
        let actual = rusqlite::Connection::open(path).unwrap();
        let tables = expected
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for table in tables {
            fn rows(c: &rusqlite::Connection, table: &str) -> Vec<Vec<rusqlite::types::Value>> {
                let mut q = c.prepare(&format!("SELECT * FROM \"{table}\"")).unwrap();
                let count = q.column_count();
                q.query_map([], |r| (0..count).map(|i| r.get(i)).collect())
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            }
            assert_eq!(
                rows(&actual, &table),
                rows(&expected, &table),
                "{name}: {table}"
            );
        }
        let schema = |c: &rusqlite::Connection| {
            let mut q = c
                .prepare("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY name")
                .unwrap();
            q.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };
        assert_eq!(port.schema().unwrap(), schema(&expected), "{name}: schema");
    }
}
#[test]
fn native_wrong_types_partial_mutations_and_unordered_set() {
    let records: Vec<Value> =
        serde_json::from_slice(include_bytes!("fixtures/settings-probes.original.json")).unwrap();
    assert!(records.len() > 300);
    for r in records {
        let mut s = AyuSettings::default();
        let result = s.from_json(&r["input"]);
        assert_eq!(
            result.is_err(),
            r["error"].as_bool().unwrap(),
            "{} error",
            r["name"]
        );
        assert_eq!(s.to_json(), r["output"], "{} output", r["name"]);
    }
}
#[test]
fn native_load_migration_bom_trailing_corruption_and_validation() {
    let cases: Vec<Value> =
        serde_json::from_slice(include_bytes!("fixtures/load-probes.original.json")).unwrap();
    for c in cases {
        let p = temporary("native-load").join("ayu_settings.json");
        let input: Vec<u8> = serde_json::from_value(c["input"].clone()).unwrap();
        std::fs::write(&p, input).unwrap();
        let mut s = AyuSettings::with_path(&p);
        let _ = s.load(c["ghost"].as_bool().unwrap(), None);
        assert_eq!(s.to_json(), c["output"], "{} state", c["name"]);
        assert_eq!(
            std::fs::read(p).unwrap(),
            serde_json::from_value::<Vec<u8>>(c["written"].clone()).unwrap(),
            "{} file",
            c["name"]
        );
    }
}
#[test]
fn every_setting_setter_saves_and_fires_changes_once() {
    let changed: Value =
        serde_json::from_slice(include_bytes!("fixtures/settings-changed.json.input")).unwrap();
    for (group, specs) in [
        (0, AYU_SETTINGS),
        (1, GHOST_MODE_ACCOUNT_SETTINGS),
        (2, MESSAGE_SHOT_SETTINGS),
    ] {
        for spec in specs {
            let path = temporary("setter").join("ayu_settings.json");
            let mut s = AyuSettings::with_path(&path);
            let section = match group {
                0 => &s.settings,
                1 => s.ghost(0),
                _ => &s.message_shot,
            };
            let stream = section.changes(spec.key);
            let expected_events = usize::from(stream.is_some());
            let count = std::rc::Rc::new(std::cell::Cell::new(0));
            let count_out = count.clone();
            let life = tg_rpl::Lifetime::new();
            if let Some(stream) = stream {
                stream.on_next(move |_| count_out.set(count_out.get() + 1), &life);
            }
            let value = match group {
                0 => changed[spec.key].clone(),
                1 => changed["ghostModeSettings"]["0"][spec.key].clone(),
                _ => changed["messageShotSettings"][spec.key].clone(),
            };
            let set = |s: &mut AyuSettings| match group {
                0 => {
                    s.set(spec.key, value.clone()).unwrap();
                }
                1 => {
                    s.set_ghost(0, spec.key, value.clone()).unwrap();
                }
                _ => {
                    s.set_message_shot(spec.key, value.clone()).unwrap();
                }
            };
            set(&mut s);
            assert_eq!(count.get(), expected_events, "{}", spec.key);
            set(&mut s);
            assert_eq!(count.get(), expected_events, "{} unchanged", spec.key);
            assert_eq!(
                std::fs::read(&path).unwrap(),
                s.json_bytes().unwrap(),
                "{} persisted",
                spec.key
            );
        }
    }
}
#[test]
fn reset_preserves_subscribers_context_and_effect_order() {
    let path = temporary("effects").join("settings.json");
    let mut s = AyuSettings::with_path(&path);
    let seen = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = seen.clone();
    let life = tg_rpl::Lifetime::new();
    s.settings
        .changes("saveForBots")
        .unwrap()
        .on_next(move |_| count.set(count.get() + 1), &life);
    s.set("saveForBots", json!(true)).unwrap();
    s.reset().unwrap();
    s.set("saveForBots", json!(true)).unwrap();
    assert_eq!(seen.get(), 3);
    let before = std::fs::read(&path).unwrap();
    let path_copy = path.clone();
    s.effects().on_next(
        move |e| {
            if e == Effect::MaterialSwitches(false) {
                assert_eq!(std::fs::read(&path_copy).unwrap(), before);
            }
        },
        &life,
    );
    s.set("materialSwitches", json!(false)).unwrap();
    let mut s = AyuSettings::with_defaults("изменено".into(), true);
    s.from_json(&json!({"translationProvider":"native"}))
        .unwrap();
    assert_eq!(s.settings.get("editedMark"), Some(json!("изменено")));
    assert_eq!(s.settings.get("translationProvider"), Some(json!("native")));
    s.reset().unwrap();
    assert_eq!(s.settings.get("editedMark"), Some(json!("изменено")));
}
#[test]
fn original_nul_text_truncation_and_sqlite_numeric_coercions() {
    let path = temporary("nul").join("ayudata.db");
    std::fs::write(&path, include_bytes!("fixtures/nul-text.db")).unwrap();
    let db = AyuDatabase::initialize(&path).unwrap();
    assert_eq!(db.get_by_id(b"a\0").text, SqlText::from("a"));
    let raw = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        raw.query_row(
            "SELECT CAST(text AS BLOB) FROM RegexFilter WHERE id=x'6100'",
            [],
            |r| r.get::<_, Vec<u8>>(0)
        )
        .unwrap(),
        b"a\0b"
    );
    // NOT NULL fields are kept valid; a nullable SQL declaration mismatch is handled by sync_schema.
    raw.execute(
        "UPDATE DeletedMessage SET views=4294967297,replyForumTopic=4294967296 WHERE fakeId=1",
        [],
    )
    .unwrap();
    let message = db.all::<DeletedMessage>().remove(0);
    assert_eq!(message.0.views, 1);
    assert!(!message.0.reply_forum_topic);
}
#[test]
fn database_error_defaults_rollback_and_corrupt_file_backup() {
    let db = database("error");
    let raw = rusqlite::Connection::open(db.path()).unwrap();
    raw.execute_batch("DROP TABLE DeletedMessage;DROP TABLE EditedMessage;DROP TABLE RegexFilter;DROP TABLE RegexFilterGlobalExclusion;").unwrap();
    db.add_edited_message(&EditedMessage::default());
    db.add_deleted_message(&DeletedMessage::default());
    assert!(db.get_edited_messages(1, 2, 3, 0, 0, 10).is_empty());
    assert!(!db.has_revisions(1, 2, 3));
    assert!(db.get_deleted_messages(1, 2, 0, 0, 0, 10, "").is_empty());
    assert!(!db.has_deleted_messages(1, 2, 0));
    db.remove_deleted_message(1, 2, 3);
    db.clear_deleted_messages(1, 2, 0);
    db.add_regex_filter(&RegexFilter::default());
    db.update_regex_filter(&RegexFilter::default());
    db.add_regex_exclusion(&RegexFilterGlobalExclusion::default());
    assert_eq!(db.get_by_id(&[]), RegexFilter::default());
    assert!(db.get_shared().is_empty());
    assert!(db.get_by_dialog_id(1).is_empty());
    assert!(db.get_excluded_by_dialog_id(1).is_empty());
    assert!(db.get_all_regex_filters().is_empty());
    assert!(db.get_all_filters_exclusions().is_empty());
    assert_eq!(db.get_count(), 0);
    assert!(!db.has_filters());
    assert!(!db.has_per_dialog_filters());
    db.delete_filter(&[]);
    db.delete_exclusions_by_filter_id(&[]);
    db.delete_exclusion(1, &[]);
    db.delete_all_filters();
    db.delete_all_exclusions();
    let dir = temporary("corrupt");
    let path = dir.join("ayudata.db");
    std::fs::write(&path, b"not a database").unwrap();
    let db = AyuDatabase::initialize(&path).unwrap();
    assert_eq!(db.all::<SchemaVersion>()[0].version, 0);
    let backup = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().starts_with("ayudata_"))
        .unwrap();
    assert_eq!(std::fs::read(backup.path()).unwrap(), b"not a database");
    drop(db);
    assert_eq!(
        AyuDatabase::initialize(&path)
            .unwrap()
            .all::<SchemaVersion>()[0]
            .version,
        1
    );
    std::fs::write(dir.join("ayudata_42.db"), b"existing").unwrap();
    AyuDatabase::move_current_database(&path, 42);
    assert!(path.exists());
    assert_eq!(
        std::fs::read(dir.join("ayudata_42.db")).unwrap(),
        b"existing"
    );
}
#[test]
fn derived_ghost_events_are_distinct_and_account_reload_keeps_optional_sections() {
    let mut s = AyuSettings::default();
    let seen = std::rc::Rc::new(std::cell::RefCell::new(vec![]));
    let out = seen.clone();
    let life = tg_rpl::Lifetime::new();
    s.ghost(0)
        .ghost_mode_active_changes()
        .on_next(move |v| out.borrow_mut().push(v), &life);
    s.set_mark_read_after_action(0, false).unwrap();
    s.set_use_scheduled_messages(0, true).unwrap();
    assert!(seen.borrow().is_empty());
    s.set_ghost_mode_enabled(0, true, None).unwrap();
    s.set_ghost_mode_enabled(0, true, None).unwrap();
    assert_eq!(*seen.borrow(), vec![true]);
    assert!(s.ghost(0).is_ghost_mode_active());
    s.set_ghost_mode_enabled(0, false, None).unwrap();
    assert_eq!(*seen.borrow(), vec![true, false]);
    s.set_cloud_theme(1, 2, 3, 4, "theme").unwrap();
    s.set_use_global_ghost_mode(false).unwrap();
    s.set_send_read_messages(7, false).unwrap();
    s.from_json(&json!({"messageShotSettings":[],"ghostModeSettings":null}))
        .unwrap();
    assert_eq!(s.message_shot.cloud_theme_id(), 2);
    s.set_use_global_ghost_mode(false).unwrap();
    assert!(!s.ghost(7).send_read_messages());
}
