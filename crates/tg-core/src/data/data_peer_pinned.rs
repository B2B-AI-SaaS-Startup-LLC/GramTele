//! PeerData pinned-message algorithms over the existing real SharedMedia store.
//! History/settings updates are ordered effects for the future Session owner.
use super::{
    data_msg_id::{FullMsgId, MsgId, SERVER_MAX_MSG_ID},
    data_peer_id::PeerId,
    data_store_keys::PeerKey,
};
use crate::storage::{storage_shared_media::*, storage_sparse_ids_list::MsgRange};
#[derive(Clone, Debug)]
pub enum PinnedEffect {
    ClearHiddenPinned { peer: PeerKey },
    SaveSettingsDelayed,
    AddExisting(SharedMediaAddExisting),
    SetHistoryHasPinned { peer: PeerKey, value: bool },
}
pub fn set_top_pinned_message_id(
    peer: PeerKey,
    message: MsgId,
    available_min: Option<MsgId>,
    hidden: MsgId,
) -> Vec<PinnedEffect> {
    if available_min.is_some_and(|min| message <= min) {
        return Vec::new();
    }
    let mut effects = Vec::new();
    if hidden.bare != 0 && hidden != message {
        effects.push(PinnedEffect::ClearHiddenPinned { peer });
        effects.push(PinnedEffect::SaveSettingsDelayed);
    }
    effects.push(PinnedEffect::AddExisting(SharedMediaAddExisting {
        peer_id: peer.id().value,
        topic_root_id: 0,
        monoforum_peer_id: 0,
        message_id: message.bare,
        no_skip_range: MsgRange {
            from: message.bare,
            till: SERVER_MAX_MSG_ID.bare,
        },
        types: SharedMediaTypesMask::from(SharedMediaType::Pinned),
    }));
    effects.push(PinnedEffect::SetHistoryHasPinned { peer, value: true });
    effects
}
fn query(peer: PeerId, topic: MsgId, monoforum: PeerId, around: i64) -> SharedMediaQuery {
    SharedMediaQuery {
        key: SharedMediaKey {
            peer_id: peer.value,
            topic_root_id: topic.bare,
            monoforum_peer_id: monoforum.value,
            type_: SharedMediaType::Pinned,
            message_id: around,
        },
        limit_before: 1,
        limit_after: 1,
    }
}
pub fn resolve_top_pinned_id(
    store: &SharedMedia,
    peer: PeerId,
    topic: MsgId,
    monoforum: PeerId,
    migrated: Option<PeerId>,
) -> FullMsgId {
    let slice = store.snapshot(&query(peer, topic, monoforum, SERVER_MAX_MSG_ID.bare - 1));
    let old = if topic.is_zero() && monoforum.is_zero() {
        migrated.map(|old| {
            store.snapshot(&query(
                old,
                MsgId::new(0),
                PeerId::new(0),
                SERVER_MAX_MSG_ID.bare - 1,
            ))
        })
    } else {
        None
    };
    if let Some(last) = slice.message_ids.back() {
        FullMsgId::new(peer, MsgId::new(*last))
    } else if let (Some(migrated), Some(old)) = (migrated, old) {
        if slice.count == Some(0) {
            if let Some(last) = old.message_ids.back() {
                return FullMsgId::new(migrated, MsgId::new(*last));
            }
        }
        FullMsgId::default()
    } else {
        FullMsgId::default()
    }
}
pub fn resolve_min_pinned_id(
    store: &SharedMedia,
    peer: PeerId,
    topic: MsgId,
    monoforum: PeerId,
    migrated: Option<PeerId>,
) -> FullMsgId {
    let slice = store.snapshot(&query(peer, topic, monoforum, 1));
    let old = if topic.is_zero() && monoforum.is_zero() {
        migrated.map(|old| store.snapshot(&query(old, MsgId::new(0), PeerId::new(0), 1)))
    } else {
        None
    };
    if let (Some(migrated), Some(first)) =
        (migrated, old.as_ref().and_then(|s| s.message_ids.front()))
    {
        return FullMsgId::new(migrated, MsgId::new(*first));
    }
    if old.as_ref().is_none_or(|s| s.count == Some(0)) {
        if let Some(first) = slice.message_ids.front() {
            return FullMsgId::new(peer, MsgId::new(*first));
        }
    }
    FullMsgId::default()
}
