//! Headless PeerData model. Mutations return ordered effects; the owning store
//! releases its mutable borrow before dispatching them to Changes/services.
use super::{
    data_changes::{PeerFlag, PeerFlags},
    data_channel::ChannelDataFlag as G,
    data_chat::ChatDataFlag as C,
    data_chat_participant_status::*,
    data_flags::Flags,
    data_image_location::ImageLocation,
    data_peer_fields::*,
    data_peer_id::*,
    data_store_keys::*,
    data_types::EmojiStatusId,
    data_user::UserDataFlag as U,
};
use std::{collections::BTreeSet, rc::Weak};
use tg_base::flags::Flags as Bits;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum PeerBarSetting {
    ReportSpam = 1 << 0,
    AddContact = 1 << 1,
    BlockContact = 1 << 2,
    ShareContact = 1 << 3,
    NeedContactsException = 1 << 4,
    AutoArchived = 1 << 5,
    RequestChat = 1 << 6,
    RequestChatIsBroadcast = 1 << 7,
    HasBusinessBot = 1 << 8,
    BusinessBotPaused = 1 << 9,
    BusinessBotCanReply = 1 << 10,
    Unknown = 1 << 11,
}
tg_base::flag_type!(PeerBarSetting:i32);
pub type PeerBarSettings = Bits<PeerBarSetting>;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeerBarDetails {
    pub phone_country_code: String,
    pub registration_date: i32,
    pub name_change_date: i32,
    pub photo_change_date: i32,
    pub request_chat_title: String,
    pub request_chat_date: i32,
    pub business_bot: Option<UserKey>,
    pub business_bot_manage_url: String,
    pub pays_per_message: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslationFlag {
    Unknown,
    Disabled,
    Enabled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockStatus {
    Unknown,
    Blocked,
    NotBlocked,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadedStatus {
    Not,
    Minimal,
    Normal,
    Full,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoriesState {
    Unknown,
    None,
    HasRead,
    HasUnread,
    HasVideoStream,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProfileTab {
    #[default]
    None,
    Posts,
    Gifts,
    Media,
    Files,
    Music,
    Voice,
    Links,
    Gifs,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestrictionCheckResult {
    Allowed,
    WithEveryone,
    Explicit,
}
impl RestrictionCheckResult {
    pub fn is_allowed(self) -> bool {
        self == Self::Allowed
    }
    pub fn is_with_everyone(self) -> bool {
        self == Self::WithEveryone
    }
    pub fn is_explicit(self) -> bool {
        self == Self::Explicit
    }
    pub fn restricted(self) -> bool {
        self != Self::Allowed
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnavailableReason {
    pub reason: String,
    pub text: String,
}
impl UnavailableReason {
    pub fn sensitive(&self) -> bool {
        self.reason == "sensitive"
    }
    pub fn sensitive_reason() -> Self {
        Self {
            reason: "sensitive".into(),
            text: String::new(),
        }
    }
    // AyuGram's pinned bodies really return constants; these are not stand-ins.
    pub fn compute(_list: &[Self]) -> String {
        String::new()
    }
    pub fn ignore_sensitive_mark() -> bool {
        true
    }
    pub fn extract(_list: Option<&[tg_scheme::RestrictionReason]>) -> Vec<Self> {
        Vec::new()
    }
}
#[derive(Clone, Debug)]
pub enum PeerEffect {
    Updated(PeerKey, PeerFlags),
    NameUpdated(PeerKey, BTreeSet<u16>),
    InvalidateEmptyUserpic(PeerKey),
    SetCloudUserpic(PeerKey, ImageLocation),
    RequestFullPeer(PeerKey),
    RequestDialogEntry(PeerKey),
    RequestSelfParticipant(PeerKey),
    RegisterEmojiStatusClear { peer: PeerKey, until: i32 },
    PreloadSensitiveContent,
    ScheduleHiddenLinksRefresh(PeerKey),
    CheckDelayedNotifications,
    CheckPremiumUnknownPhoto { peer: PeerKey, was_known: bool },
    SetUserBlocked { peer: PeerKey, blocked: bool },
    SetStoriesHidden { peer: PeerKey, hidden: bool },
    SetStoriesState { peer: PeerKey, state: StoriesState },
    RefreshChatThemesFor { peer: PeerKey, token: String },
    TogglePeerTranslations { peer: PeerKey, disabled: bool },
}
#[must_use = "dispatch effects after releasing the model/store borrow"]
pub struct PeerMutation<T> {
    pub value: T,
    pub effects: Vec<PeerEffect>,
}
impl<T> PeerMutation<T> {
    pub fn quiet(value: T) -> Self {
        Self {
            value,
            effects: Vec::new(),
        }
    }
}
/// Real common fields only. No default-specific User/Chat/Channel record exists.
pub struct PeerData {
    key: PeerKey,
    notify: super::notify::data_peer_notify_settings::PeerNotifySettings,
    alive: Weak<ScopeState>,
    name: String,
    name_version: u16,
    name_words: BTreeSet<Vec<u16>>,
    name_first_letters: BTreeSet<u16>,
    about: String,
    bar_settings: Flags<PeerBarSetting>,
    bar_details: Option<PeerBarDetails>,
    emoji_status_id: EmojiStatusId,
    background_emoji_id: u64,
    profile_background_emoji_id: u64,
    last_full_update: i64,
    ttl_period: i32,
    block_status: BlockStatus,
    loaded_status: LoadedStatus,
    translation_flag: TranslationFlag,
    main_profile_tab: ProfileTab,
    color_index: u8,
    color_index_cloud: bool,
    color_profile_index: Option<u8>,
    userpic: ImageLocation,
    userpic_photo_id: u64,
    userpic_has_video: bool,
    sensitive_content: bool,
    theme_token: String,
    checked_trusted_pay_for_message: bool,
}
impl PeerData {
    fn check_fields(&self, fields: &PeerFields<'_>) {
        assert_eq!(self.key, fields.key(), "foreign peer projection");
    }
    pub const SERVICE_NOTIFICATIONS_ID: PeerId = PeerId::from_chat_id(UserId::new(777000));
    pub const SAVED_HIDDEN_AUTHOR_ID: PeerId = PeerId::from_chat_id(UserId::new(2666000));
    pub const UNKNOWN_PHOTO_ID: u64 = u64::MAX;
    pub fn new(scope: &SessionScope, id: PeerId) -> Self {
        Self::from_scope(scope.id(), scope.downgrade(), scope.peer_key(id))
    }
    pub(crate) fn from_scope(
        _scope: SessionScopeId,
        alive: Weak<ScopeState>,
        key: PeerKey,
    ) -> Self {
        assert_eq!(_scope, key.scope());
        assert!(
            peer_is_user(key.id()) || peer_is_chat(key.id()) || peer_is_channel(key.id()),
            "unsupported PeerId tag"
        );
        Self {
            key,
            notify: super::notify::data_peer_notify_settings::PeerNotifySettings::new(),
            alive,
            name: String::new(),
            name_version: 1,
            name_words: BTreeSet::new(),
            name_first_letters: BTreeSet::new(),
            about: String::new(),
            bar_settings: Flags::new(PeerBarSetting::Unknown.into()),
            bar_details: None,
            emoji_status_id: EmojiStatusId::default(),
            background_emoji_id: 0,
            profile_background_emoji_id: 0,
            last_full_update: 0,
            ttl_period: 0,
            block_status: BlockStatus::Unknown,
            loaded_status: LoadedStatus::Not,
            translation_flag: TranslationFlag::Unknown,
            main_profile_tab: ProfileTab::None,
            color_index: decide_color_index(key.id()),
            color_index_cloud: false,
            color_profile_index: None,
            userpic: ImageLocation::default(),
            userpic_photo_id: Self::UNKNOWN_PHOTO_ID,
            userpic_has_video: false,
            sensitive_content: false,
            theme_token: String::new(),
            checked_trusted_pay_for_message: false,
        }
    }
    pub fn key(&self) -> PeerKey {
        self.key
    }
    pub fn id(&self) -> PeerId {
        self.key.id()
    }
    pub fn session(&self) -> SessionScopeId {
        assert!(self.alive.upgrade().is_some(), "SessionScope has ended");
        self.key.scope()
    }
    pub fn owner(&self) -> SessionScopeId {
        self.session()
    }
    fn changed<T>(&self, value: T, flag: PeerFlag) -> PeerMutation<T> {
        PeerMutation {
            value,
            effects: vec![PeerEffect::Updated(self.key, flag.into())],
        }
    }
    pub fn is_user(&self) -> bool {
        peer_is_user(self.id())
    }
    pub fn is_chat(&self) -> bool {
        peer_is_chat(self.id())
    }
    pub fn is_channel(&self) -> bool {
        peer_is_channel(self.id())
    }
    pub fn is_notifications_user(&self) -> bool {
        self.id() == peer_from_user(333000u64) || self.id() == Self::SERVICE_NOTIFICATIONS_ID
    }
    pub fn is_service_user(&self) -> bool {
        self.is_user() && self.id().value.is_multiple_of(1000)
    }
    pub fn is_saved_hidden_author(&self) -> bool {
        self.id() == Self::SAVED_HIDDEN_AUTHOR_ID
    }
    pub fn is_verify_codes(&self) -> bool {
        self.id() == peer_from_user(489000u64)
    }
    pub fn is_replies_chat(&self, production: bool) -> bool {
        self.id() == peer_from_user(if production { 1271266957u64 } else { 708513u64 })
    }
    pub fn is_freeze_appeal_chat(&self, fields: &PeerFields<'_>) -> bool {
        self.check_fields(fields);
        is_freeze_appeal_username(self.username(fields))
    }
    pub fn raw_name(&self) -> &str {
        &self.name
    }
    pub fn name_version(&self) -> i32 {
        self.name_version.into()
    }
    pub fn name_words(&self) -> &BTreeSet<Vec<u16>> {
        &self.name_words
    }
    pub fn name_first_letters(&self) -> &BTreeSet<u16> {
        &self.name_first_letters
    }
    pub fn about(&self) -> &str {
        &self.about
    }
    pub fn set_about(&mut self, new_about: &str) -> PeerMutation<bool> {
        if self.about == new_about {
            return PeerMutation::quiet(false);
        }
        self.about = new_about.into();
        self.changed(true, PeerFlag::About)
    }
    pub fn loaded_status(&self) -> LoadedStatus {
        self.loaded_status
    }
    pub fn set_loaded_status(&mut self, status: LoadedStatus) {
        self.loaded_status = status;
    }
    pub fn is_minimal_loaded(&self) -> bool {
        self.loaded_status != LoadedStatus::Not
    }
    pub fn is_loaded(&self) -> bool {
        matches!(
            self.loaded_status,
            LoadedStatus::Normal | LoadedStatus::Full
        )
    }
    pub fn is_full_loaded(&self) -> bool {
        self.loaded_status == LoadedStatus::Full
    }
    pub fn messages_ttl(&self) -> i32 {
        self.ttl_period
    }
    pub fn set_messages_ttl(&mut self, period: i32) -> PeerMutation<()> {
        if self.ttl_period == period {
            return PeerMutation::quiet(());
        }
        self.ttl_period = period;
        self.changed((), PeerFlag::MessagesTTL)
    }
    pub fn full_updated(&mut self, now: i64) {
        self.last_full_update = now;
        self.set_loaded_status(LoadedStatus::Full);
    }
    pub fn was_full_updated(&self) -> bool {
        self.last_full_update != 0
    }
    pub fn update_full(&self, now: i64, fields: &PeerFields<'_>) -> PeerMutation<()> {
        self.check_fields(fields);
        if self.last_full_update == 0 || now > self.last_full_update + 5000 {
            self.update_full_forced(fields)
        } else {
            PeerMutation::quiet(())
        }
    }
    pub fn update_full_forced(&self, fields: &PeerFields<'_>) -> PeerMutation<()> {
        self.check_fields(fields);
        let mut effects = vec![PeerEffect::RequestFullPeer(self.key)];
        if let PeerFields::Channel(v) = fields {
            if !fields.is_creator() && v.inviter.is_zero() {
                effects.push(PeerEffect::RequestSelfParticipant(self.key));
            }
        }
        PeerMutation { value: (), effects }
    }
    pub fn block_status(&self) -> BlockStatus {
        self.block_status
    }
    pub fn is_blocked(&self) -> bool {
        self.block_status == BlockStatus::Blocked
    }
    pub fn set_is_blocked(&mut self, blocked: bool) -> PeerMutation<()> {
        let status = if blocked {
            BlockStatus::Blocked
        } else {
            BlockStatus::NotBlocked
        };
        if status == self.block_status {
            return PeerMutation::quiet(());
        }
        self.block_status = status;
        let mut out = self.changed((), PeerFlag::IsBlocked);
        if self.is_user() {
            out.effects.insert(
                0,
                PeerEffect::SetUserBlocked {
                    peer: self.key,
                    blocked,
                },
            );
        }
        out.effects.push(PeerEffect::CheckDelayedNotifications);
        out
    }
    pub fn translation_flag(&self) -> TranslationFlag {
        self.translation_flag
    }
    pub fn set_translation_disabled(&mut self, disabled: bool) -> PeerMutation<()> {
        let f = if disabled {
            TranslationFlag::Disabled
        } else {
            TranslationFlag::Enabled
        };
        if self.translation_flag == f {
            return PeerMutation::quiet(());
        }
        self.translation_flag = f;
        self.changed((), PeerFlag::TranslationDisabled)
    }
    pub fn save_translation_disabled(&mut self, disabled: bool) -> PeerMutation<()> {
        let mut r = self.set_translation_disabled(disabled);
        r.effects.push(PeerEffect::TogglePeerTranslations {
            peer: self.key,
            disabled,
        });
        r
    }
    pub fn color_index(&self) -> u8 {
        self.color_index
    }
    pub fn change_color_index(&mut self, index: u8) -> bool {
        let index = index % 64;
        if self.color_index_cloud && self.color_index == index {
            return false;
        }
        self.color_index_cloud = true;
        self.color_index = index;
        true
    }
    pub fn clear_color_index(&mut self) -> bool {
        if !self.color_index_cloud {
            return false;
        }
        self.color_index_cloud = false;
        self.color_index = decide_color_index(self.id());
        true
    }
    pub fn color_profile_index(&self) -> Option<u8> {
        self.color_profile_index
    }
    pub fn change_color_profile_index(&mut self, index: u8) -> bool {
        let index = index % 64;
        if self.color_profile_index == Some(index) {
            return false;
        }
        self.color_profile_index = Some(index);
        true
    }
    pub fn clear_color_profile_index(&mut self) -> bool {
        self.color_profile_index.take().is_some()
    }
    pub fn background_emoji_id(&self) -> u64 {
        self.background_emoji_id
    }
    pub fn change_background_emoji_id(&mut self, id: u64) -> bool {
        if self.background_emoji_id == id {
            return false;
        }
        self.background_emoji_id = id;
        true
    }
    pub fn profile_background_emoji_id(&self) -> u64 {
        self.profile_background_emoji_id
    }
    pub fn change_profile_background_emoji_id(&mut self, id: u64) -> bool {
        if self.profile_background_emoji_id == id {
            return false;
        }
        self.profile_background_emoji_id = id;
        true
    }
    pub fn emoji_status_id(&self) -> &EmojiStatusId {
        &self.emoji_status_id
    }
    pub fn set_emoji_status(&mut self, id: EmojiStatusId, until: i32) -> PeerMutation<()> {
        let mut r = PeerMutation::quiet(());
        if self.emoji_status_id != id {
            self.emoji_status_id = id;
            r.effects
                .push(PeerEffect::Updated(self.key, PeerFlag::EmojiStatus.into()));
        }
        r.effects.push(PeerEffect::RegisterEmojiStatusClear {
            peer: self.key,
            until,
        });
        r
    }
    pub fn main_profile_tab(&self) -> ProfileTab {
        self.main_profile_tab
    }
    pub fn set_main_profile_tab(&mut self, tab: ProfileTab) -> PeerMutation<()> {
        if self.main_profile_tab == tab {
            return PeerMutation::quiet(());
        }
        self.main_profile_tab = tab;
        self.changed((), PeerFlag::MainProfileTab)
    }
    pub fn bar_settings(&self) -> Option<PeerBarSettings> {
        let f = self.bar_settings.current();
        if (f & PeerBarSetting::Unknown).any() {
            None
        } else {
            Some(f)
        }
    }
    pub fn bar_settings_value(
        &self,
    ) -> tg_rpl::Producer<super::data_flags::Change<PeerBarSetting>> {
        if self.bar_settings().is_none() {
            self.bar_settings.changes()
        } else {
            self.bar_settings.value()
        }
    }
    pub(crate) fn bar_flags(&self) -> Flags<PeerBarSetting> {
        self.bar_settings.clone()
    }
    pub fn hide_links(&self) -> bool {
        self.bar_settings()
            .is_none_or(|f| (f & PeerBarSetting::ReportSpam).any())
    }
    pub fn pays_per_message(&self) -> i32 {
        self.bar_details.as_ref().map_or(0, |v| v.pays_per_message)
    }
    pub fn clear_pays_per_message(&mut self) -> PeerMutation<()> {
        let Some(d) = self.bar_details.as_mut() else {
            return PeerMutation::quiet(());
        };
        if d.pays_per_message == 0 {
            return PeerMutation::quiet(());
        }
        if d.business_bot.is_some() || d.request_chat_date != 0 || !d.phone_country_code.is_empty()
        {
            d.pays_per_message = 0;
        } else {
            self.bar_details = None;
        }
        self.changed((), PeerFlag::PaysPerMessage)
    }
    pub fn business_bot(&self) -> Option<UserKey> {
        self.bar_details.as_ref().and_then(|v| v.business_bot)
    }
    pub fn request_chat_title(&self) -> &str {
        self.bar_details
            .as_ref()
            .map_or("", |v| v.request_chat_title.as_str())
    }
    pub fn request_chat_date(&self) -> i32 {
        self.bar_details.as_ref().map_or(0, |v| v.request_chat_date)
    }
    pub fn phone_country_code(&self) -> &str {
        self.bar_details
            .as_ref()
            .map_or("", |v| v.phone_country_code.as_str())
    }
    pub fn business_bot_manage_url(&self) -> &str {
        self.bar_details
            .as_ref()
            .map_or("", |v| v.business_bot_manage_url.as_str())
    }
    pub fn registration_month(&self) -> i32 {
        self.bar_details
            .as_ref()
            .map_or(0, |v| registration_month(v.registration_date))
    }
    pub fn registration_year(&self) -> i32 {
        self.bar_details
            .as_ref()
            .map_or(0, |v| registration_year(v.registration_date))
    }
    pub fn name_change_date(&self) -> i32 {
        self.bar_details.as_ref().map_or(0, |v| v.name_change_date)
    }
    pub fn photo_change_date(&self) -> i32 {
        self.bar_details.as_ref().map_or(0, |v| v.photo_change_date)
    }
    pub fn has_sensitive_content(&self) -> bool {
        self.sensitive_content
    }
    /// The base virtual implementation returns an empty list. Actual subclass
    /// overrides are supplied through Peers::set_unavailable_reasons callbacks.
    pub fn unavailable_reasons(&self) -> &[UnavailableReason] {
        &[]
    }
    pub fn set_unavailable_reasons_list(&mut self, _reasons: Vec<UnavailableReason>) {
        // data_peer.cpp:627: the base virtual body calls Unexpected, even for
        // an empty list; only actual subclass overrides may store a list.
        panic!("PeerData::setUnavailableReasonsList.");
    }
    pub(crate) fn prepare_unavailable_reasons(
        &mut self,
        mut reasons: Vec<UnavailableReason>,
    ) -> PeerMutation<(Vec<UnavailableReason>, bool)> {
        let sensitive = reasons.iter().position(UnavailableReason::sensitive);
        if let Some(i) = sensitive {
            reasons.remove(i);
        }
        let has = sensitive.is_some();
        let changed = has != self.sensitive_content;
        let mut r = PeerMutation::quiet((reasons, changed));
        if changed {
            self.sensitive_content = has;
            if has {
                r.effects.push(PeerEffect::PreloadSensitiveContent);
            }
        }
        r
    }
    pub fn userpic_location(&self) -> &ImageLocation {
        &self.userpic
    }
    pub fn has_userpic(&self) -> bool {
        self.userpic.valid()
    }
    pub fn userpic_photo_unknown(&self) -> bool {
        self.userpic_photo_id == Self::UNKNOWN_PHOTO_ID
    }
    pub fn userpic_photo_id(&self) -> u64 {
        if self.userpic_photo_unknown() {
            0
        } else {
            self.userpic_photo_id
        }
    }
    pub fn userpic_has_video(&self) -> bool {
        self.userpic_has_video
    }
    pub fn set_userpic(
        &mut self,
        photo_id: u64,
        location: ImageLocation,
        has_video: bool,
    ) -> PeerMutation<()> {
        self.userpic_photo_id = photo_id;
        self.userpic_has_video = has_video;
        self.userpic = location.clone();
        PeerMutation {
            value: (),
            effects: vec![PeerEffect::SetCloudUserpic(self.key, location)],
        }
    }
    pub fn set_userpic_checked(
        &mut self,
        photo_id: u64,
        location: ImageLocation,
        has_video: bool,
    ) -> PeerMutation<()> {
        if self.userpic_photo_id == photo_id
            && self.userpic == location
            && self.userpic_has_video == has_video
        {
            return PeerMutation::quiet(());
        }
        let known = !self.userpic_photo_unknown();
        let mut r = self.set_userpic(photo_id, location, has_video);
        r.effects
            .push(PeerEffect::Updated(self.key, PeerFlag::Photo.into()));
        if known {
            r.effects.push(PeerEffect::CheckPremiumUnknownPhoto {
                peer: self.key,
                was_known: known,
            });
        }
        r
    }
    pub fn clear_userpic(&mut self) -> PeerMutation<()> {
        self.set_userpic_checked(0, ImageLocation::default(), false)
    }
    pub fn userpic_photo_key(&self) -> Option<PhotoKey> {
        (!self.userpic_photo_unknown() && self.userpic_photo_id != 0)
            .then(|| PhotoKey::from_scoped_id(self.key.scope(), self.userpic_photo_id))
    }
    pub fn username<'a>(&self, fields: &'a PeerFields<'_>) -> &'a str {
        self.check_fields(fields);
        fields.usernames().map_or("", |n| n.username())
    }
    pub fn editable_username<'a>(&self, fields: &'a PeerFields<'_>) -> &'a str {
        self.check_fields(fields);
        fields.usernames().map_or("", |n| n.editable_username())
    }
    pub fn usernames<'a>(&self, fields: &'a PeerFields<'_>) -> &'a [String] {
        self.check_fields(fields);
        fields.usernames().map_or(&[], |n| n.usernames())
    }
    pub fn is_username_editable(&self, fields: &PeerFields<'_>, username: &str) -> bool {
        self.check_fields(fields);
        fields.usernames().is_some_and(|n| n.is_editable(username))
    }
}
pub fn decide_color_index(id: PeerId) -> u8 {
    ((id.value & PeerId::CHAT_TYPE_MASK) % 7) as u8
}
pub fn fake_peer_id_for_just_name(name: &str) -> PeerId {
    let bytes: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let base = if name.is_empty() {
        777
    } else {
        tg_base::crc32hash::crc32(&bytes)
    };
    peer_from_user((0xfeu64 << 32).wrapping_add(base.wrapping_abs() as u64))
}
pub fn parse_registration_date(text: &str) -> i32 {
    let c: Vec<u16> = text.encode_utf16().collect();
    if c.len() != 7 || c[2] != u16::from(b'.') {
        return 0;
    }
    let month = String::from_utf16_lossy(&c[..2])
        .trim()
        .parse::<i32>()
        .unwrap_or(0);
    let year = String::from_utf16_lossy(&c[3..])
        .trim()
        .parse::<i32>()
        .unwrap_or(0);
    if (2013..2100).contains(&year) && (1..=12).contains(&month) {
        year * 100 + month
    } else {
        0
    }
}
pub fn registration_year(date: i32) -> i32 {
    let y = date / 100;
    if (2013..2100).contains(&y) {
        y
    } else {
        0
    }
}
pub fn registration_month(date: i32) -> i32 {
    let m = date % 100;
    if (1..=12).contains(&m) {
        m
    } else {
        0
    }
}

impl PeerData {
    pub fn is_verified(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => (v.flags.current() & U::Verified).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::Verified).any(),
            PeerFields::Chat(_) => false,
        }
    }
    pub fn is_scam(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => (v.flags.current() & U::Scam).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::Scam).any(),
            PeerFields::Chat(_) => false,
        }
    }
    pub fn is_fake(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => (v.flags.current() & U::Fake).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::Fake).any(),
            PeerFields::Chat(_) => false,
        }
    }
    pub fn has_stories_hidden(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => (v.flags.current() & U::StoriesHidden).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::StoriesHidden).any(),
            PeerFields::Chat(_) => false,
        }
    }
    pub fn has_active_stories(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => (v.flags.current() & U::HasActiveStories).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::HasActiveStories).any(),
            PeerFields::Chat(_) => false,
        }
    }
    pub fn has_unread_stories(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => (v.flags.current() & U::HasUnreadStories).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::HasUnreadStories).any(),
            PeerFields::Chat(_) => false,
        }
    }
    pub fn has_active_video_stream(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => (v.flags.current() & U::HasActiveVideoStream).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::HasActiveVideoStream).any(),
            PeerFields::Chat(_) => false,
        }
    }
    pub fn is_megagroup(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if (v.flags.current()&G::Megagroup).any())
    }
    pub fn is_broadcast(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if (v.flags.current()&G::Broadcast).any())
    }
    pub fn is_monoforum(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if (v.flags.current()&G::Monoforum).any())
    }
    pub fn is_gigagroup(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if (v.flags.current()&G::Gigagroup).any())
    }
    pub fn auto_translation(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if (v.flags.current()&G::AutoTranslation).any())
    }
    pub fn am_monoforum_admin(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if (v.flags.current()&G::MonoforumAdmin).any())
    }
    pub fn view_forum_as_messages(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if (v.flags.current()&G::ViewAsMessages).any())
    }

    pub fn is_self(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::User(v) if (v.flags.current()&U::SelfUser).any())
    }
    pub fn is_bot(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::User(v) if v.bot_info_present)
    }
    pub fn is_premium(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::User(v) if v.effective_premium)
    }
    pub fn is_forum(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => v.bot_info_present && (v.flags.current() & U::Forum).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::Forum).any(),
            _ => false,
        }
    }
    pub fn shared_media_info(&self, f: &PeerFields<'_>, production: bool) -> bool {
        self.check_fields(f);
        self.is_self(f) || self.is_replies_chat(production)
    }
    pub fn saved_sublists_info(&self, f: &PeerFields<'_>, supported: bool) -> bool {
        self.check_fields(f);
        self.is_self(f) && supported
    }
    pub fn stars_per_message(&self, f: &PeerFields<'_>) -> i32 {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => v.stars_per_message,
            PeerFields::Channel(v) => v.stars_per_message,
            _ => 0,
        }
    }
    pub fn stars_per_message_checked(&self, f: &PeerFields<'_>) -> i32 {
        self.check_fields(f);
        if matches!(f, PeerFields::Channel(_))
            && (f.admin_rights().any() || f.is_creator() || self.am_monoforum_admin(f))
        {
            0
        } else {
            self.stars_per_message(f)
        }
    }
    pub fn stars_rating(&self, f: &PeerFields<'_>) -> super::data_peer_common::StarsRating {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => v.stars_rating,
            _ => Default::default(),
        }
    }
    pub fn peer_gifts_count(&self, f: &PeerFields<'_>) -> i32 {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => v.peer_gifts_count,
            PeerFields::Channel(v) => v.peer_gifts_count,
            _ => 0,
        }
    }
    pub fn is_ayu_no_forwards(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => {
                (v.flags.current() & (U::NoForwardsMyEnabled | U::NoForwardsPeerEnabled)).any()
            }
            PeerFields::Chat(v) => (v.flags.current() & C::AyuNoForwards).any(),
            PeerFields::Channel(v) => (v.flags.current() & G::AyuNoForwards).any(),
        }
    }
    pub fn allows_forwarding(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(_) => true,
            PeerFields::Chat(v) => (v.flags.current() & C::NoForwards).is_empty(),
            PeerFields::Channel(v) => (v.flags.current() & G::NoForwards).is_empty(),
        }
    }
    pub fn am_restricted(
        &self,
        f: &PeerFields<'_>,
        right: ChatRestriction,
        session_premium: bool,
    ) -> RestrictionCheckResult {
        self.check_fields(f);
        use RestrictionCheckResult::*;
        let admin = |rights: ChatAdminRights| -> bool {
            match right {
                ChatRestriction::AddParticipants => {
                    (rights & ChatAdminRight::InviteByLinkOrAdd).any()
                }
                ChatRestriction::ChangeInfo => (rights & ChatAdminRight::ChangeInfo).any(),
                ChatRestriction::CreateTopics => (rights & ChatAdminRight::ManageTopics).any(),
                ChatRestriction::PinMessages => (rights & ChatAdminRight::PinMessages).any(),
                _ => rights.any(),
            }
        };
        match f {
            PeerFields::User(v) => {
                let flags = v.flags.current();
                if !self.is_self(f) && (flags & U::RequiresPremiumToWrite).any() && !session_premium
                {
                    return Explicit;
                }
                if matches!(
                    right,
                    ChatRestriction::SendVoiceMessages | ChatRestriction::SendVideoMessages
                ) {
                    if (flags & U::VoiceMessagesForbidden).any() {
                        Explicit
                    } else {
                        Allowed
                    }
                } else if right == ChatRestriction::PinMessages {
                    if (flags & U::CanPinMessages).any() {
                        Allowed
                    } else {
                        Explicit
                    }
                } else {
                    Allowed
                }
            }
            PeerFields::Chat(v) => {
                if f.is_creator() || admin(v.admin_rights) {
                    Allowed
                } else if (v.default_restrictions & right).any() {
                    WithEveryone
                } else {
                    Allowed
                }
            }
            PeerFields::Channel(v) => {
                if (v.flags.current() & G::MonoforumDisabled).any() {
                    return WithEveryone;
                }
                let defaults = v.default_restrictions
                    | if (v.flags.current() & (G::Username | G::Location)).any() {
                        ChatRestriction::PinMessages | ChatRestriction::ChangeInfo
                    } else {
                        ChatRestrictions::empty()
                    };
                if f.is_creator() || admin(v.admin_rights) {
                    Allowed
                } else if (defaults & right).any()
                    && !(v.has_megagroup_info
                        && v.boosts_unrestrict > 0
                        && v.boosts_applied >= v.boosts_unrestrict)
                {
                    WithEveryone
                } else if (v.restrictions & right).any() {
                    Explicit
                } else {
                    Allowed
                }
            }
        }
    }
    pub fn am_anonymous(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        if let PeerFields::Channel(v) = f {
            if self.is_broadcast(f) {
                (v.flags.current() & G::SignatureProfiles).is_empty()
            } else {
                (v.admin_rights & ChatAdminRight::Anonymous).any()
            }
        } else {
            false
        }
    }
    pub fn slowmode_applied(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::Channel(v) if !f.is_creator()&&v.admin_rights.is_empty()&&(v.flags.current()&G::SlowmodeEnabled).any())
    }
    pub fn slowmode_seconds_left(&self, f: &PeerFields<'_>, now: i32) -> i32 {
        self.check_fields(f);
        if let PeerFields::Channel(v) = f {
            let unrestricted = v.has_megagroup_info
                && v.boosts_unrestrict > 0
                && v.boosts_applied >= v.boosts_unrestrict;
            let last = if f.is_creator()
                || v.admin_rights.any()
                || unrestricted
                || !v.has_megagroup_info
            {
                0
            } else {
                v.slowmode_last_message
            };
            if v.slowmode_seconds != 0 && last != 0 {
                return v
                    .slowmode_seconds
                    .wrapping_sub(now.wrapping_sub(last))
                    .max(0);
            }
        }
        0
    }
    pub fn can_manage_group_call(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(_) => self.is_self(f),
            PeerFields::Chat(v) => {
                f.is_creator() || (v.admin_rights & ChatAdminRight::ManageCall).any()
            }
            PeerFields::Channel(v) => {
                !self.is_monoforum(f)
                    && (f.is_creator() || (v.admin_rights & ChatAdminRight::ManageCall).any())
            }
        }
    }
    pub fn can_manage_ranks(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(_) => false,
            PeerFields::Chat(v) => {
                f.is_creator() || (v.admin_rights & ChatAdminRight::ManageRanks).any()
            }
            PeerFields::Channel(v) => {
                (v.flags.current() & G::Community).is_empty()
                    && (f.is_creator() || (v.admin_rights & ChatAdminRight::ManageRanks).any())
            }
        }
    }
    pub fn can_manage_welcome_messages(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        !matches!(f, PeerFields::User(_))
            && (f.is_creator() || (f.admin_rights() & ChatAdminRight::ManageWelcomeMessages).any())
    }
    pub fn can_pin_messages(&self, f: &PeerFields<'_>, premium: bool) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(_) => !self
                .am_restricted(f, ChatRestriction::PinMessages, premium)
                .restricted(),
            PeerFields::Chat(v) => {
                (v.flags.current() & (C::Forbidden | C::Deactivated | C::Left)).is_empty()
                    && !self
                        .am_restricted(f, ChatRestriction::PinMessages, premium)
                        .restricted()
            }
            PeerFields::Channel(v) => {
                if self.is_megagroup(f) {
                    !self
                        .am_restricted(f, ChatRestriction::PinMessages, premium)
                        .restricted()
                } else {
                    f.is_creator() || (v.admin_rights & ChatAdminRight::EditMessages).any()
                }
            }
        }
    }
    pub fn set_stories_hidden(&self, hidden: bool) -> PeerMutation<()> {
        assert!(!self.is_chat());
        PeerMutation {
            value: (),
            effects: vec![PeerEffect::SetStoriesHidden {
                peer: self.key,
                hidden,
            }],
        }
    }
    pub fn set_stories_state(&self, state: StoriesState) -> PeerMutation<()> {
        assert!(!self.is_chat());
        assert_ne!(state, StoriesState::Unknown);
        PeerMutation {
            value: (),
            effects: vec![PeerEffect::SetStoriesState {
                peer: self.key,
                state,
            }],
        }
    }
    pub fn check_trusted_pay_for_message(
        &mut self,
        stars: i32,
        read: bool,
        has_entry: bool,
    ) -> bool {
        if !self.checked_trusted_pay_for_message && stars == 0 && read {
            self.checked_trusted_pay_for_message = true;
            has_entry
        } else {
            false
        }
    }
}

