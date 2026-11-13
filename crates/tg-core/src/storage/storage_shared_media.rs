use super::storage_sparse_ids_list::*;
use std::collections::BTreeMap;
use tg_rpl::{EventStream, Lifetime, Producer};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SharedMediaType {
    Photo,
    Video,
    PhotoVideo,
    MusicFile,
    File,
    VoiceFile,
    Link,
    ChatPhoto,
    RoundVoiceFile,
    GIF,
    RoundFile,
    Pinned,
    Poll,
}
pub const SHARED_MEDIA_TYPE_COUNT: usize = 13;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaAddNew {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
    pub message_id: i64,
    pub types: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaAddExisting {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
    pub message_id: i64,
    pub no_skip_range: MsgRange,
    pub types: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaAddSlice {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
    pub message_ids: Vec<i64>,
    pub no_skip_range: MsgRange,
    pub type_: SharedMediaType,
    pub count: Option<i32>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaRemoveOne {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
    pub message_id: i64,
    pub types: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaRemoveAll {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
    pub types: u16,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaInvalidateBottom {
    pub peer_id: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaKey {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
    pub type_: SharedMediaType,
    pub message_id: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaQuery {
    pub key: SharedMediaKey,
    pub limit_before: i32,
    pub limit_after: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaUnloadThread {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedMediaSliceUpdate {
    pub peer_id: u64,
    pub topic_root_id: i64,
    pub monoforum_peer_id: u64,
    pub type_: SharedMediaType,
    pub data: SparseIdsSliceUpdate,
}
const TYPES: [SharedMediaType; 13] = [
    SharedMediaType::Photo,
    SharedMediaType::Video,
    SharedMediaType::PhotoVideo,
    SharedMediaType::MusicFile,
    SharedMediaType::File,
    SharedMediaType::VoiceFile,
    SharedMediaType::Link,
    SharedMediaType::ChatPhoto,
    SharedMediaType::RoundVoiceFile,
    SharedMediaType::GIF,
    SharedMediaType::RoundFile,
    SharedMediaType::Pinned,
    SharedMediaType::Poll,
];
type Key = (u64, i64, u64);
pub struct SharedMedia {
    lifetime: Lifetime,
    invalidated: EventStream<SharedMediaInvalidateBottom>,
    all_removed: EventStream<SharedMediaRemoveAll>,
    one_removed: EventStream<SharedMediaRemoveOne>,
    updated: EventStream<SharedMediaSliceUpdate>,
    lists: BTreeMap<Key, [SparseIdsList; 13]>,
}
impl Default for SharedMedia {
    fn default() -> Self {
        Self {
            lifetime: Lifetime::new(),
            invalidated: EventStream::new(),
            all_removed: EventStream::new(),
            one_removed: EventStream::new(),
            updated: EventStream::new(),
            lists: BTreeMap::new(),
        }
    }
}
impl SharedMedia {
    fn enforce_lists(&mut self, key: Key) -> &[SparseIdsList; 13] {
        self.lists.entry(key).or_insert_with(|| {
            let lists = std::array::from_fn(|_| SparseIdsList::default());
            for (i, list) in lists.iter().enumerate() {
                list.slice_updated()
                    .map(move |data| SharedMediaSliceUpdate {
                        peer_id: key.0,
                        topic_root_id: key.1,
                        monoforum_peer_id: key.2,
                        type_: TYPES[i],
                        data,
                    })
                    .start_to_stream(&self.updated, &self.lifetime);
            }
            lists
        })
    }
    fn selected(lists: &[SparseIdsList; 13], mask: u16, mut f: impl FnMut(&SparseIdsList)) {
        for (i, list) in lists.iter().enumerate() {
            if mask & (1 << i) != 0 {
                f(list);
            }
        }
    }
    pub fn add_new(&mut self, q: SharedMediaAddNew) {
        Self::selected(self.enforce_lists((q.peer_id, 0, 0)), q.types, |l| {
            l.add_new(q.message_id)
        });
        for key in [
            (q.peer_id, q.topic_root_id, 0),
            (q.peer_id, 0, q.monoforum_peer_id),
        ] {
            if (key.1 != 0 || key.2 != 0) && self.lists.contains_key(&key) {
                Self::selected(&self.lists[&key], q.types, |l| l.add_new(q.message_id));
            }
        }
    }
    pub fn add_existing(&mut self, q: SharedMediaAddExisting) {
        Self::selected(
            self.enforce_lists((q.peer_id, q.topic_root_id, q.monoforum_peer_id)),
            q.types,
            |l| l.add_existing(q.message_id, q.no_skip_range),
        );
    }
    pub fn add_slice(&mut self, q: SharedMediaAddSlice) {
        self.enforce_lists((q.peer_id, q.topic_root_id, q.monoforum_peer_id))[q.type_ as usize]
            .add_slice(q.message_ids, q.no_skip_range, q.count);
    }
    pub fn remove_one(&self, q: SharedMediaRemoveOne) {
        for key in std::iter::once((q.peer_id, 0, 0)).chain(
            [
                (q.peer_id, q.topic_root_id, 0),
                (q.peer_id, 0, q.monoforum_peer_id),
            ]
            .into_iter()
            .filter(|k| k.1 != 0 || k.2 != 0),
        ) {
            if let Some(lists) = self.lists.get(&key) {
                Self::selected(lists, q.types, |l| l.remove_one(q.message_id));
            }
        }
        self.one_removed.fire(q);
    }
    pub fn remove_all(&self, q: SharedMediaRemoveAll) {
        for (key, lists) in self
            .lists
            .range((q.peer_id, q.topic_root_id, q.monoforum_peer_id)..)
        {
            if key.0 != q.peer_id
                || (q.topic_root_id != 0 && key.1 != q.topic_root_id)
                || (q.monoforum_peer_id != 0 && key.2 != q.monoforum_peer_id)
            {
                break;
            }
            Self::selected(lists, q.types, SparseIdsList::remove_all);
        }
        self.all_removed.fire(q);
    }
    pub fn invalidate(&self, q: SharedMediaInvalidateBottom) {
        for (key, lists) in self.lists.range((q.peer_id, 0, 0)..) {
            if key.0 != q.peer_id {
                break;
            }
            for list in lists {
                list.invalidate_bottom();
            }
        }
        self.invalidated.fire(q);
    }
    pub fn unload(&mut self, q: SharedMediaUnloadThread) {
        self.lists
            .remove(&(q.peer_id, q.topic_root_id, q.monoforum_peer_id));
    }
    pub fn snapshot(&self, q: &SharedMediaQuery) -> SparseIdsListResult {
        self.lists
            .get(&(q.key.peer_id, q.key.topic_root_id, q.key.monoforum_peer_id))
            .map_or_else(Default::default, |lists| {
                lists[q.key.type_ as usize].snapshot(&SparseIdsListQuery {
                    around_id: q.key.message_id,
                    limit_before: q.limit_before,
                    limit_after: q.limit_after,
                })
            })
    }
    pub fn query(&self, q: SharedMediaQuery) -> Producer<SparseIdsListResult> {
        self.lists
            .get(&(q.key.peer_id, q.key.topic_root_id, q.key.monoforum_peer_id))
            .map_or_else(tg_rpl::complete, |lists| {
                lists[q.key.type_ as usize].query(SparseIdsListQuery {
                    around_id: q.key.message_id,
                    limit_before: q.limit_before,
                    limit_after: q.limit_after,
                })
            })
    }
    pub fn empty(&self, key: &SharedMediaKey) -> bool {
        self.lists
            .get(&(key.peer_id, key.topic_root_id, key.monoforum_peer_id))
            .is_none_or(|lists| lists[key.type_ as usize].empty())
    }
    pub fn slice_updated(&self) -> Producer<SharedMediaSliceUpdate> {
        self.updated.events()
    }
    pub fn one_removed(&self) -> Producer<SharedMediaRemoveOne> {
        self.one_removed.events()
    }
    pub fn all_removed(&self) -> Producer<SharedMediaRemoveAll> {
        self.all_removed.events()
    }
    pub fn bottom_invalidated(&self) -> Producer<SharedMediaInvalidateBottom> {
        self.invalidated.events()
    }
}
