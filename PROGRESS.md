# Прогресс

## Этап 0

- Корневой workspace (`crates/*`, resolver 2, общие edition и license, мягкие lints).
- Пустые крейты-библиотеки по плану (23 штуки), в `lib.rs` — doc-комментарий с каталогом оригинала.
- Окружение: `shell.nix` и `flake.nix` (devShell) с cargo, rustc, clippy, rustfmt, gcc, pkg-config.
- `.gitignore`, `LICENSE` (GPLv3 с исключением для OpenSSL, перенесено из оригинала как есть).
- CI: `.github/workflows/ci.yml` (fmt, clippy, test).
- Реестр `ledger/files.csv`: 3892 файла со статусом `todo`, 1 356 737 строк, 965 `vendored`, `UNMAPPED` нет.

## Этап 1

### tg-runtime (lib_crl)

Переведён целиком, 56 тестов. Проверки `fmt`, `clippy`, `test` пройдены в копии workspace; в основном дереве их нужно повторить после появления `tg-rpl`.

- `replaced` (22 файла): платформенные реализации `dispatch`, `tmc`, `winapi`, `mac`, `qt/crl_qt_async.h`, `crl_async.h`, `crl_common_config.h` — их роль выполняют tokio (blocking-пул) и `std::time::Instant`.
- `partial`:
  - `common/crl_common_object_async.h` — нет `producer` / `producer_on_main`, ждут `tg-rpl`; `with` всегда даёт `&mut T`.
  - `crl_object_on_thread.h/.cpp` — поток завершается после уничтожения объекта, в оригинале он остаётся навсегда.
  - `crl_on_main.h` — `on_main_update_requests` сделан подпиской на колбэки, обёртка в `rpl::producer` отложена.
- Отличия от оригинала описаны в `crates/tg-runtime/README.md`.
