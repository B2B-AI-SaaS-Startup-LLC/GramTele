//! The sole Peers store: common user slice and concrete chat/channel records.
//! Full Data::Session and UserData remain separate future models.
use super::{
    data_changes::{Changes, PeerFlag},
    data_peer::*,
    data_peer_id::*,
    data_store_keys::*,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Weak,
};
use tg_rpl::{EventStream, Producer};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameIndex {
    pub words: BTreeSet<Vec<u16>>,
    pub first_letters: BTreeSet<u16>,
}
impl NameIndex {
    /// Input is the real text service's PrepareSearchWords output (UTF-16).
    pub fn from_words(words: impl IntoIterator<Item = Vec<u16>>) -> Self {
        let words: BTreeSet<_> = words.into_iter().filter(|w| !w.is_empty()).collect();
        let first_letters = words.iter().map(|w| w[0]).collect();
        Self {
            words,
            first_letters,
        }
    }
}
/// Narrow access to real subclass methods; callbacks must release their store
/// borrows before firing Flags/Changes. No default or stand-in implementation.
pub enum NameFields<'a> {
    Chat,
    User {
        editable_username: &'a dyn Fn() -> String,
        name_or_phone: &'a dyn Fn() -> String,
        set_username: &'a dyn Fn(&str),
        set_name_or_phone: &'a dyn Fn(&str),
    },
    Channel {
        editable_username: &'a dyn Fn() -> String,
        username: &'a dyn Fn() -> String,
        set_username: &'a dyn Fn(&str),
        set_username_flag: &'a dyn Fn(bool),
    },
}
pub struct Peers {
    scope: SessionScopeId,
    alive: Weak<ScopeState>,
    pub(crate) peers: RefCell<BTreeMap<PeerId, PeerRecord>>,
    pub(crate) group_effects: std::rc::Rc<EventStream<super::data_group_model::GroupEffect>>,
    changes: Changes,
    effects: EventStream<PeerEffect>,
}
impl Peers {
    pub fn new(scope: &SessionScope, changes: Changes) -> Self {
        assert_eq!(scope.id(), changes.session());
        Self {
            scope: scope.id(),
            alive: scope.downgrade(),
            peers: RefCell::default(),
            group_effects: std::rc::Rc::new(EventStream::new()),
            changes,
            effects: EventStream::new(),
        }
    }
    pub(crate) fn check_scope(&self, key: PeerKey) {
        assert!(self.alive.upgrade().is_some(), "SessionScope has ended");
        assert_eq!(key.scope(), self.scope, "foreign SessionScope key");
    }
    pub fn ensure_peer(&self, id: PeerId) -> PeerKey {
        assert!(self.alive.upgrade().is_some(), "SessionScope has ended");
        let key = PeerKey::from_scoped_id(self.scope, id);
        self.peers.borrow_mut().entry(id).or_insert_with(|| {
            let p = PeerData::from_scope(self.scope, self.alive.clone(), key);
            if peer_is_chat(id) {
                PeerRecord::Chat(Box::new(super::data_chat::ChatData::new(
                    p,
                    self.group_effects.clone(),
                )))
            } else if peer_is_channel(id) {
                PeerRecord::Channel(Box::new(super::data_channel::ChannelData::new(p)))
            } else {
                PeerRecord::UserBase(Box::new(p))
            }
        });
        key
    }
    pub fn peer_loaded(&self, id: PeerId) -> Option<PeerKey> {
        self.alive.upgrade()?;
        self.peers
            .borrow()
            .get(&id)
            .filter(|p| p.peer().is_loaded())
            .map(|p| p.peer().key())
    }
    pub fn read<R>(&self, key: PeerKey, read: impl FnOnce(&PeerData) -> R) -> R {
        self.check_scope(key);
        let peers = self.peers.borrow();
        read(peers.get(&key.id()).expect("peer not ensured").peer())
    }
    pub fn update<R>(
        &self,
        key: PeerKey,
        write: impl FnOnce(&mut PeerData) -> PeerMutation<R>,
    ) -> R {
        self.check_scope(key);
        let mutation = {
            let mut peers = self.peers.borrow_mut();
            write(
                peers
                    .get_mut(&key.id())
                    .expect("peer not ensured")
                    .peer_mut(),
            )
        };
        self.dispatch(mutation.effects);
        mutation.value
    }
    pub fn effects(&self) -> Producer<PeerEffect> {
        self.effects.events()
    }
    pub fn dispatch(&self, effects: Vec<PeerEffect>) {
        for effect in effects {
            match effect {
                PeerEffect::Updated(key, flags) => self.changes.peer_updated(key, flags),
                PeerEffect::NameUpdated(key, letters) => self.changes.name_updated(key, letters),
                PeerEffect::SetStoriesState { peer, state } if peer_is_channel(peer.id()) => {
                    self.set_channel_stories_state(ChannelKey::try_from(peer).unwrap(), state)
                }
                PeerEffect::SetStoriesHidden { peer, hidden } if peer_is_channel(peer.id()) => {
                    let key = ChannelKey::try_from(peer).unwrap();
                    if hidden {
                        self.add_channel_flags(
                            key,
                            super::data_channel::ChannelDataFlag::StoriesHidden.into(),
                        );
                    } else {
                        self.remove_channel_flags(
                            key,
                            super::data_channel::ChannelDataFlag::StoriesHidden.into(),
                        );
                    }
                }
                PeerEffect::CheckPremiumUnknownPhoto { peer, .. }
                    if peer_is_channel(peer.id()) || peer_is_chat(peer.id()) => {}
                effect => self.effects.fire(effect),
            }
        }
    }
    pub fn set_bar_settings(&self, key: PeerKey, which: PeerBarSettings) {
        let (was, flags) = self.read(key, |p| (p.hide_links(), p.bar_flags()));
        flags.set(which);
        if was && !self.read(key, PeerData::hide_links) {
            self.effects
                .fire(PeerEffect::ScheduleHiddenLinksRefresh(key));
        }
    }
    pub fn clear_business_bot(&self, key: PeerKey) {
        let next = self.update(key, |p| PeerMutation::quiet(p.clear_business_bot_details()));
        if let Some(flags) = next {
            self.set_bar_settings(
                key,
                flags
                    & !PeerBarSettings::from(PeerBarSetting::BusinessBotPaused)
                    & !PeerBarSettings::from(PeerBarSetting::BusinessBotCanReply)
                    & !PeerBarSettings::from(PeerBarSetting::HasBusinessBot),
            );
        }
    }
    /// Exact updateNameDelayed phases. Name indexing is a synchronous effect on
    /// the real text service, before NameUpdate; its renderer is never required.
    pub fn update_name_delayed(
        &self,
        key: PeerKey,
        new_name: &str,
        new_name_or_phone: &str,
        new_username: &str,
        fields: NameFields<'_>,
        fill_names: impl Fn() -> NameIndex,
    ) {
        let skip = self.read(key, |p| p.raw_name() == new_name && p.name_version() > 1)
            && match &fields {
                NameFields::Chat => true,
                NameFields::User {
                    editable_username,
                    name_or_phone,
                    ..
                } => name_or_phone() == new_name_or_phone && editable_username() == new_username,
                NameFields::Channel {
                    editable_username, ..
                } => editable_username() == new_username,
            };
        if skip {
            return;
        }
        self.update(key, |p| p.assign_name(new_name));
        let (changed, old_letters) =
            self.update(key, |p| PeerMutation::quiet(p.increment_name_version()));
        let mut flags = super::data_changes::PeerFlags::empty();
        if changed {
            flags |= PeerFlag::Name;
        }
        match fields {
            NameFields::Chat => {}
            NameFields::User {
                editable_username,
                set_username,
                set_name_or_phone,
                ..
            } => {
                if editable_username() != new_username {
                    set_username(new_username);
                    flags |= PeerFlag::Username;
                }
                set_name_or_phone(new_name_or_phone);
            }
            NameFields::Channel {
                editable_username,
                username,
                set_username,
                set_username_flag,
            } => {
                if editable_username() != new_username {
                    set_username(new_username);
                    set_username_flag(!username().is_empty());
                    flags |= PeerFlag::Username;
                }
            }
        }
        let index = fill_names();
        self.update(key, |p| {
            p.apply_name_index(index);
            PeerMutation::quiet(())
        });
        if changed {
            self.changes.name_updated(key, old_letters);
        }
        if flags.any() {
            self.changes.peer_updated(key, flags);
        }
    }
    pub fn set_name(&self, key: PeerKey, name: &str, fill_names: impl Fn() -> NameIndex) {
        assert!(peer_is_chat(key.id()));
        // AyuGram filtering belongs to the real subtype setter; this entry takes
        // its already-filtered value. Empty values retain the existing name.
        let old = self.read(key, |p| p.raw_name().to_owned());
        self.update_name_delayed(
            key,
            if name.is_empty() { &old } else { name },
            "",
            "",
            NameFields::Chat,
            fill_names,
        );
    }
}

