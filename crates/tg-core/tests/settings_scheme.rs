use tg_base::qt_stream::{Reader, Writer};
use tg_core::data::data_peer_id::deserialize_peer_id;
use tg_core::storage::details::storage_settings_scheme::*;
use tg_mtproto::mtproto_proxy_data::{Settings as ProxySettings, Type as ProxyType};

fn run(
    block: u32,
    version: i32,
    c: &mut ReadSettingsContext,
    body: impl FnOnce(&mut Writer),
) -> bool {
    let mut w = Writer::default();
    body(&mut w);
    let mut r = Reader::new(&w.0);
    let ok = read_setting(block, &mut r, version, c);
    if ok {
        assert!(r.at_end(), "block {block:#x} left bytes");
    }
    ok
}
fn read(block: u32, body: impl FnOnce(&mut Writer)) -> (bool, ReadSettingsContext) {
    let mut c = ReadSettingsContext::default();
    let ok = run(block, 4000000, &mut c, body);
    (ok, c)
}
fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn qstr(w: &mut Writer, s: &str) {
    w.string(s);
}
fn ushort(w: &mut Writer, v: u16) {
    w.0.extend(v.to_be_bytes());
}
fn proxy(w: &mut Writer, kind: i32, host: &str, port: i32) {
    w.i32(kind);
    qstr(w, host);
    w.i32(port);
    qstr(w, "u");
    qstr(w, "p");
}

#[test]
fn auth_blocks_fill_legacy_context() {
    let (ok, c) = read(DBI_USER, |w| {
        w.i32(777);
        w.i32(2);
    });
    assert!(ok);
    assert_eq!((c.mtp_legacy_user_id, c.mtp_legacy_main_dc_id), (777, 2));
    let (ok, c) = read(DBI_KEY, |w| {
        w.i32(4);
        w.0.extend([7u8; 256]);
    });
    assert!(ok);
    assert_eq!(c.mtp_legacy_keys.len(), 1);
    assert_eq!(c.mtp_legacy_keys[0].dc_id(), 4);
    let (ok, _) = read(DBI_KEY, |w| {
        w.i32(4);
        w.0.extend([7u8; 255]);
    });
    assert!(!ok);
    let (ok, c) = read(DBI_MTP_AUTHORIZATION, |w| w.bytes(b"auth"));
    assert!(ok && c.mtp_authorization == b"auth" && !c.legacy_read);
}

#[test]
fn unknown_and_truncated_blocks_fail() {
    assert!(!read(0x10, |w| w.i32(1)).0);
    assert!(!read(0x62, |w| w.i32(1)).0);
    assert!(!read(DBI_AUTO_START, |_| {}).0);
    assert!(!read(DBI_THEME_KEY, |w| w.u64(1)).0);
    // QString with odd byte count is ReadCorruptData.
    assert!(
        !read(DBI_DIALOG_LAST_PATH, |w| {
            w.i32(3);
            w.0.extend([0, 0x41, 0]);
        })
        .0
    );
}

#[test]
fn legacy_limits_feed_fallback_config() {
    let mut c = ReadSettingsContext::default();
    assert!(run(DBI_CHAT_SIZE_MAX_OLD, 1, &mut c, |w| w.i32(200)));
    assert!(run(DBI_MEGAGROUP_SIZE_MAX_OLD, 1, &mut c, |w| w.i32(0)));
    assert!(run(DBI_STICKERS_RECENT_LIMIT_OLD, 1, &mut c, |w| w.i32(30)));
    assert!(run(DBI_TXT_DOMAIN_STRING_OLD, 1, &mut c, |w| qstr(
        w, "t.me"
    )));
    assert!(c.legacy_read);
    let (effect, options) = apply_read_fallback_config(&mut c);
    assert_eq!(
        effect,
        FallbackConfigEffect::Legacy {
            chat_size_max: Some(200),
            stickers_recent_limit: Some(30),
            megagroup_size_max: None,
            txt_domain_string: Some("t.me".into()),
        }
    );
    assert!(options.is_some());
    let (ok, mut c) = read(DBI_FALLBACK_PRODUCTION_CONFIG, |w| w.bytes(b"cfg"));
    assert!(ok);
    let (effect, options) = apply_read_fallback_config(&mut c);
    assert_eq!(effect, FallbackConfigEffect::Construct(b"cfg".to_vec()));
    assert!(options.is_none());
}

