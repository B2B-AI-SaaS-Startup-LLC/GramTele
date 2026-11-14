# storage/localstorage → Rust

Оригинал `Telegram/SourceFiles/storage/localstorage.{h,cpp}` (AyuGram 0f2a31bb).
Реализация `crates/tg-core/src/storage/localstorage.rs` (`Local` — файловые
статические переменные оригинала), тесты `crates/tg-core/tests/localstorage.rs`.
Значения из Core::App/cSet*/темы для записи передаются `SettingsSnapshot`;
вызовы в темы, фон, язык, UpdateChecker и Core::Settings возвращаются `LocalEffect`.

| Оригинал | Rust | Статус | Тест |
|---|---|---|---|
| file-scope `_basePath, _settingsSalt, OldKey, SettingsKey, _theme*/_lang*/_background* keys, _useGlobalBackgroundKeys, _oldSettingsVersion, _settingsRewriteNeeded, _settingsWriteAllowed` | поля `Local` | done | все |
| `CheckStreamStatus` | `Option` reader | done | — |
| `applyReadContext` | `apply_read_context` (+ `fallback_dc_options` для addFromOther) | done; Core::App fallback config → 30 | `legacy_files`, `settings_round_trip` |
| `_readOldSettings` (tdata/config, dbiVersion > AppVersion — стоп) | `read_old_settings` | done | `legacy_files` |
| `_readOldUserSettingsFields` (dbiEncryptedWithSalt, OldKey из соли, рекурсия) | `read_old_fields` | done | `legacy_files` |
| `_readOldUserSettings` (`<cDataFile>_config`) / `readOldUserSettings` | `read_old_user_settings` | done | `legacy_files` |
| `_readOldMtpDataFields` (dbiEncrypted, без OldKey — пропуск) | `read_old_fields` | done | `legacy_mtp_without_key_and_newer_version` |
| `_readOldMtpData` / `readOldMtpData` | `read_old_mtp_data` | done | то же |
| `start` | `start` → `LocalEffect` (ApplyDefaultNightMode/InitialLoadTheme/BackgroundTile/ReadLangPack) | done; `_localLoader` TaskQueue → partial (CountWaveformTask) | `settings_round_trip`, `bad_salt_rewrites_settings` |
| `writeSettings` (ранняя заглушка, генерация соли, порядок блоков, null QString пути) | `write_settings` | done; `AyuSettings::save()` — вызывающий (ayu) | `settings_round_trip`, `placeholder_until_writes_are_allowed` |
| `rewriteSettingsIfNeeded` | `rewrite_settings_if_needed` | done | `settings_round_trip` |
| `LookupFallbackConfig` | `SettingsSnapshot::fallback_config` | partial → 29/30 (Domain/Account) | — |
| `sync`, `finish` | `storage_file_utilities::{sync,finish}` (уже были) | done; `_localLoader` → partial | — |
| `AutoupdatePrefix`, `autoupdatePrefixFile`, `readAutoupdatePrefixRaw`, `writeAutoupdatePrefix` (Ayu: аргумент игнорируется), `readAutoupdatePrefix` | `*_autoupdate_prefix*` | done; UpdateChecker → 34 | `autoupdate_prefix_and_manifest` |
| `updateManifestFile`, `writeUpdateManifest` (QSaveFile), `readUpdateManifest` | `*_update_manifest` | done | `autoupdate_prefix_and_manifest` |
| `reset` | `reset` → StopLocalLoader/ResetBackground/ResetSettingsOnLastLogout + запись | done; эффекты → 5/31 | `reset_writes_with_effects` |
| `oldSettingsVersion` | `old_settings_version` | done | `settings_round_trip` |
| `writeBackground`, `readBackground`, `moveLegacyBackground` | — | partial → 16 (WallPaper) / 5 (QImage) | — |
| `countVoiceWaveform`, `CountWaveformTask`, `cancelTask` | — | partial → 9 (media audio) | — |
| `readThemeUsingKey`, `InitialLoadThemeUsingKey`, `writeTheme`, `clearTheme`, `InitialLoadTheme`, `ApplyDefaultNightMode`, `readThemeAfterSwitch`, `ReadThemeContent` | — | partial → 5 (Window::Theme) | — |
| `readLangPack`, `writeLangPack`, `saveRecentLanguages`, `pushRecentLanguage`, `removeRecentLanguage`, `readRecentLanguages` | — | partial → lang 01 (Claude) | — |
| `incrementRecentHashtag` | — | partial → 12/7 (RecentHashtagPack) | — |

## Расхождения и непроверенное

- QSaveFile заменён записью во временный `<имя>.tmp` и rename; имя временного файла другое.
- Нативной сверки нет: start/writeSettings тянут Core::App, тему и язык; формат
  файла собран из уже сверенных в обе стороны FileWriteDescriptor/EncryptedDescriptor
  и ReadSetting. Статус — own tests + чтение.
- `AyuSettings::save()` оригинал вызывает внутри writeSettings перед записью; в порте
  это обязанность вызывающего (результаты Placeholder/Written).