impl Peers {
    pub fn set_bar_settings_from_mtp(&self, key: PeerKey, data: &tg_scheme::PeerSettingsData) {
        let was = self.read(key, PeerData::pays_per_message);
        let bot = data
            .vbusiness_bot_id()
            .map(|id| UserKey::try_from(self.ensure_peer(peer_from_user(id))).expect("user key"));
        let flags = self.update(key, |p| {
            PeerMutation::quiet(p.prepare_bar_settings(data, bot))
        });
        self.set_bar_settings(key, flags);
        if was != self.read(key, PeerData::pays_per_message) {
            self.changes.peer_updated(key, PeerFlag::PaysPerMessage);
        }
    }
}

impl Peers {
    pub fn set_unavailable_reasons(
        &self,
        key: PeerKey,
        reasons: Vec<UnavailableReason>,
        current: impl Fn() -> Vec<UnavailableReason>,
        set: impl Fn(Vec<UnavailableReason>),
    ) {
        let (reasons, mut changed) = self.update(key, |p| p.prepare_unavailable_reasons(reasons));
        if reasons != current() {
            set(reasons);
            changed = true;
        }
        if changed {
            self.changes.peer_updated(key, PeerFlag::UnavailableReason);
        }
    }
}

impl Peers {
    /// Canonical setUserpicChecked owner phase: premium and the current photo are
    /// read AFTER realtime Photo callbacks, and the clock is read at updateFull.
    pub fn set_userpic_checked(
        &self,
        key: PeerKey,
        photo_id: u64,
        location: super::data_image_location::ImageLocation,
        has_video: bool,
        premium: impl Fn() -> bool,
        now: impl Fn() -> i64,
    ) {
        self.check_scope(key);
        let effects = {
            let mut peers = self.peers.borrow_mut();
            peers
                .get_mut(&key.id())
                .expect("peer not ensured")
                .peer_mut()
                .set_userpic_checked(photo_id, location, has_video)
                .effects
        };
        for effect in effects {
            if let PeerEffect::CheckPremiumUnknownPhoto { was_known, .. } = effect {
                if was_known && premium() && self.read(key, PeerData::userpic_photo_unknown) {
                    // PeerData::isPremium only succeeds for users, so the channel
                    // participant-request branch in updateFullForced is inapplicable.
                    assert!(peer_is_user(key.id()), "premium projection must be a user");
                    if self.read(key, |p| p.should_update_full(now())) {
                        self.effects.fire(PeerEffect::RequestFullPeer(key));
                    }
                }
            } else {
                self.dispatch(vec![effect]);
            }
        }
    }
    pub fn hide_links_with_migration(
        &self,
        key: PeerKey,
        migrated_to: &impl Fn(PeerKey) -> Option<PeerKey>,
    ) -> bool {
        if let Some(to) = migrated_to(key) {
            self.hide_links_with_migration(to, migrated_to)
        } else {
            self.read(key, PeerData::hide_links)
        }
    }
    pub fn set_bar_settings_with_migration(
        &self,
        key: PeerKey,
        which: PeerBarSettings,
        migrated_to: impl Fn(PeerKey) -> Option<PeerKey>,
        migrated_from: impl Fn(PeerKey) -> Option<PeerKey>,
    ) {
        let was = self.hide_links_with_migration(key, &migrated_to);
        let flags = self.read(key, PeerData::bar_flags);
        flags.set(which);
        if was && !self.hide_links_with_migration(key, &migrated_to) {
            self.effects
                .fire(PeerEffect::ScheduleHiddenLinksRefresh(key));
            if let Some(from) = migrated_from(key) {
                self.check_scope(from);
                self.effects
                    .fire(PeerEffect::ScheduleHiddenLinksRefresh(from));
            }
        }
    }
}