#[test]
fn cache_settings_validation_and_5611_workaround() {
    let big = 1i64 << 30;
    let small = (1i64 << 24) - 1;
    let (ok, c) = read(DBI_CACHE_SETTINGS, |w| {
        w.u64(big as u64);
        w.i32(-1); // int64 max stored as qint32
        w.u64(big as u64 * 2);
        w.i32(i32::MAX);
    });
    assert!(ok && !c.legacy_read);
    assert_eq!(
        (c.cache_total_size_limit, c.cache_total_time_limit),
        (big, 0)
    );
    assert_eq!(
        (
            c.cache_big_file_total_size_limit,
            c.cache_big_file_total_time_limit
        ),
        (big * 2, 0)
    );
    assert!(
        !read(DBI_CACHE_SETTINGS, |w| {
            w.u64(small as u64);
            w.i32(10);
            w.u64(big as u64);
            w.i32(10);
        })
        .0
    );
    assert!(
        !read(DBI_CACHE_SETTINGS_OLD, |w| {
            w.u64(big as u64);
            w.i32(-5);
        })
        .0
    );
    let (ok, c) = read(DBI_CACHE_SETTINGS_OLD, |w| {
        w.u64(big as u64);
        w.i32(3600);
    });
    assert!(ok && c.legacy_read);
    assert_eq!(c.cache_big_file_total_time_limit, 3600);
}

#[test]
fn app_flag_blocks() {
    let (_, c) = read(DBI_SOUND_FLASH_BOUNCE_NOTIFY_OLD, |w| w.i32(0x01));
    assert_eq!(
        c.app_effects,
        vec![
            AppEffect::SoundNotify(true),
            AppEffect::FlashBounceNotify(true)
        ]
    );
    let (_, c) = read(DBI_SOUND_FLASH_BOUNCE_NOTIFY_OLD, |w| w.i32(0x02));
    assert_eq!(
        c.app_effects,
        vec![
            AppEffect::SoundNotify(false),
            AppEffect::FlashBounceNotify(false)
        ]
    );
    assert_eq!(
        read(DBI_NOTIFICATIONS_COUNT_OLD, |w| w.i32(0))
            .1
            .app_effects,
        vec![AppEffect::NotificationsCount(3)]
    );
    assert_eq!(
        read(DBI_NOTIFICATIONS_CORNER_OLD, |w| w.i32(4))
            .1
            .app_effects,
        vec![AppEffect::NotificationsCorner(2)]
    );
    assert_eq!(
        read(DBI_NOTIFICATIONS_CORNER_OLD, |w| w.i32(3))
            .1
            .app_effects,
        vec![AppEffect::NotificationsCorner(3)]
    );
    for (v, mode) in [
        (0, WorkMode::WindowAndTray),
        (1, WorkMode::TrayOnly),
        (2, WorkMode::WindowOnly),
        (9, WorkMode::WindowAndTray),
    ] {
        assert_eq!(
            read(DBI_WORK_MODE_OLD, |w| w.i32(v)).1.app_effects,
            vec![AppEffect::WorkMode(mode)]
        );
    }
    for (v, view) in [
        (0, NotifyView::ShowPreview),
        (1, NotifyView::ShowName),
        (2, NotifyView::ShowNothing),
        (-1, NotifyView::ShowPreview),
    ] {
        assert_eq!(
            read(DBI_NOTIFY_VIEW_OLD, |w| w.i32(v)).1.app_effects,
            vec![AppEffect::NotifyView(view)]
        );
    }
    assert_eq!(
        read(DBI_DIALOGS_WIDTH_RATIO_OLD, |w| w.i32(250000))
            .1
            .app_effects,
        vec![AppEffect::DialogsWidthRatio(0.25)]
    );
    assert_eq!(
        read(DBI_SONG_VOLUME_OLD, |w| w.i32(3_000_000))
            .1
            .app_effects,
        vec![AppEffect::SongVolume(1.)]
    );
    assert!(read(DBI_PLAYBACK_SPEED_OLD, |w| w.i32(1))
        .1
        .app_effects
        .is_empty());
    assert_eq!(
        read(DBI_PLAYBACK_SPEED_OLD, |w| w.i32(2))
            .1
            .app_effects
            .len(),
        2
    );
    assert!(!read(DBI_SEND_KEY_OLD, |w| w.i32(2)).0);
    assert_eq!(
        read(DBI_SEND_KEY_OLD, |w| w.i32(1)).1.app_effects,
        vec![AppEffect::SendSubmitWay(SendSubmitWay::CtrlEnter)]
    );
}

