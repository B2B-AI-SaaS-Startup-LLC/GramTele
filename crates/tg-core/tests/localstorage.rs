use std::fs;
use tg_base::qt_stream::Writer;
use tg_core::storage::details::storage_file_utilities::*;
use tg_core::storage::details::storage_settings_scheme::*;
use tg_core::storage::localstorage::*;
use tg_mtproto::core_types::Random;

struct Entropy(u8);
impl Random for Entropy {
    fn fill(&mut self, data: &mut [u8]) {
        for b in data {
            self.0 = self.0.wrapping_mul(31).wrapping_add(7);
            *b = self.0;
        }
    }
}

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ayugram-localstorage-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn snapshot() -> SettingsSnapshot {
    SettingsSnapshot {
        auto_start: true,
        start_minimized: false,
        send_to_menu: true,
        seen_tray_tooltip: true,
        auto_update: false,
        last_update_check: 123,
        config_scale: 150,
        fallback_config: b"cfg".to_vec(),
        application_settings: b"app".to_vec(),
        dialog_last_path: None,
        power_saving: 5,
        night_mode: true,
        tile_day: true,
        tile_night: false,
    }
}

fn start(local: &mut Local) -> (Vec<LocalEffect>, ReadSettingsContext) {
    let mut ctx = ReadSettingsContext::default();
    let mut m = WriteManager::default();
    let effects = local.start(&mut ctx, &mut m, &mut Entropy(1)).unwrap();
    m.write_sync_all().unwrap();
    (effects, ctx)
}

#[test]
fn settings_round_trip() {
    let d = dir("round");
    let mut local = Local::new(&d);
    let (effects, _) = start(&mut local);
    assert!(local.settings_rewrite_needed);
    assert!(effects.contains(&LocalEffect::ApplyDefaultNightMode));
    assert!(!d.join("tdata/settingss").exists());

    local.theme_key_day = 11;
    local.theme_key_night = 22;
    local.lang_pack_key = 33;
    local.use_global_background_keys = true;
    local.background_key_day = 44;
    let mut m = WriteManager::default();
    let r = local
        .rewrite_settings_if_needed(&snapshot(), &mut m, &mut Entropy(2))
        .unwrap();
    assert_eq!(r, WriteSettingsResult::Written);
    m.write_sync_all().unwrap();

    let mut again = Local::new(&d);
    let (effects, ctx) = start(&mut again);
    assert_eq!(again.old_settings_version(), APP_VERSION);
    assert_eq!(
        (
            again.theme_key_day,
            again.theme_key_night,
            again.lang_pack_key
        ),
        (11, 22, 33)
    );
    assert!(again.use_global_background_keys);
    assert_eq!(again.background_key_day, 44);
    assert!(effects.contains(&LocalEffect::InitialLoadTheme));
    assert!(effects.contains(&LocalEffect::BackgroundTile {
        day: true,
        night: false
    }));
    assert_eq!(effects.last(), Some(&LocalEffect::ReadLangPack));
    assert!(ctx.app_effects.contains(&AppEffect::AutoStart(true)));
    assert!(ctx.app_effects.contains(&AppEffect::Scale(150)));
    assert!(ctx
        .app_effects
        .contains(&AppEffect::ApplicationSettings(b"app".to_vec())));
    assert!(ctx
        .app_effects
        .contains(&AppEffect::DialogLastPath(Vec::new())));
    assert!(ctx.app_effects.contains(&AppEffect::NightModeValue(true)));
    assert_eq!(ctx.fallback_config, b"cfg");
    assert!(!ctx.legacy_read);
    // Same version and no rewrite request: nothing is written again.
    let mut m = WriteManager::default();
    assert_eq!(
        again
            .rewrite_settings_if_needed(&snapshot(), &mut m, &mut Entropy(3))
            .unwrap(),
        WriteSettingsResult::Skipped
    );
}

#[test]
fn placeholder_until_writes_are_allowed() {
    let d = dir("placeholder");
    let mut local = Local::new(&d);
    start(&mut local);
    let mut m = WriteManager::default();
    assert_eq!(
        local.write_settings(None, &mut m, &mut Entropy(4)).unwrap(),
        WriteSettingsResult::Placeholder
    );
    m.write_sync_all().unwrap();
    // The key now exists, so later early writes are deferred.
    assert_eq!(
        local.write_settings(None, &mut m, &mut Entropy(4)).unwrap(),
        WriteSettingsResult::Skipped
    );
    let mut again = Local::new(&d);
    let (effects, ctx) = start(&mut again);
    assert!(effects.contains(&LocalEffect::InitialLoadTheme));
    assert!(ctx.app_effects.is_empty());
    assert!(again.settings_key().is_some());
}

#[test]
fn bad_salt_rewrites_settings() {
    let d = dir("badsalt");
    fs::create_dir_all(d.join("tdata")).unwrap();
    let mut w = Writer::default();
    w.bytes(&[1; 16]);
    w.bytes(&[2; 32]);
    fs::write(d.join("tdata/settingss"), encode_file(&w.0, APP_VERSION)).unwrap();
    let mut local = Local::new(&d);
    let (effects, _) = start(&mut local);
    assert!(effects.is_empty());
    assert!(local.settings_rewrite_needed);
    assert!(local.settings_key().is_some());
}

fn encrypted_block(
    key: &tg_mtproto::mtproto_auth_key::AuthKey,
    body: impl FnOnce(&mut Writer),
) -> Vec<u8> {
    let mut data = EncryptedDescriptor::new(0);
    body(&mut data.stream);
    prepare_encrypted(&mut data, key, &mut Entropy(9))
}

