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

### tg-base: контейнеры и утилиты (часть lib_base)

Ветка `port/tg-base-containers` влита в `main`. 84 теста и 1 doctest, перенесены все 14 секций тестов оригинала. Таблица соответствий — `crates/tg-base/docs/containers-and-utils.md`.

- `replaced`: `ordered_set`, `const_string`, `optional`, `variant`, `expected`, `overload`, `match_method`, `functors`, `type_traits`, `unique_any`, `unique_function`, `virtual_method`, `required`, `custom_delete`, `atomic`, `build_config` — их роль выполняют язык и стандартная библиотека.
- Ошибки оригинала, перенесённые как есть и закреплённые тестами: `flat_set::modify` сохраняет порядок только при сдвиге на одну позицию; `LogWriteDebug` печатает номер строки из `debug_log.cpp`, а не из места вызова.
- Долг: в `tg-base` живость объекта выражена трейтом `Alive`, в `tg-runtime` — трейтом `AsGuard`. При портировании остатка `lib_base` их нужно связать: `tg-base` зависит от `tg-runtime` и реализует `AsGuard` для `WeakPtr` и `BinaryGuard`.
- Долг: `profile_log!` считает время сам, должен перейти на `tg_runtime::profile`.

### tg-rpl (lib_rpl)

Переведён целиком: все операторы и типы, 28 секций тестов оригинала и 50 своих тестов (реентерабельность, порядок уничтожения). Таблица соответствий — `crates/tg-rpl/README.md`.

- `replaced`: `mappers.h` (замыкания), `details/*` (система типов Rust).
- Все типы `!Send` и `!Sync`, как и однопоточный оригинал.
- Ошибка оригинала перенесена как есть и закреплена двумя тестами: при вложенном `fire` с отписавшимся подписчиком в списке `event_stream` может потерять живого подписчика или продублировать нового.
- Остальные отличия перечислены в README крейта.

### tg-tl, tg-tl-gen, tg-scheme (продолжение после Claude Code)

- Сохранён и доведён до проверок незакоммиченный перенос Claude: runtime TL,
  парсер/генератор Rust и схема Telegram API + MTProto layer 229.
- Четыре файла `lib_tl/tl/tl_basic_types.{h,cpp}`, `tl_boxed.h`,
  `tl_type_owner.h` отражены в реестре. `type_owner` заменён `Arc`,
  boxed/bare выражены типами и трейтами, conditional — `Option`.
- `lib_tl/tl/generate_tl.py` и `SourceFiles/codegen/scheme/codegen_scheme.py`
  не входили в исходный CSV (реестр собирался только для C++/заголовков).
  Их read/write-часть перенесена в `tg-tl-gen` и
  `tg-scheme/codegen_scheme.rs`; backend конвертации внешнего API не перенесён,
  поскольку вызов генератора AyuGram его не использует.
- `api.tl` и `mtproto.tl` сверены через `cmp` с исходной точкой без различий.
  Патчи AyuGram для noforwards/restricted сохранены.
- Исправлена зависимость build.rs от `codegen_scheme.rs`: правки опций теперь
  вызывают повторную генерацию. Исправлено замечание Clippy в текстовом дампере.
- Добавлены проверки текстового дампа и чтения/повторной записи каждого
  конструктора и метода по образцам из метаданных с отсутствующими и
  заполненными условными полями. Метаданные используют тот же парсер:
  независимыми эталонами остаются 37 тестов `reference_bytes.rs`.
- Карта имён и отличия: `crates/tg-scheme/README.md`. `count_length` использует
  временный буфер. Default выбирает первый конструктор вместо неинициализированного
  значения C++. Ошибки чтения возвращают Result.
- Отложено до этапа 2: реализация `DumpCore` для примитивов, векторов, gzip
  и обёрток слоя из `mtproto/details/mtproto_dump_to_text.cpp`; сейчас проверен
  только машинный обход схемы с тестовой реализацией целых чисел.
- Крипто/таймеры `tg-base` и генераторы стилей/языков `tg-codegen`, над которыми
  Claude также работал перед лимитом, в основном дереве ещё не реализованы.
- В `Reader::get_bytes` проверка длины перенесена перед выделением памяти:
  запрос за границами буфера, включая `usize::MAX`, возвращает UnexpectedEnd
  без паники из Vec::with_capacity и без продвижения курсора; добавлен тест.
- Полный `cargo test --workspace` через Nix: 278 тестов, 0 ошибок.
  Общая проверка охватывает 2 532 определения схемы (конструкторы + методы)
  в двух состояниях условных полей; 37 байтовых эталонов и 5 тестов дампа
  проходят. `cargo fmt --check` также проходит.
- Финальный `cargo clippy --workspace --all-targets -- -D warnings` после
  исправления чтения буфера: exit 0. Все обязательные проверки пройдены.

### tg-base: криптография, байты, ZIP, время и служебные функции

