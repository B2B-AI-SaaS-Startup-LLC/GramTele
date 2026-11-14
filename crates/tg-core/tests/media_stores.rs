use std::{cell::RefCell, rc::Rc};
use tg_core::{
    core::mime_type::*,
    data::{
        data_document::*,
        data_media_location::*,
        data_media_store::*,
        data_media_types::UploadState,
        data_msg_id::*,
        data_peer_id::PeerId,
        data_photo::*,
        data_session::{Documents, Photos},
        data_store_keys::*,
    },
};
use tg_rpl::{Lifetime, Producer};
use tg_scheme as tl;
struct Registry;
impl MimeDatabase for Registry {
    fn for_name(&self, name: &str) -> MimeType {
        panic!("unexpected platform registry query: {name}")
    }
}
fn listen(events: Producer<MediaEvent>, life: &Lifetime, next: impl Fn(MediaEvent) + 'static) {
    events.start(next, |_| {}, || {}, life);
}
fn photo(id: u64, date: i32, cached: bool) -> tl::Photo {
    tl::mtp_photo(
        tl::tl::Flags::from_raw(0),
        id,
        123,
        vec![1],
        date,
        vec![if cached {
            tl::mtp_photo_cached_size(b"x".to_vec(), 50, 50, vec![1, 2])
        } else {
            tl::mtp_photo_size(b"x".to_vec(), 50, 50, 100)
        }],
        None,
        4,
    )
}
fn doc(id: u64, date: i32, dc: i32, thumb: bool) -> tl::Document {
    tl::mtp_document(
        tl::tl::Flags::from_raw(0),
        id,
        123,
        vec![1],
        date,
        b"application/octet-stream".to_vec(),
        100,
        thumb.then(|| vec![tl::mtp_photo_cached_size(b"m".to_vec(), 50, 50, vec![1, 2])]),
        None,
        dc,
        vec![],
    )
}
fn message(scope: &SessionScope) -> MessageKey {
    MessageKeys::new(scope).insert(FullMsgId::new(PeerId::default(), MsgId::new(1)))
}
#[test]
fn photo_noncollision_rekeys_lookup_not_identity_or_indices() {
    let s = SessionScope::new();
    let p = Photos::new(&s);
    let a = p.process_photo(&photo(10, 1, false));
    let m = message(&s);
    p.register_photo_item(a, m);
    p.register_photo_item(a, m);
    p.schedule_cache_clear(a);
    let lease = p.edit(a, |p| {
        p.uploading_data = Some(UploadState::new(100));
        p.create_media_view()
    });
    lease
        .borrow_mut()
        .set_video(PhotoSize::Large, false, vec![9]);
    p.photo_convert(a, &photo(20, 2, false));
    assert_eq!(p.find(s.photo_key(10)), None);
    assert_eq!(p.find(s.photo_key(20)), Some(a));
    assert_eq!(
        p.read(a, |p| (p.id(), p.date(), p.uploading())),
        Some((20, 2, false))
    );
    assert_eq!(p.photo_items(a)[&m], 2);
    assert_eq!(p.take_scheduled_cache_clear(), [a].into_iter().collect());
    assert!(Rc::ptr_eq(&lease, &p.edit(a, PhotoData::create_media_view)));
    let other = p.ensure_photo(10);
    assert_ne!(other, a);
    assert_eq!(p.read(a, PhotoData::id), Some(20));
    p.unregister_photo_item(a, m);
    assert_eq!(p.photo_items(a)[&m], 1);
    p.unregister_photo_item(a, m);
    p.unregister_photo_item(a, m);
    assert!(p.photo_items(a).is_empty());
}
#[test]
fn photo_collision_and_repeated_conversion_preserve_original_lookup_quirk() {
    let s = SessionScope::new();
    let p = Photos::new(&s);
    let a = p.process_photo(&photo(10, 1, false));
    let b = p.process_photo(&photo(20, 2, false));
    p.photo_convert(a, &photo(20, 3, false));
    assert_eq!(p.find(s.photo_key(10)), Some(a));
    assert_eq!(p.find(s.photo_key(20)), Some(b));
    assert_eq!(p.read(a, |p| (p.id(), p.date())), Some((20, 3)));
    assert_eq!(p.read(b, |p| (p.id(), p.date())), Some((20, 3)));
    p.photo_convert(a, &photo(30, 4, false));
    assert_eq!(p.find(s.photo_key(10)), Some(a));
    assert_eq!(p.find(s.photo_key(20)), None);
    assert_eq!(p.find(s.photo_key(30)), Some(b));
    assert_eq!(p.read(a, |p| (p.id(), p.date())), Some((30, 4)));
    assert_eq!(p.read(b, |p| (p.id(), p.date())), Some((20, 4)));
}
#[test]
fn convert_effects_run_after_atomic_model_and_lookup_mutation() {
    let s = SessionScope::new();
    let p = Rc::new(Photos::new(&s));
    let a = p.ensure_photo(10);
    let b = p.ensure_photo(20);
    let seen = Rc::new(RefCell::new(vec![]));
    let life = Lifetime::new();
    listen(p.effects(), &life, {
        let p = Rc::downgrade(&p);
        let seen = seen.clone();
        move |event| {
            let p = p.upgrade().unwrap();
            let MediaEvent::Photo { key, .. } = event else {
                panic!()
            };
            assert_eq!(p.read(a, PhotoData::id), Some(20));
            assert_eq!(p.read(b, PhotoData::date), Some(7));
            p.edit(key, |p| p.set_has_attached_stickers(true));
            seen.borrow_mut().push(key);
        }
    });
    p.photo_convert(a, &photo(20, 7, true));
    assert_eq!(*seen.borrow(), vec![b, a]);
    assert_eq!(p.read(a, PhotoData::has_attached_stickers), Some(true));
    assert_eq!(p.read(b, PhotoData::has_attached_stickers), Some(true));
}
#[test]
fn document_convert_cache_moves_order_uploads_and_stable_photo_reference() {
    let s = SessionScope::new();
    let d = Documents::new(&s, Rc::new(NoFiles));
    let p = Photos::new(&s);
    let image = p.ensure_photo(5);
    let a = d.process_document(&doc(10, 1, 4, false), &Registry);
    d.edit(a, |d| {
        d.set_good_thumbnail_photo(image);
        d.uploading_data = Some(UploadState::new(100));
        d.status = FileStatus::UploadFailed;
    });
    let m = message(&s);
    d.register_document_item(a, m);
    let events = Rc::new(RefCell::new(vec![]));
    let life = Lifetime::new();
    listen(d.effects(), &life, {
        let e = events.clone();
        move |v| e.borrow_mut().push(v)
    });
    d.document_convert(a, &doc(20, 2, 5, false), &Registry, true);
    assert_eq!(
        *events.borrow(),
        vec![
            MediaEvent::MoveCacheIfEmpty {
                from: document_cache_key(4, 10),
                to: document_cache_key(5, 20)
            },
            MediaEvent::MoveCacheIfEmpty {
                from: document_thumb_cache_key(4, 10),
                to: document_thumb_cache_key(5, 20)
            },
            MediaEvent::WriteSavedGifs
        ]
    );
    assert_eq!(d.find(s.document_key(10)), None);
    assert_eq!(d.find(s.document_key(20)), Some(a));
    assert_eq!(
        d.read(a, |d| (
            d.id(),
            d.status,
            d.uploading(),
            d.good_thumbnail_photo()
        )),
        Some((20, FileStatus::Ready, false, Some(image)))
    );
    assert_eq!(d.document_items(a), [m].into_iter().collect());
    d.unregister_document_item(a, m);
    assert!(d.document_items(a).is_empty());
    events.borrow_mut().clear();
    d.edit(a, |d| d.uploading_data = Some(UploadState::new(100)));
    d.document_convert(a, &doc(20, 3, 5, false), &Registry, true);
    assert!(events.borrow().is_empty());
    assert_eq!(d.read(a, DocumentData::uploading), Some(true));
}
#[test]
fn document_collision_repeated_empty_convert_and_reentrant_cache_sink() {
    let s = SessionScope::new();
    let d = Rc::new(Documents::new(&s, Rc::new(NoFiles)));
    let a = d.process_document(&doc(10, 1, 4, false), &Registry);
    let b = d.process_document(&doc(20, 2, 4, false), &Registry);
    let life = Lifetime::new();
    let count = Rc::new(RefCell::new(0));
    listen(d.effects(), &life, {
        let d = Rc::downgrade(&d);
        let count = count.clone();
        move |e| {
            let d = d.upgrade().unwrap();
            if matches!(e, MediaEvent::MoveCacheIfEmpty { .. }) {
                assert!(d.read(a, DocumentData::id).is_some());
                d.edit(b, |_| ());
                *count.borrow_mut() += 1;
            }
        }
    });
    d.document_convert(a, &doc(20, 3, 5, false), &Registry, false);
    assert_eq!(d.find(s.document_key(10)), Some(a));
    assert_eq!(d.find(s.document_key(20)), Some(b));
    assert_eq!(d.read(b, |d| (d.id(), d.date)), Some((20, 3)));
    d.document_convert(a, &tl::mtp_document_empty(30), &Registry, false);
    assert_eq!(d.find(s.document_key(30)), Some(b));
    assert_eq!(d.read(a, |d| (d.id(), d.date)), Some((30, 3)));
    assert_eq!(d.read(b, |d| (d.id(), d.date)), Some((20, 3)));
    assert_eq!(*count.borrow(), 4);
}
#[test]
fn owner_liveness_foreign_scopes_clear_and_lease_lifetime() {
    let s = SessionScope::new();
    let p = Photos::new(&s);
    let d = Documents::new(&s, Rc::new(NoFiles));
    let a = p.ensure_photo(1);
    let b = d.ensure_document(1);
    let lease = d.edit(b, DocumentData::create_media_view);
    lease.borrow_mut().set_bytes(&[7]);
    let t = SessionScope::new();
    let other = Photos::new(&t).ensure_photo(1);
    assert!(p.read(other, PhotoData::id).is_none());
    assert!(p.find(t.photo_key(1)).is_none());
    p.clear();
    d.clear();
    assert!(p.read(a, PhotoData::id).is_none());
    assert!(d.read(b, DocumentData::id).is_none());
    assert_ne!(p.ensure_photo(1), a);
    assert_ne!(d.ensure_document(1), b);
    drop(s);
    assert!(p.find(t.photo_key(1)).is_none());
    assert!(d.read(b, DocumentData::id).is_none());
    assert_eq!(lease.borrow().bytes(), &[7]);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| p.ensure_photo(2))).is_err());
}
#[test]
fn document_tl_qualities_keep_record_links_across_remap() {
    fn video(id: u64, w: i32, h: i32) -> tl::Document {
        tl::mtp_document(
            tl::tl::Flags::from_raw(0),
            id,
            123,
            vec![],
            42,
            b"video/mp4".to_vec(),
            100,
            None,
            None,
            4,
            vec![tl::mtp_document_attribute_video(
                tl::tl::Flags::from_raw(2),
                1.,
                w,
                h,
                None,
                None,
                None,
            )],
        )
    }
    let s = SessionScope::new();
    let d = Documents::new(&s, Rc::new(NoFiles));
    let a =
        d.process_document_with_qualities(&video(1, 1920, 1080), &[video(2, 1280, 720)], &Registry);
    let b = d.find(s.document_key(2)).unwrap();
    assert_eq!(
        d.read(a, |d| d.video().unwrap().qualities.clone()),
        Some(vec![a, b])
    );
    d.document_convert(b, &video(20, 1280, 720), &Registry, false);
    assert_eq!(d.resolve_video_quality(a), 1080);
    assert_eq!(
        d.read(a, |d| d.video().unwrap().qualities.clone()),
        Some(vec![a, b])
    );
    d.process_document_with_qualities(&tl::mtp_document_empty(1), &[video(3, 640, 360)], &Registry);
    assert!(d.find(s.document_key(3)).is_none());
}

