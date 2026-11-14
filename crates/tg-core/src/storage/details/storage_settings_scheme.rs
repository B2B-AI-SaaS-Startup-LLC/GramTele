//! Settings blocks of tdata (storage_settings_scheme). Bytes are parsed in full;
//! writes into Core::Settings, globals and SessionSettings become ordered effects
//! for their future owners (packages 29/31, emoji ids stage 5).
use super::storage_file_utilities::FileKey;
use std::{collections::BTreeMap, sync::Arc};
use tg_base::qt_stream::Reader;
use tg_mtproto::{
    mtproto_auth_key::{AuthKey, KeyType},
    mtproto_dc_options::{DcOptions, Endpoint, Environment},
    mtproto_proxy_data::{ProxyData, Settings as ProxySettings, Type as ProxyType},
};

pub const DBI_KEY: u32 = 0x00;
pub const DBI_USER: u32 = 0x01;
pub const DBI_DC_OPTION_OLD_OLD: u32 = 0x02;
pub const DBI_CHAT_SIZE_MAX_OLD: u32 = 0x03;
pub const DBI_MUTE_PEER_OLD: u32 = 0x04;
pub const DBI_SEND_KEY_OLD: u32 = 0x05;
pub const DBI_AUTO_START: u32 = 0x06;
pub const DBI_START_MINIMIZED: u32 = 0x07;
pub const DBI_SOUND_FLASH_BOUNCE_NOTIFY_OLD: u32 = 0x08;
pub const DBI_WORK_MODE_OLD: u32 = 0x09;
pub const DBI_SEEN_TRAY_TOOLTIP: u32 = 0x0a;
pub const DBI_DESKTOP_NOTIFY_OLD: u32 = 0x0b;
pub const DBI_AUTO_UPDATE: u32 = 0x0c;
pub const DBI_LAST_UPDATE_CHECK: u32 = 0x0d;
pub const DBI_WINDOW_POSITION_OLD: u32 = 0x0e;
pub const DBI_CONNECTION_TYPE_OLD_OLD: u32 = 0x0f;
pub const DBI_DEFAULT_ATTACH: u32 = 0x11;
pub const DBI_CATS_AND_DOGS_OLD: u32 = 0x12;
pub const DBI_REPLACE_EMOJI_OLD: u32 = 0x13;
pub const DBI_ASK_DOWNLOAD_PATH_OLD: u32 = 0x14;
pub const DBI_DOWNLOAD_PATH_OLD_OLD: u32 = 0x15;
pub const DBI_SCALE_OLD: u32 = 0x16;
pub const DBI_EMOJI_TAB_OLD: u32 = 0x17;
pub const DBI_RECENT_EMOJI_OLD_OLD_OLD: u32 = 0x18;
pub const DBI_LOGGED_PHONE_NUMBER_OLD: u32 = 0x19;
pub const DBI_MUTED_PEERS_OLD: u32 = 0x1a;
pub const DBI_NOTIFY_VIEW_OLD: u32 = 0x1c;
pub const DBI_SEND_TO_MENU: u32 = 0x1d;
pub const DBI_COMPRESS_PASTED_IMAGE_OLD: u32 = 0x1e;
pub const DBI_LANG_OLD: u32 = 0x1f;
pub const DBI_LANG_FILE_OLD: u32 = 0x20;
pub const DBI_TILE_BACKGROUND_OLD: u32 = 0x21;
pub const DBI_AUTO_LOCK_OLD: u32 = 0x22;
pub const DBI_DIALOG_LAST_PATH: u32 = 0x23;
pub const DBI_RECENT_EMOJI_OLD_OLD: u32 = 0x24;
pub const DBI_EMOJI_VARIANTS_OLD_OLD: u32 = 0x25;
pub const DBI_RECENT_STICKERS: u32 = 0x26;
pub const DBI_DC_OPTION_OLD: u32 = 0x27;
pub const DBI_TRY_IPV6_OLD: u32 = 0x28;
pub const DBI_SONG_VOLUME_OLD: u32 = 0x29;
pub const DBI_WINDOWS_NOTIFICATIONS_OLD: u32 = 0x30;
pub const DBI_INCLUDE_MUTED_OLD: u32 = 0x31;
pub const DBI_MEGAGROUP_SIZE_MAX_OLD: u32 = 0x32;
pub const DBI_DOWNLOAD_PATH_OLD: u32 = 0x33;
pub const DBI_AUTO_DOWNLOAD_OLD: u32 = 0x34;
pub const DBI_SAVED_GIFS_LIMIT_OLD: u32 = 0x35;
pub const DBI_SHOWING_SAVED_GIFS_OLD: u32 = 0x36;
pub const DBI_AUTO_PLAY_OLD: u32 = 0x37;
pub const DBI_ADAPTIVE_FOR_WIDE_OLD: u32 = 0x38;
pub const DBI_HIDDEN_PINNED_MESSAGES_OLD: u32 = 0x39;
pub const DBI_RECENT_EMOJI_OLD: u32 = 0x3a;
pub const DBI_EMOJI_VARIANTS_OLD: u32 = 0x3b;
pub const DBI_DIALOGS_MODE_OLD: u32 = 0x40;
pub const DBI_MODERATE_MODE_OLD: u32 = 0x41;
pub const DBI_VIDEO_VOLUME_OLD: u32 = 0x42;
pub const DBI_STICKERS_RECENT_LIMIT_OLD: u32 = 0x43;
pub const DBI_NATIVE_NOTIFICATIONS_OLD: u32 = 0x44;
pub const DBI_NOTIFICATIONS_COUNT_OLD: u32 = 0x45;
pub const DBI_NOTIFICATIONS_CORNER_OLD: u32 = 0x46;
pub const DBI_THEME_KEY_OLD: u32 = 0x47;
pub const DBI_DIALOGS_WIDTH_RATIO_OLD: u32 = 0x48;
pub const DBI_USE_EXTERNAL_VIDEO_PLAYER_OLD: u32 = 0x49;
pub const DBI_DC_OPTIONS_OLD: u32 = 0x4a;
pub const DBI_MTP_AUTHORIZATION: u32 = 0x4b;
pub const DBI_LAST_SEEN_WARNING_SEEN_OLD: u32 = 0x4c;
pub const DBI_SESSION_SETTINGS: u32 = 0x4d;
pub const DBI_LANG_PACK_KEY: u32 = 0x4e;
pub const DBI_CONNECTION_TYPE_OLD: u32 = 0x4f;
pub const DBI_STICKERS_FAVED_LIMIT_OLD: u32 = 0x50;
pub const DBI_SUGGEST_STICKERS_BY_EMOJI_OLD: u32 = 0x51;
pub const DBI_SUGGEST_EMOJI_OLD: u32 = 0x52;
pub const DBI_TXT_DOMAIN_STRING_OLD_OLD: u32 = 0x53;
pub const DBI_THEME_KEY: u32 = 0x54;
pub const DBI_TILE_BACKGROUND: u32 = 0x55;
pub const DBI_CACHE_SETTINGS_OLD: u32 = 0x56;
pub const DBI_POWER_SAVING: u32 = 0x57;
pub const DBI_SCALE_PERCENT: u32 = 0x58;
pub const DBI_PLAYBACK_SPEED_OLD: u32 = 0x59;
pub const DBI_LANGUAGES_KEY: u32 = 0x5a;
pub const DBI_CALL_SETTINGS_OLD: u32 = 0x5b;
pub const DBI_CACHE_SETTINGS: u32 = 0x5c;
pub const DBI_TXT_DOMAIN_STRING_OLD: u32 = 0x5d;
pub const DBI_APPLICATION_SETTINGS: u32 = 0x5e;
pub const DBI_DIALOGS_FILTERS_OLD: u32 = 0x5f;
pub const DBI_FALLBACK_PRODUCTION_CONFIG: u32 = 0x60;
pub const DBI_BACKGROUND_KEY: u32 = 0x61;
pub const DBI_ENCRYPTED_WITH_SALT: u32 = 333;
pub const DBI_ENCRYPTED: u32 = 444;
pub const DBI_VERSION: u32 = 666;

