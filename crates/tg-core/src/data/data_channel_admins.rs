//! Original scoped admin/rank batches. finish returns the History notification;
//! the Peers owner fires it after releasing the mutable record borrow.
use super::{data_channel::MegagroupInfo, data_peer_id::*, data_store_keys::UserKey};
use std::collections::BTreeSet;
pub struct ChannelAdminChanges {
    changes: BTreeSet<UserId>,
    old_creator: Option<UserKey>,
}
impl ChannelAdminChanges {
    pub fn new(info: &MegagroupInfo) -> Self {
        Self {
            changes: BTreeSet::new(),
            old_creator: info.creator,
        }
    }
    pub fn add(&mut self, info: &mut MegagroupInfo, user: UserId, rank: &str) {
        if info.admins.insert(user) {
            self.changes.insert(user);
        }
        if rank.is_empty() {
            if info.member_ranks.remove(&user).is_some() {
                self.changes.insert(user);
            }
        } else if info.member_ranks.get(&user).is_none_or(|r| r != rank) {
            info.member_ranks.insert(user, rank.into());
            self.changes.insert(user);
        }
    }
    pub fn remove(&mut self, info: &mut MegagroupInfo, user: UserId) {
        if info.admins.remove(&user) {
            self.changes.insert(user);
        }
    }
    pub fn finish(mut self, info: &MegagroupInfo, self_id: UserId) -> Option<BTreeSet<UserId>> {
        if info.creator != self.old_creator {
            if let Some(k) = info.creator {
                self.changes.insert(peer_to_user(k.peer().id()));
            }
            if let Some(k) = self.old_creator {
                self.changes.insert(peer_to_user(k.peer().id()));
            }
        }
        notify(self.changes, self_id)
    }
}
pub struct ChannelMemberRankChanges {
    changes: BTreeSet<UserId>,
}
impl Default for ChannelMemberRankChanges {
    fn default() -> Self {
        Self::new()
    }
}
impl ChannelMemberRankChanges {
    pub fn new() -> Self {
        Self {
            changes: BTreeSet::new(),
        }
    }
    pub fn feed(&mut self, info: &mut MegagroupInfo, user: UserId, rank: &str) {
        if rank.is_empty() {
            if info.member_ranks.remove(&user).is_some() {
                self.changes.insert(user);
            }
        } else if info.member_ranks.get(&user).is_none_or(|r| r != rank) {
            info.member_ranks.insert(user, rank.into());
            self.changes.insert(user);
        }
    }
    pub fn finish(self, info: &MegagroupInfo, self_id: UserId) -> Option<BTreeSet<UserId>> {
        notify(
            self.changes
                .into_iter()
                .filter(|id| {
                    info.admins.contains(id)
                        || info
                            .creator
                            .is_some_and(|k| peer_to_user(k.peer().id()) == *id)
                })
                .collect(),
            self_id,
        )
    }
}
fn notify(changes: BTreeSet<UserId>, self_id: UserId) -> Option<BTreeSet<UserId>> {
    (changes.len() > 1 || changes.first().is_some_and(|u| *u != self_id)).then_some(changes)
}