#[test]
fn auto_update_and_scale_use_globals() {
    let (_, c) = read(DBI_AUTO_UPDATE, |w| w.i32(0));
    assert_eq!(
        c.app_effects,
        vec![AppEffect::AutoUpdate(false), AppEffect::StopUpdateChecker]
    );
    let mut c = ReadSettingsContext {
        updater_disabled: true,
        ..Default::default()
    };
    run(DBI_AUTO_UPDATE, 1, &mut c, |w| w.i32(0));
    assert_eq!(c.app_effects, vec![AppEffect::AutoUpdate(false)]);

    for (v, scale) in [(0, SCALE_AUTO), (2, 125), (4, 200), (7, 175)] {
        let mut c = ReadSettingsContext {
            config_scale: 175,
            ..Default::default()
        };
        run(DBI_SCALE_OLD, 1, &mut c, |w| w.i32(v));
        assert_eq!(c.app_effects, vec![AppEffect::Scale(scale)]);
    }
    let mut c = ReadSettingsContext {
        config_scale: 150,
        ..Default::default()
    };
    assert!(run(DBI_SCALE_PERCENT, 1, &mut c, |w| w.i32(110)));
    assert!(c.app_effects.is_empty());
    assert_eq!(
        read(DBI_SCALE_PERCENT, |w| w.i32(110)).1.app_effects,
        vec![AppEffect::Scale(110)]
    );
}

#[test]
fn auto_download_creates_session_settings_lazily() {
    let (_, c) = read(DBI_AUTO_DOWNLOAD_OLD, |w| {
        w.i32(0x01);
        w.i32(0x02);
        w.i32(0);
    });
    use AutoDownloadSource::*;
    use AutoDownloadType::*;
    assert_eq!(
        c.session_settings.unwrap(),
        vec![
            SessionEffect::AutoDownloadBytesLimit(User, Photo),
            SessionEffect::AutoDownloadBytesLimit(Group, VoiceMessage),
            SessionEffect::AutoDownloadBytesLimit(Channel, VoiceMessage),
        ]
    );
    let (_, c) = read(DBI_AUTO_PLAY_OLD, |w| w.i32(1));
    assert!(c.session_settings.is_none() && c.legacy_read);
    let (_, c) = read(DBI_AUTO_PLAY_OLD, |w| w.i32(0));
    assert_eq!(c.session_settings.unwrap().len(), 9);
}

#[test]
fn proxy_old_old() {
    let (ok, c) = read(DBI_CONNECTION_TYPE_OLD_OLD, |w| {
        w.i32(3);
        qstr(w, "proxy.example");
        w.i32(1080);
        qstr(w, "u");
        qstr(w, "p");
    });
    assert!(ok);
    let AppEffect::ProxySelected(p) = &c.app_effects[0] else {
        panic!()
    };
    assert_eq!(
        (p.kind, p.host.as_str(), p.port),
        (ProxyType::Socks5, "proxy.example", 1080)
    );
    assert_eq!(
        c.app_effects[1],
        AppEffect::ProxySettings(ProxySettings::Enabled)
    );
    assert!(matches!(&c.app_effects[2], AppEffect::ProxyList(l) if l.len() == 1));
    assert_eq!(c.app_effects[3], AppEffect::RefreshGlobalProxy);
    // dbictAuto reads nothing more and resets to System.
    let (ok, c) = read(DBI_CONNECTION_TYPE_OLD_OLD, |w| w.i32(0));
    assert!(ok);
    assert_eq!(
        c.app_effects[1],
        AppEffect::ProxySettings(ProxySettings::System)
    );
    assert!(matches!(&c.app_effects[2], AppEffect::ProxyList(l) if l.is_empty()));
}