Ветка `port/tg-base-crypto` синхронизирована с main (включая TL layer 229)
и влита в main. Коммит реализации — `e9a2b74`.
Работа велась только в `ayugram-rs-wt/base-crypto`; соседние worktree
base-runtime/codegen не изменялись. Cargo.lock не добавлялся.

- Перечитаны исходники и незавершённые заготовки bytes, crc32hash, file_lock,
  base_file_utilities; остальные заготовки также проверены и доведены до тестов.
- Добавлены openssl_help, zlib_help, random, unixtime, parse_helper,
  qthelp_url, qthelp_regex. Подключены отдельными модулями, в алфавитном порядке
  вместе с модулями main; в tg-base стоит forbid(unsafe_code).
- Единственный bigint — crypto-bigint 0.7.11 (RustCrypto). Montgomery pow
  для нечётных модулей и fixed-precision + CtSelect для чётных не ветвятся
  по битам показателя. Это проверка реализации чтением, не timing-аудит.
  BigNum очищается при Drop. SHA/HMAC/PBKDF2 — RustCrypto; flate2 использует
  только rust_backend/miniz_oxide, без native zlib.
- Исправлены пропущенный memmove, округление отрицательных дат Qt (усечение
  к нулю вместо floor), уведомления unixtime через очередь главного потока,
  обработка ZIP64 при чтении. Windows filename helpers реализованы, Windows
  lock/Restart Manager — через safe обёртки file-guard/restart-manager.
- Исправлены тестовые гонки глобального времени и зависание теста FileLock
  на zombie-процессе: ребёнок reap-ится параллельно. Алгоритм оригинального
  KillProcess сохранён, включая SIGTERM/SIGKILL и ожидание ESRCH. Сохранён
  исходный отказ для POSIX descriptor 0.
- Эталоны: SHA-1/256/512 FIPS, HMAC RFC 2202/4231, PBKDF2 RFC 6070,
  Wycheproof SHA512 (65 байт), AES FIPS 197/SP 800-38A, OpenSSL IGE vector 1,
  CRC32, signed/modular BigNum, малый raw RSA. ZIP/zlib-образцы созданы
  reference zlib/minizip 1.3.2 и CPython, хранятся в tests/fixtures.
  Stored ZIP writer побайтно совпал с minizip; для deflate этого не обещаем.
- В реестре обновлены 20 своих файлов: 13 done и 7 partial.
  Partial: zlib_help (ZIP64 writer/сжатые байты и text heuristic), random
  (.h/.cpp: RAND_seed отсутствует у OS RNG, вход явно отбрасывается),
  qthelp_regex (не весь PCRE, offsets UTF-8, поведение $), base_file_utilities
  (.h/.cpp: Qt RegisterBundledResources отложен), file_lock_win
  (реализован под cfg, но не собран и не запущен на Windows).
- Три обязательные команды через Nix после интеграции main: fmt --check —
  exit 0; clippy --workspace --all-targets -- -D warnings — exit 0;
  test --workspace — exit 0, всего 349 тестов, 0 ошибок.
- Сверены места вызова в mtproto/storage/core/passport/calls и lib_storage:
  BE большие числа, левое дополнение до 256 байт для DH/SRP/RSA,
  256-байтовые local PBKDF2 keys, AES IGE/CTR/CBC, HTTP Date.
  Это проверено чтением, не запуском клиента или открытием tdata.
  Реальные tdata и аккаунты пользователя не использовались.
- Этап 2 не начинался. В SourceFiles/mtproto остаются AES/RSA-обёртки,
  DH/SRP проверки, PEM/OAEP, X25519, fake TLS, derivation/framing и HTTP Date;
  в storage/passport/core — прикладные derivation/шифрование/подписи.

Полный отчёт по файлам, таблица C++ → Rust с примерами клиента, версии
крейтов, эталоны, вывод команд и отличия:
`crates/tg-base/docs/crypto-bytes-time.md`.

### tg-base: таймеры, служебные части и аудит Linux/Qt (2026-11-12)

- Работа выполнена в `base-runtime`, ветка `port/tg-base-runtime`.
  Унаследованные файлы сохранены как есть коммитом `7ea2f4e`
  (`wip: tg-base timers (unfinished)`), затем main влит коммитом `80b45d0`.
  Реализация и документация — `0f057d2`; ветка влита в main.
- Пофайловое решение для **118 файлов**, включая каждый общий platform/,
  platform/linux и 7 XML: [timers-and-process.md](crates/tg-base/docs/timers-and-process.md).
  Из 111 файлов, присутствующих в ledger: 23 done, 24 partial, 3 replaced,
  61 todo (явно отложены с этапом и зависимостями). XML отсутствуют в исходном
  code ledger и перечислены в документе. stage=1 в ledger сохраняет исходную
  принадлежность lib_base; этап возврата указан в документе.