/// style::kScaleAuto.
pub const SCALE_AUTO: i32 = 0;
/// Storage::Cache::Database::Settings().maxDataSize.
const CACHE_MAX_DATA_SIZE: i64 = tg_cache::cache::storage_cache_types::DATA_SIZE_LIMIT as i64 - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkMode {
    WindowAndTray = 0,
    TrayOnly = 1,
    WindowOnly = 2,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotifyView {
    ShowPreview = 0,
    ShowName = 1,
    ShowNothing = 2,
}
/// Ui::InputSubmitSettings values accepted by dbiSendKeyOld.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendSubmitWay {
    Enter = 0,
    CtrlEnter = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoDownloadSource {
    User,
    Group,
    Channel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoDownloadType {
    Photo,
    AutoPlayVideo,
    VoiceMessage,
    AutoPlayVideoMessage,
    AutoPlayGIF,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowPosition {
    pub moncrc: i32,
    pub maximized: i32,
    pub scale: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// One setter call on Core::App().settings(), a cSet* global or a Core service,
/// in the order the original performs it.
#[derive(Clone, Debug, PartialEq)]
pub enum AppEffect {
    AutoStart(bool),
    StartMinimized(bool),
    SendToMenu(bool),
    PowerSaving(i32),
    SoundNotify(bool),
    FlashBounceNotify(bool),
    ModerateModeEnabled(bool),
    IncludeMutedCounter(bool),
    DesktopNotify(bool),
    NativeNotifications(bool),
    NotificationsCount(i32),
    NotificationsCorner(i32),
    /// updateDialogsWidthRatio(value, false).
    DialogsWidthRatio(f64),
    LastSeenWarningSeen(bool),
    WorkMode(WorkMode),
    ProxySelected(ProxyData),
    ProxySettings(ProxySettings),
    ProxyList(Vec<ProxyData>),
    ProxyUseForCalls(bool),
    ProxyTryIPv6(bool),
    RefreshGlobalProxy,
    /// Window::Theme::SetNightModeValue.
    NightModeValue(bool),
    SeenTrayTooltip(bool),
    AutoUpdate(bool),
    StopUpdateChecker,
    LastUpdateCheck(i32),
    /// SetScaleChecked.
    Scale(i32),
    WindowPosition(WindowPosition),
    SendSubmitWay(SendSubmitWay),
    AdaptiveForWide(bool),
    AutoLock(i32),
    ReplaceEmoji(bool),
    SuggestEmoji(bool),
    SuggestStickersByEmoji(bool),
    NotifyView(NotifyView),
    AskDownloadPath(bool),
    DownloadPathBookmark(Vec<u8>),
    DownloadPath(Vec<u16>),
    DownloadPathEnableAccess,
    SendFilesWay {
        group_files: bool,
        send_images_as_photos: bool,
    },
    /// Old emoji keys; Ui::Emoji::IdFromOldKey (stage 5) drops unknown ones.
    LegacyRecentEmojiOldKeys(Vec<(u64, u16)>),
    LegacyRecentEmoji(Vec<(Vec<u16>, u16)>),
    RecentStickersPreload(Vec<(u64, u16)>),
    /// Old key → old color value; IdFromOldKey/ColorIndexFromOldKey are stage 5.
    LegacyEmojiVariantsOldKeys(BTreeMap<u32, u64>),
    LegacyEmojiVariants(BTreeMap<Vec<u16>, i32>),
    DialogLastPath(Vec<u16>),
    SongVolume(f64),
    VideoVolume(f64),
    VoicePlaybackSpeed(f64),
    AudioPlaybackSpeed(f64),
    CallPlaybackDeviceId(Vec<u16>),
    CallCaptureDeviceId(Vec<u16>),
    CallOutputVolume(i32),
    CallInputVolume(i32),
    CallAudioDuckingEnabled(bool),
    /// Core::Settings::addFromSerialized (package 31).
    ApplicationSettings(Vec<u8>),
}

/// One call on the lazily created Main::SessionSettings (package 29).
#[derive(Clone, Debug, PartialEq)]
pub enum SessionEffect {
    AutoDownloadBytesLimit(AutoDownloadSource, AutoDownloadType),
    DialogsFiltersEnabled(bool),
    AddFromSerialized(Vec<u8>),
    HiddenPinnedMessageId { peer: u64, msg: i64 },
}

pub struct ReadSettingsContext {
    pub legacy_has_custom_day_background: bool,
    pub fallback_config_legacy_dc_options: DcOptions,
    pub fallback_config_legacy_chat_size_max: i32,
    pub fallback_config_legacy_saved_gifs_limit: i32,
    pub fallback_config_legacy_stickers_recent_limit: i32,
    pub fallback_config_legacy_stickers_faved_limit: i32,
    pub fallback_config_legacy_megagroup_size_max: i32,
    pub fallback_config_legacy_txt_domain_string: String,
    pub fallback_config: Vec<u8>,
    pub cache_total_size_limit: i64,
    pub cache_total_time_limit: i32,
    pub cache_big_file_total_size_limit: i64,
    pub cache_big_file_total_time_limit: i32,
    /// sessionSettingsStorage: None until a block touches sessionSettings().
    pub session_settings: Option<Vec<SessionEffect>>,
    pub theme_key_legacy: FileKey,
    pub theme_key_day: FileKey,
    pub theme_key_night: FileKey,
    pub background_key_day: FileKey,
    pub background_key_night: FileKey,
    pub background_keys_read: bool,
    pub tile_day: bool,
    pub tile_night: bool,
    pub tile_read: bool,
    pub lang_pack_key: FileKey,
    pub languages_key: FileKey,
    pub mtp_authorization: Vec<u8>,
    pub mtp_legacy_keys: Vec<Arc<AuthKey>>,
    pub mtp_legacy_main_dc_id: i32,
    pub mtp_legacy_user_id: i32,
    pub legacy_read: bool,

    /// Inputs/state the original reads from globals.
    pub night_mode: bool,
    pub config_scale: i32,
    pub updater_disabled: bool,
    pub app_effects: Vec<AppEffect>,
}

impl Default for ReadSettingsContext {
    fn default() -> Self {
        Self {
            legacy_has_custom_day_background: false,
            fallback_config_legacy_dc_options: DcOptions::new(Environment::Production),
            fallback_config_legacy_chat_size_max: 0,
            fallback_config_legacy_saved_gifs_limit: 0,
            fallback_config_legacy_stickers_recent_limit: 0,
            fallback_config_legacy_stickers_faved_limit: 0,
            fallback_config_legacy_megagroup_size_max: 0,
            fallback_config_legacy_txt_domain_string: String::new(),
            fallback_config: Vec::new(),
            cache_total_size_limit: 0,
            cache_total_time_limit: 0,
            cache_big_file_total_size_limit: 0,
            cache_big_file_total_time_limit: 0,
            session_settings: None,
            theme_key_legacy: 0,
            theme_key_day: 0,
            theme_key_night: 0,
            background_key_day: 0,
            background_key_night: 0,
            background_keys_read: false,
            tile_day: false,
            tile_night: true,
            tile_read: false,
            lang_pack_key: 0,
            languages_key: 0,
            mtp_authorization: Vec::new(),
            mtp_legacy_keys: Vec::new(),
            mtp_legacy_main_dc_id: 0,
            mtp_legacy_user_id: 0,
            legacy_read: false,
            night_mode: false,
            config_scale: SCALE_AUTO,
            updater_disabled: false,
            app_effects: Vec::new(),
        }
    }
}

impl ReadSettingsContext {
    pub fn session_settings(&mut self) -> &mut Vec<SessionEffect> {
        self.session_settings.get_or_insert_with(Vec::new)
    }
    fn app(&mut self, effect: AppEffect) {
        self.app_effects.push(effect);
    }
}

fn no_time_limit(stored: i32) -> bool {
    // Workaround for tdesktop#5611; int64 max truncates to -1 as qint32.
    stored == 0 || stored == i32::MAX || stored == i64::MAX as i32
}

fn i16_u(r: &mut Reader<'_>) -> Option<u16> {
    Some(u16::from_be_bytes(r.raw(2)?.try_into().ok()?))
}
fn u32_(r: &mut Reader<'_>) -> Option<u32> {
    Some(r.i32()? as u32)
}
fn i64_(r: &mut Reader<'_>) -> Option<i64> {
    Some(r.u64()? as i64)
}
/// QString as raw UTF-16; null reads as empty, odd byte count fails.
fn qstring(r: &mut Reader<'_>) -> Option<Vec<u16>> {
    Some(r.nullable_utf16()?.unwrap_or_default())
}
/// QString converted like toStdString/ProxyData fields: lone surrogates become U+FFFD.
fn proxy_string(r: &mut Reader<'_>, nulls: &mut u8, bit: u8) -> Option<String> {
    let value = r.nullable_utf16()?;
    *nulls &= !bit;
    match value {
        None => {
            *nulls |= bit;
            Some(String::new())
        }
        Some(v) => Some(String::from_utf16_lossy(&v)),
    }
}
fn qvector<T>(
    r: &mut Reader<'_>,
    mut item: impl FnMut(&mut Reader<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    let count = u32_(r)?;
    let mut out = Vec::new();
    for _ in 0..count {
        out.push(item(r)?);
    }
    Some(out)
}
fn qmap<K: Ord, V>(
    r: &mut Reader<'_>,
    mut item: impl FnMut(&mut Reader<'_>) -> Option<(K, V)>,
) -> Option<BTreeMap<K, V>> {
    qvector(r, &mut item).map(|v| v.into_iter().collect())
}

/// Storage::details::ReadSetting.
pub fn read_setting(
    block_id: u32,
    stream: &mut Reader<'_>,
    version: i32,
    context: &mut ReadSettingsContext,
) -> bool {
    macro_rules! read {
        ($e:expr) => {
            match $e {
                Some(v) => v,
                None => return false,
            }
        };
    }
    let c = context;
    match block_id {
        DBI_DC_OPTION_OLD_OLD => {
            let dc_id = read!(u32_(stream));
            let _host = read!(qstring(stream));
            let ip = read!(proxy_string(stream, &mut 0, 0));
            let port = read!(u32_(stream));
            c.fallback_config_legacy_dc_options
                .construct_add_one(Endpoint {
                    id: dc_id as i32,
                    flags: 0,
                    ip,
                    port: port as i32,
                    secret: Vec::new(),
                });
            c.legacy_read = true;
        }
        DBI_DC_OPTION_OLD => {
            let dc_id_with_shift = read!(u32_(stream));
            let flags = read!(stream.i32());
            let ip = read!(proxy_string(stream, &mut 0, 0));
            let port = read!(u32_(stream));
            c.fallback_config_legacy_dc_options
                .construct_add_one(Endpoint {
                    id: dc_id_with_shift as i32,
                    flags: flags as u32,
                    ip,
                    port: port as i32,
                    secret: Vec::new(),
                });
            c.legacy_read = true;
        }
        DBI_DC_OPTIONS_OLD => {
            let serialized = read!(stream.bytes());
            c.fallback_config_legacy_dc_options
                .construct_from_serialized(&serialized);
            c.legacy_read = true;
        }
        DBI_APPLICATION_SETTINGS => {
            let serialized = read!(stream.bytes());
            c.app(AppEffect::ApplicationSettings(serialized));
        }
        DBI_CHAT_SIZE_MAX_OLD => {
            c.fallback_config_legacy_chat_size_max = read!(stream.i32());
            c.legacy_read = true;
        }
        DBI_SAVED_GIFS_LIMIT_OLD => {
            c.fallback_config_legacy_saved_gifs_limit = read!(stream.i32());
            c.legacy_read = true;
        }
        DBI_STICKERS_RECENT_LIMIT_OLD => {
            c.fallback_config_legacy_stickers_recent_limit = read!(stream.i32());
            c.legacy_read = true;
        }
        DBI_STICKERS_FAVED_LIMIT_OLD => {
            c.fallback_config_legacy_stickers_faved_limit = read!(stream.i32());
            c.legacy_read = true;
        }
        DBI_MEGAGROUP_SIZE_MAX_OLD => {
            c.fallback_config_legacy_megagroup_size_max = read!(stream.i32());
            c.legacy_read = true;
        }
        DBI_USER => {
            let user_id = read!(stream.i32());
            let dc_id = read!(u32_(stream));
            c.mtp_legacy_main_dc_id = dc_id as i32;
            c.mtp_legacy_user_id = user_id;
        }
        DBI_KEY => {
            let dc_id = read!(stream.i32());
            let data: [u8; 256] = read!(stream.raw(256)).try_into().unwrap();
            c.mtp_legacy_keys.push(Arc::new(AuthKey::new(
                KeyType::ReadFromFile,
                dc_id,
                data,
                0,
            )));
        }
        DBI_MTP_AUTHORIZATION => c.mtp_authorization = read!(stream.bytes()),
        DBI_AUTO_START => {
            let v = read!(stream.i32());
            c.app(AppEffect::AutoStart(v == 1));
        }
        DBI_START_MINIMIZED => {
            let v = read!(stream.i32());
            c.app(AppEffect::StartMinimized(v == 1));
        }
        DBI_SEND_TO_MENU => {
            let v = read!(stream.i32());
            c.app(AppEffect::SendToMenu(v == 1));
        }
        DBI_USE_EXTERNAL_VIDEO_PLAYER_OLD | DBI_DEFAULT_ATTACH => {
            read!(stream.i32());
        }
        DBI_CACHE_SETTINGS_OLD => {
            let size = read!(i64_(stream));
            let time = read!(stream.i32());
            if size <= CACHE_MAX_DATA_SIZE || (!no_time_limit(time) && time < 0) {
                return false;
            }
            let time = if no_time_limit(time) { 0 } else { time };
            c.cache_total_size_limit = size;
            c.cache_total_time_limit = time;
            c.cache_big_file_total_size_limit = size;
            c.cache_big_file_total_time_limit = time;
            c.legacy_read = true;
        }
        DBI_CACHE_SETTINGS => {
            let size = read!(i64_(stream));
            let time = read!(stream.i32());
            let size_big = read!(i64_(stream));
            let time_big = read!(stream.i32());
            if size <= CACHE_MAX_DATA_SIZE
                || size_big <= CACHE_MAX_DATA_SIZE
                || (!no_time_limit(time) && time < 0)
                || (!no_time_limit(time_big) && time_big < 0)
            {
                return false;
            }
            c.cache_total_size_limit = size;
            c.cache_total_time_limit = if no_time_limit(time) { 0 } else { time };
            c.cache_big_file_total_size_limit = size_big;
            c.cache_big_file_total_time_limit = if no_time_limit(time_big) { 0 } else { time_big };
        }
        DBI_POWER_SAVING => {
            let v = read!(stream.i32());
            c.app(AppEffect::PowerSaving(v));
        }
        DBI_SOUND_FLASH_BOUNCE_NOTIFY_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::SoundNotify((v & 0x01) == 0x01));
            c.app(AppEffect::FlashBounceNotify((v & 0x02) == 0x00));
            c.legacy_read = true;
        }
        DBI_AUTO_DOWNLOAD_OLD => {
            let photo = read!(stream.i32());
            let audio = read!(stream.i32());
            let gif = read!(stream.i32());
            let settings = c.session_settings();
            let mut set = |kind, value: i32| {
                use AutoDownloadSource::*;
                if value & 0x01 != 0 {
                    settings.push(SessionEffect::AutoDownloadBytesLimit(User, kind));
                }
                if value & 0x02 != 0 {
                    settings.push(SessionEffect::AutoDownloadBytesLimit(Group, kind));
                    settings.push(SessionEffect::AutoDownloadBytesLimit(Channel, kind));
                }
            };
            set(AutoDownloadType::Photo, photo);
            set(AutoDownloadType::VoiceMessage, audio);
            set(AutoDownloadType::AutoPlayGIF, gif);
            set(AutoDownloadType::AutoPlayVideoMessage, gif);
            c.legacy_read = true;
        }
        DBI_AUTO_PLAY_OLD => {
            let gif = read!(stream.i32());
            if gif == 0 {
                let settings = c.session_settings();
                for source in [
                    AutoDownloadSource::User,
                    AutoDownloadSource::Group,
                    AutoDownloadSource::Channel,
                ] {
                    for kind in [
                        AutoDownloadType::AutoPlayGIF,
                        AutoDownloadType::AutoPlayVideo,
                        AutoDownloadType::AutoPlayVideoMessage,
                    ] {
                        settings.push(SessionEffect::AutoDownloadBytesLimit(source, kind));
                    }
                }
            }
            c.legacy_read = true;
        }
        DBI_DIALOGS_MODE_OLD => {
            read!(stream.i32());
            read!(stream.i32());
            c.legacy_read = true;
        }
        DBI_DIALOGS_FILTERS_OLD => {
            let enabled = read!(stream.i32());
            c.session_settings()
                .push(SessionEffect::DialogsFiltersEnabled(enabled == 1));
            c.legacy_read = true;
        }
        DBI_MODERATE_MODE_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::ModerateModeEnabled(v == 1));
            c.legacy_read = true;
        }
        DBI_INCLUDE_MUTED_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::IncludeMutedCounter(v == 1));
            c.legacy_read = true;
        }
        DBI_SHOWING_SAVED_GIFS_OLD
        | DBI_WINDOWS_NOTIFICATIONS_OLD
        | DBI_LANG_OLD
        | DBI_CATS_AND_DOGS_OLD
        | DBI_EMOJI_TAB_OLD => {
            read!(stream.i32());
            c.legacy_read = true;
        }
        DBI_DESKTOP_NOTIFY_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::DesktopNotify(v == 1));
            c.legacy_read = true;
        }
        DBI_NATIVE_NOTIFICATIONS_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::NativeNotifications(v == 1));
            c.legacy_read = true;
        }
        DBI_NOTIFICATIONS_COUNT_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::NotificationsCount(if v > 0 { v } else { 3 }));
            c.legacy_read = true;
        }
        DBI_NOTIFICATIONS_CORNER_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::NotificationsCorner(if (0..4).contains(&v) {
                v
            } else {
                2
            }));
            c.legacy_read = true;
        }
        DBI_DIALOGS_WIDTH_RATIO_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::DialogsWidthRatio(v as f64 / 1000000.));
            c.legacy_read = true;
        }
        DBI_LAST_SEEN_WARNING_SEEN_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::LastSeenWarningSeen(v == 1));
            c.legacy_read = true;
        }
        DBI_SESSION_SETTINGS => {
            let v = read!(stream.bytes());
            c.session_settings()
                .push(SessionEffect::AddFromSerialized(v));
        }
        DBI_WORK_MODE_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::WorkMode(match v {
                1 => WorkMode::TrayOnly,
                2 => WorkMode::WindowOnly,
                _ => WorkMode::WindowAndTray,
            }));
            c.legacy_read = true;
        }
        DBI_TXT_DOMAIN_STRING_OLD_OLD | DBI_LANG_FILE_OLD | DBI_LOGGED_PHONE_NUMBER_OLD => {
            read!(qstring(stream));
            c.legacy_read = true;
        }
        DBI_TXT_DOMAIN_STRING_OLD => {
            let v = read!(proxy_string(stream, &mut 0, 0));
            c.fallback_config_legacy_txt_domain_string = v;
            c.legacy_read = true;
        }
        DBI_CONNECTION_TYPE_OLD_OLD => {
            let v = read!(stream.i32());
            const HTTP_PROXY: i32 = 2;
            const TCP_PROXY: i32 = 3;
            let mut proxy = ProxyData::default();
            if v == HTTP_PROXY || v == TCP_PROXY {
                let mut nulls = proxy.string_nulls;
                proxy.host = read!(proxy_string(stream, &mut nulls, 1));
                let port = read!(stream.i32());
                proxy.user = read!(proxy_string(stream, &mut nulls, 2));
                proxy.password = read!(proxy_string(stream, &mut nulls, 4));
                proxy.string_nulls = nulls;
                proxy.port = port as u32;
                proxy.kind = if v == TCP_PROXY {
                    ProxyType::Socks5
                } else {
                    ProxyType::Http
                };
            }
            let valid = proxy.valid();
            c.app(AppEffect::ProxySelected(if valid {
                proxy.clone()
            } else {
                ProxyData::default()
            }));
            c.app(AppEffect::ProxySettings(if valid {
                ProxySettings::Enabled
            } else {
                ProxySettings::System
            }));
            c.app(AppEffect::ProxyList(if valid {
                vec![proxy]
            } else {
                Vec::new()
            }));
            c.app(AppEffect::RefreshGlobalProxy);
            c.legacy_read = true;
        }
        DBI_CONNECTION_TYPE_OLD => {
            let connection_type = read!(stream.i32());
            if !read_connection_type_old(connection_type, stream, c) {
                return false;
            }
            c.app(AppEffect::RefreshGlobalProxy);
            c.legacy_read = true;
        }
        DBI_THEME_KEY_OLD => {
            c.theme_key_legacy = read!(stream.u64());
            c.legacy_read = true;
        }
        DBI_THEME_KEY => {
            let key_day = read!(stream.u64());
            let key_night = read!(stream.u64());
            let night_mode = read!(u32_(stream));
            c.theme_key_day = key_day;
            c.theme_key_night = key_night;
            c.night_mode = night_mode == 1;
            c.app(AppEffect::NightModeValue(night_mode == 1));
        }
        DBI_BACKGROUND_KEY => {
            let key_day = read!(stream.u64());
            let key_night = read!(stream.u64());
            c.background_key_day = key_day;
            c.background_key_night = key_night;
            c.background_keys_read = true;
        }
        DBI_LANG_PACK_KEY => c.lang_pack_key = read!(stream.u64()),
        DBI_LANGUAGES_KEY => c.languages_key = read!(stream.u64()),
        DBI_TRY_IPV6_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::ProxyTryIPv6(v == 1));
            c.legacy_read = true;
        }
        DBI_SEEN_TRAY_TOOLTIP => {
            let v = read!(stream.i32());
            c.app(AppEffect::SeenTrayTooltip(v == 1));
        }
        DBI_AUTO_UPDATE => {
            let v = read!(stream.i32());
            c.app(AppEffect::AutoUpdate(v == 1));
            if !c.updater_disabled && v != 1 {
                c.app(AppEffect::StopUpdateChecker);
            }
        }
        DBI_LAST_UPDATE_CHECK => {
            let v = read!(stream.i32());
            c.app(AppEffect::LastUpdateCheck(v));
        }
        DBI_SCALE_OLD => {
            let v = read!(stream.i32());
            let scale = match v {
                0 => SCALE_AUTO,
                1 => 100,
                2 => 125,
                3 => 150,
                4 => 200,
                _ => c.config_scale,
            };
            c.app(AppEffect::Scale(scale));
            c.legacy_read = true;
        }
        DBI_SCALE_PERCENT => {
            let v = read!(stream.i32());
            if c.config_scale == SCALE_AUTO {
                c.app(AppEffect::Scale(v));
            }
        }
        DBI_WINDOW_POSITION_OLD => {
            let mut p = WindowPosition {
                x: read!(stream.i32()),
                y: read!(stream.i32()),
                w: read!(stream.i32()),
                h: read!(stream.i32()),
                ..Default::default()
            };
            p.moncrc = read!(stream.i32());
            p.maximized = read!(stream.i32());
            c.app(AppEffect::WindowPosition(p));
            c.legacy_read = true;
        }
        DBI_MUTE_PEER_OLD => {
            read!(stream.u64());
            c.legacy_read = true;
        }
        DBI_MUTED_PEERS_OLD => {
            let count = read!(u32_(stream));
            for _ in 0..count {
                read!(stream.u64());
            }
            c.legacy_read = true;
        }
        DBI_SEND_KEY_OLD => {
            let v = read!(stream.i32());
            let way = match v {
                0 => SendSubmitWay::Enter,
                1 => SendSubmitWay::CtrlEnter,
                _ => return false,
            };
            c.app(AppEffect::SendSubmitWay(way));
            c.legacy_read = true;
        }
        DBI_TILE_BACKGROUND_OLD => {
            let v = read!(stream.i32());
            let tile = if version < 8005 && !c.legacy_has_custom_day_background {
                false
            } else {
                v == 1
            };
            // Original writes the opposite mode's flag; kept as is.
            if c.night_mode {
                c.tile_day = tile;
            } else {
                c.tile_night = tile;
            }
            c.tile_read = true;
            c.legacy_read = true;
        }
        DBI_TILE_BACKGROUND => {
            let tile_day = read!(stream.i32());
            let tile_night = read!(stream.i32());
            c.tile_day = tile_day != 0;
            c.tile_night = tile_night != 0;
            c.tile_read = true;
        }
        DBI_ADAPTIVE_FOR_WIDE_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::AdaptiveForWide(v == 1));
            c.legacy_read = true;
        }
        DBI_AUTO_LOCK_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::AutoLock(v));
            c.legacy_read = true;
        }
        DBI_REPLACE_EMOJI_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::ReplaceEmoji(v == 1));
            c.legacy_read = true;
        }
        DBI_SUGGEST_EMOJI_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::SuggestEmoji(v == 1));
            c.legacy_read = true;
        }
        DBI_SUGGEST_STICKERS_BY_EMOJI_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::SuggestStickersByEmoji(v == 1));
            c.legacy_read = true;
        }
        DBI_NOTIFY_VIEW_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::NotifyView(match v {
                2 => NotifyView::ShowNothing,
                1 => NotifyView::ShowName,
                _ => NotifyView::ShowPreview,
            }));
            c.legacy_read = true;
        }
        DBI_ASK_DOWNLOAD_PATH_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::AskDownloadPath(v == 1));
            c.legacy_read = true;
        }
        DBI_DOWNLOAD_PATH_OLD_OLD => {
            let v = read!(qstring(stream));
            c.app(AppEffect::DownloadPathBookmark(Vec::new()));
            c.app(AppEffect::DownloadPath(download_path(v)));
            c.legacy_read = true;
        }
        DBI_DOWNLOAD_PATH_OLD => {
            let v = read!(qstring(stream));
            let bookmark = read!(stream.bytes());
            c.app(AppEffect::DownloadPathBookmark(bookmark));
            c.app(AppEffect::DownloadPath(download_path(v)));
            c.app(AppEffect::DownloadPathEnableAccess);
            c.legacy_read = true;
        }
        DBI_COMPRESS_PASTED_IMAGE_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::SendFilesWay {
                group_files: v == 1,
                send_images_as_photos: v == 1,
            });
            c.legacy_read = true;
        }
        DBI_RECENT_EMOJI_OLD_OLD_OLD => {
            let v = read!(qvector(stream, |r| Some((u32_(r)?, i16_u(r)?))));
            if !v.is_empty() {
                let keys = v
                    .into_iter()
                    .map(|(key, count)| (flag_old_key(key as u64), count))
                    .collect();
                c.app(AppEffect::LegacyRecentEmojiOldKeys(keys));
            }
            c.legacy_read = true;
        }
        DBI_RECENT_EMOJI_OLD_OLD => {
            let v = read!(qvector(stream, |r| Some((r.u64()?, i16_u(r)?))));
            if !v.is_empty() {
                c.app(AppEffect::LegacyRecentEmojiOldKeys(v));
            }
            c.legacy_read = true;
        }
        DBI_RECENT_EMOJI_OLD => {
            let v = read!(qvector(stream, |r| Some((qstring(r)?, i16_u(r)?))));
            c.app(AppEffect::LegacyRecentEmoji(v));
            c.legacy_read = true;
        }
        DBI_RECENT_STICKERS => {
            let v = read!(qvector(stream, |r| Some((r.u64()?, i16_u(r)?))));
            c.app(AppEffect::RecentStickersPreload(v));
        }
        DBI_EMOJI_VARIANTS_OLD_OLD => {
            let v = read!(qmap(stream, |r| Some((u32_(r)?, r.u64()?))));
            c.app(AppEffect::LegacyEmojiVariantsOldKeys(v));
            c.legacy_read = true;
        }
        DBI_EMOJI_VARIANTS_OLD => {
            let v = read!(qmap(stream, |r| Some((qstring(r)?, r.i32()?))));
            c.app(AppEffect::LegacyEmojiVariants(v));
            c.legacy_read = true;
        }
        DBI_HIDDEN_PINNED_MESSAGES_OLD => {
            let v = read!(qmap(stream, |r| Some((r.u64()?, r.i32()?))));
            for (peer, msg) in v {
                let peer = crate::data::data_peer_id::deserialize_peer_id(peer);
                c.session_settings()
                    .push(SessionEffect::HiddenPinnedMessageId {
                        peer: peer.value,
                        msg: msg as i64,
                    });
            }
            c.legacy_read = true;
        }
        DBI_DIALOG_LAST_PATH => {
            let path = read!(qstring(stream));
            c.app(AppEffect::DialogLastPath(path));
        }
        DBI_SONG_VOLUME_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::SongVolume((v as f64 / 1e6).clamp(0., 1.)));
            c.legacy_read = true;
        }
        DBI_VIDEO_VOLUME_OLD => {
            let v = read!(stream.i32());
            c.app(AppEffect::VideoVolume((v as f64 / 1e6).clamp(0., 1.)));
            c.legacy_read = true;
        }
        DBI_PLAYBACK_SPEED_OLD => {
            let v = read!(stream.i32());
            if v == 2 {
                c.app(AppEffect::VoicePlaybackSpeed(2.));
                c.app(AppEffect::AudioPlaybackSpeed(2.));
            }
            c.legacy_read = true;
        }
        DBI_CALL_SETTINGS_OLD => {
            let call_settings = read!(stream.bytes());
            let mut s = Reader::new(&call_settings);
            let parsed = (|| {
                let output = qstring(&mut s)?;
                let output_volume = s.i32()?;
                let input = qstring(&mut s)?;
                let input_volume = s.i32()?;
                let ducking = s.i32()?;
                Some((output, output_volume, input, input_volume, ducking))
            })();
            if let Some((output, output_volume, input, input_volume, ducking)) = parsed {
                c.app(AppEffect::CallPlaybackDeviceId(output));
                c.app(AppEffect::CallCaptureDeviceId(input));
                c.app(AppEffect::CallOutputVolume(output_volume));
                c.app(AppEffect::CallInputVolume(input_volume));
                c.app(AppEffect::CallAudioDuckingEnabled(ducking != 0));
            }
            c.legacy_read = true;
        }
        DBI_FALLBACK_PRODUCTION_CONFIG => c.fallback_config = read!(stream.bytes()),
        _ => return false,
    }
    true
}