impl PeerData {
    pub(crate) fn assign_name(&mut self, value: &str) -> PeerMutation<()> {
        self.name = value.into();
        PeerMutation {
            value: (),
            effects: vec![PeerEffect::InvalidateEmptyUserpic(self.key)],
        }
    }
    pub(crate) fn increment_name_version(&mut self) -> (bool, BTreeSet<u16>) {
        let changed = self.name_version > 1;
        self.name_version = self.name_version.wrapping_add(1);
        (
            changed,
            if changed {
                self.name_first_letters.clone()
            } else {
                BTreeSet::new()
            },
        )
    }
    pub(crate) fn apply_name_index(&mut self, index: super::data_session::NameIndex) {
        self.name_words = index.words;
        self.name_first_letters = index.first_letters;
    }
    pub(crate) fn clear_business_bot_details(&mut self) -> Option<PeerBarSettings> {
        if let Some(d) = self.bar_details.as_mut() {
            if d.request_chat_date != 0
                || d.pays_per_message != 0
                || !d.phone_country_code.is_empty()
            {
                d.business_bot = None;
                d.business_bot_manage_url.clear();
            } else {
                self.bar_details = None;
            }
        }
        self.bar_settings()
    }
    pub fn apply_bar_details(&mut self, details: Option<PeerBarDetails>) {
        self.bar_details = details;
    }
    pub fn prepare_bar_settings(
        &mut self,
        data: &tg_scheme::PeerSettingsData,
        business_bot: Option<UserKey>,
    ) -> PeerBarSettings {
        if data.vbusiness_bot_id().is_none()
            && data.vrequest_chat_title().is_none()
            && data.vcharge_paid_message_stars().is_none()
            && data.vphone_country().is_none()
            && data.vregistration_month().is_none()
            && data.vname_change_date().is_none()
            && data.vphoto_change_date().is_none()
        {
            self.bar_details = None;
        } else {
            assert_eq!(
                business_bot.map(|k| k.peer().id().to::<0>().bare),
                data.vbusiness_bot_id()
            );
            if let Some(bot) = business_bot {
                assert_eq!(bot.scope(), self.key.scope());
            }
            let text = |v: Option<&Vec<u8>>| {
                String::from_utf8_lossy(v.map_or(&[], Vec::as_slice)).into_owned()
            };
            self.bar_details = Some(PeerBarDetails {
                phone_country_code: text(data.vphone_country()),
                registration_date: parse_registration_date(&text(data.vregistration_month())),
                name_change_date: data.vname_change_date().unwrap_or(0),
                photo_change_date: data.vphoto_change_date().unwrap_or(0),
                request_chat_title: text(data.vrequest_chat_title()),
                request_chat_date: data.vrequest_chat_date().unwrap_or(0),
                business_bot,
                business_bot_manage_url: text(data.vbusiness_bot_manage_url()),
                pays_per_message: data.vcharge_paid_message_stars().unwrap_or(0) as i32,
            });
        }
        let mut bits = PeerBarSettings::empty();
        for (yes, flag) in [
            (data.is_add_contact(), PeerBarSetting::AddContact),
            (data.is_autoarchived(), PeerBarSetting::AutoArchived),
            (data.is_block_contact(), PeerBarSetting::BlockContact),
            (
                data.is_need_contacts_exception(),
                PeerBarSetting::NeedContactsException,
            ),
            (data.is_report_spam(), PeerBarSetting::ReportSpam),
            (data.is_share_contact(), PeerBarSetting::ShareContact),
            (
                data.vrequest_chat_title().is_some(),
                PeerBarSetting::RequestChat,
            ),
            (
                data.vbusiness_bot_id().is_some(),
                PeerBarSetting::HasBusinessBot,
            ),
            (
                data.is_request_chat_broadcast(),
                PeerBarSetting::RequestChatIsBroadcast,
            ),
            (
                data.is_business_bot_paused(),
                PeerBarSetting::BusinessBotPaused,
            ),
            (
                data.is_business_bot_can_reply(),
                PeerBarSetting::BusinessBotCanReply,
            ),
        ] {
            if yes {
                bits |= flag;
            }
        }
        bits
    }
}
/// The producer deliberately differs from slowmodeApplied(): original doesn't
/// fold Creator into hasAdminRights. Callers supply the real admin-rights value.
pub fn channel_slowmode_applied_value(
    has_admin_rights: tg_rpl::Producer<bool>,
    flags: tg_rpl::Producer<super::data_flags::Change<G>>,
) -> tg_rpl::Producer<bool> {
    let rights = has_admin_rights.distinct_until_changed();
    let enabled = flags
        .filter(|c| (c.diff & G::SlowmodeEnabled).any())
        .map(|c| (c.value & G::SlowmodeEnabled).any())
        .distinct_until_changed();
    tg_rpl::combine((rights, enabled)).map(|(admin, enabled)| !admin && enabled)
}