impl Peers {
    pub fn change_color(
        &self,
        key: PeerKey,
        color: Option<&tg_scheme::PeerColor>,
        collectible: impl FnOnce(Option<&tg_scheme::PeerColor>) -> bool,
    ) -> bool {
        let changed = self.update(key, |p| {
            let a = match super::data_peer::color_index_from_color(color) {
                Some(i) => p.change_color_index(i),
                None => p.clear_color_index(),
            };
            let b = p.change_background_emoji_id(super::data_peer::background_emoji_id_from_color(
                color,
            ));
            PeerMutation::quiet(a || b)
        });
        let ui_changed = collectible(color);
        changed || ui_changed
    }
    pub fn change_color_profile(
        &self,
        key: PeerKey,
        color: Option<&tg_scheme::PeerColor>,
        collectible: impl FnOnce(Option<&tg_scheme::PeerColor>) -> bool,
    ) -> bool {
        let changed = self.update(key, |p| {
            let a = match super::data_peer::color_index_from_color(color) {
                Some(i) => p.change_color_profile_index(i),
                None => p.clear_color_profile_index(),
            };
            let b = p.change_profile_background_emoji_id(
                super::data_peer::background_emoji_id_from_color(color),
            );
            PeerMutation::quiet(a || b)
        });
        let ui_changed = collectible(color);
        changed || ui_changed
    }
}