/// The fixed flag-pair remap of dbiRecentEmojiOldOldOld.
fn flag_old_key(key: u64) -> u64 {
    match key {
        0xD83CDDEF => 0xD83CDDEFD83CDDF5,
        0xD83CDDF0 => 0xD83CDDF0D83CDDF7,
        0xD83CDDE9 => 0xD83CDDE9D83CDDEA,
        0xD83CDDE8 => 0xD83CDDE8D83CDDF3,
        0xD83CDDFA => 0xD83CDDFAD83CDDF8,
        0xD83CDDEB => 0xD83CDDEBD83CDDF7,
        0xD83CDDEA => 0xD83CDDEAD83CDDF8,
        0xD83CDDEE => 0xD83CDDEED83CDDF9,
        0xD83CDDF7 => 0xD83CDDF7D83CDDFA,
        0xD83CDDEC => 0xD83CDDECD83CDDE7,
        other => other,
    }
}

/// Non-store build: append '/' unless empty, FileDialog::Tmp() or already ends with it.
fn download_path(mut v: Vec<u16>) -> Vec<u16> {
    let tmp: Vec<u16> = "tmp".encode_utf16().collect();
    if !v.is_empty() && v != tmp && v.last() != Some(&(b'/' as u16)) {
        v.push(b'/' as u16);
    }
    v
}