impl PeerData {
    pub fn set_theme_token(
        &mut self,
        token: &str,
        same_emoji: bool,
        theme_known: bool,
    ) -> PeerMutation<()> {
        if self.theme_token == token {
            return PeerMutation::quiet(());
        }
        self.theme_token = token.into();
        if !token.starts_with("gift:") && same_emoji {
            return PeerMutation::quiet(());
        }
        let mut r = self.changed((), PeerFlag::ChatThemeToken);
        if !token.is_empty() && !theme_known {
            r.effects.insert(
                0,
                PeerEffect::RefreshChatThemesFor {
                    peer: self.key,
                    token: token.into(),
                },
            );
        }
        r
    }
    pub fn theme_token(&self) -> &str {
        &self.theme_token
    }
    pub fn as_user(&self) -> Option<UserKey> {
        UserKey::try_from(self.key).ok()
    }
    pub fn as_chat(&self) -> Option<ChatKey> {
        ChatKey::try_from(self.key).ok()
    }
    pub fn as_channel(&self) -> Option<ChannelKey> {
        ChannelKey::try_from(self.key).ok()
    }
    pub fn as_bot(&self, f: &PeerFields<'_>) -> Option<UserKey> {
        self.is_bot(f).then(|| self.as_user().unwrap())
    }
    pub fn as_megagroup(&self, f: &PeerFields<'_>) -> Option<ChannelKey> {
        self.is_megagroup(f).then(|| self.as_channel().unwrap())
    }
    pub fn as_broadcast(&self, f: &PeerFields<'_>) -> Option<ChannelKey> {
        self.is_broadcast(f).then(|| self.as_channel().unwrap())
    }
    pub fn as_monoforum(&self, f: &PeerFields<'_>) -> Option<ChannelKey> {
        (self.is_megagroup(f) && self.is_monoforum(f)).then(|| self.as_channel().unwrap())
    }
    pub fn short_name<'a>(&'a self, f: &'a PeerFields<'_>) -> &'a str {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => {
                if v.first_name.is_empty() {
                    v.last_name
                } else {
                    v.first_name
                }
            }
            _ => &self.name,
        }
    }
    pub fn top_bar_name_text<'a>(&'a self, f: &'a PeerFields<'_>) -> &'a str {
        self.check_fields(f);
        match f {
            PeerFields::User(v) if !v.name_or_phone.is_empty() => v.name_or_phone,
            _ => &self.name,
        }
    }
    pub fn can_create_polls(
        &self,
        f: &PeerFields<'_>,
        production: bool,
        forbid_in_forums: bool,
        can_send: impl Fn(ChatRestriction, bool) -> bool,
    ) -> bool {
        self.check_fields(f);
        if let PeerFields::User(v) = f {
            return self.is_self(f)
                || (v.bot_info_present
                    && (v.flags.current() & U::Support).is_empty()
                    && !self.is_replies_chat(production)
                    && !self.is_verify_codes());
        }
        if self.is_monoforum(f) {
            return false;
        }
        can_send(ChatRestriction::SendPolls, forbid_in_forums)
    }
    pub fn can_create_todo_lists(
        &self,
        f: &PeerFields<'_>,
        session_premium: bool,
        forbid_in_forums: bool,
        can_send: impl Fn(ChatRestriction, bool) -> bool,
    ) -> bool {
        self.check_fields(f);
        !self.is_monoforum(f)
            && !self.is_broadcast(f)
            && session_premium
            && (can_send(ChatRestriction::SendPolls, forbid_in_forums) || self.is_user())
    }
    pub fn can_create_topics(&self, f: &PeerFields<'_>, premium: bool) -> bool {
        self.check_fields(f);
        if self.is_bot(f) {
            return self.is_forum(f);
        }
        matches!(f, PeerFields::Channel(_))
            && self.is_forum(f)
            && !self
                .am_restricted(f, ChatRestriction::CreateTopics, premium)
                .restricted()
    }
    pub fn can_manage_topics(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        if self.is_bot(f) {
            return self.is_forum(f);
        }
        matches!(f, PeerFields::Channel(_))
            && self.is_forum(f)
            && (f.is_creator() || (f.admin_rights() & ChatAdminRight::ManageTopics).any())
    }
    pub fn can_post_stories(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::Channel(v) => v.can_post_stories,
            _ => self.is_self(f),
        }
    }
    pub fn can_edit_stories(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::Channel(v) => v.can_edit_stories,
            _ => self.is_self(f),
        }
    }
    pub fn can_delete_stories(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::Channel(v) => v.can_delete_stories,
            _ => self.is_self(f),
        }
    }
    pub fn can_manage_gifts(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::Channel(v) => v.can_post_messages,
            _ => self.is_self(f),
        }
    }
    pub fn can_transfer_gifts(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::Channel(_) => f.is_creator(),
            _ => self.is_self(f),
        }
    }
    pub fn can_edit_messages_indefinitely(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(_) => self.is_self(f),
            PeerFields::Chat(_) => false,
            PeerFields::Channel(v) => {
                if self.is_megagroup(f) {
                    v.can_pin_messages
                } else {
                    v.can_edit_messages
                }
            }
        }
    }
    pub fn can_revoke_full_history(
        &self,
        f: &PeerFields<'_>,
        revoke_inbox: bool,
        revoke_time_limit: i32,
    ) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => {
                !self.is_self(f)
                    && (!self.is_bot(f) || (v.flags.current() & U::Support).any())
                    && (v.flags.current() & U::Deleted).is_empty()
                    && revoke_inbox
                    && revoke_time_limit == i32::MAX
            }
            PeerFields::Chat(_) => f.is_creator(),
            PeerFields::Channel(v) => {
                self.is_megagroup(f)
                    && f.is_creator()
                    && v.members_count_known
                    && v.can_delete
                    && !self.is_monoforum(f)
            }
        }
    }
    pub fn is_bot_user_creates_topics(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::User(v) if v.bot_info_present&&v.user_creates_topics)
    }
    pub fn is_bot_creates_topics(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        matches!(f,PeerFields::User(v) if v.bot_info_present&&!v.user_creates_topics)
    }
    pub fn display_as_forum(&self, f: &PeerFields<'_>, has_topics: impl Fn() -> bool) -> bool {
        self.is_forum(f) && (!self.is_bot_creates_topics(f) || has_topics())
    }
}

