//! Data::Session's actual photo/document ownership slices. Stable record keys
//! replace pointers; wire lookup and the mutable id field remain distinct.
use super::{
    data_document::*, data_media_from_tl::*, data_media_location::CacheKey, data_photo::*,
    data_store_keys::*,
};
use crate::core::mime_type::MimeDatabase;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::{Rc, Weak},
};
use tg_rpl::{EventStream, Producer};
/// Actual tg-cache queue adapter. False means an owner/localstorage/UI command
/// which the host must route to its corresponding service.
#[must_use]
pub fn apply_cache_event(event: &MediaEvent, cache: &tg_cache::cache::Database) -> bool {
    use tg_cache::cache::TaggedValue;
    match event {
        MediaEvent::MoveCacheIfEmpty { from, to } => cache.move_if_empty(*from, *to, None),
        MediaEvent::Photo {
            effect: PhotoEffect::CachePutIfEmpty { key, bytes, tag },
            ..
        } => cache.put_if_empty(*key, TaggedValue::new(bytes.clone(), *tag), None),
        MediaEvent::Document {
            effect: DocumentEffect::CachePut { key, bytes, tag },
            ..
        } => cache.put(*key, TaggedValue::new(bytes.clone(), *tag), None),
        MediaEvent::Document {
            effect:
                DocumentEffect::CachePutIfEmpty {
                    key,
                    bytes,
                    animation,
                },
            ..
        } => cache.put_if_empty(
            *key,
            TaggedValue::new(bytes.clone(), if *animation { 5 } else { 1 }),
            None,
        ),
        _ => return false,
    }
    true
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaEvent {
    Photo {
        key: PhotoRecordKey,
        effect: PhotoEffect,
    },
    Document {
        key: DocumentRecordKey,
        effect: DocumentEffect,
    },
    MoveCacheIfEmpty {
        from: CacheKey,
        to: CacheKey,
    },
    WriteSavedGifs,
    LoadSavedMusicIds,
}
#[derive(Default)]
struct PhotoState {
    lookup: BTreeMap<u64, PhotoRecordKey>,
    records: BTreeMap<PhotoRecordKey, PhotoData>,
    items: BTreeMap<PhotoRecordKey, BTreeMap<MessageKey, i32>>,
    scheduled_for_cache_clear: BTreeSet<PhotoRecordKey>,
}
pub struct Photos {
    scope: SessionScopeId,
    alive: Weak<ScopeState>,
    state: RefCell<PhotoState>,
    effects: EventStream<MediaEvent>,
}
impl Photos {
    pub fn session(&self) -> SessionScopeId {
        self.scope
    }
    pub fn new(scope: &SessionScope) -> Self {
        Self {
            scope: scope.id(),
            alive: scope.downgrade(),
            state: RefCell::default(),
            effects: EventStream::new(),
        }
    }
    fn check(&self, key: PhotoRecordKey) {
        assert!(self.alive.upgrade().is_some(), "SessionScope has ended");
        assert_eq!(key.scope(), self.scope, "foreign photo key");
    }
    pub fn ensure_photo(&self, id: u64) -> PhotoRecordKey {
        let alive = self.alive.upgrade().expect("SessionScope has ended");
        let mut s = self.state.borrow_mut();
        if let Some(&k) = s.lookup.get(&id) {
            return k;
        }
        let k = PhotoRecordKey::allocate(self.scope, &alive);
        s.records.insert(k, PhotoData::with_key(k, id));
        s.lookup.insert(id, k);
        k
    }
    /// Unlike peerLoaded, media has no LoadedStatus in the original: this is
    /// a non-creating lookup, not a promise that network/pixels are loaded.
    pub fn find(&self, key: PhotoKey) -> Option<PhotoRecordKey> {
        self.alive.upgrade()?;
        if key.scope() != self.scope {
            return None;
        }
        self.state.borrow().lookup.get(&key.id()).copied()
    }
    pub fn read<R>(&self, key: PhotoRecordKey, read: impl FnOnce(&PhotoData) -> R) -> Option<R> {
        self.alive.upgrade()?;
        if key.scope() != self.scope {
            return None;
        }
        self.state.borrow().records.get(&key).map(read)
    }
    pub fn edit<R>(&self, key: PhotoRecordKey, edit: impl FnOnce(&mut PhotoData) -> R) -> R {
        self.check(key);
        edit(
            self.state
                .borrow_mut()
                .records
                .get_mut(&key)
                .expect("photo not owned"),
        )
    }
    pub fn effects(&self) -> Producer<MediaEvent> {
        self.effects.events()
    }
    fn dispatch(&self, effects: Vec<MediaEvent>) {
        for e in effects {
            self.effects.fire(e);
        }
    }
    pub fn process_photo(&self, data: &tg_scheme::Photo) -> PhotoRecordKey {
        let id = photo_id(data);
        let key = self.ensure_photo(id);
        let effects = self.edit(key, |p| photo_apply_fields(p, data));
        self.dispatch(
            effects
                .into_iter()
                .map(|effect| MediaEvent::Photo { key, effect })
                .collect(),
        );
        key
    }
    pub fn photo_convert(&self, original: PhotoRecordKey, data: &tg_scheme::Photo) {
        self.check(original);
        let effects = {
            let mut s = self.state.borrow_mut();
            let id = photo_id(data);
            let old = s.records.get(&original).expect("photo not owned").id();
            let mut effects = Vec::new();
            if old != id {
                // data_session.cpp:3885: intentionally remove by the mutable
                // old id, not by original identity. After a previous collision
                // this can move another record. Preserve the actual C++ rule.
                let destination = if let Some(&k) = s.lookup.get(&id) {
                    k
                } else {
                    let k = s
                        .lookup
                        .remove(&old)
                        .expect("original C++ Assert: old photo lookup absent");
                    s.lookup.insert(id, k);
                    k
                };
                let p = s.records.get_mut(&original).unwrap();
                p.id = id;
                p.uploading_data = None;
                if destination != original {
                    effects.extend(
                        photo_apply_fields(s.records.get_mut(&destination).unwrap(), data)
                            .into_iter()
                            .map(|effect| MediaEvent::Photo {
                                key: destination,
                                effect,
                            }),
                    );
                }
            }
            effects.extend(
                photo_apply_fields(s.records.get_mut(&original).unwrap(), data)
                    .into_iter()
                    .map(|effect| MediaEvent::Photo {
                        key: original,
                        effect,
                    }),
            );
            effects
        };
        self.dispatch(effects);
    }
    pub fn register_photo_item(&self, key: PhotoRecordKey, item: MessageKey) {
        self.check(key);
        assert_eq!(item.scope(), self.scope);
        let mut s = self.state.borrow_mut();
        assert!(s.records.contains_key(&key));
        let count = s.items.entry(key).or_default().entry(item).or_default();
        *count = count.wrapping_add(1);
    }
    pub fn unregister_photo_item(&self, key: PhotoRecordKey, item: MessageKey) {
        self.check(key);
        assert_eq!(item.scope(), self.scope);
        let mut s = self.state.borrow_mut();
        if let Some(items) = s.items.get_mut(&key) {
            if let Some(count) = items.get_mut(&item) {
                *count = count.wrapping_sub(1);
                if *count == 0 {
                    items.remove(&item);
                }
            }
            if items.is_empty() {
                s.items.remove(&key);
            }
        }
    }
    pub fn photo_items(&self, key: PhotoRecordKey) -> BTreeMap<MessageKey, i32> {
        self.check(key);
        self.state
            .borrow()
            .items
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }
    pub fn schedule_cache_clear(&self, key: PhotoRecordKey) {
        self.check(key);
        let mut s = self.state.borrow_mut();
        assert!(s.records.contains_key(&key));
        s.scheduled_for_cache_clear.insert(key);
    }
    pub fn take_scheduled_cache_clear(&self) -> BTreeSet<PhotoRecordKey> {
        assert!(self.alive.upgrade().is_some());
        std::mem::take(&mut self.state.borrow_mut().scheduled_for_cache_clear)
    }
    pub fn clear(&self) {
        assert!(self.alive.upgrade().is_some());
        *self.state.borrow_mut() = PhotoState::default();
    }
}
pub fn photo_id(data: &tg_scheme::Photo) -> u64 {
    match data {
        tg_scheme::Photo::Photo(d) => d.id,
        tg_scheme::Photo::PhotoEmpty(d) => d.id,
    }
}
pub fn document_id(data: &tg_scheme::Document) -> u64 {
    match data {
        tg_scheme::Document::Document(d) => d.id,
        tg_scheme::Document::DocumentEmpty(d) => d.id,
    }
}
#[derive(Default)]
struct DocumentState {
    lookup: BTreeMap<u64, DocumentRecordKey>,
    records: BTreeMap<DocumentRecordKey, DocumentData>,
    items: BTreeMap<DocumentRecordKey, BTreeSet<MessageKey>>,
}
pub struct Documents {
    scope: SessionScopeId,
    alive: Weak<ScopeState>,
    state: RefCell<DocumentState>,
    effects: EventStream<MediaEvent>,
    files: Rc<dyn crate::core::file_location::FileLocations>,
}
impl Documents {
    pub fn new(
        scope: &SessionScope,
        files: Rc<dyn crate::core::file_location::FileLocations>,
    ) -> Self {
        Self {
            scope: scope.id(),
            alive: scope.downgrade(),
            state: RefCell::default(),
            effects: EventStream::new(),
            files,
        }
    }
    fn apply_document(
        &self,
        document: &mut DocumentData,
        data: &tg_scheme::Document,
        database: &impl MimeDatabase,
    ) -> Vec<DocumentEffect> {
        document_apply_fields_with_files(document, data, database, self.files.as_ref())
    }
    fn check(&self, key: DocumentRecordKey) {
        assert!(self.alive.upgrade().is_some(), "SessionScope has ended");
        assert_eq!(key.scope(), self.scope, "foreign document key");
    }
    pub fn ensure_document(&self, id: u64) -> DocumentRecordKey {
        let alive = self.alive.upgrade().expect("SessionScope has ended");
        let mut s = self.state.borrow_mut();
        if let Some(&k) = s.lookup.get(&id) {
            return k;
        }
        let k = DocumentRecordKey::allocate(self.scope, &alive);
        s.records.insert(k, DocumentData::with_key(k, id));
        s.lookup.insert(id, k);
        k
    }
    pub fn find(&self, key: DocumentKey) -> Option<DocumentRecordKey> {
        self.alive.upgrade()?;
        if key.scope() != self.scope {
            return None;
        }
        self.state.borrow().lookup.get(&key.id()).copied()
    }
    pub fn read<R>(
        &self,
        key: DocumentRecordKey,
        read: impl FnOnce(&DocumentData) -> R,
    ) -> Option<R> {
        self.alive.upgrade()?;
        if key.scope() != self.scope {
            return None;
        }
        self.state.borrow().records.get(&key).map(read)
    }
    pub fn edit<R>(&self, key: DocumentRecordKey, edit: impl FnOnce(&mut DocumentData) -> R) -> R {
        self.check(key);
        edit(
            self.state
                .borrow_mut()
                .records
                .get_mut(&key)
                .expect("document not owned"),
        )
    }
    pub fn effects(&self) -> Producer<MediaEvent> {
        self.effects.events()
    }
    fn dispatch(&self, effects: Vec<MediaEvent>) {
        for e in effects {
            self.effects.fire(e);
        }
    }
    pub fn process_document(
        &self,
        data: &tg_scheme::Document,
        database: &impl MimeDatabase,
    ) -> DocumentRecordKey {
        let key = self.ensure_document(document_id(data));
        let effects = self.edit(key, |d| self.apply_document(d, data, database));
        self.dispatch(
            effects
                .into_iter()
                .map(|effect| MediaEvent::Document { key, effect })
                .collect(),
        );
        key
    }
    pub fn process_document_with_qualities(
        &self,
        data: &tg_scheme::Document,
        qualities: &[tg_scheme::Document],
        database: &impl MimeDatabase,
    ) -> DocumentRecordKey {
        let key = self.process_document(data, database);
        if matches!(data, tg_scheme::Document::DocumentEmpty(_)) {
            return key;
        }
        self.set_video_qualities(key, qualities, database);
        key
    }
    pub fn set_video_qualities(
        &self,
        key: DocumentRecordKey,
        qualities: &[tg_scheme::Document],
        database: &impl MimeDatabase,
    ) {
        self.check(key);
        let keys: Vec<_> = qualities
            .iter()
            .map(|d| self.process_document(d, database))
            .collect();
        let mut s = self.state.borrow_mut();
        let variants = keys
            .into_iter()
            .map(|k| s.records[&k].video_variant(quality_of(&s.records, k)))
            .collect();
        s.records
            .get_mut(&key)
            .unwrap()
            .set_video_qualities(variants);
    }
    pub fn resolve_video_quality(&self, key: DocumentRecordKey) -> i32 {
        self.check(key);
        quality_of(&self.state.borrow().records, key)
    }
    /// saved_gifs_contains is the real Stickers09 owner's pointer-membership
    /// projection. No fake Stickers model/vector is installed in this store.
    pub fn document_convert(
        &self,
        original: DocumentRecordKey,
        data: &tg_scheme::Document,
        database: &impl MimeDatabase,
        saved_gifs_contains: bool,
    ) {
        self.check(original);
        let effects = {
            let mut s = self.state.borrow_mut();
            let id = document_id(data);
            let d = s.records.get(&original).expect("document not owned");
            let old = d.id();
            let cache = d.cache_key();
            let good = d.good_thumbnail_cache_key();
            let mut effects = Vec::new();
            if old != id {
                let destination = if let Some(&k) = s.lookup.get(&id) {
                    k
                } else {
                    let k = s
                        .lookup
                        .remove(&old)
                        .expect("original C++ Assert: old document lookup absent");
                    s.lookup.insert(id, k);
                    k
                };
                let d = s.records.get_mut(&original).unwrap();
                d.id = id;
                d.status = FileStatus::Ready;
                d.uploading_data = None;
                if destination != original {
                    effects.extend(
                        self.apply_document(
                            s.records.get_mut(&destination).unwrap(),
                            data,
                            database,
                        )
                        .into_iter()
                        .map(|effect| MediaEvent::Document {
                            key: destination,
                            effect,
                        }),
                    );
                }
            }
            effects.extend(
                self.apply_document(s.records.get_mut(&original).unwrap(), data, database)
                    .into_iter()
                    .map(|effect| MediaEvent::Document {
                        key: original,
                        effect,
                    }),
            );
            if old != id {
                let d = &s.records[&original];
                effects.push(MediaEvent::MoveCacheIfEmpty {
                    from: cache,
                    to: d.cache_key(),
                });
                effects.push(MediaEvent::MoveCacheIfEmpty {
                    from: good,
                    to: d.good_thumbnail_cache_key(),
                });
                if saved_gifs_contains {
                    effects.push(MediaEvent::WriteSavedGifs);
                }
            }
            effects
        };
        self.dispatch(effects);
    }
    pub fn register_document_item(&self, key: DocumentRecordKey, item: MessageKey) {
        self.check(key);
        assert_eq!(item.scope(), self.scope);
        let song = self
            .read(key, DocumentData::is_music_for_profile)
            .expect("document not owned");
        if song {
            self.effects.fire(MediaEvent::LoadSavedMusicIds);
        }
        self.state
            .borrow_mut()
            .items
            .entry(key)
            .or_default()
            .insert(item);
    }
    pub fn unregister_document_item(&self, key: DocumentRecordKey, item: MessageKey) {
        self.check(key);
        assert_eq!(item.scope(), self.scope);
        let mut s = self.state.borrow_mut();
        if let Some(items) = s.items.get_mut(&key) {
            if items.remove(&item) && items.is_empty() {
                s.items.remove(&key);
            }
        }
    }
    pub fn document_items(&self, key: DocumentRecordKey) -> BTreeSet<MessageKey> {
        self.check(key);
        self.state
            .borrow()
            .items
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }
    pub fn clear(&self) {
        assert!(self.alive.upgrade().is_some());
        *self.state.borrow_mut() = DocumentState::default();
    }
}
fn quality_of(records: &BTreeMap<DocumentRecordKey, DocumentData>, key: DocumentRecordKey) -> i32 {
    records
        .get(&key)
        .expect("video quality document not owned")
        .resolve_video_quality(|other| quality_of(records, other))
}
