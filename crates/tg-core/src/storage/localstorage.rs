//! Local:: global settings file, legacy unencrypted tdata files, autoupdate
//! prefix and update manifest (storage/localstorage). Themes, backgrounds,
//! language packs and the waveform task are partial (stage 5 / lang 01).
use super::details::storage_file_utilities::{
    create_legacy_local_key, decrypt_local, read_file, EncryptedDescriptor, FileKey,
    FileWriteDescriptor, Result, WriteManager, APP_VERSION, SALT_SIZE,
};
use super::details::storage_settings_scheme::{
    apply_read_fallback_config, read_setting, FallbackConfigEffect, ReadSettingsContext,
    DBI_APPLICATION_SETTINGS, DBI_AUTO_START, DBI_AUTO_UPDATE, DBI_BACKGROUND_KEY,
    DBI_DIALOG_LAST_PATH, DBI_ENCRYPTED, DBI_ENCRYPTED_WITH_SALT, DBI_FALLBACK_PRODUCTION_CONFIG,
    DBI_LANGUAGES_KEY, DBI_LANG_PACK_KEY, DBI_LAST_UPDATE_CHECK, DBI_POWER_SAVING,
    DBI_SCALE_PERCENT, DBI_SEEN_TRAY_TOOLTIP, DBI_SEND_TO_MENU, DBI_START_MINIMIZED, DBI_THEME_KEY,
    DBI_TILE_BACKGROUND, DBI_VERSION,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tg_base::qt_stream::Reader;
use tg_mtproto::{core_types::Random, mtproto_auth_key::AuthKey, mtproto_dc_options::DcOptions};

pub const DEFAULT_AUTOUPDATE_PREFIX: &str = "https://update.ayugram.one/";
const SIGNATURE_SIZE: usize = 64;
const MAX_MANIFEST_SIZE: usize = 256 * 1024;

/// What start()/reset() ask their owners to do, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum LocalEffect {
    /// ApplyReadFallbackConfig result for Core::App (package 30).
    FallbackConfig(FallbackConfigEffect),
    /// ApplyDefaultNightMode (stage 5 theme).
    ApplyDefaultNightMode,
    /// InitialLoadTheme (stage 5 theme).
    InitialLoadTheme,
    /// Background()->setTileDayValue/NightValue.
    BackgroundTile {
        day: bool,
        night: bool,
    },
    /// readLangPack (lang 01).
    ReadLangPack,
    /// _localLoader->stop(), Background()->reset(), settings().resetOnLastLogout().
    StopLocalLoader,
    ResetBackground,
    ResetSettingsOnLastLogout,
    /// UpdateChecker().start() after the prefix changed with autoupdate on.
    StartUpdateChecker,
}

