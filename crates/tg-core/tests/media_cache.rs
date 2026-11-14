#[path = "../../tg-cache/tests/support/database.rs"]
mod support;
use std::sync::mpsc;
use support::*;
use tg_cache::cache::{Database, Key};
use tg_core::data::{data_document::*, data_media_store::*, data_photo::*, data_store_keys::*};
#[test]
fn media_effects_use_real_encrypted_cache_and_preserve_destination_tags() {
    let host = Host::new();
    let temp = Temp::new("media-effects");
    let db = Database::with_environment(&temp.0, settings("stores"), host.env.clone());
    assert!(open(&db, key()).is_ok());
    let s = SessionScope::new();
    let p = PhotoData::new(&s, 1);
    let d = DocumentData::new(&s, 2);
    let a = Key::new(1, 1);
    let b = Key::new(1, 2);
    let read = |key| {
        let (tx, rx) = mpsc::channel();
        db.get_with_tag(key, move |v| tx.send(v).unwrap());
        rx.recv().unwrap()
    };
    assert!(apply_cache_event(
        &MediaEvent::Photo {
            key: p.key(),
            effect: PhotoEffect::CachePutIfEmpty {
                key: a,
                bytes: vec![1, 2],
                tag: 1
            }
        },
        &db
    ));
    assert_eq!((read(a).bytes, read(a).tag), (vec![1, 2], 1));
    assert!(apply_cache_event(
        &MediaEvent::Document {
            key: d.key(),
            effect: DocumentEffect::CachePut {
                key: a,
                bytes: vec![3, 4],
                tag: 3
            }
        },
        &db
    ));
    assert!(apply_cache_event(
        &MediaEvent::MoveCacheIfEmpty { from: a, to: b },
        &db
    ));
    assert!(read(a).bytes.is_empty());
    assert_eq!((read(b).bytes, read(b).tag), (vec![3, 4], 3));
    assert!(apply_cache_event(
        &MediaEvent::Photo {
            key: p.key(),
            effect: PhotoEffect::CachePutIfEmpty {
                key: a,
                bytes: vec![9],
                tag: 5
            }
        },
        &db
    ));
    assert!(apply_cache_event(
        &MediaEvent::MoveCacheIfEmpty { from: a, to: b },
        &db
    ));
    assert_eq!(read(a).bytes, vec![9]);
    assert_eq!(read(b).bytes, vec![3, 4]);
    assert!(!apply_cache_event(&MediaEvent::WriteSavedGifs, &db));
    closed(&db);
}