fn read_connection_type_old(
    connection_type: i32,
    stream: &mut Reader<'_>,
    c: &mut ReadSettingsContext,
) -> bool {
    const LEGACY_PROXY_TYPE_SHIFT: i32 = 1024;
    const HTTP_PROXY: i32 = 2;
    const TCP_PROXY: i32 = 3;
    const PROXIES_LIST_OLD: i32 = 4;
    const PROXIES_LIST: i32 = 5;
    let read_proxy = |r: &mut Reader<'_>| -> Option<ProxyData> {
        let proxy_type = r.i32()?;
        let mut proxy = ProxyData::default();
        let mut nulls = proxy.string_nulls;
        proxy.host = proxy_string(r, &mut nulls, 1)?;
        let port = r.i32()?;
        proxy.user = proxy_string(r, &mut nulls, 2)?;
        proxy.password = proxy_string(r, &mut nulls, 4)?;
        proxy.string_nulls = nulls;
        proxy.port = port as u32;
        proxy.kind = match proxy_type {
            TCP_PROXY => ProxyType::Socks5,
            HTTP_PROXY => ProxyType::Http,
            t if t == LEGACY_PROXY_TYPE_SHIFT + 1 => ProxyType::Socks5,
            t if t == LEGACY_PROXY_TYPE_SHIFT + 2 => ProxyType::Http,
            t if t == LEGACY_PROXY_TYPE_SHIFT + 3 => ProxyType::Mtproto,
            _ => ProxyType::None,
        };
        Some(proxy)
    };
    if connection_type == PROXIES_LIST_OLD || connection_type == PROXIES_LIST {
        let parsed = (|| {
            let count = stream.i32()?;
            let mut index = stream.i32()?;
            let (mut settings, mut calls) = (0, 0);
            if connection_type == PROXIES_LIST {
                settings = stream.i32()?;
                calls = stream.i32()?;
            } else if index.unsigned_abs() as i64 > count as i64 {
                calls = 1;
                index -= if index > 0 { count } else { -count };
            }
            let mut list: Vec<ProxyData> = Vec::new();
            for _ in 0..count.max(0) {
                let proxy = read_proxy(stream)?;
                if proxy.valid() {
                    list.push(proxy);
                } else if (index as i64) < -(list.len() as i64) {
                    index += 1;
                } else if index as i64 > list.len() as i64 {
                    index -= 1;
                }
            }
            Some((index, settings, calls, list))
        })();
        let Some((mut index, mut settings, calls, list)) = parsed else {
            return false;
        };
        let len = list.len() as i64;
        c.app(AppEffect::ProxyList(list.clone()));
        if connection_type == PROXIES_LIST_OLD {
            settings = if index > 0 && index as i64 <= len {
                1
            } else {
                0
            };
            index = index.abs();
        }
        let selected = if index > 0 && index as i64 <= len {
            list[index as usize - 1].clone()
        } else {
            ProxyData::default()
        };
        let selected_valid = selected.valid();
        c.app(AppEffect::ProxySelected(selected));
        c.app(AppEffect::ProxySettings(match settings {
            1 if selected_valid => ProxySettings::Enabled,
            2 => ProxySettings::Disabled,
            _ => ProxySettings::System,
        }));
        c.app(AppEffect::ProxyUseForCalls(calls == 1));
    } else {
        let Some(proxy) = read_proxy(stream) else {
            return false;
        };
        if proxy.valid() {
            c.app(AppEffect::ProxyList(vec![proxy.clone()]));
            c.app(AppEffect::ProxySelected(proxy));
            c.app(AppEffect::ProxySettings(
                if connection_type == TCP_PROXY || connection_type == HTTP_PROXY {
                    ProxySettings::Enabled
                } else {
                    ProxySettings::System
                },
            ));
        } else {
            c.app(AppEffect::ProxyList(Vec::new()));
            c.app(AppEffect::ProxySelected(ProxyData::default()));
            c.app(AppEffect::ProxySettings(ProxySettings::System));
        }
    }
    true
}

