# storage_settings_scheme → Rust

Оригинал: `Telegram/SourceFiles/storage/details/storage_settings_scheme.{h,cpp}`
(AyuGram 0f2a31bb). Реализация:
`crates/tg-core/src/storage/details/storage_settings_scheme.rs`, тесты
`crates/tg-core/tests/settings_scheme.rs`.

Разбор байтов каждого блока перенесён полностью. Записи в `Core::App().settings()`,
`cSet*`-глобалы, `PowerSaving`, `Window::Theme` и `UpdateChecker` возвращаются
упорядоченными `AppEffect`; вызовы `Main::SessionSettings` — `SessionEffect` в
лениво создаваемом `session_settings` (как `sessionSettingsStorage`). Их применение —
partial: Core::Settings (пакет 31), Main::SessionSettings/AutoDownload (29/06),
emoji id по старым ключам (`Ui::Emoji::IdFromOldKey`, этап 5).
Глобальные входы оригинала (`IsNightMode`, `cConfigScale`, `UpdaterDisabled`)
переданы полями контекста `night_mode`, `config_scale`, `updater_disabled`.

| Оригинал | Rust | Статус | Тест |
|---|---|---|---|
| `ReadSettingsContext` (все поля, default tileNight=true, DcOptions Production) | `ReadSettingsContext` + `Default` | done | все тесты |
| `ReadSettingsContext::sessionSettings()` | `session_settings()` (None до первого обращения) | done; сам SessionSettings — 29 | `auto_download_creates_session_settings_lazily` |
| `NoTimeLimit` | `no_time_limit` (0, int32 max, int64 max→-1) | done | `cache_settings_validation_and_5611_workaround` |
| enum `dbi*` (90 значений) | `DBI_*` | done | — |
| `ReadSetting`: dbiDcOptionOldOld / dbiDcOptionOld / dbiDcOptionsOld | `construct_add_one` / `construct_from_serialized` | done | `dc_options_blocks` |
| dbiApplicationSettings | `AppEffect::ApplicationSettings` | parse done; `addFromSerialized` → 31 | `session_and_application_settings_pass_through` |
| dbiChatSizeMaxOld, dbiSavedGifsLimitOld, dbiStickersRecentLimitOld, dbiStickersFavedLimitOld, dbiMegagroupSizeMaxOld | поля fallback | done | `legacy_limits_feed_fallback_config` |
| dbiUser, dbiKey, dbiMtpAuthorization | mtp_legacy_* / mtp_authorization | done | `auth_blocks_fill_legacy_context` |
| dbiAutoStart, dbiStartMinimized, dbiSendToMenu, dbiSeenTrayTooltip, dbiLastUpdateCheck, dbiDialogLastPath, dbiRecentStickers | `AppEffect` (cSet*) | parse done; globals → 31/35 | `app_flag_blocks`, `paths_and_window_position`, `emoji_and_sticker_containers` |
| dbiAutoUpdate (+ `UpdateChecker().stop()` при !UpdaterDisabled) | `AutoUpdate`, `StopUpdateChecker` | parse done; updater → 34 | `auto_update_and_scale_use_globals` |
| dbiUseExternalVideoPlayerOld, dbiDefaultAttach | только чтение | done | `unknown_and_truncated_blocks_fail` (формат) |
| dbiCacheSettingsOld, dbiCacheSettings (проверка > maxDataSize) | cache_* поля | done | `cache_settings_validation_and_5611_workaround` |
| dbiPowerSaving | `AppEffect::PowerSaving` | parse done; PowerSaving::Set → 5 | — (тривиально, i32) |
| dbiSoundFlashBounceNotifyOld | SoundNotify, FlashBounceNotify | parse done; apply 31 | `app_flag_blocks` |
| dbiAutoDownloadOld, dbiAutoPlayOld | `SessionEffect::AutoDownloadBytesLimit` | parse done; apply 06/29 | `auto_download_creates_session_settings_lazily` |
| dbiDialogsModeOld, dbiShowingSavedGifsOld, dbiWindowsNotificationsOld, dbiLangOld, dbiCatsAndDogsOld, dbiEmojiTabOld, dbiMutePeerOld, dbiMutedPeersOld, dbiTxtDomainStringOldOld, dbiLangFileOld, dbiLoggedPhoneNumberOld | чтение + legacyRead | done | `hidden_pinned_and_muted_peers` |
| dbiDialogsFiltersOld, dbiSessionSettings, dbiHiddenPinnedMessagesOld | `SessionEffect` | parse done; apply 29 | `hidden_pinned_and_muted_peers`, `session_and_application_settings_pass_through` |
| dbiModerateModeOld, dbiIncludeMutedOld, dbiDesktopNotifyOld, dbiNativeNotificationsOld, dbiNotificationsCountOld, dbiNotificationsCornerOld, dbiDialogsWidthRatioOld, dbiLastSeenWarningSeenOld, dbiWorkModeOld, dbiAdaptiveForWideOld, dbiAutoLockOld, dbiReplaceEmojiOld, dbiSuggestEmojiOld, dbiSuggestStickersByEmojiOld, dbiNotifyViewOld, dbiAskDownloadPathOld, dbiCompressPastedImageOld, dbiSongVolumeOld, dbiVideoVolumeOld, dbiPlaybackSpeedOld, dbiSendKeyOld, dbiWindowPositionOld | `AppEffect` | parse done; apply 31 | `app_flag_blocks`, `paths_and_window_position` |
| dbiTxtDomainStringOld | fallback txt domain | done | `legacy_limits_feed_fallback_config` |
| dbiConnectionTypeOldOld, dbiConnectionTypeOld (списки, legacy shift 1024, коррекция индекса, calls) | `ProxyList/Selected/Settings/UseForCalls`, `RefreshGlobalProxy` | parse+логика done; применение к ProxySettings → 31 | `proxy_old_old`, `proxy_lists` |
| dbiThemeKeyOld, dbiThemeKey (SetNightModeValue), dbiBackgroundKey, dbiLangPackKey, dbiLanguagesKey | поля + `NightModeValue` | done; тема → 5/6 | `theme_and_tile_backgrounds` |
| dbiTileBackgroundOld (version<8005, запись в «противоположный» режим), dbiTileBackground | tile_* | done | `theme_and_tile_backgrounds` |
| dbiTryIPv6Old | `ProxyTryIPv6` | parse done; apply 31 | — (i32 flag) |
| dbiScaleOld, dbiScalePercent | `Scale` | parse done; SetScaleChecked → 5 | `auto_update_and_scale_use_globals` |
| dbiDownloadPathOldOld, dbiDownloadPathOld (+'/', FileDialog::Tmp, psDownloadPathEnableAccess) | DownloadPath* | parse done; apply 31 | `paths_and_window_position` |
| dbiRecentEmojiOldOldOld (флаги 10 стран), dbiRecentEmojiOldOld, dbiRecentEmojiOld, dbiEmojiVariantsOldOld, dbiEmojiVariantsOld | Legacy* effects | parse done; IdFromOldKey/ColorIndexFromOldKey → 5 | `emoji_and_sticker_containers` |
| dbiCallSettingsOld (внутренний поток необязателен) | Call* | parse done; apply 31 | `call_settings_inner_stream_is_optional` |
| dbiFallbackProductionConfig | fallback_config | done | `legacy_limits_feed_fallback_config` |
| default: неизвестный blockId → false | `_ => false` | done | `unknown_and_truncated_blocks_fail` |
| `ApplyReadFallbackConfig` | `apply_read_fallback_config` → `FallbackConfigEffect` + DcOptions для `add_from_other` | logic done; Core::App fallback config → 30 | `legacy_limits_feed_fallback_config` |

Связанный вызов: `Storage::Account::readMtpData` теперь читает все блоки через
`read_setting` (раньше неавторизационные блоки давали ошибку). Остаток
`applyReadContext` (cache limits, tile → Background, sessionSettings, writeMtpData
при сбое) — партия storage_account/29.

## Расхождения и непроверенное

- QString читается как сырой UTF-16 (`Vec<u16>`) там, где оригинал хранит QString;
  в поля `ProxyData` и `DcOptions::Endpoint::ip` (String в порте) непарные суррогаты
  попадают как U+FFFD — как `toStdString` для ip; для proxy host/user/password
  оригинал сохранил бы их как есть.
- Нативная сверка не выполнялась: настоящий `ReadSetting` требует `Core::App()`
  и UI-глобалов, без подмены тел его не собрать. Статус — own tests + чтение.
  Формат Qt-контейнеров (QVector/QMap/QPair, Qt_5_1) взят из Qt 6 исходников:
  null-QString → пустая строка, нечётная длина → ошибка, QMap — последний ключ побеждает.
- При ошибке потока C++ продолжает цикл с нулевыми значениями до проверки статуса;
  порт прерывается на первой ошибке. Результат (false, без изменений состояния) тот же.