fn proxy_effects(c: &ReadSettingsContext) -> (usize, String, ProxySettings, Option<bool>) {
    let mut list = 0;
    let mut selected = String::new();
    let mut settings = ProxySettings::System;
    let mut calls = None;
    for e in &c.app_effects {
        match e {
            AppEffect::ProxyList(l) => list = l.len(),
            AppEffect::ProxySelected(p) => selected = p.host.clone(),
            AppEffect::ProxySettings(s) => settings = *s,
            AppEffect::ProxyUseForCalls(v) => calls = Some(*v),
            _ => {}
        }
    }
    (list, selected, settings, calls)
}

#[test]
fn proxy_lists() {
    let (ok, c) = read(DBI_CONNECTION_TYPE_OLD, |w| {
        w.i32(5);
        w.i32(2); // count
        w.i32(2); // index
        w.i32(1); // Enabled
        w.i32(1); // calls
        proxy(w, 1024 + 1, "a", 1);
        proxy(w, 1024 + 2, "b", 2);
    });
    assert!(ok);
    assert_eq!(
        proxy_effects(&c),
        (2, "b".into(), ProxySettings::Enabled, Some(true))
    );
    assert_eq!(c.app_effects.last(), Some(&AppEffect::RefreshGlobalProxy));

    // Old list: |index| > count marks calls and shifts; negative means disabled.
    let (ok, c) = read(DBI_CONNECTION_TYPE_OLD, |w| {
        w.i32(4);
        w.i32(1);
        w.i32(-2);
        proxy(w, 3, "a", 1);
    });
    assert!(ok);
    assert_eq!(
        proxy_effects(&c),
        (1, "a".into(), ProxySettings::System, Some(true))
    );

    // An invalid entry before the selected one moves the index back.
    let (ok, c) = read(DBI_CONNECTION_TYPE_OLD, |w| {
        w.i32(4);
        w.i32(2);
        w.i32(2);
        proxy(w, 3, "", 1);
        proxy(w, 3, "b", 2);
    });
    assert!(ok);
    assert_eq!(
        proxy_effects(&c),
        (1, "b".into(), ProxySettings::Enabled, Some(false))
    );

    // Enabled without a valid selection falls back to System; unknown values too.
    for settings in [1, 7] {
        let (_, c) = read(DBI_CONNECTION_TYPE_OLD, |w| {
            w.i32(5);
            w.i32(1);
            w.i32(5);
            w.i32(settings);
            w.i32(0);
            proxy(w, 3, "a", 1);
        });
        assert_eq!(proxy_effects(&c).2, ProxySettings::System);
    }
    let (_, c) = read(DBI_CONNECTION_TYPE_OLD, |w| {
        w.i32(5);
        w.i32(0);
        w.i32(0);
        w.i32(2);
        w.i32(0);
    });
    assert_eq!(proxy_effects(&c).2, ProxySettings::Disabled);

    // Single proxy; tcp/http types enable it.
    let (_, c) = read(DBI_CONNECTION_TYPE_OLD, |w| {
        w.i32(2);
        proxy(w, 2, "h", 8080);
    });
    assert_eq!(
        proxy_effects(&c),
        (1, "h".into(), ProxySettings::Enabled, None)
    );
    assert!(
        !read(DBI_CONNECTION_TYPE_OLD, |w| {
            w.i32(5);
            w.i32(2);
            w.i32(1);
            w.i32(1);
            w.i32(0);
            proxy(w, 3, "a", 1);
        })
        .0
    );
}