/// Storage::details::ApplyReadFallbackConfig: what goes into the fallback
/// production config owned by Core::App (package 30).
#[derive(Clone, Debug, PartialEq)]
pub enum FallbackConfigEffect {
    /// constructFallbackProductionConfig(serialized).
    Construct(Vec<u8>),
    Legacy {
        chat_size_max: Option<i32>,
        stickers_recent_limit: Option<i32>,
        megagroup_size_max: Option<i32>,
        txt_domain_string: Option<String>,
    },
}

/// Returns the effect and, for the legacy branch, the dc options to merge via
/// `DcOptions::add_from_other`.
pub fn apply_read_fallback_config(
    context: &mut ReadSettingsContext,
) -> (FallbackConfigEffect, Option<DcOptions>) {
    if context.fallback_config.is_empty() {
        let options = std::mem::replace(
            &mut context.fallback_config_legacy_dc_options,
            DcOptions::new(Environment::Production),
        );
        let positive = |v: i32| (v > 0).then_some(v);
        let txt = &context.fallback_config_legacy_txt_domain_string;
        (
            FallbackConfigEffect::Legacy {
                chat_size_max: positive(context.fallback_config_legacy_chat_size_max),
                stickers_recent_limit: positive(
                    context.fallback_config_legacy_stickers_recent_limit,
                ),
                megagroup_size_max: positive(context.fallback_config_legacy_megagroup_size_max),
                txt_domain_string: (!txt.is_empty()).then(|| txt.clone()),
            },
            Some(options),
        )
    } else {
        (
            FallbackConfigEffect::Construct(context.fallback_config.clone()),
            None,
        )
    }
}