/// Values writeSettings() pulls from Core::App, cSet* globals and the theme.
#[derive(Clone, Debug, Default)]
pub struct SettingsSnapshot {
    pub auto_start: bool,
    pub start_minimized: bool,
    pub send_to_menu: bool,
    pub seen_tray_tooltip: bool,
    pub auto_update: bool,
    pub last_update_check: i32,
    pub config_scale: i32,
    /// LookupFallbackConfig().serialize().
    pub fallback_config: Vec<u8>,
    /// Core::App().settings().serialize().
    pub application_settings: Vec<u8>,
    /// cDialogLastPath(); None is the default null QString.
    pub dialog_last_path: Option<Vec<u16>>,
    pub power_saving: i32,
    pub night_mode: bool,
    pub tile_day: bool,
    pub tile_night: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum WriteSettingsResult {
    /// Nothing written (no base path, or deferred with a key already present).
    Skipped,
    /// Salt plus an empty encrypted part: writes are not allowed yet.
    /// AyuSettings::save() precedes this and Written in the original.
    Placeholder,
    Written,
}

/// The file-scope state of localstorage.cpp.
#[derive(Default)]
pub struct Local {
    pub working_dir: PathBuf,
    /// cDataFile(): "data" unless a key file was given.
    pub data_file: String,
    pub base_path: Option<PathBuf>,
    settings_salt: Vec<u8>,
    pub old_key: Option<Arc<AuthKey>>,
    settings_key: Option<Arc<AuthKey>>,
    pub theme_key_day: FileKey,
    pub theme_key_night: FileKey,
    pub theme_key_legacy: FileKey,
    pub lang_pack_key: FileKey,
    pub languages_key: FileKey,
    pub background_key_day: FileKey,
    pub background_key_night: FileKey,
    pub use_global_background_keys: bool,
    pub old_settings_version: i32,
    pub settings_rewrite_needed: bool,
    pub settings_write_allowed: bool,
    autoupdate_prefix: String,
    /// Legacy dc options for Core::App fallback config addFromOther (package 30).
    pub fallback_dc_options: Option<DcOptions>,
}

fn version_or_block(r: &mut Reader<'_>, version: &mut i32) -> Option<Option<u32>> {
    // None: stop reading; Some(Some(id)): a block id still to handle.
    let block_id = r.i32()? as u32;
    if block_id == DBI_VERSION {
        *version = r.i32()?;
        return Some(None);
    }
    Some(Some(block_id))
}

impl Local {
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
            data_file: "data".into(),
            ..Default::default()
        }
    }

    pub fn settings_key(&self) -> Option<&Arc<AuthKey>> {
        self.settings_key.as_ref()
    }

    fn apply_read_context(
        &mut self,
        context: &mut ReadSettingsContext,
        out: &mut Vec<LocalEffect>,
    ) {
        let (fallback, options) = apply_read_fallback_config(context);
        self.fallback_dc_options = options;
        out.push(LocalEffect::FallbackConfig(fallback));
        self.theme_key_legacy = context.theme_key_legacy;
        self.theme_key_day = context.theme_key_day;
        self.theme_key_night = context.theme_key_night;
        self.background_key_day = context.background_key_day;
        self.background_key_night = context.background_key_night;
        self.use_global_background_keys = context.background_keys_read;
        self.lang_pack_key = context.lang_pack_key;
        self.languages_key = context.languages_key;
    }

    /// _readOldSettings: tdata/config, plain QDataStream.
    pub fn read_old_settings(&self, remove: bool, context: &mut ReadSettingsContext) -> bool {
        let path = self.working_dir.join("tdata/config");
        let result = match fs::read(&path) {
            Ok(bytes) => {
                let mut r = Reader::new(&bytes);
                let mut version = 0;
                while !r.at_end() {
                    match version_or_block(&mut r, &mut version) {
                        None => break,
                        Some(None) if version > APP_VERSION => break,
                        Some(None) => {}
                        Some(Some(id)) => {
                            if !read_setting(id, &mut r, version, context) {
                                break;
                            }
                        }
                    }
                }
                true
            }
            Err(_) => false,
        };
        if remove {
            let _ = fs::remove_file(&path);
        }
        result
    }

    /// Shared loop of _readOldUserSettingsFields / _readOldMtpDataFields.
    fn read_old_fields(
        &mut self,
        bytes: &[u8],
        version: &mut i32,
        context: &mut ReadSettingsContext,
        encrypted_block: u32,
    ) {
        let mut r = Reader::new(bytes);
        while !r.at_end() {
            let Some(step) = version_or_block(&mut r, version) else {
                break;
            };
            let Some(id) = step else {
                if *version > APP_VERSION {
                    return;
                }
                continue;
            };
            if id == encrypted_block {
                let salted = encrypted_block == DBI_ENCRYPTED_WITH_SALT;
                let salt = if salted {
                    match r.bytes() {
                        Some(v) => Some(v),
                        None => break,
                    }
                } else {
                    None
                };
                let Some(data) = r.bytes() else {
                    break;
                };
                if let Some(salt) = salt {
                    if salt.len() != SALT_SIZE {
                        continue;
                    }
                    self.old_key = Some(create_legacy_local_key(&[], &salt));
                }
                let Some(key) = self.old_key.clone() else {
                    continue;
                };
                let Ok(decrypted) = decrypt_local(&data, &key) else {
                    continue;
                };
                self.read_old_fields(&decrypted.stream.0[4..], version, context, encrypted_block);
            } else if !read_setting(id, &mut r, *version, context) {
                return;
            }
        }
    }

    fn read_old_file(
        &mut self,
        path: PathBuf,
        remove: bool,
        context: &mut ReadSettingsContext,
        encrypted_block: u32,
    ) -> bool {
        let result = match fs::read(&path) {
            Ok(bytes) => {
                let mut version = 0;
                self.read_old_fields(&bytes, &mut version, context, encrypted_block);
                true
            }
            Err(_) => false,
        };
        if remove {
            let _ = fs::remove_file(&path);
        }
        result
    }

    /// readOldUserSettings / _readOldUserSettings: `<data>_config`.
    pub fn read_old_user_settings(
        &mut self,
        remove: bool,
        context: &mut ReadSettingsContext,
    ) -> bool {
        let path = self.working_dir.join(format!("{}_config", self.data_file));
        self.read_old_file(path, remove, context, DBI_ENCRYPTED_WITH_SALT)
    }

    /// readOldMtpData / _readOldMtpData: `<data>`.
    pub fn read_old_mtp_data(&mut self, remove: bool, context: &mut ReadSettingsContext) -> bool {
        let path = self.working_dir.join(&self.data_file);
        self.read_old_file(path, remove, context, DBI_ENCRYPTED)
    }

    /// Local::start. `context` carries the globals ReadSetting consults;
    /// its app/session effects are left in it for the owners.
    pub fn start(
        &mut self,
        context: &mut ReadSettingsContext,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<Vec<LocalEffect>> {
        assert!(self.base_path.is_none());
        let base = self.working_dir.join("tdata/");
        let _ = fs::create_dir_all(&base);
        self.base_path = Some(base.clone());
        let mut out = Vec::new();

        let Ok(settings_data) = read_file("settings", &base) else {
            self.read_old_settings(true, context);
            self.read_old_user_settings(false, context);
            self.read_old_mtp_data(false, context);
            self.apply_read_context(context, &mut out);
            self.settings_rewrite_needed = true;
            out.push(LocalEffect::ApplyDefaultNightMode);
            return Ok(out);
        };
        let mut r = settings_data.reader();
        let (Some(salt), Some(encrypted)) = (r.bytes(), r.bytes()) else {
            self.write_settings(None, manager, random)?;
            return Ok(out);
        };
        if salt.len() != SALT_SIZE {
            self.write_settings(None, manager, random)?;
            return Ok(out);
        }
        let key = create_legacy_local_key(&[], &salt);
        self.settings_key = Some(key.clone());
        let Ok(settings) = decrypt_local(&encrypted, &key) else {
            self.write_settings(None, manager, random)?;
            return Ok(out);
        };
        let mut s = Reader::new(&settings.stream.0[4..]);
        while !s.at_end() {
            let ok = s
                .i32()
                .is_some_and(|id| read_setting(id as u32, &mut s, settings_data.version, context));
            if !ok {
                self.write_settings(None, manager, random)?;
                return Ok(out);
            }
        }
        self.old_settings_version = settings_data.version;
        self.settings_salt = salt;

        self.apply_read_context(context, &mut out);
        if context.legacy_read {
            self.write_settings(None, manager, random)?;
        }
        out.push(LocalEffect::InitialLoadTheme);
        if context.tile_read && self.use_global_background_keys {
            out.push(LocalEffect::BackgroundTile {
                day: context.tile_day,
                night: context.tile_night,
            });
        }
        out.push(LocalEffect::ReadLangPack);
        Ok(out)
    }

    /// Local::writeSettings. `snapshot` is required once writes are allowed.
    pub fn write_settings(
        &mut self,
        snapshot: Option<&SettingsSnapshot>,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<WriteSettingsResult> {
        if !self.settings_write_allowed {
            self.settings_rewrite_needed = true;
            // SettingsKey is still generated for moveLegacyBackground.
            if self.settings_key.is_some() {
                return Ok(WriteSettingsResult::Skipped);
            }
        }
        let Some(base) = self.base_path.clone() else {
            return Ok(WriteSettingsResult::Skipped);
        };
        let _ = fs::create_dir_all(&base);
        let mut file = FileWriteDescriptor::new("settings", &base);
        if self.settings_salt.is_empty() || self.settings_key.is_none() {
            self.settings_salt = vec![0; SALT_SIZE];
            random.fill(&mut self.settings_salt);
            self.settings_key = Some(create_legacy_local_key(&[], &self.settings_salt));
        }
        let key = self.settings_key.clone().unwrap();
        file.write_data(Some(&self.settings_salt));

        if !self.settings_write_allowed {
            let mut data = EncryptedDescriptor::new(0);
            file.write_encrypted(&mut data, &key, random);
            file.finish(manager, false)?;
            return Ok(WriteSettingsResult::Placeholder);
        }
        let s = snapshot.expect("writeSettings needs current values once writes are allowed");
        let mut data = EncryptedDescriptor::new(0);
        let w = &mut data.stream;
        for (id, value) in [
            (DBI_AUTO_START, s.auto_start as i32),
            (DBI_START_MINIMIZED, s.start_minimized as i32),
            (DBI_SEND_TO_MENU, s.send_to_menu as i32),
            (DBI_SEEN_TRAY_TOOLTIP, s.seen_tray_tooltip as i32),
            (DBI_AUTO_UPDATE, s.auto_update as i32),
            (DBI_LAST_UPDATE_CHECK, s.last_update_check),
            (DBI_SCALE_PERCENT, s.config_scale),
        ] {
            w.i32(id as i32);
            w.i32(value);
        }
        w.i32(DBI_FALLBACK_PRODUCTION_CONFIG as i32);
        w.bytes(&s.fallback_config);
        w.i32(DBI_APPLICATION_SETTINGS as i32);
        w.bytes(&s.application_settings);
        w.i32(DBI_DIALOG_LAST_PATH as i32);
        w.nullable_utf16(s.dialog_last_path.as_deref());
        w.i32(DBI_POWER_SAVING as i32);
        w.i32(s.power_saving);

        w.i32(DBI_THEME_KEY as i32);
        w.u64(self.theme_key_day);
        w.u64(self.theme_key_night);
        w.i32(s.night_mode as i32);
        if self.use_global_background_keys {
            w.i32(DBI_BACKGROUND_KEY as i32);
            w.u64(self.background_key_day);
            w.u64(self.background_key_night);
            w.i32(DBI_TILE_BACKGROUND as i32);
            w.i32(s.tile_day as i32);
            w.i32(s.tile_night as i32);
        }
        if self.lang_pack_key != 0 {
            w.i32(DBI_LANG_PACK_KEY as i32);
            w.u64(self.lang_pack_key);
        }
        if self.languages_key != 0 {
            w.i32(DBI_LANGUAGES_KEY as i32);
            w.u64(self.languages_key);
        }
        file.write_encrypted(&mut data, &key, random);
        file.finish(manager, false)?;
        Ok(WriteSettingsResult::Written)
    }

    pub fn rewrite_settings_if_needed(
        &mut self,
        snapshot: &SettingsSnapshot,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<WriteSettingsResult> {
        if self.settings_write_allowed {
            return Ok(WriteSettingsResult::Skipped);
        }
        self.settings_write_allowed = true;
        if self.old_settings_version < APP_VERSION || self.settings_rewrite_needed {
            return self.write_settings(Some(snapshot), manager, random);
        }
        Ok(WriteSettingsResult::Skipped)
    }

    /// Local::reset; the owners run the returned effects before the write.
    pub fn reset(
        &mut self,
        snapshot_after_reset: impl FnOnce(&[LocalEffect]) -> SettingsSnapshot,
        manager: &mut WriteManager,
        random: &mut impl Random,
    ) -> Result<Vec<LocalEffect>> {
        let effects = vec![
            LocalEffect::StopLocalLoader,
            LocalEffect::ResetBackground,
            LocalEffect::ResetSettingsOnLastLogout,
        ];
        self.old_settings_version = 0;
        let snapshot = snapshot_after_reset(&effects);
        let allowed = self.settings_write_allowed;
        self.write_settings(allowed.then_some(&snapshot), manager, random)?;
        Ok(effects)
    }

    pub fn old_settings_version(&self) -> i32 {
        self.old_settings_version
    }

    fn autoupdate_prefix_file(&self) -> PathBuf {
        self.working_dir.join("tdata/prefix")
    }

    /// readAutoupdatePrefixRaw.
    fn read_autoupdate_prefix_raw(&mut self) -> String {
        if !self.autoupdate_prefix.is_empty() {
            return self.autoupdate_prefix.clone();
        }
        if let Ok(bytes) = fs::read(self.autoupdate_prefix_file()) {
            let value = String::from_utf8_lossy(&bytes).into_owned();
            if !value.is_empty() {
                self.autoupdate_prefix = value;
                return self.autoupdate_prefix.clone();
            }
        }
        self.autoupdate_prefix = DEFAULT_AUTOUPDATE_PREFIX.into();
        self.autoupdate_prefix.clone()
    }

    /// writeAutoupdatePrefix: AyuGram ignores the argument and pins its own host.
    pub fn write_autoupdate_prefix(
        &mut self,
        _prefix: &str,
        updater_disabled: bool,
        auto_update: bool,
    ) -> Vec<LocalEffect> {
        if updater_disabled {
            return Vec::new();
        }
        let current = self.read_autoupdate_prefix_raw();
        if current == DEFAULT_AUTOUPDATE_PREFIX {
            return Vec::new();
        }
        self.autoupdate_prefix = DEFAULT_AUTOUPDATE_PREFIX.into();
        let _ = fs::write(self.autoupdate_prefix_file(), DEFAULT_AUTOUPDATE_PREFIX);
        if auto_update {
            vec![LocalEffect::StartUpdateChecker]
        } else {
            Vec::new()
        }
    }

    /// readAutoupdatePrefix: raw value without trailing slashes.
    pub fn read_autoupdate_prefix(&mut self) -> String {
        self.read_autoupdate_prefix_raw()
            .trim_end_matches('/')
            .to_owned()
    }

    fn update_manifest_file(&self) -> PathBuf {
        self.working_dir.join("tdata/update-manifest")
    }

    /// writeUpdateManifest: signature then manifest, written atomically.
    pub fn write_update_manifest(&self, manifest: &[u8], signature: &[u8], updater_disabled: bool) {
        if updater_disabled || signature.len() != SIGNATURE_SIZE || manifest.is_empty() {
            return;
        }
        let path = self.update_manifest_file();
        let _ = save_file(&path, &[signature, manifest].concat());
    }

    /// readUpdateManifest: (manifest, signature).
    pub fn read_update_manifest(&self, updater_disabled: bool) -> Option<(Vec<u8>, Vec<u8>)> {
        if updater_disabled {
            return None;
        }
        let content = fs::read(self.update_manifest_file()).ok()?;
        if content.len() <= SIGNATURE_SIZE || content.len() > SIGNATURE_SIZE + MAX_MANIFEST_SIZE {
            return None;
        }
        let (signature, manifest) = content.split_at(SIGNATURE_SIZE);
        Some((manifest.to_vec(), signature.to_vec()))
    }
}

/// QSaveFile: write a sibling temp file, then rename over the target.
fn save_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}