#[test]
fn theme_and_tile_backgrounds() {
    let mut c = ReadSettingsContext::default();
    assert!(run(DBI_THEME_KEY, 1, &mut c, |w| {
        w.u64(11);
        w.u64(22);
        w.i32(1);
    }));
    assert_eq!(
        (c.theme_key_day, c.theme_key_night, c.night_mode),
        (11, 22, true)
    );
    assert_eq!(c.app_effects, vec![AppEffect::NightModeValue(true)]);
    // In night mode the old block lands in tileDay.
    assert!(run(DBI_TILE_BACKGROUND_OLD, 9000, &mut c, |w| w.i32(1)));
    assert!(c.tile_day && c.tile_night && c.tile_read && c.legacy_read);

    let mut c = ReadSettingsContext::default();
    run(DBI_TILE_BACKGROUND_OLD, 8004, &mut c, |w| w.i32(1));
    assert!(!c.tile_night);
    let mut c = ReadSettingsContext {
        legacy_has_custom_day_background: true,
        ..Default::default()
    };
    run(DBI_TILE_BACKGROUND_OLD, 8004, &mut c, |w| w.i32(1));
    assert!(c.tile_night);

    let (_, c) = read(DBI_TILE_BACKGROUND, |w| {
        w.i32(5);
        w.i32(0);
    });
    assert!(c.tile_day && !c.tile_night && c.tile_read && !c.legacy_read);
    let (_, c) = read(DBI_BACKGROUND_KEY, |w| {
        w.u64(1);
        w.u64(2);
    });
    assert!(c.background_keys_read);
}

#[test]
fn paths_and_window_position() {
    for (input, output) in [("dl", "dl/"), ("dl/", "dl/"), ("tmp", "tmp"), ("", "")] {
        let (_, c) = read(DBI_DOWNLOAD_PATH_OLD, |w| {
            qstr(w, input);
            w.bytes(b"bm");
        });
        assert_eq!(
            c.app_effects,
            vec![
                AppEffect::DownloadPathBookmark(b"bm".to_vec()),
                AppEffect::DownloadPath(u16s(output)),
                AppEffect::DownloadPathEnableAccess,
            ]
        );
    }
    let (_, c) = read(DBI_DOWNLOAD_PATH_OLD_OLD, |w| w.i32(-1));
    assert_eq!(
        c.app_effects,
        vec![
            AppEffect::DownloadPathBookmark(Vec::new()),
            AppEffect::DownloadPath(Vec::new())
        ]
    );
    // Lone surrogates survive in path values.
    let (_, c) = read(DBI_DIALOG_LAST_PATH, |w| {
        w.nullable_utf16(Some(&[0xD800, 0x41]))
    });
    assert_eq!(
        c.app_effects,
        vec![AppEffect::DialogLastPath(vec![0xD800, 0x41])]
    );
    let (_, c) = read(DBI_WINDOW_POSITION_OLD, |w| {
        for v in [1, 2, 3, 4, 5, 1] {
            w.i32(v);
        }
    });
    assert_eq!(
        c.app_effects,
        vec![AppEffect::WindowPosition(WindowPosition {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
            moncrc: 5,
            maximized: 1,
            scale: 0,
        })]
    );
}