// Test service for the real empty localstorage database in these scenarios.
struct NoFiles;
impl tg_core::core::file_location::FileLocations for NoFiles {
    fn inspect(
        &self,
        _: &tg_core::core::file_location::FileLocation,
    ) -> tg_core::core::file_location::FileInfo {
        panic!("empty location should not be inspected")
    }
    fn read(&self, _: (u64, u64)) -> tg_core::core::file_location::FileLocation {
        Default::default()
    }
}
#[test]
fn peer_photo_binding_uses_wire_id_and_clears_empty_without_creating_photo() {
    use tg_core::data::{data_changes::Changes, data_peer_id::peer_from_chat};
    let s = SessionScope::new();
    let photos = Photos::new(&s);
    let changes = Changes::with_queue(&s, tg_runtime::MainQueue::new());
    let peers = tg_core::data::data_session::Peers::new(&s, changes);
    let peer = peers.ensure_peer(peer_from_chat(tg_core::data::data_peer_id::ChatId::new(7)));
    peers.set_userpic_photo(&photos, peer, &photo(10, 1, false));
    let key = photos.find(s.photo_key(10)).unwrap();
    assert_eq!(photos.read(key, |p| p.peer), Some(Some(peer)));
    assert_eq!(
        peers.read(peer, |p| p.userpic_photo_key()),
        Some(s.photo_key(10))
    );
    photos.photo_convert(key, &photo(20, 2, false));
    assert_eq!(
        peers.read(peer, |p| p.userpic_photo_key()),
        Some(s.photo_key(10))
    );
    peers.set_userpic_photo(&photos, peer, &tl::mtp_photo_empty(99));
    assert_eq!(peers.read(peer, |p| p.userpic_photo_key()), None);
    assert!(photos.find(s.photo_key(99)).is_none());
}
#[test]
fn local_file_metadata_paths_cache_marker_and_remote_registration() {
    use std::cell::Cell;
    use tg_core::core::file_location::*;
    struct Files {
        saved: RefCell<FileLocation>,
        info: RefCell<FileInfo>,
        reads: Cell<usize>,
    }
    impl FileLocations for Files {
        fn inspect(&self, _: &FileLocation) -> FileInfo {
            self.info.borrow().clone()
        }
        fn read(&self, _: (u64, u64)) -> FileLocation {
            self.reads.set(self.reads.get() + 1);
            self.saved.borrow().clone()
        }
    }
    let info = FileInfo {
        path: "file.bin".into(),
        exists: true,
        readable: true,
        modified_ms: Some(1234),
        size: 5,
    };
    let loc = FileLocation::from_info(&info);
    let f = Files {
        saved: RefCell::new(loc.clone()),
        info: RefCell::new(info.clone()),
        reads: Cell::new(0),
    };
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 1);
    assert!(d.filepath(true, &f).is_empty());
    assert_eq!(f.reads.get(), 0);
    assert_eq!(d.location(true, &f), &loc);
    assert_eq!(f.reads.get(), 1);
    assert!(!d.is_null());
    assert!(d.set_loaded_in_media_cache(true).is_empty());
    assert!(d.loaded_in_media_cache());
    assert_eq!(
        d.set_remote_location(4, 123, &[1], &f),
        vec![DocumentEffect::WriteFileLocation {
            key: d.media_key(),
            location: loc.clone()
        }]
    );
    f.info.borrow_mut().readable = false;
    *f.saved.borrow_mut() = FileLocation::in_media_cache_location();
    assert!(d.location(true, &f).is_empty());
    assert!(d.loaded_in_media_cache());
    assert_eq!(
        d.set_loaded_in_media_cache(false),
        vec![
            DocumentEffect::RemoveFileLocation { key: d.media_key() },
            DocumentEffect::RequestViewRepaint(d.key())
        ]
    );
    assert!(d.set_loaded_in_media_cache(false).is_empty());
    assert!(d.set_remote_location(5, 123, &[2], &f).is_empty());
    assert!(d.loaded_in_media_cache());
    let before = f.reads.get();
    assert!(d.set_remote_location(5, 123, &[3], &f).is_empty());
    assert_eq!(f.reads.get(), before);
    assert_eq!(d.file_reference(), &[3]);
    let mut p = PhotoData::new(&s, 2);
    assert!(p.location(true, &f).is_empty());
    f.info.borrow_mut().readable = true;
    assert_eq!(
        p.set_location(loc.clone(), &f),
        vec![PhotoEffect::WriteFileLocation {
            key: p.media_key(),
            location: loc.clone()
        }]
    );
    f.info.borrow_mut().size = 6;
    assert_eq!(p.location(true, &f), &loc); // cache marker keeps old invalid photo location.
    assert!(FileLocation::from_info(&FileInfo {
        size: MAX_FILE_SIZE + 1,
        ..info.clone()
    })
    .is_empty());
    assert!(!loc.check(&FileInfo {
        modified_ms: Some(1235),
        ..info
    }));
}
#[test]
fn document_registration_music_effect_precedes_index_and_is_reentrant() {
    let s = SessionScope::new();
    let d = Rc::new(Documents::new(&s, Rc::new(NoFiles)));
    let key = d.ensure_document(1);
    let item = message(&s);
    d.edit(key, |d| d.kind = DocumentType::Song);
    let life = Lifetime::new();
    let count = Rc::new(RefCell::new(0));
    listen(d.effects(), &life, {
        let d = Rc::downgrade(&d);
        let count = count.clone();
        move |e| {
            assert_eq!(e, MediaEvent::LoadSavedMusicIds);
            let d = d.upgrade().unwrap();
            if *count.borrow() == 0 {
                assert!(d.document_items(key).is_empty());
            }
            d.edit(key, |_| ());
            *count.borrow_mut() += 1;
        }
    });
    d.register_document_item(key, item);
    d.register_document_item(key, item);
    assert_eq!(d.document_items(key).len(), 1);
    assert_eq!(*count.borrow(), 2);
}