impl Peers {
    pub fn monoforum_sublist_for(
        &self,
        peer: PeerKey,
        sublist_id: PeerId,
        monoforum: Option<PeerKey>,
        lookup: impl Fn(PeerKey, PeerKey) -> Option<SublistKey>,
    ) -> Option<SublistKey> {
        self.check_scope(peer);
        if sublist_id.is_zero() {
            return None;
        }
        let monoforum = monoforum?;
        self.check_scope(monoforum);
        let sublist_peer = self.ensure_peer(sublist_id);
        lookup(monoforum, sublist_peer)
    }
}

/// Concrete group records plus the already accepted user common slice. UserData
/// itself is package02; UserBase does not pretend to be a complete UserData.
pub(crate) enum PeerRecord {
    UserBase(Box<PeerData>),
    Chat(Box<super::data_chat::ChatData>),
    Channel(Box<super::data_channel::ChannelData>),
}
impl PeerRecord {
    pub(crate) fn peer(&self) -> &PeerData {
        match self {
            Self::UserBase(p) => p,
            Self::Chat(c) => &c.peer,
            Self::Channel(c) => &c.peer,
        }
    }
    pub(crate) fn peer_mut(&mut self) -> &mut PeerData {
        match self {
            Self::UserBase(p) => p,
            Self::Chat(c) => &mut c.peer,
            Self::Channel(c) => &mut c.peer,
        }
    }
}