impl PeerData {
    pub fn notify(&self) -> &super::notify::data_peer_notify_settings::PeerNotifySettings {
        &self.notify
    }
    pub fn notify_mut(
        &mut self,
    ) -> &mut super::notify::data_peer_notify_settings::PeerNotifySettings {
        &mut self.notify
    }
}

pub fn is_freeze_appeal_username(username: &str) -> bool {
    username
        .chars()
        .map(|c| {
            if c == 'ſ' {
                's'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .eq("spambot".chars())
}

impl PeerData {
    /// input is the actual subclass input() result; no fabricated access hash.
    pub fn update_userpic(
        &mut self,
        photo_id: u64,
        dc_id: i32,
        has_video: bool,
        fields: &PeerFields<'_>,
        input: &tg_scheme::InputPeer,
    ) -> PeerMutation<()> {
        self.check_fields(fields);
        let self_id = if self.is_self(fields) {
            peer_to_user(self.id())
        } else {
            UserId::new(0)
        };
        let file = super::data_image_location::StorageFileLocation::peer_photo(
            dc_id, self_id, input, photo_id, false,
        );
        let location = ImageLocation::new(
            super::data_image_location::DownloadLocation::Storage(file),
            160,
            160,
        );
        self.set_userpic_checked(photo_id, location, has_video)
    }
    /// Called after Data::Session::processPhoto and assigning PhotoData::peer.
    /// The MTP/PhotoData processing half belongs to the real photo model (03).
    pub fn set_userpic_photo_id(&mut self, photo: Option<PhotoKey>) -> PeerMutation<()> {
        if let Some(p) = photo {
            assert_eq!(p.scope(), self.key.scope());
        }
        let id = photo.map_or(0, PhotoKey::id);
        if self.userpic_photo_id == id {
            return PeerMutation::quiet(());
        }
        self.userpic_photo_id = id;
        self.changed((), PeerFlag::Photo)
    }
}

pub fn background_emoji_id_from_color(color: Option<&tg_scheme::PeerColor>) -> u64 {
    match color {
        Some(tg_scheme::PeerColor::PeerColor(d)) => d.vbackground_emoji_id().unwrap_or(0),
        Some(tg_scheme::PeerColor::PeerColorCollectible(d)) => d.background_emoji_id,
        _ => 0,
    }
}
pub fn color_index_from_color(color: Option<&tg_scheme::PeerColor>) -> Option<u8> {
    match color {
        Some(tg_scheme::PeerColor::PeerColor(d)) => d.vcolor().map(|n| n as u8),
        _ => None,
    }
}
pub fn parse_profile_tab(tab: Option<&tg_scheme::ProfileTab>) -> ProfileTab {
    use tg_scheme::ProfileTab as T;
    match tab {
        None => ProfileTab::None,
        Some(T::ProfileTabPosts(_)) => ProfileTab::Posts,
        Some(T::ProfileTabGifts(_)) => ProfileTab::Gifts,
        Some(T::ProfileTabMedia(_)) => ProfileTab::Media,
        Some(T::ProfileTabFiles(_)) => ProfileTab::Files,
        Some(T::ProfileTabMusic(_)) => ProfileTab::Music,
        Some(T::ProfileTabVoice(_)) => ProfileTab::Voice,
        Some(T::ProfileTabLinks(_)) => ProfileTab::Links,
        Some(T::ProfileTabGifs(_)) => ProfileTab::Gifs,
    }
}
pub fn profile_tab_to_mtp(tab: ProfileTab) -> tg_scheme::ProfileTab {
    match tab {
        ProfileTab::Posts => tg_scheme::mtp_profile_tab_posts(),
        ProfileTab::Gifts => tg_scheme::mtp_profile_tab_gifts(),
        ProfileTab::Media => tg_scheme::mtp_profile_tab_media(),
        ProfileTab::Files => tg_scheme::mtp_profile_tab_files(),
        ProfileTab::Music => tg_scheme::mtp_profile_tab_music(),
        ProfileTab::Voice => tg_scheme::mtp_profile_tab_voice(),
        ProfileTab::Links => tg_scheme::mtp_profile_tab_links(),
        ProfileTab::Gifs => tg_scheme::mtp_profile_tab_gifs(),
        ProfileTab::None => panic!("Tab in Data::ProfileTabToMTP"),
    }
}
impl PeerData {
    pub fn use_subsection_tabs(&self, f: &PeerFields<'_>) -> bool {
        self.check_fields(f);
        match f {
            PeerFields::User(v) => v.bot_info_present && (v.flags.current() & U::Forum).any(),
            PeerFields::Channel(v) => {
                self.am_monoforum_admin(f)
                    || (self.is_forum(f) && (v.flags.current() & G::ForumTabs).any())
            }
            _ => false,
        }
    }
    pub fn display_subsection_tabs(
        &self,
        f: &PeerFields<'_>,
        has_topics: impl Fn() -> bool,
    ) -> bool {
        if self.is_bot(f) {
            self.display_as_forum(f, has_topics)
        } else {
            self.use_subsection_tabs(f)
        }
    }
    pub fn can_export_chat_history(
        &self,
        f: &PeerFields<'_>,
        production: bool,
        channel_is_in: bool,
        invite_peek_expires: i32,
        contains_non_service: impl Fn() -> bool,
        migrated: Option<&dyn Fn() -> bool>,
    ) -> bool {
        self.check_fields(f);
        if self.is_replies_chat(production) || self.is_verify_codes() || !self.allows_forwarding(f)
        {
            return false;
        }
        if self.is_channel() && !channel_is_in && invite_peek_expires != 0 {
            return false;
        }
        if contains_non_service() {
            return true;
        }
        migrated.is_some_and(|f| f())
    }
}

impl PeerData {
    /// Resolved target names are borrowed projections from the actual peer graph.
    pub fn name<'a>(
        &'a self,
        migrated_to: Option<&'a str>,
        monoforum_broadcast: Option<&'a str>,
    ) -> &'a str {
        migrated_to.or(monoforum_broadcast).unwrap_or(&self.name)
    }
    pub fn short_name_with_links<'a>(
        &'a self,
        fields: &'a PeerFields<'_>,
        migrated_to: Option<&'a str>,
        monoforum_broadcast: Option<&'a str>,
    ) -> &'a str {
        self.check_fields(fields);
        if self.is_user() {
            self.short_name(fields)
        } else {
            migrated_to.or(monoforum_broadcast).unwrap_or(&self.name)
        }
    }
    pub fn top_bar_name_text_with_migration<'a>(
        &'a self,
        fields: &'a PeerFields<'_>,
        migrated_to: Option<&'a str>,
    ) -> &'a str {
        self.check_fields(fields);
        migrated_to.unwrap_or_else(|| self.top_bar_name_text(fields))
    }
}