#[test]
fn legacy_files() {
    let d = dir("legacy");
    fs::create_dir_all(d.join("tdata")).unwrap();
    let mut config = Writer::default();
    config.i32(DBI_VERSION as i32);
    config.i32(1000);
    config.i32(DBI_AUTO_START as i32);
    config.i32(1);
    fs::write(d.join("tdata/config"), &config.0).unwrap();

    let salt = [5u8; 32];
    let key = create_legacy_local_key(&[], &salt);
    let mut user = Writer::default();
    user.i32(DBI_ENCRYPTED_WITH_SALT as i32);
    user.bytes(&salt);
    user.bytes(&encrypted_block(&key, |w| {
        w.i32(DBI_SEND_TO_MENU as i32);
        w.i32(1);
    }));
    fs::write(d.join("data_config"), &user.0).unwrap();

    let mut mtp = Writer::default();
    mtp.i32(DBI_ENCRYPTED as i32);
    mtp.bytes(&encrypted_block(&key, |w| {
        w.i32(DBI_USER as i32);
        w.i32(42);
        w.i32(2);
    }));
    fs::write(d.join("data"), &mtp.0).unwrap();

    let mut local = Local::new(&d);
    let (effects, ctx) = start(&mut local);
    assert!(matches!(effects[0], LocalEffect::FallbackConfig(_)));
    assert!(local.fallback_dc_options.is_some());
    assert!(ctx.app_effects.contains(&AppEffect::AutoStart(true)));
    assert!(ctx.app_effects.contains(&AppEffect::SendToMenu(true)));
    assert_eq!((ctx.mtp_legacy_user_id, ctx.mtp_legacy_main_dc_id), (42, 2));
    // config is removed, user config and mtp data stay for later readers.
    assert!(!d.join("tdata/config").exists());
    assert!(d.join("data_config").exists() && d.join("data").exists());
    assert!(local.old_key.is_some());
}

#[test]
fn legacy_mtp_without_key_and_newer_version() {
    let d = dir("legacy2");
    let key = create_legacy_local_key(&[], &[1; 32]);
    let mut mtp = Writer::default();
    mtp.i32(DBI_ENCRYPTED as i32);
    mtp.bytes(&encrypted_block(&key, |w| {
        w.i32(DBI_USER as i32);
        w.i32(1);
        w.i32(1);
    }));
    mtp.i32(DBI_VERSION as i32);
    mtp.i32(APP_VERSION + 1);
    mtp.i32(DBI_AUTO_START as i32);
    mtp.i32(1);
    fs::write(d.join("data"), &mtp.0).unwrap();
    let mut local = Local::new(&d);
    let mut ctx = ReadSettingsContext::default();
    assert!(local.read_old_mtp_data(true, &mut ctx));
    // No OldKey: the encrypted part is skipped; the newer version stops reading.
    assert_eq!(ctx.mtp_legacy_user_id, 0);
    assert!(ctx.app_effects.is_empty());
    assert!(!d.join("data").exists());
    assert!(!local.read_old_mtp_data(false, &mut ctx));
}

#[test]
fn autoupdate_prefix_and_manifest() {
    let d = dir("prefix");
    fs::create_dir_all(d.join("tdata")).unwrap();
    let mut local = Local::new(&d);
    assert_eq!(local.read_autoupdate_prefix(), "https://update.ayugram.one");
    assert!(local
        .write_autoupdate_prefix("https://other/", false, true)
        .is_empty());

    let mut local = Local::new(&d);
    fs::write(d.join("tdata/prefix"), "https://mirror.example//").unwrap();
    assert_eq!(local.read_autoupdate_prefix(), "https://mirror.example");
    assert_eq!(
        local.write_autoupdate_prefix("ignored", false, true),
        vec![LocalEffect::StartUpdateChecker]
    );
    assert_eq!(
        fs::read_to_string(d.join("tdata/prefix")).unwrap(),
        DEFAULT_AUTOUPDATE_PREFIX
    );
    assert!(Local::new(&d)
        .write_autoupdate_prefix("x", true, true)
        .is_empty());

    local.write_update_manifest(b"{}", &[7; 64], false);
    assert_eq!(
        local.read_update_manifest(false),
        Some((b"{}".to_vec(), vec![7; 64]))
    );
    assert_eq!(local.read_update_manifest(true), None);
    local.write_update_manifest(b"x", &[7; 63], false);
    assert_eq!(
        local.read_update_manifest(false),
        Some((b"{}".to_vec(), vec![7; 64]))
    );
    fs::write(d.join("tdata/update-manifest"), [0u8; 64]).unwrap();
    assert_eq!(local.read_update_manifest(false), None);
    fs::write(
        d.join("tdata/update-manifest"),
        vec![0u8; 64 + 256 * 1024 + 1],
    )
    .unwrap();
    assert_eq!(local.read_update_manifest(false), None);
}

#[test]
fn reset_writes_with_effects() {
    let d = dir("reset");
    let mut local = Local::new(&d);
    start(&mut local);
    local.settings_write_allowed = true;
    local.old_settings_version = APP_VERSION;
    let mut m = WriteManager::default();
    let effects = local
        .reset(
            |e| {
                assert_eq!(e.len(), 3);
                snapshot()
            },
            &mut m,
            &mut Entropy(5),
        )
        .unwrap();
    assert_eq!(effects[0], LocalEffect::StopLocalLoader);
    assert_eq!(local.old_settings_version(), 0);
}
