//! Data::Changes value updates and reentrant notification managers.
//! Source: AyuGramDesktop 0f2a31bb, data/data_changes.{h,cpp}.
use super::data_chat_participant_status::ChatAdminRights;
use super::data_store_keys::*;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::{Rc, Weak},
};
use tg_base::flags::Flags;
use tg_rpl::{EventStream, Producer};
use tg_runtime::MainQueue;

/// CountBit's power-of-two precondition, including unnamed enum bits.
pub fn count_bit(flag: u64) -> usize {
    assert!(flag.is_power_of_two(), "CountBit requires exactly one bit");
    flag.trailing_zeros() as usize
}

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerFlag {
    None = 0,
    Name = 1 << 0,
    Username = 1 << 1,
    Photo = 1 << 2,
    About = 1 << 3,
    Notifications = 1 << 4,
    Migration = 1 << 5,
    UnavailableReason = 1 << 6,
    ChatThemeToken = 1 << 7,
    ChatWallPaper = 1 << 8,
    IsBlocked = 1 << 9,
    MessagesTTL = 1 << 10,
    FullInfo = 1 << 11,
    Usernames = 1 << 12,
    TranslationDisabled = 1 << 13,
    Color = 1 << 14,
    ColorProfile = 1 << 15,
    BackgroundEmoji = 1 << 16,
    StoriesState = 1 << 17,
    VerifyInfo = 1 << 18,
    StarsPerMessage = 1 << 19,
    CanShareContact = 1 << 20,
    IsContact = 1 << 21,
    PhoneNumber = 1 << 22,
    OnlineStatus = 1 << 23,
    BotCommands = 1 << 24,
    BotCanBeInvited = 1 << 25,
    BotStartToken = 1 << 26,
    CommonChats = 1 << 27,
    PeerGifts = 1 << 28,
    HasCalls = 1 << 29,
    SupportInfo = 1 << 30,
    IsBot = 1 << 31,
    EmojiStatus = 1 << 32,
    BusinessDetails = 1 << 33,
    Birthday = 1 << 34,
    PersonalChannel = 1 << 35,
    StarRefProgram = 1 << 36,
    PaysPerMessage = 1 << 37,
    GiftSettings = 1 << 38,
    StarsRating = 1 << 39,
    ContactNote = 1 << 40,
    InviteLinks = 1 << 41,
    Members = 1 << 42,
    Admins = 1 << 43,
    BannedUsers = 1 << 44,
    Rights = 1 << 45,
    PendingRequests = 1 << 46,
    Reactions = 1 << 47,
    ChannelAmIn = 1 << 48,
    StickersSet = 1 << 49,
    EmojiSet = 1 << 50,
    DiscussionLink = 1 << 51,
    MonoforumLink = 1 << 52,
    ChannelLocation = 1 << 53,
    Slowmode = 1 << 54,
    GroupCall = 1 << 55,
    ManagedBot = 1 << 56,
    MainProfileTab = 1 << 57,
}
tg_base::flag_type!(PeerFlag: u64);
impl PeerFlag {
    pub const LAST_USED_BIT: Self = Self::MainProfileTab;
}
pub type PeerFlags = Flags<PeerFlag>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerUpdate {
    pub peer: PeerKey,
    pub flags: PeerFlags,
}
impl Update for PeerUpdate {
    type Key = PeerKey;
    const COUNT: usize = 58;
    fn key(&self) -> Self::Key {
        self.peer
    }
    fn bits(&self) -> u64 {
        self.flags.value()
    }
    fn from_bits(key: Self::Key, flags: u64) -> Self {
        Self {
            peer: key,
            flags: Flags::from_raw(flags),
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryFlag {
    None = 0,
    IsPinned = 1 << 0,
    UnreadView = 1 << 1,
    TopPromoted = 1 << 2,
    Folder = 1 << 3,
    UnreadMentions = 1 << 4,
    UnreadReactions = 1 << 5,
    ClientSideMessages = 1 << 6,
    ChatOccupied = 1 << 7,
    MessageSent = 1 << 8,
    ScheduledSent = 1 << 9,
    OutboxRead = 1 << 10,
    BotKeyboard = 1 << 11,
    CloudDraft = 1 << 12,
    TranslateFrom = 1 << 13,
    TranslatedTo = 1 << 14,
    UnreadPollVotes = 1 << 15,
    StreamedDrafts = 1 << 16,
}
tg_base::flag_type!(HistoryFlag: u32);
impl HistoryFlag {
    pub const LAST_USED_BIT: Self = Self::StreamedDrafts;
}
pub type HistoryFlags = Flags<HistoryFlag>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryUpdate {
    pub history: PeerKey,
    pub flags: HistoryFlags,
}
impl Update for HistoryUpdate {
    type Key = PeerKey;
    const COUNT: usize = 17;
    fn key(&self) -> Self::Key {
        self.history
    }
    fn bits(&self) -> u64 {
        self.flags.value() as u64
    }
    fn from_bits(key: Self::Key, flags: u64) -> Self {
        Self {
            history: key,
            flags: Flags::from_raw(flags as u32),
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopicFlag {
    None = 0,
    UnreadView = 1 << 1,
    UnreadMentions = 1 << 2,
    UnreadReactions = 1 << 3,
    Notifications = 1 << 4,
    Title = 1 << 5,
    IconId = 1 << 6,
    ColorId = 1 << 7,
    CloudDraft = 1 << 8,
    Closed = 1 << 9,
    Creator = 1 << 10,
    Destroyed = 1 << 11,
    UnreadPollVotes = 1 << 12,
}
tg_base::flag_type!(TopicFlag: u32);
impl TopicFlag {
    pub const LAST_USED_BIT: Self = Self::UnreadPollVotes;
}
pub type TopicFlags = Flags<TopicFlag>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicUpdate {
    pub topic: TopicKey,
    pub flags: TopicFlags,
}
impl Update for TopicUpdate {
    type Key = TopicKey;
    const COUNT: usize = 13;
    fn key(&self) -> Self::Key {
        self.topic
    }
    fn bits(&self) -> u64 {
        self.flags.value() as u64
    }
    fn from_bits(key: Self::Key, flags: u64) -> Self {
        Self {
            topic: key,
            flags: Flags::from_raw(flags as u32),
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SublistFlag {
    None = 0,
    UnreadView = 1 << 1,
    UnreadReactions = 1 << 2,
    CloudDraft = 1 << 3,
    Destroyed = 1 << 4,
    UnreadPollVotes = 1 << 5,
}
tg_base::flag_type!(SublistFlag: u32);
impl SublistFlag {
    pub const LAST_USED_BIT: Self = Self::UnreadPollVotes;
}
pub type SublistFlags = Flags<SublistFlag>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SublistUpdate {
    pub sublist: SublistKey,
    pub flags: SublistFlags,
}
impl Update for SublistUpdate {
    type Key = SublistKey;
    const COUNT: usize = 6;
    fn key(&self) -> Self::Key {
        self.sublist
    }
    fn bits(&self) -> u64 {
        self.flags.value() as u64
    }
    fn from_bits(key: Self::Key, flags: u64) -> Self {
        Self {
            sublist: key,
            flags: Flags::from_raw(flags as u32),
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageFlag {
    None = 0,
    Edited = 1 << 0,
    Destroyed = 1 << 1,
    DialogRowRepaint = 1 << 2,
    DialogRowRefresh = 1 << 3,
    NewAdded = 1 << 4,
    ReplyMarkup = 1 << 5,
    BotCallbackSent = 1 << 6,
    NewMaybeAdded = 1 << 7,
    ReplyToTopAdded = 1 << 8,
    NewUnreadReaction = 1 << 9,
}
tg_base::flag_type!(MessageFlag: u32);
impl MessageFlag {
    pub const LAST_USED_BIT: Self = Self::NewUnreadReaction;
}
pub type MessageFlags = Flags<MessageFlag>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageUpdate {
    pub item: MessageKey,
    pub flags: MessageFlags,
}
impl Update for MessageUpdate {
    type Key = MessageKey;
    const COUNT: usize = 10;
    fn key(&self) -> Self::Key {
        self.item
    }
    fn bits(&self) -> u64 {
        self.flags.value() as u64
    }
    fn from_bits(key: Self::Key, flags: u64) -> Self {
        Self {
            item: key,
            flags: Flags::from_raw(flags as u32),
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryFlag {
    None = 0,
    Repaint = 1 << 0,
    HasPinnedMessages = 1 << 1,
    ForwardDraft = 1 << 2,
    LocalDraftSet = 1 << 3,
    Height = 1 << 4,
    Destroyed = 1 << 5,
}
tg_base::flag_type!(EntryFlag: u32);
impl EntryFlag {
    pub const LAST_USED_BIT: Self = Self::Destroyed;
}
pub type EntryFlags = Flags<EntryFlag>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryUpdate {
    pub entry: EntryKey,
    pub flags: EntryFlags,
}
impl Update for EntryUpdate {
    type Key = EntryKey;
    const COUNT: usize = 6;
    fn key(&self) -> Self::Key {
        self.entry
    }
    fn bits(&self) -> u64 {
        self.flags.value() as u64
    }
    fn from_bits(key: Self::Key, flags: u64) -> Self {
        Self {
            entry: key,
            flags: Flags::from_raw(flags as u32),
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryFlag {
    None = 0,
    Edited = 1 << 0,
    Destroyed = 1 << 1,
    NewAdded = 1 << 2,
    ViewsChanged = 1 << 3,
    MarkRead = 1 << 4,
    Reaction = 1 << 5,
}
tg_base::flag_type!(StoryFlag: u32);
impl StoryFlag {
    pub const LAST_USED_BIT: Self = Self::Reaction;
}
pub type StoryFlags = Flags<StoryFlag>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoryUpdate {
    pub story: StoryKey,
    pub flags: StoryFlags,
}
impl Update for StoryUpdate {
    type Key = StoryKey;
    const COUNT: usize = 6;
    fn key(&self) -> Self::Key {
        self.story
    }
    fn bits(&self) -> u64 {
        self.flags.value() as u64
    }
    fn from_bits(key: Self::Key, flags: u64) -> Self {
        Self {
            story: key,
            flags: Flags::from_raw(flags as u32),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameUpdate {
    pub peer: PeerKey,
    pub old_first_letters: BTreeSet<u16>,
}
impl NameUpdate {
    pub fn new(peer: PeerKey, old_first_letters: BTreeSet<u16>) -> Self {
        Self {
            peer,
            old_first_letters,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatAdminChange {
    pub peer: PeerKey,
    pub user: UserKey,
    pub rights: ChatAdminRights,
    pub rank: Vec<u16>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMemberRankChange {
    pub peer: PeerKey,
    pub user: UserKey,
    pub rank: Vec<u16>,
}

trait Update: Clone + 'static {
    type Key: ScopedKey + Ord + Copy + 'static;
    const COUNT: usize;
    fn key(&self) -> Self::Key;
    fn bits(&self) -> u64;
    fn from_bits(key: Self::Key, flags: u64) -> Self;
}
struct Manager<U: Update> {
    realtime: Vec<EventStream<U>>,
    pending: RefCell<BTreeMap<U::Key, u64>>,
    stream: EventStream<U>,
}
impl<U: Update> Manager<U> {
    fn new() -> Self {
        Self {
            realtime: (0..U::COUNT).map(|_| EventStream::new()).collect(),
            pending: RefCell::default(),
            stream: EventStream::new(),
        }
    }
    fn updated(&self, key: U::Key, mut flags: u64, drop_scheduled: bool) {
        self.send_realtime_notifications(key, flags);
        if drop_scheduled {
            // Erase and release the mutable borrow BEFORE calling external code.
            let previous = self.pending.borrow_mut().remove(&key).unwrap_or(0);
            flags |= previous;
            self.stream.fire(U::from_bits(key, flags));
        } else {
            *self.pending.borrow_mut().entry(key).or_default() |= flags;
        }
    }
    fn send_realtime_notifications(&self, key: U::Key, flags: u64) {
        for (i, stream) in self.realtime.iter().enumerate() {
            // The original uses 1U << i, including Peer bits 32..57 (C++ UB).
            // Preserve GCC 15.3/x86_64 O0 AND O2's observed 32-bit wrap, NOT
            // 1u64 << i: high-only flags send nothing; low bits also fire i+32.
            // Source: https://github.com/AyuGram/AyuGramDesktop/blob/0f2a31bb420de98f331990432265b131b8d836b5/Telegram/SourceFiles/data/data_changes.cpp#L37
            // Native trace: tests/reference-tools/data-changes/README.md.
            let flag = u64::from(1u32.wrapping_shl(i as u32));
            if flags & flag != 0 {
                stream.fire(U::from_bits(key, flags));
            }
        }
    }
    fn updates(&self, flags: u64) -> Producer<U> {
        self.stream
            .events()
            .filter(move |update| update.bits() & flags != 0)
    }
    fn updates_for(&self, key: U::Key, flags: u64) -> Producer<U> {
        self.stream
            .events()
            .filter(move |update| update.key() == key && update.bits() & flags != 0)
    }
    fn flags_value(&self, key: U::Key, flags: u64) -> Producer<U> {
        tg_rpl::single(U::from_bits(key, flags)).then(self.updates_for(key, flags))
    }
    fn realtime_updates(&self, flag: u64) -> Producer<U> {
        self.realtime[count_bit(flag)].events()
    }
    fn drop_scheduled(&self, key: U::Key) {
        self.pending.borrow_mut().remove(&key);
    }
    fn send_notifications(&self) {
        // Source flat_map sorts pointer identities. Here identities are typed keys;
        // see the explicit ordering contract/limitation in data-changes-map.md.
        let updates = std::mem::take(&mut *self.pending.borrow_mut());
        for (key, flags) in updates {
            self.stream.fire(U::from_bits(key, flags));
        }
    }
}

/// One service per SessionScope. Clone shares the service, not model records.
/// All calls run on the main thread; producers keep tg-rpl's !Send semantics.
#[derive(Clone)]
pub struct Changes {
    inner: Rc<Inner>,
}
struct Inner {
    scope: SessionScopeId,
    alive: Weak<ScopeState>,
    queue: MainQueue,
    notify: Cell<bool>,
    name_stream: EventStream<NameUpdate>,
    chat_admin_stream: EventStream<ChatAdminChange>,
    chat_rank_stream: EventStream<ChatMemberRankChange>,
    peer: Manager<PeerUpdate>,
    history: Manager<HistoryUpdate>,
    topic: Manager<TopicUpdate>,
    sublist: Manager<SublistUpdate>,
    message: Manager<MessageUpdate>,
    entry: Manager<EntryUpdate>,
    story: Manager<StoryUpdate>,
}
impl Changes {
    pub fn new(session: &SessionScope) -> Self {
        Self::with_queue(session, tg_runtime::main_queue().clone())
    }
    pub fn with_queue(session: &SessionScope, queue: MainQueue) -> Self {
        Self {
            inner: Rc::new(Inner {
                scope: session.id(),
                alive: session.downgrade(),
                queue,
                notify: Cell::new(false),
                name_stream: EventStream::new(),
                chat_admin_stream: EventStream::new(),
                chat_rank_stream: EventStream::new(),
                peer: Manager::new(),
                history: Manager::new(),
                topic: Manager::new(),
                sublist: Manager::new(),
                message: Manager::new(),
                entry: Manager::new(),
                story: Manager::new(),
            }),
        }
    }
    pub fn session(&self) -> SessionScopeId {
        self.assert_alive();
        self.inner.scope
    }
    fn assert_alive(&self) {
        assert!(
            self.inner.alive.upgrade().is_some(),
            "SessionScope has ended"
        );
    }
    fn check(&self, key: impl ScopedKey) {
        assert_eq!(key.scope(), self.session(), "foreign SessionScope key");
    }
    pub fn name_updated(&self, peer: PeerKey, old_first_letters: BTreeSet<u16>) {
        self.check(peer);
        self.inner
            .name_stream
            .fire(NameUpdate::new(peer, old_first_letters));
    }
    pub fn realtime_name_updates(&self) -> Producer<NameUpdate> {
        self.inner.name_stream.events()
    }
    pub fn realtime_name_updates_for(&self, peer: PeerKey) -> Producer<NameUpdate> {
        self.check(peer);
        self.realtime_name_updates().filter(move |u| u.peer == peer)
    }
    pub fn peer_updated(&self, key: PeerKey, flags: impl Into<PeerFlags>) {
        self.check(key);
        let flags = flags.into().value();
        self.inner.peer.updated(key, flags, false);
        self.schedule_notifications();
    }
    pub fn peer_updates(&self, flags: impl Into<PeerFlags>) -> Producer<PeerUpdate> {
        self.inner.peer.updates(flags.into().value())
    }
    pub fn peer_updates_for(
        &self,
        key: PeerKey,
        flags: impl Into<PeerFlags>,
    ) -> Producer<PeerUpdate> {
        self.check(key);
        self.inner.peer.updates_for(key, flags.into().value())
    }
    pub fn peer_flags_value(
        &self,
        key: PeerKey,
        flags: impl Into<PeerFlags>,
    ) -> Producer<PeerUpdate> {
        self.check(key);
        self.inner.peer.flags_value(key, flags.into().value())
    }
    pub fn realtime_peer_updates(&self, flag: PeerFlag) -> Producer<PeerUpdate> {
        self.inner.peer.realtime_updates(flag as u64)
    }
    pub fn history_updated(&self, key: PeerKey, flags: impl Into<HistoryFlags>) {
        self.check(key);
        let flags = u64::from(flags.into().value());
        self.inner.history.updated(key, flags, false);
        self.schedule_notifications();
    }
    pub fn history_updates(&self, flags: impl Into<HistoryFlags>) -> Producer<HistoryUpdate> {
        self.inner.history.updates(flags.into().value() as u64)
    }
    pub fn history_updates_for(
        &self,
        key: PeerKey,
        flags: impl Into<HistoryFlags>,
    ) -> Producer<HistoryUpdate> {
        self.check(key);
        self.inner
            .history
            .updates_for(key, flags.into().value() as u64)
    }
    pub fn history_flags_value(
        &self,
        key: PeerKey,
        flags: impl Into<HistoryFlags>,
    ) -> Producer<HistoryUpdate> {
        self.check(key);
        self.inner
            .history
            .flags_value(key, flags.into().value() as u64)
    }
    pub fn realtime_history_updates(&self, flag: HistoryFlag) -> Producer<HistoryUpdate> {
        self.inner.history.realtime_updates(flag as u64)
    }
    pub fn topic_updated(&self, key: TopicKey, flags: impl Into<TopicFlags>) {
        self.check(key);
        let flags = flags.into().value() as u64;
        let drop = flags & (1 << 11) != 0;
        self.inner.topic.updated(key, flags, drop);
        if !drop {
            self.schedule_notifications();
        }
    }
    pub fn topic_updates(&self, flags: impl Into<TopicFlags>) -> Producer<TopicUpdate> {
        self.inner.topic.updates(flags.into().value() as u64)
    }
    pub fn topic_updates_for(
        &self,
        key: TopicKey,
        flags: impl Into<TopicFlags>,
    ) -> Producer<TopicUpdate> {
        self.check(key);
        self.inner
            .topic
            .updates_for(key, flags.into().value() as u64)
    }
    pub fn topic_flags_value(
        &self,
        key: TopicKey,
        flags: impl Into<TopicFlags>,
    ) -> Producer<TopicUpdate> {
        self.check(key);
        self.inner
            .topic
            .flags_value(key, flags.into().value() as u64)
    }
    pub fn realtime_topic_updates(&self, flag: TopicFlag) -> Producer<TopicUpdate> {
        self.inner.topic.realtime_updates(flag as u64)
    }
    pub fn topic_removed(&self, key: TopicKey) {
        self.check(key);
        self.inner.topic.drop_scheduled(key);
    }
    pub fn sublist_updated(&self, key: SublistKey, flags: impl Into<SublistFlags>) {
        self.check(key);
        let flags = flags.into().value() as u64;
        let drop = flags & (1 << 4) != 0;
        self.inner.sublist.updated(key, flags, drop);
        if !drop {
            self.schedule_notifications();
        }
    }
    pub fn sublist_updates(&self, flags: impl Into<SublistFlags>) -> Producer<SublistUpdate> {
        self.inner.sublist.updates(flags.into().value() as u64)
    }
    pub fn sublist_updates_for(
        &self,
        key: SublistKey,
        flags: impl Into<SublistFlags>,
    ) -> Producer<SublistUpdate> {
        self.check(key);
        self.inner
            .sublist
            .updates_for(key, flags.into().value() as u64)
    }
    pub fn sublist_flags_value(
        &self,
        key: SublistKey,
        flags: impl Into<SublistFlags>,
    ) -> Producer<SublistUpdate> {
        self.check(key);
        self.inner
            .sublist
            .flags_value(key, flags.into().value() as u64)
    }
    pub fn realtime_sublist_updates(&self, flag: SublistFlag) -> Producer<SublistUpdate> {
        self.inner.sublist.realtime_updates(flag as u64)
    }
    pub fn sublist_removed(&self, key: SublistKey) {
        self.check(key);
        self.inner.sublist.drop_scheduled(key);
    }
    pub fn message_updated(&self, key: MessageKey, flags: impl Into<MessageFlags>) {
        self.check(key);
        let flags = flags.into().value() as u64;
        let drop = flags & (1 << 1) != 0;
        self.inner.message.updated(key, flags, drop);
        if !drop {
            self.schedule_notifications();
        }
    }
    pub fn message_updates(&self, flags: impl Into<MessageFlags>) -> Producer<MessageUpdate> {
        self.inner.message.updates(flags.into().value() as u64)
    }
    pub fn message_updates_for(
        &self,
        key: MessageKey,
        flags: impl Into<MessageFlags>,
    ) -> Producer<MessageUpdate> {
        self.check(key);
        self.inner
            .message
            .updates_for(key, flags.into().value() as u64)
    }
    pub fn message_flags_value(
        &self,
        key: MessageKey,
        flags: impl Into<MessageFlags>,
    ) -> Producer<MessageUpdate> {
        self.check(key);
        self.inner
            .message
            .flags_value(key, flags.into().value() as u64)
    }
    pub fn realtime_message_updates(&self, flag: MessageFlag) -> Producer<MessageUpdate> {
        self.inner.message.realtime_updates(flag as u64)
    }
    pub fn entry_updated(&self, key: EntryKey, flags: impl Into<EntryFlags>) {
        self.check(key);
        let flags = flags.into().value() as u64;
        let drop = flags & (1 << 5) != 0;
        self.inner.entry.updated(key, flags, drop);
        if !drop {
            self.schedule_notifications();
        }
    }
    pub fn entry_updates(&self, flags: impl Into<EntryFlags>) -> Producer<EntryUpdate> {
        self.inner.entry.updates(flags.into().value() as u64)
    }
    pub fn entry_updates_for(
        &self,
        key: EntryKey,
        flags: impl Into<EntryFlags>,
    ) -> Producer<EntryUpdate> {
        self.check(key);
        self.inner
            .entry
            .updates_for(key, flags.into().value() as u64)
    }
    pub fn entry_flags_value(
        &self,
        key: EntryKey,
        flags: impl Into<EntryFlags>,
    ) -> Producer<EntryUpdate> {
        self.check(key);
        self.inner
            .entry
            .flags_value(key, flags.into().value() as u64)
    }
    pub fn realtime_entry_updates(&self, flag: EntryFlag) -> Producer<EntryUpdate> {
        self.inner.entry.realtime_updates(flag as u64)
    }
    pub fn entry_removed(&self, key: EntryKey) {
        self.check(key);
        self.inner.entry.drop_scheduled(key);
    }
    pub fn story_updated(&self, key: StoryKey, flags: impl Into<StoryFlags>) {
        self.check(key);
        let flags = flags.into().value() as u64;
        let drop = flags & (1 << 1) != 0;
        self.inner.story.updated(key, flags, drop);
        if !drop {
            self.schedule_notifications();
        }
    }
    pub fn story_updates(&self, flags: impl Into<StoryFlags>) -> Producer<StoryUpdate> {
        self.inner.story.updates(flags.into().value() as u64)
    }
    pub fn story_updates_for(
        &self,
        key: StoryKey,
        flags: impl Into<StoryFlags>,
    ) -> Producer<StoryUpdate> {
        self.check(key);
        self.inner
            .story
            .updates_for(key, flags.into().value() as u64)
    }
    pub fn story_flags_value(
        &self,
        key: StoryKey,
        flags: impl Into<StoryFlags>,
    ) -> Producer<StoryUpdate> {
        self.check(key);
        self.inner
            .story
            .flags_value(key, flags.into().value() as u64)
    }
    pub fn realtime_story_updates(&self, flag: StoryFlag) -> Producer<StoryUpdate> {
        self.inner.story.realtime_updates(flag as u64)
    }
    pub fn chat_admin_changed(
        &self,
        peer: PeerKey,
        user: UserKey,
        rights: ChatAdminRights,
        rank: Vec<u16>,
    ) {
        self.check(peer);
        self.check(user);
        self.inner.chat_admin_stream.fire(ChatAdminChange {
            peer,
            user,
            rights,
            rank,
        });
    }
    pub fn chat_admin_changes(&self) -> Producer<ChatAdminChange> {
        self.inner.chat_admin_stream.events()
    }
    pub fn chat_member_rank_changed(&self, peer: PeerKey, user: UserKey, rank: Vec<u16>) {
        self.check(peer);
        self.check(user);
        self.inner
            .chat_rank_stream
            .fire(ChatMemberRankChange { peer, user, rank });
    }
    pub fn chat_member_rank_changes(&self) -> Producer<ChatMemberRankChange> {
        self.inner.chat_rank_stream.events()
    }
    fn schedule_notifications(&self) {
        if !self.inner.notify.replace(true) {
            let weak = Rc::downgrade(&self.inner);
            self.inner.queue.post_local(move || {
                if let Some(inner) = weak.upgrade().filter(|i| i.alive.upgrade().is_some()) {
                    Changes { inner }.send_notifications();
                }
            });
        }
    }
    pub fn send_notifications(&self) {
        self.assert_alive();
        if !self.inner.notify.replace(false) {
            return;
        }
        self.inner.peer.send_notifications();
        self.inner.history.send_notifications();
        self.inner.message.send_notifications();
        self.inner.entry.send_notifications();
        self.inner.topic.send_notifications();
        self.inner.sublist.send_notifications();
        self.inner.story.send_notifications();
    }
}

#[cfg(test)]
#[path = "data_changes_tests.rs"]
mod tests;