impl PeerData {
    pub(crate) fn should_update_full(&self, now: i64) -> bool {
        self.last_full_update == 0 || now > self.last_full_update + 5000
    }
}

pub fn apply_bot_menu_button(
    text: &mut String,
    url: &mut String,
    button: Option<&tg_scheme::BotMenuButton>,
) -> bool {
    let (next_text, next_url) = match button {
        Some(tg_scheme::BotMenuButton::BotMenuButton(v)) => (
            String::from_utf8_lossy(&v.text).into_owned(),
            String::from_utf8_lossy(&v.url).into_owned(),
        ),
        _ => (String::new(), String::new()),
    };
    let changed = *text != next_text || *url != next_url;
    *text = next_text;
    *url = next_url;
    changed
}
impl PeerData {
    pub fn slowmode_applied_value(
        &self,
        channel_sources: Option<(
            tg_rpl::Producer<bool>,
            tg_rpl::Producer<super::data_flags::Change<G>>,
        )>,
    ) -> tg_rpl::Producer<bool> {
        if !self.is_channel() {
            return tg_rpl::single(false);
        }
        let (rights, flags) = channel_sources.expect("real channel sources required");
        channel_slowmode_applied_value(rights, flags)
    }
}

impl PeerData {
    pub fn migrate_from(&self, f: &PeerFields<'_>) -> Option<ChatKey> {
        self.check_fields(f);
        if let PeerFields::Channel(v) = f {
            if self.is_megagroup(f)
                && (v.flags.current() & (G::Forbidden | G::Left | G::Community)).is_empty()
            {
                if let Some(k) = v.migrated_from {
                    assert_eq!(k.scope(), self.key.scope());
                }
                return v.migrated_from;
            }
        }
        None
    }
    pub fn migrate_to(
        &self,
        f: &PeerFields<'_>,
        target_is_in: impl Fn(ChannelKey) -> bool,
    ) -> Option<ChannelKey> {
        self.check_fields(f);
        if let PeerFields::Chat(v) = f {
            if let Some(k) = v.migrated_to {
                assert_eq!(k.scope(), self.key.scope());
                if target_is_in(k) {
                    return Some(k);
                }
            }
        }
        None
    }
    pub fn as_chat_not_migrated(
        &self,
        f: &PeerFields<'_>,
        target_is_in: impl Fn(ChannelKey) -> bool,
    ) -> Option<ChatKey> {
        self.check_fields(f);
        self.as_chat()
            .filter(|_| self.migrate_to(f, target_is_in).is_none())
    }
    pub fn as_channel_or_migrated(
        &self,
        f: &PeerFields<'_>,
        target_is_in: impl Fn(ChannelKey) -> bool,
    ) -> Option<ChannelKey> {
        self.check_fields(f);
        self.as_channel()
            .or_else(|| self.migrate_to(f, target_is_in))
    }
    pub fn migrate_to_or_me(
        &self,
        f: &PeerFields<'_>,
        target_is_in: impl Fn(ChannelKey) -> bool,
    ) -> PeerKey {
        self.migrate_to(f, target_is_in)
            .map_or(self.key, ChannelKey::peer)
    }
    pub fn monoforum_broadcast(&self, f: &PeerFields<'_>) -> Option<ChannelKey> {
        self.check_fields(f);
        if self.as_monoforum(f).is_some() {
            if let PeerFields::Channel(v) = f {
                if let Some(k) = v.monoforum_link {
                    assert_eq!(k.scope(), self.key.scope());
                }
                return v.monoforum_link;
            }
        }
        None
    }
    pub fn broadcast_monoforum(&self, f: &PeerFields<'_>) -> Option<ChannelKey> {
        self.check_fields(f);
        if self.is_broadcast(f) {
            if let PeerFields::Channel(v) = f {
                if let Some(k) = v.monoforum_link {
                    assert_eq!(k.scope(), self.key.scope());
                }
                return v.monoforum_link;
            }
        }
        None
    }
    pub fn userpic_painting_peer(&self, f: &PeerFields<'_>) -> PeerKey {
        self.monoforum_broadcast(f)
            .map_or(self.key, ChannelKey::peer)
    }
    pub fn forum(&self, f: &PeerFields<'_>) -> Option<PeerKey> {
        self.check_fields(f);
        let key = match f {
            PeerFields::User(v) if v.bot_info_present => v.forum,
            PeerFields::Channel(v) => v.forum,
            _ => None,
        };
        if let Some(k) = key {
            assert_eq!(k.scope(), self.key.scope());
        }
        key
    }
    pub fn forum_topic_for(
        &self,
        f: &PeerFields<'_>,
        root: super::data_msg_id::MsgId,
        lookup: impl Fn(PeerKey, super::data_msg_id::MsgId) -> Option<TopicKey>,
    ) -> Option<TopicKey> {
        if root.is_zero() {
            return None;
        }
        self.forum(f).and_then(|forum| lookup(forum, root))
    }
    pub fn monoforum(&self, f: &PeerFields<'_>) -> Option<PeerKey> {
        self.check_fields(f);
        if let PeerFields::Channel(v) = f {
            if let Some(k) = v.monoforum {
                assert_eq!(k.scope(), self.key.scope());
            }
            v.monoforum
        } else {
            None
        }
    }
    pub fn process_topics(
        &self,
        f: &PeerFields<'_>,
        topics: &[tg_scheme::ForumTopic],
        apply: impl Fn(PeerKey, &[tg_scheme::ForumTopic]),
    ) {
        if let Some(forum) = self.forum(f) {
            apply(forum, topics);
        }
    }
    pub fn group_call(&self, f: &PeerFields<'_>) -> Option<GroupCallKey> {
        self.check_fields(f);
        let call = match f {
            PeerFields::Chat(v) => v.group_call,
            PeerFields::Channel(v) => v.group_call,
            _ => None,
        };
        if let Some(k) = call {
            assert_eq!(k.scope(), self.key.scope());
        }
        call
    }
    pub fn group_call_default_join_as(&self, f: &PeerFields<'_>) -> PeerId {
        self.check_fields(f);
        match f {
            PeerFields::Chat(v) => v.group_call_default_join_as,
            PeerFields::Channel(v) => v.group_call_default_join_as,
            _ => PeerId::new(0),
        }
    }
}

impl PeerData {
    pub fn check_folder(
        &self,
        folder_id: super::data_types::FolderId,
        folder_loaded: bool,
        history_folder: Option<super::data_types::FolderId>,
    ) -> PeerMutation<()> {
        if folder_id != 0
            && folder_loaded
            && history_folder.is_some_and(|actual| actual != folder_id)
        {
            PeerMutation {
                value: (),
                effects: vec![PeerEffect::RequestDialogEntry(self.key)],
            }
        } else {
            PeerMutation::quiet(())
        }
    }
}