#[test]
fn emoji_and_sticker_containers() {
    let (_, c) = read(DBI_RECENT_EMOJI_OLD_OLD_OLD, |w| {
        w.i32(2);
        w.i32(0xD83CDDEFu32 as i32);
        ushort(w, 3);
        w.i32(0x263A);
        ushort(w, 1);
    });
    assert_eq!(
        c.app_effects,
        vec![AppEffect::LegacyRecentEmojiOldKeys(vec![
            (0xD83CDDEFD83CDDF5, 3),
            (0x263A, 1)
        ])]
    );
    let (_, c) = read(DBI_RECENT_EMOJI_OLD_OLD, |w| w.i32(0));
    assert!(c.app_effects.is_empty() && c.legacy_read);
    let (_, c) = read(DBI_RECENT_EMOJI_OLD, |w| w.i32(0));
    assert_eq!(
        c.app_effects,
        vec![AppEffect::LegacyRecentEmoji(Vec::new())]
    );
    let (_, c) = read(DBI_EMOJI_VARIANTS_OLD, |w| {
        w.i32(2);
        qstr(w, "x");
        w.i32(1);
        qstr(w, "x");
        w.i32(4);
    });
    let AppEffect::LegacyEmojiVariants(map) = &c.app_effects[0] else {
        panic!()
    };
    assert_eq!(map.get(&u16s("x")), Some(&4));
    let (_, c) = read(DBI_RECENT_STICKERS, |w| {
        w.i32(1);
        w.u64(99);
        ushort(w, 7);
    });
    assert_eq!(
        c.app_effects,
        vec![AppEffect::RecentStickersPreload(vec![(99, 7)])]
    );
    assert!(
        !read(DBI_RECENT_STICKERS, |w| {
            w.i32(2);
            w.u64(99);
            ushort(w, 7);
        })
        .0
    );
}

#[test]
fn hidden_pinned_and_muted_peers() {
    let (_, c) = read(DBI_HIDDEN_PINNED_MESSAGES_OLD, |w| {
        w.i32(2);
        w.u64(5);
        w.i32(10);
        w.u64(3);
        w.i32(20);
    });
    let expected: Vec<_> = [(3u64, 20i64), (5, 10)]
        .into_iter()
        .map(|(peer, msg)| SessionEffect::HiddenPinnedMessageId {
            peer: deserialize_peer_id(peer).value,
            msg,
        })
        .collect();
    assert_eq!(c.session_settings.unwrap(), expected);
    assert!(
        read(DBI_MUTED_PEERS_OLD, |w| {
            w.i32(2);
            w.u64(1);
            w.u64(2);
        })
        .0
    );
    assert!(
        !read(DBI_MUTED_PEERS_OLD, |w| {
            w.i32(-1);
            w.u64(1);
        })
        .0
    );
}

#[test]
fn call_settings_inner_stream_is_optional() {
    let mut inner = Writer::default();
    qstr(&mut inner, "out");
    inner.i32(50);
    qstr(&mut inner, "in");
    inner.i32(60);
    inner.i32(1);
    let (ok, c) = read(DBI_CALL_SETTINGS_OLD, |w| w.bytes(&inner.0));
    assert!(ok);
    assert_eq!(c.app_effects.len(), 5);
    assert_eq!(
        c.app_effects[0],
        AppEffect::CallPlaybackDeviceId(u16s("out"))
    );
    let (ok, c) = read(DBI_CALL_SETTINGS_OLD, |w| w.bytes(&inner.0[..10]));
    assert!(ok && c.app_effects.is_empty() && c.legacy_read);
}

#[test]
fn dc_options_blocks() {
    let mut c = ReadSettingsContext::default();
    assert!(run(DBI_DC_OPTION_OLD_OLD, 1, &mut c, |w| {
        w.i32(2);
        qstr(w, "host");
        qstr(w, "149.154.167.50");
        w.i32(443);
    }));
    assert!(run(DBI_DC_OPTION_OLD, 1, &mut c, |w| {
        w.i32(3);
        w.i32(0);
        qstr(w, "149.154.175.100");
        w.i32(443);
    }));
    assert!(c.legacy_read);
    assert!(run(DBI_DC_OPTIONS_OLD, 1, &mut c, |w| w.bytes(b"garbage")));
}

#[test]
fn session_and_application_settings_pass_through() {
    let (_, c) = read(DBI_SESSION_SETTINGS, |w| w.bytes(b"s"));
    assert_eq!(
        c.session_settings.unwrap(),
        vec![SessionEffect::AddFromSerialized(b"s".to_vec())]
    );
    let (_, c) = read(DBI_APPLICATION_SETTINGS, |w| w.bytes(b"a"));
    assert_eq!(
        c.app_effects,
        vec![AppEffect::ApplicationSettings(b"a".to_vec())]
    );
    let (_, c) = read(DBI_LANG_PACK_KEY, |w| w.u64(5));
    assert_eq!(c.lang_pack_key, 5);
}