- Полностью: timer, concurrent_timer, timer_rpl, call_delayed, invoke_queued,
  options, debug_destroy_informer, platform-independent system_unlock и
  настоящий Linux fallback system_unlock/layout_switch/network Create=None.
  base_pch и tests_main заменены модулями Cargo и libtest.
- Унаследованный код перечитан и проверен: исправлены оставленные вне lib.rs
  модули, цикл владения BatterySaving/helper callback, семантика Adjust и
  remainingTime, сброс HTTP времени при CheckLocalTime. Таймеры проверяются
  виртуальными Clock/Alarm без sleep, включая реальную очередь tg-runtime.
- Долги Alive/AsGuard и profile_log закрыты: Alive — alias GuardToken из
  tg-runtime, WeakPtr/HasWeakPtr/BinaryGuard реализуют AsGuard; BinaryGuardToken
  не убивает пару при drop клона; profile_log использует tg_runtime::profile.
  tg-runtime и tg-rpl не изменялись.
- Частично: single_instance (Linux FileLock/Unix socket, wire и имя Qt,
  readiness polling; активация окна/автоматическая доставка — этап 5/6),
  integration (logging singleton/executable info), crash_report_header
  (owned formatter, не signal handler), generic battery/network/power-save/
  last-input/screen-reader, custom_app_icon (QImage overload ждёт этапа 5),
  platform file utilities (fs operations, не desktop opener).
- Отложены: event_filter, qt_connection, qt_signal_producer, object_ptr,
  unique_qptr, weak_qptr и qt/ до оконного/текстового слоя (4/5);
  global_shortcuts/generic и Linux desktop backend-и до 6, MPRIS/player до 7.
  D-Bus, порталы, GLib, XCB, AT-SPI требуют реальной сессии рабочего стола.
  CrashReportWriter — этап 6: безопасный high-level signal-hook не разрешает
  fatal SIGSEGV/SIGILL/SIGFPE; low-level API требует unsafe. Варианты:
  отдельный FFI-крейт Crashpad/Breakpad либо внешний crash helper.
  platform/mac и platform/win не трогались.
- Отличия: Fn/FnMut callbacks, явные Registry/handles, coarse timer types
  сохраняются без coalescing; равные deadlines упорядочены по старту;
  PowerSaveBlocker выбирает первый id вместо адреса C++ и ждёт tg-window
  для автообнуления WindowId. Две ошибки ConcurrentTimer C++ сохранены и
  проверены: callEach фактически одноразовый, cancel не отменяет method
  environment из-за порядка reset guard/isActive, но guard блокирует вызов.
  DeleteDirectory сохраняет исходный переход по symlink и последующий failure.
- Проверено тестами: Timer lifecycle/order/adjust, queued guards/coalescing,
  fake system helpers, options Qt JSON sample, atomic import, delayed write,
  lenient int read, IPC двумя процессами/фрагментация/фиксированный wire,
  portable filesystem и crash formatter. Только чтением: соответствия Qt
  JSON/timer/local socket и границы отложенных интерфейсов. Нативный Qt-клиент,
  перевод системных часов/suspend, D-Bus/desktop backend и crash signals
  запуском в этой среде не проверены. Образец JSON выписан по QJsonWriter 6.8,
  не снят с запущенного Qt-клиента; не объявляется проверкой всех Unicode cases.
- Зависимости: добавлен serde_json="1" (при проверке 1.0.154); используется
  существующий tg-runtime=0.0.0 и tg-rpl=0.0.0. zbus, GLib и signal-hook
  не добавлены. В tg-runtime отсутствует deadline/alarm/reactor API — Clock/Alarm
  реализованы в tg-base; глобальные часы не инъецируются, тесты используют
  отдельный Clock и injected adjust decision. В tg-rpl нет QObject bridge и
  Send producers; rpl остаётся на главном потоке, background timer использует runner.
- Три команды через Nix: `cargo fmt --check` — exit 0, пустой вывод;
  `cargo clippy --workspace --all-targets -- -D warnings` — exit 0,
  Finished dev profile in 29.68s (включая build lock), без предупреждений;
  `cargo test --workspace` — exit 0, **424 теста, 0 failed, 0 ignored**.
  tg-base: **208 unit + 22 integration + 1 doctest = 231**, прирост к main
  до этой работы — **75** (66 unit + 9 integration). Повторный запуск helper
  в дочернем процессе не посчитан дважды. Полные логи:
  `/tmp/base-runtime-verified-{fmt,clippy,tests}.log`.
- TL: таблица tg-scheme README расширена до **27 строк** с SourceFiles/путями
  и реальными клиентскими выражениями. Чистая сборка в разных пустых target-dir
  (Nix, rustc/cargo 1.95.0, x86_64 Linux, warm download cache, без параллельной
  компиляции): debug **82.927 с**, release **194.744 с**, обе exit 0.
  Команды и CPU times записаны в README. MTProto dispatcher остаётся этапом 2.
- Cargo.lock не добавлен, push не выполнялся. Этап 2 не начат.
