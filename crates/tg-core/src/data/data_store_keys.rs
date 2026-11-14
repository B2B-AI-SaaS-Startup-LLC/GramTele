//! Session-local identity contracts for the actual Data::Session stores.
//! No record model or ensure/loaded implementation is introduced here.
//! Source headers and conversion/lifetime gates: docs/data-changes-map.md.
use super::{
    data_msg_id::{FullMsgId, FullStoryId, MsgId},
    data_peer_id::PeerId,
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    num::NonZeroU64,
    rc::{Rc, Weak},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionScopeId(NonZeroU64);
impl SessionScopeId {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}
pub(crate) struct ScopeState {
    next_message: Cell<u64>,
}
/// Non-cloneable owner: clearing/replacing a Session creates a NEW scope.
/// This is a local lifetime token, never an MTProto session_id or wire field.
pub struct SessionScope {
    id: SessionScopeId,
    state: Rc<ScopeState>,
}
impl Default for SessionScope {
    fn default() -> Self {
        Self::new()
    }
}
impl SessionScope {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("SessionScope IDs exhausted");
        Self {
            id: SessionScopeId(NonZeroU64::new(id).unwrap()),
            state: Rc::new(ScopeState {
                next_message: Cell::new(1),
            }),
        }
    }
    pub fn id(&self) -> SessionScopeId {
        self.id
    }
    pub(crate) fn downgrade(&self) -> Weak<ScopeState> {
        Rc::downgrade(&self.state)
    }
    /// An identity, not proof that a placeholder or loaded record exists.
    /// Future ensure_peer/peer_loaded return this same identity after lookup.
    pub fn peer_key(&self, id: PeerId) -> PeerKey {
        PeerKey { scope: self.id, id }
    }
    pub fn document_key(&self, id: DocumentId) -> DocumentKey {
        DocumentKey { scope: self.id, id }
    }
    pub fn photo_key(&self, id: PhotoId) -> PhotoKey {
        PhotoKey { scope: self.id, id }
    }
    pub fn story_key(&self, id: FullStoryId) -> StoryKey {
        StoryKey { scope: self.id, id }
    }
    pub fn folder_key(&self, id: FolderId) -> FolderKey {
        FolderKey { scope: self.id, id }
    }
}
pub use super::data_types::{DocumentId, FolderId, PhotoId};

pub trait ScopedKey {
    fn scope(&self) -> SessionScopeId;
}
macro_rules! key {
    ($name:ident, $id:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            scope: SessionScopeId,
            id: $id,
        }
        impl $name {
            pub fn id(self) -> $id {
                self.id
            }
        }
        impl ScopedKey for $name {
            fn scope(&self) -> SessionScopeId {
                self.scope
            }
        }
    };
}
key!(PeerKey, PeerId);
impl PeerKey {
    pub(crate) fn from_scoped_id(scope: SessionScopeId, id: PeerId) -> Self {
        Self { scope, id }
    }
}
key!(DocumentKey, DocumentId);
key!(PhotoKey, PhotoId);
impl PhotoKey {
    pub(crate) fn from_scoped_id(scope: SessionScopeId, id: PhotoId) -> Self {
        Self { scope, id }
    }
}
key!(StoryKey, FullStoryId);
key!(FolderKey, FolderId);
macro_rules! view {
    ($name:ident, $shift:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(PeerKey);
        impl $name {
            pub fn peer(self) -> PeerKey {
                self.0
            }
        }
        impl TryFrom<PeerKey> for $name {
            type Error = PeerKey;
            fn try_from(key: PeerKey) -> Result<Self, PeerKey> {
                if key.id.is::<$shift>() {
                    Ok(Self(key))
                } else {
                    Err(key)
                }
            }
        }
        impl ScopedKey for $name {
            fn scope(&self) -> SessionScopeId {
                self.0.scope
            }
        }
    };
}
view!(UserKey, 0);
view!(ChatKey, 1);
view!(ChannelKey, 2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessageKey {
    scope: SessionScopeId,
    id: FullMsgId,
    generation: NonZeroU64,
}
impl MessageKey {
    pub fn id(self) -> FullMsgId {
        self.id
    }
    pub fn generation(self) -> NonZeroU64 {
        self.generation
    }
}
impl ScopedKey for MessageKey {
    fn scope(&self) -> SessionScopeId {
        self.scope
    }
}
/// Identity index only. The future message store owns actual HistoryItem records.
/// unregisterMessage emits Destroyed while the key is LIVE, then removes it here.
/// Wire-ID remapping must update all secondary indices (packages 13/14, 18..20).
pub struct MessageKeys {
    scope: SessionScopeId,
    alive: Weak<ScopeState>,
    keys: BTreeMap<FullMsgId, MessageKey>,
}
impl MessageKeys {
    pub fn new(scope: &SessionScope) -> Self {
        Self {
            scope: scope.id,
            alive: scope.downgrade(),
            keys: BTreeMap::new(),
        }
    }
    pub fn get(&self, id: FullMsgId) -> Option<MessageKey> {
        self.alive.upgrade()?;
        self.keys.get(&id).copied()
    }
    pub fn insert(&mut self, id: FullMsgId) -> MessageKey {
        let state = self.alive.upgrade().expect("SessionScope has ended");
        assert!(!id.is_zero(), "message lookup returns null for MsgId(0)");
        *self.keys.entry(id).or_insert_with(|| {
            let n = state.next_message.get();
            state
                .next_message
                .set(n.checked_add(1).expect("message generations exhausted"));
            MessageKey {
                scope: self.scope,
                id,
                generation: NonZeroU64::new(n).unwrap(),
            }
        })
    }
    pub fn remove(&mut self, key: MessageKey) -> bool {
        if self.get(key.id) != Some(key) {
            return false;
        }
        self.keys.remove(&key.id);
        true
    }
    pub fn contains(&self, key: MessageKey) -> bool {
        self.get(key.id) == Some(key)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopicKey {
    pub history: PeerKey,
    pub root_id: MsgId,
}
impl ScopedKey for TopicKey {
    fn scope(&self) -> SessionScopeId {
        self.history.scope
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SublistKey {
    history: PeerKey,
    peer: PeerKey,
}
impl SublistKey {
    pub fn new(history: PeerKey, peer: PeerKey) -> Self {
        assert_eq!(history.scope, peer.scope);
        Self { history, peer }
    }
    pub fn history(self) -> PeerKey {
        self.history
    }
    pub fn peer(self) -> PeerKey {
        self.peer
    }
}
impl ScopedKey for SublistKey {
    fn scope(&self) -> SessionScopeId {
        self.history.scope
    }
}
/// Dialogs::Entry::Type's four real variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntryKey {
    History(PeerKey),
    Folder(FolderKey),
    ForumTopic(TopicKey),
    SavedSublist(SublistKey),
}
impl ScopedKey for EntryKey {
    fn scope(&self) -> SessionScopeId {
        match self {
            Self::History(k) => k.scope(),
            Self::Folder(k) => k.scope(),
            Self::ForumTopic(k) => k.scope(),
            Self::SavedSublist(k) => k.scope(),
        }
    }
}

// data_group_call.h: immutable CallId, scoped to its owning Data::Session.
key!(GroupCallKey, u64);
impl SessionScope {
    pub fn group_call_key(&self, id: u64) -> GroupCallKey {
        GroupCallKey { scope: self.id, id }
    }
}
impl GroupCallKey {
    pub(crate) fn from_scoped_id(scope: SessionScopeId, id: u64) -> Self {
        Self { scope, id }
    }
}
