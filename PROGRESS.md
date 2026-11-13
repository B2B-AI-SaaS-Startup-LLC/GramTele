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

### Закрепление toolchain и tg-codegen (2026-11-12)

- Отдельный коммит main **15fdf80**, `build: pin Rust toolchain`: Rust **1.95.0**,
  nixpkgs **c56a87d8e41ce69bd2af57eb35dcc117a9dfca33**, narHash в flake.lock.
  shell.nix читает этот же lock, а не канал; CI использует dtolnay/rust-toolchain@1.95.0.
  Проверены nix-shell и nix develop: rustc 1.95.0 (59807616e), cargo 1.95.0.
  В PORTING.md записана одна команда всех трёх проверок. Замечания clippy 1.99.1
  не подавлялись: выбран проверенный 1.95.0.
- Worktree codegen сначала fast-forward с 58c9118 до main после pin. Работа —
  **6b3fd8f** (`port: style, lang and emoji resource generators`), bytes policy
  fixtures — **8d419e7**; обе влиты в main (**53e8a67**, **add44da**).
  [README](crates/tg-codegen/README.md) содержит решение по каждому из **51 файла**:
  **39 done, 4 replaced, 6 partial, 2 todo**. Все partial/todo — графические
  части до этапа 5; ниже они также включены в общий пофайловый список.
- Библиотека input → Rust text + три тонких CLI, параметры оригинала. Порядок
  includes/structs/fields/variables, inheritance status, вычисления, ключи,
  теги, plural slots, packed UTF-16, исторические aliases и CRC сохранены.
  Состояние lang .tags/.indices — явный input/output, удалённые slots не теряются.
  --subsets-only заменён отсутствием C++ scan/headers с пояснением; Rust Phrase
  не требует C++ subset specialization cache.
- Настоящие входы: все 76 .style + colors.palette, 9193 строки lang.strings,
  lib_ui/emoji.txt и lib_ui/emoji_suggestions/emoji_autocomplete.json. Включены
  три Ayu style files из cmake/td_ui.cmake и AyuGram language block. Сохранён
  getWideMultiplied (factor 1.0/0.6/0.75), ConvertScale и contrast pairs.
  Неподвижные тестовые снимки ресурсов с SHA256 и scale variants хранятся в
  tests/fixtures/Telegram; .gitattributes запрещает их newline conversion.
- Новая direct dependency: tg-base=0.0.0 (готовый CRC32); dev dependency:
  serde_json="1" (при проверке 1.0.154, эталоны). Qt-compatible JSON reader
  сохраняет UTF-16, lone surrogates, порядок QString, unknown escapes и глубину
  1024; безопасная рекурсия работает в parser thread со stack 16 МиБ.
  Новые SVG/PNG/WebP крейты не выбирались. tg-ui/tg-core/tg-runtime/tg-rpl не менялись;
  compile contracts находятся только в тестах tg-codegen.
- **Оригинал собран и запущен**. Корневой CMake не нашёл cmake/version.cmake и
  validate_special_target.cmake. Standalone CMake вне клона затем собрал именно
  codegen_style, codegen_lang и codegen_emoji из неизменённых .cpp/.h (Nix,
  GCC 15.3.0, C++23, Qt 6.11.2). Prefix header — только includes/typedefs,
  не замены алгоритмов. Запущены palette, lang, emoji и все 76 style inputs.
  Полная сборка Telegram не запускалась. Клон остался чистым.
- **Проверено эталоном оригинала**: все 588 palette colors/names/indices/fallback,
  checksum **-1755758547** и точная CRC input string; все 10997 lang slots,
  175 tags, state files, **249625 UTF-16 units / 10998 offsets**; структуры,
  поля, значения, copies/inheritance каждого из 76 стилей; **3736 emoji**, flags,
  variation parents, sections, **3767 map entries** с old aliases, postfixRequired
  и emoticons; **2018 suggestions**, UTF-16 payload/lengths/init/char indices/CRC.
  Отдельно сверены выбранные JSON diagnostics, unknown escapes, lone-surrogate
  преобразование и native предел вложенности 1024/1025. Данные native запусков
  и SHA256 C++ outputs сохранены в fixtures/reference, capture tools — в тестах.
- **Своими тестами**: малые валидные/ошибочные образцы, filename/line/code/messages,
  UTF-16, exact checksum string, slots/holes, исходный strict plural-at-EOF дефект,
  CLI errors и два независимых запуска CLI с побайтовым сравнением, Ayu hooks;
  вся Rust-выдача настоящих входов компилируется с test-only contracts.
- **Только чтением / отложено**: lang Instance serialize/cloud uses имена ключей
  (QByteArray key/value), не ordinal indices; реальные tdata/lang runtime ещё
  не запускались. PNG composition/transforms, QSvgRenderer validation и отдельный
  --render-svg, emoji QPainter/font/WebP sprites — этап 5. SVG crate не задан
  PLAN.md: варианты usvg/resvg + tiny-skia или отдельный backend, выбор не сделан.
  --render-svg/непустой --images возвращают явную error 910, не успешную заглушку.
  PNG 1x/2x/3x и SVG payload сохранены без изменения с modifiers в IconSource;
  pixel parity не заявлена. Готовые emoji_*.webp не пересоздавались.
- Отличия выдачи: Rust getter возвращает значение вместо C++ reference и читает
  scale при вызове вместо C++ init-фазы; будущий UI фиксирует scale/palette на
  одну generation. Деревья switch заменены таблицами; lookup/plural/reactive
  функции — контракт будущих tg-ui/tg-core, перечислен в README. Ошибки —
  fail-fast Result вместо ostream chains; исходный silent rejection имеет code=0,
  CLI сохраняет отсутствие сообщения и failure exit. Форматы входов не менялись.
- Три проверки на закреплённом Rust через команду PORTING.md: **fmt --check exit 0**,
  **clippy --workspace --all-targets -- -D warnings exit 0**, **test --workspace exit 0**;
  **457 passed, 0 failed, 0 ignored**, без двойного подсчёта дочернего helper.
  tg-codegen: **33 теста** (24 format/edge, 5 native reference, 3 compile-real,
  1 CLI). Лог /tmp/codegen-verified-checks.log, fmt отдельно /tmp/codegen-verified-fmt.log.
- Последовательный замер без других сборок на Rust 1.95.0: styles+palette
  **8.711 с** генерация / **44.706 с** rustc output compilation; lang **1.395 / 9.122 с**;
  emoji **0.293 / 2.682 с**. Определения/команда/ограничения замера — в README,
  лог /tmp/codegen-benchmark-final.log.
- Cargo.lock не добавлен, push не выполнялся. Этап 2 не начат.

## Итог этапа 1

Критерий PLAN.md: «портированные тесты lib_rpl и lib_base проходят;
сериализация каждого типа TL-схемы совпадает с оригиналом на наборе образцов».

| Пункт критерия / состава этапа 1 | Выполнен? | Подтверждение и граница |
|---|---|---|
| Портированные тесты lib_rpl | Да | Все 28 секций оригинала и собственные tests/doctests проходят в workspace. |
| Портированные тесты lib_base | Да, для перенесённой части | Все 14 исходных секций + tests переносимых модулей; tg-base 231 проверка, 0 ошибок. Графические/платформенные файлы ниже остаются partial/todo. |
| TL: независимые образцы | Да, для набора покрытых форм | 37 reference_bytes tests, boundaries, flags/flags2, вложенные constructors, bytes/vector, Ayu hooks. |
| TL: каждый тип против эталонного C++ исполнения | **Да, на конечном наборе образцов** | Оригинальные Python generators + lib_tl собраны отдельно; 1697 constructors + 823 methods × 6 режимов = 15 120 образцов, плюс 3 образца встроенного vector. Rust write/read/rewrite совпали побайтно. Recipe не зависит от tg-tl-gen; commit 9f6ead6, 2521 определение с builtin vector; fixtures и воспроизводимый CMake в tg-scheme/tests. |
| tg-runtime | Переносимая часть да; есть partial | 56 tests и workspace checks; остаются reactive bridges и lifetime потока, см. список. |
| tg-codegen common/style/lang/emoji | Переносимая обработка и Rust-выдача да; графика отложена | Все настоящие входы разобраны, вся выдача компилируется; native эталоны таблиц/значений совпали, 33 теста. Image backend/sprites — этап 5. |
| Обязательные проверки на едином Rust | Да | Rust 1.95.0 закреплён для nix-shell/nix develop/CI; последний полный прогон после native TL и MTProto: все три команды exit 0, 515 тестов, 2 live ignore. |

**Критерий готовности этапа 1 выполнен:** портированные тесты lib_rpl/lib_base
проходят; каждое определение TL проверено исполнением оригинального C++ writer
на описанном конечном наборе значений. Отложенные платформенные и графические
части остаются в реестре для соответствующих этапов. Ниже полный список
всех файлов с stage=1 и status=partial/todo
после этой задачи: **174 файла**. Номер возврата — планируемый этап работы,
столбец stage в ledger сохраняет исходную принадлежность файлу этапа 1.

- `codegen/codegen/emoji/generator.cpp` → этап **5** → Все таблицы Rust сверены с native C++ arrays; writeImages/generateImage/QPainter/WebP sprites отложены, --images error 910.
- `codegen/codegen/emoji/generator.h` → этап **5** → Все таблицы Rust сверены с native C++ arrays; writeImages/generateImage/QPainter/WebP sprites отложены, --images error 910.
- `codegen/codegen/style/generator.cpp` → этап **5** → Rust structs/values/palette, scale и Ayu wide policy; PNG composition/transforms и SVG validation требуют графического backend.
- `codegen/codegen/style/generator.h` → этап **5** → Rust structs/values/palette, scale и Ayu wide policy; PNG composition/transforms и SVG validation требуют графического backend.
- `codegen/codegen/style/parsed_file.cpp` → этап **5** → Обработка всех реальных стилей и модификаторов проверена native IR; QImage GetModifier pixel transforms переданы будущему IconSource backend.
- `codegen/codegen/style/parsed_file.h` → этап **5** → Обработка всех реальных стилей и модификаторов проверена native IR; QImage GetModifier pixel transforms переданы будущему IconSource backend.
- `codegen/codegen/style/render_svg.cpp` → этап **5** → Отложен: SVG parser/backend ещё не выбран; tiny-skia — только rasterizer. --render-svg явно возвращает error 910.
- `codegen/codegen/style/render_svg.h` → этап **5** → Отложен: SVG parser/backend ещё не выбран; tiny-skia — только rasterizer. --render-svg явно возвращает error 910.
- `lib_base/base/base_file_utilities.cpp` → этап **5** → RegisterBundledResources: ресурсы/Qt .rcc заменяются ресурсной системой tg-window/tg-gfx.
- `lib_base/base/base_file_utilities.h` → этап **5** → RegisterBundledResources: ресурсы/Qt .rcc заменяются ресурсной системой tg-window/tg-gfx.
- `lib_base/base/battery_saving.cpp` → этап **6** → Generic helper, Option<bool> и поток значений проверены; Linux GPowerProfileMonitor/GLib требует этап 6. Фабрика backend явная.
- `lib_base/base/battery_saving.h` → этап **6** → Generic helper, Option<bool> и поток значений проверены; Linux GPowerProfileMonitor/GLib требует этап 6. Фабрика backend явная.
- `lib_base/base/crash_report_header.cpp` → этап **6** → Проверен owned-форматтер 64 КиБ; глобальный буфер и async-signal-safe публикация требуют аварийной подсистемы.
- `lib_base/base/crash_report_header.h` → этап **6** → Проверен owned-форматтер 64 КиБ; глобальный буфер и async-signal-safe публикация требуют аварийной подсистемы.
- `lib_base/base/crash_report_writer.cpp` → этап **6** → Отложен: SIGSEGV/backtrace/ucontext, аварийный fd, аннотации, внешняя crash-подсистема; unsafe запрещён.
- `lib_base/base/crash_report_writer.h` → этап **6** → Отложен: SIGSEGV/backtrace/ucontext, аварийный fd, аннотации, внешняя crash-подсистема; unsafe запрещён.
- `lib_base/base/custom_app_icon.h` → этап **5** → Linux path/digest/clear None/false проверены; overload QImage ждёт tg-gfx этапа 5, mac — этап 10.
- `lib_base/base/event_filter.cpp` → этап **5** → Отложен: Доставка/отмена событий и parent lifetime: tg-window dispatcher; используется виджетами и LastUserInput.
- `lib_base/base/event_filter.h` → этап **5** → Отложен: Доставка/отмена событий и parent lifetime: tg-window dispatcher; используется виджетами и LastUserInput.
- `lib_base/base/file_lock_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/global_shortcuts.h` → этап **6** → Отложен: native key descriptors/names, запись сочетаний и источник системных событий; клиент calls, запись PTT; tg-window этап 5, X11 backend этап 6.
- `lib_base/base/global_shortcuts_generic.cpp` → этап **6** → Отложен: native key descriptors/names, запись сочетаний и источник системных событий; клиент calls, запись PTT; tg-window этап 5, X11 backend этап 6.
- `lib_base/base/global_shortcuts_generic.h` → этап **6** → Отложен: native key descriptors/names, запись сочетаний и источник системных событий; клиент calls, запись PTT; tg-window этап 5, X11 backend этап 6.
- `lib_base/base/integration.cpp` → этап **5** → Logging singleton и ExecutableInfo/std::env проверены; enterFromEventLoop и crash annotations требуют launcher/tg-window (5/6).
- `lib_base/base/integration.h` → этап **5** → Logging singleton и ExecutableInfo/std::env проверены; enterFromEventLoop и crash annotations требуют launcher/tg-window (5/6).
- `lib_base/base/last_user_input.cpp` → этап **5** → Приоритет provider и отметка ввода проверены; tg-window event filter (5), XCB/Mutter (6) отложены.
- `lib_base/base/last_user_input.h` → этап **5** → Приоритет provider и отметка ввода проверены; tg-window event filter (5), XCB/Mutter (6) отложены.
- `lib_base/base/network_reachability.cpp` → этап **6** → Общий приоритет/поток доступности проверен; обязательный источник заменяет отсутствующий QNetworkInformation backend (этап 6).
- `lib_base/base/network_reachability.h` → этап **6** → Общий приоритет/поток доступности проверен; обязательный источник заменяет отсутствующий QNetworkInformation backend (этап 6).
- `lib_base/base/object_ptr.h` → этап **5** → Отложен: Родитель QObject, QPointer обнуление, hide/deleteLater: хранилище виджетов tg-window; Box не сохраняет контракт.
- `lib_base/base/platform/base_platform_custom_app_icon.h` → этап **5** → Linux path/digest/clear проверены; overload QImage ждёт tg-gfx этапа 5.
- `lib_base/base/platform/base_platform_file_utilities.h` → этап **6** → Linux операции fs и current executable проверены; ShowInFolder + ресурсы требуют desktop backend.
- `lib_base/base/platform/base_platform_global_shortcuts.h` → этап **6** → Отложен: глобальные события, native descriptors и их display names.
- `lib_base/base/platform/base_platform_haptic.h` → этап **6** → IsSwipeBackEnabled=true как на Linux; Haptic требует feedbackd D-Bus.
- `lib_base/base/platform/base_platform_info.cpp` → этап **4** → Отложен: Qt Unicode CleanAndSimplify, UTF-16 длина и device model; нужен текстовый adapter для parity.
- `lib_base/base/platform/base_platform_info.h` → этап **6** → Отложен: сведения ОС/сессии/локали, X11/Wayland, версия glibc; client launcher и device model.
- `lib_base/base/platform/base_platform_last_input.h` → этап **6** → Инъекция provider и приоритет проверены; XCB/Mutter implementation отложена.
- `lib_base/base/platform/base_platform_power_save_blocker.h` → этап **6** → Индексы типов, WindowId и backend trait; реальный Inhibit/XCB отложен.
- `lib_base/base/platform/base_platform_process.h` → этап **6** → Отложен: активация окна процесса, native WId и desktop connection.
- `lib_base/base/platform/base_platform_system_media_controls.h` → этап **7** → Отложен: MPRIS и команды медиаплеера, metadata/artwork, tg-gfx; Linux backend этап 6/7.
- `lib_base/base/platform/base_platform_url_scheme.h` → этап **6** → Отложен: desktop registration, Gio AppInfo, Flatpak/Snap и URL launcher.
- `lib_base/base/platform/linux/base_battery_saving_linux.cpp` → этап **6** → GLib/GPowerProfileMonitor, notify callback через enterFromEventLoop; sysfs battery probe.
- `lib_base/base/platform/linux/base_battery_saving_linux.h` → этап **6** → GLib/GPowerProfileMonitor, notify callback через enterFromEventLoop; sysfs battery probe.
- `lib_base/base/platform/linux/base_file_utilities_linux.cpp` → этап **6** → Проверены executable, rename overwrite, flush/sync, recursive delete (включая исходную ошибку symlink), Linux identity/no-op. ShowInFolder D-Bus/portal и BundledResourcesPath отложены.
- `lib_base/base/platform/linux/base_file_utilities_linux.h` → этап **6** → Проверены executable, rename overwrite, flush/sync, recursive delete (включая исходную ошибку symlink), Linux identity/no-op. ShowInFolder D-Bus/portal и BundledResourcesPath отложены.
- `lib_base/base/platform/linux/base_global_shortcuts_linux.cpp` → этап **6** → XCB RECORD, xkbcommon имена клавиш, QSocketNotifier; источник событий tg-window.
- `lib_base/base/platform/linux/base_global_shortcuts_linux.h` → этап **6** → XCB RECORD, xkbcommon имена клавиш, QSocketNotifier; источник событий tg-window.
- `lib_base/base/platform/linux/base_haptic_linux.cpp` → этап **6** → feedbackd org.sigxcpu.Feedback, desktop application id; константа swipe находится в общем модуле.
- `lib_base/base/platform/linux/base_haptic_linux.h` → этап **6** → feedbackd org.sigxcpu.Feedback, desktop application id; константа swipe находится в общем модуле.
- `lib_base/base/platform/linux/base_info_linux.cpp` → этап **6** → DMI/sysfs, os-release/locale/uname, glibc, session backend, desktop environment.
- `lib_base/base/platform/linux/base_info_linux.h` → этап **6** → DMI/sysfs, os-release/locale/uname, glibc, session backend, desktop environment.
- `lib_base/base/platform/linux/base_last_input_linux.cpp` → этап **6** → MIT-SCREEN-SAVER XCB и Mutter IdleMonitor D-Bus, часы/приоритет fallback.
- `lib_base/base/platform/linux/base_last_input_linux.h` → этап **6** → MIT-SCREEN-SAVER XCB и Mutter IdleMonitor D-Bus, часы/приоритет fallback.
- `lib_base/base/platform/linux/base_linux_allocation_tracer.cpp` → этап **6** → Отложен: malloc hooks и адреса выделений, unsafe/FFI; безопасный tg-base не перехватывает allocator.
- `lib_base/base/platform/linux/base_linux_allocation_tracer.h` → этап **6** → Отложен: malloc hooks и адреса выделений, unsafe/FFI; безопасный tg-base не перехватывает allocator.
- `lib_base/base/platform/linux/base_linux_app_launch_context.cpp` → этап **6** → Gio AppLaunchContext, startup id/activation token и env.
- `lib_base/base/platform/linux/base_linux_app_launch_context.h` → этап **6** → Gio AppLaunchContext, startup id/activation token и env.
- `lib_base/base/platform/linux/base_linux_dbus_utilities.cpp` → этап **6** → NameHasOwner/ListActivatableNames/StartServiceByName и ServiceWatcher; допустим zbus, пока не добавлен.
- `lib_base/base/platform/linux/base_linux_dbus_utilities.h` → этап **6** → NameHasOwner/ListActivatableNames/StartServiceByName и ServiceWatcher; допустим zbus, пока не добавлен.
- `lib_base/base/platform/linux/base_linux_library.cpp` → этап **6** → dlopen/dlsym требуют unsafe: вынести FFI или заменить конечные API безопасными крейтами.
- `lib_base/base/platform/linux/base_linux_library.h` → этап **6** → dlopen/dlsym требуют unsafe: вынести FFI или заменить конечные API безопасными крейтами.
- `lib_base/base/platform/linux/base_linux_xcb_library.cpp` → этап **6** → Набор динамических XCB symbols; нужен безопасный X11 backend, tg-window, этап 5/6.
- `lib_base/base/platform/linux/base_linux_xcb_library.h` → этап **6** → Набор динамических XCB symbols; нужен безопасный X11 backend, tg-window, этап 5/6.
- `lib_base/base/platform/linux/base_linux_xcb_utilities.cpp` → этап **6** → Соединение, native event filter, atom/window/property, mask lifetime; tg-window X11 backend.
- `lib_base/base/platform/linux/base_linux_xcb_utilities.h` → этап **6** → Соединение, native event filter, atom/window/property, mask lifetime; tg-window X11 backend.
- `lib_base/base/platform/linux/base_linux_xdg_activation_token.cpp` → этап **6** → Wayland xdg activation и активное окно; tg-window stage 5.
- `lib_base/base/platform/linux/base_linux_xdg_activation_token.h` → этап **6** → Wayland xdg activation и активное окно; tg-window stage 5.
- `lib_base/base/platform/linux/base_linux_xdp_utilities.cpp` → этап **6** → Portal Settings watch, parent window export X11/Wayland; tg-window + D-Bus.
- `lib_base/base/platform/linux/base_linux_xdp_utilities.h` → этап **6** → Portal Settings watch, parent window export X11/Wayland; tg-window + D-Bus.
- `lib_base/base/platform/linux/base_linux_xsettings.cpp` → этап **6** → XSETTINGS property parser/cache, XCB events, QColor; tg-window + tg-gfx.
- `lib_base/base/platform/linux/base_linux_xsettings.h` → этап **6** → XSETTINGS property parser/cache, XCB events, QColor; tg-window + tg-gfx.
- `lib_base/base/platform/linux/base_power_save_blocker_linux.cpp` → этап **6** → Portal Inhibit/Request.Close, parent window export и XCB screensaver reset раз в 10 с.
- `lib_base/base/platform/linux/base_power_save_blocker_linux.h` → этап **6** → Portal Inhibit/Request.Close, parent window export и XCB screensaver reset раз в 10 с.
- `lib_base/base/platform/linux/base_process_linux.cpp` → этап **6** → X11 _NET_ACTIVE_WINDOW/_NET_WM_USER_TIME, Wayland activation; нет window ids.
- `lib_base/base/platform/linux/base_process_linux.h` → этап **6** → X11 _NET_ACTIVE_WINDOW/_NET_WM_USER_TIME, Wayland activation; нет window ids.
- `lib_base/base/platform/linux/base_screen_reader_state_linux.cpp` → этап **6** → AT-SPI/org.a11y.Status, watchers и GLib callback delivery; accessibility stage 5.
- `lib_base/base/platform/linux/base_screen_reader_state_linux.h` → этап **6** → AT-SPI/org.a11y.Status, watchers и GLib callback delivery; accessibility stage 5.
- `lib_base/base/platform/linux/base_system_media_controls_linux.cpp` → этап **7** → MPRIS org.mpris.MediaPlayer2, playback/seek/artwork/URI metadata; player + gfx stage 7.
- `lib_base/base/platform/linux/base_url_scheme_linux.cpp` → этап **6** → Gio DesktopAppInfo, Snap settings, .desktop и sandbox; регистрация tg:// из launcher.
- `lib_base/base/platform/linux/base_url_scheme_linux.h` → этап **6** → Gio DesktopAppInfo, Snap settings, .desktop и sandbox; регистрация tg:// из launcher.
- `lib_base/base/platform/mac/base_battery_saving_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_battery_saving_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_confirm_quit.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_confirm_quit.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_custom_app_icon_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_custom_app_icon_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_file_utilities_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_file_utilities_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_global_shortcuts_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_global_shortcuts_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_haptic_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_haptic_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_info_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_info_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_last_input_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_last_input_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_layout_switch_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_layout_switch_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_network_reachability_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_power_save_blocker_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_power_save_blocker_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_process_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_process_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_screen_reader_state_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_screen_reader_state_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_system_media_controls_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_system_unlock_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_system_unlock_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_url_scheme_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_url_scheme_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_utilities_mac.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/mac/base_utilities_mac.mm` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_battery_saving_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_battery_saving_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_file_utilities_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_file_utilities_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_global_shortcuts_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_global_shortcuts_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_haptic_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_haptic_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_info_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_info_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_last_input_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_last_input_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_layout_switch_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_layout_switch_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_network_reachability_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_power_save_blocker_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_power_save_blocker_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_process_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_process_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_screen_reader_state_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_screen_reader_state_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_system_media_controls_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_system_unlock_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_system_unlock_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_url_scheme_win.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_url_scheme_win.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_co_task_mem.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_gdiplus_h.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_rpcndr_h.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_safe_library.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_safe_library.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_shlobj_h.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_winrt.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_winrt.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_wrl.cpp` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/base_windows_wrl.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/wrl/wrl_implements_h.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/platform/win/wrl/wrl_module_h.h` → этап **10** → Нативный macOS/Windows backend, профильная ОС/toolchain и тесты; файл в этой работе не менялся.
- `lib_base/base/power_save_blocker.cpp` → этап **6** → Группы блокировок, перевыбор окна и backend trait проверены fake backend; portal Inhibit/XCB требует этап 6.
- `lib_base/base/power_save_blocker.h` → этап **6** → Группы блокировок, перевыбор окна и backend trait проверены fake backend; portal Inhibit/XCB требует этап 6.
- `lib_base/base/qt/qt_common_adapters.h` → этап **5** → Отложен: TouchDevice зависит от модели устройств ввода tg-window.
- `lib_base/base/qt/qt_compare.h` → этап **4** → Отложен: Ord/PartialOrd покрывают Vec/enum; порядок QString — UTF-16, не String::cmp. Отложено до миграции текстовых call sites этапа 4/5.
- `lib_base/base/qt/qt_key_modifiers.h` → этап **5** → Отложен: Глобальное состояние клавиатуры tg-window; сравнение равно одному модификатору, а не contains.
- `lib_base/base/qt/qt_string_view.h` → этап **4** → Отложен: StringViewMid принимает отрицательные индексы, UTF-16 и null/empty; нужен текстовый adapter, этап 4/5.
- `lib_base/base/qt/qt_tab_key.cpp` → этап **5** → Отложен: Цепочка фокуса, Tab/Backtab и фильтр событий tg-window.
- `lib_base/base/qt/qt_tab_key.h` → этап **5** → Отложен: Цепочка фокуса, Tab/Backtab и фильтр событий tg-window.
- `lib_base/base/qt_connection.h` → этап **5** → Отложен: QObject connect/disconnect, queued delivery и уничтожение sender/receiver: tg-window.
- `lib_base/base/qt_signal_producer.h` → этап **5** → Отложен: Qt signal-to-rpl bridge с enterFromEventLoop: tg-window + tg-rpl, одного EventStream недостаточно.
- `lib_base/base/qthelp_regex.h` → этап **4** → Полный PCRE контракт против regex, lookaround/backreferences, UTF-16 offsets и поведение $; совместимость клиентских вызовов.
- `lib_base/base/random.cpp` → этап **2** → При интеграции криптографии проверить OS RNG вместо RAND_seed; исходный seed не используется и это описанное отличие.
- `lib_base/base/random.h` → этап **2** → При интеграции криптографии проверить OS RNG вместо RAND_seed; исходный seed не используется и это описанное отличие.
- `lib_base/base/screen_reader_state.cpp` → этап **5** → Generic observer/Variable проверен; QAccessible adapter (5) и Linux AT-SPI D-Bus (6) отложены.
- `lib_base/base/screen_reader_state.h` → этап **5** → Generic observer/Variable проверен; QAccessible adapter (5) и Linux AT-SPI D-Bus (6) отложены.
- `lib_base/base/single_instance.cpp` → этап **6** → Linux UnixStream/FileLock, Qt-имя и обмен проверены; автоматическая readiness-доставка и активация PID/WId требуют tg-window.
- `lib_base/base/single_instance.h` → этап **6** → Linux UnixStream/FileLock, Qt-имя и обмен проверены; автоматическая readiness-доставка и активация PID/WId требуют tg-window.
- `lib_base/base/unique_qptr.h` → этап **5** → Отложен: QPointer плюс владение/очистка QObject: хранилище tg-window.
- `lib_base/base/weak_qptr.h` → этап **5** → Отложен: Автоматическое обнуление и const observer QObject: tg-window IDs/generations.
- `lib_base/base/zlib_help.h` → этап **6** → ZIP64 writer, parity сжатых bytes и minizip text heuristic для тем/языковых архивов.
- `lib_crl/crl/common/crl_common_object_async.h` → этап **4** → producer/producer_on_main bridge с tg-rpl, контракт with(&mut T); tg-runtime сейчас не менялся.
- `lib_crl/crl/crl_object_on_thread.cpp` → этап **4** → Согласовать lifetime фонового потока: в Rust завершается после drop объекта, C++ оставляет его навсегда; runtime tests.
- `lib_crl/crl/crl_object_on_thread.h` → этап **4** → Согласовать lifetime фонового потока: в Rust завершается после drop объекта, C++ оставляет его навсегда; runtime tests.
- `lib_crl/crl/crl_on_main.h` → этап **5** → Обёртка callback subscription on_main_update_requests в rpl::producer и связь с оконным loop.

## Native TL reference: закрытие критерия этапа 1 (9f6ead6)

Оригинальная схема слоя 229 сгенерирована неизменёнными generate_tl.py и
codegen_scheme.py, скомпилирована вместе с lib_tl вне клона в
`/tmp/ayugram-tl-reference`. Проверены все 2520 конкретных определений
(1697 constructors, 823 methods) и встроенный полиморфный vector: 15 123
C++ образца. Расхождений нет: Rust write совпадает с C++, deserialize_exact
и повторная запись сохраняют байты. Шесть режимов покрывают absent/present,
flags2, длины 0/253/254/3/1, empty/nonempty vector, boxed/bare; для `!X`
использован help.getConfig. Это не полный перебор combinations всех флагов
и вложенных alternatives. Полиморфные встроенные числовые/string примитивы
покрыты в составе полей; UTF-8 и семантическая допустимость данных на сервере
не являются предметом этого сравнения.

Описание значений независимо от tg-tl-gen: отдельный raw-TL Python reader,
checked-in JSON и два типизированных builder. Native writer не заменяется
алгоритмом стенда. Binary fixtures, manifest SHA-256, CMake, capture.sh и
границы проверки: `crates/tg-scheme/tests/reference-tools/README.md`.
Удалены codegen worktree и полностью влитая port/tg-codegen; stash
`base-crypto before integrating main` сохранён.

## Этап 2, часть 1: транспорт MTProto (492c058, merge 951e585)

Учёт восстановлен после слияния: по 49 файлам из таблицы
`crates/tg-mtproto/README.md` в ledger отмечено **30 done, 18 partial, 1 replaced**.
Пофайловые решения и границы — в README и `docs/files.json`. PCH заменён Rust
imports. Остальные файлы MTProto этой задачей не портировались.

**Эталон оригинала:** отдельными CMake вне клона собраны lib_tl/generated scheme,
AuthKey/RSA/DC/config и стенды TCP/fake TLS. Совпали keyId, v1/v2 AES schedules
в обоих направлениях, raw RSA/fingerprint, AES-IGE, встроенные DC/options config,
все поля Config с разными значениями/UTF-16/null, proxy records, TCP prefixes и
первые frames 0/1/D, ClientHello/GREASE/extensions/HMAC. Native Qt SOCKS5/Basic
CONNECT сняты на локальном сервере. Полного native proof OAEP, ASN.1, IDNA и
Qt double error/boundary cases нет. Capture scripts и SHA-256 manifest сохранены.

**Своими тестами:** сервер со своим RSA ключом расшифровывает RSA_PAD и завершает
DH, проверяет одинаковый auth key и retry. Отдельные tests проверяют nonce/hash/
stage/length/prime/g_a/fail/retry limit; не все ошибки криптобэкендов достижимы.
Duplex TCP 0/1/D, fake TLS, SOCKS5/CONNECT; виртуальные stagger/cache/IP deadlines;
Dcenter threads; bind decrypt; request identity/padding; DumpCore primitives/vector/
gzip/layers. Переносимые lifecycle wrappers и некоторые CDN/ASN.1 ветви проверены
только чтением, что отдельно указано в README.

**Живой сервер:** test DC 2 149.154.167.40:443, DH по TCP прошёл. HTTP:80
доступен и отвечает `HTTP/1.0  200 OK` с двумя пробелами; reqwest возвращает
Parse(Status), поэтому HTTP live test завершился ошибкой. Ни аккаунта, ни API
запросов, ни production DC. Internet proxy/fake TLS не проверялись.

Rust 1.95.0: cargo fmt --check **0**, cargo clippy --workspace --all-targets --
-D warnings **0**, cargo test --workspace **0**: 515 passed, 0 failed, 2 ignored.
В tg-mtproto — 55 ordinary tests. Ручной live run: **101**, 1 passed/1 failed.
Вывод всех команд: `crates/tg-mtproto/docs/verification/`.

Зависимости tg-mtproto: aes 0.9.4, ctr 0.10.1, ige 0.2.1, reqwest 0.12.28,
tokio 1.54.1, zeroize 1.9.1 и workspace crates. В tg-base добавлены публичные
JSON и gzip helpers поверх уже выбранных serde_json/flate2; нового backend нет.

**Отложено в этих 18 файлах:**

- connection_abstract.h/.cpp → этап 2 → общая factory после решений по backend.
- connection_http.h/.cpp → этап 2 → tolerant HTTP и HTTP-over-SOCKS.
- details/mtproto_tcp_socket.h/.cpp → этап 2 → Digest/NTLM/Negotiate.
- details/mtproto_tls_socket.h/.cpp → этап 2 → production X25519 и timeout→sync owner.
- details/mtproto_web_proxy_socket.h/.cpp → этап 5 → browser Transport/webview.
- details/mtproto_rsa_public_key.h/.cpp → этап 2 → native OAEP/malformed PEM parity.
- details/mtproto_dump_to_text.h/.cpp → этап 2 → Qt double edge formatting.
- mtproto_proxy_data.h/.cpp → этап 3 → raw UTF-16/полный IDNA matrix.
- mtproto_config.h/.cpp → этап 3 → непарные UTF-16.

Сохранены странности оригинала: FactorizePQ принимает пустой pq как p=1/q=0;
validate_secret допускает секреты, которые собственный reader отвергает из-за
лимита 32; release_on_done при несовпадении persistent key не снимает creating.
Последний пункт подтверждён чтением. Отличия: строгая проверка некорректных
строк/PEM, ограниченный logging gzip и double formatting, owner/events вместо Qt
reentrancy. Часть 1 целиком не объявлена готовой, сессия на этом слиянии не начата.
Cargo.lock не добавлен, push не выполнялся; stash base-crypto сохранён.

## Этап 2, часть 2: промежуточная фиксация HTTP/X25519 и конверта сессии

Работа **не завершена**. Main содержит восстановленный учёт прошлого задания
(14d0007, `ledger: native TL reference and MTProto transport`). Удалены старый
mtproto-transport worktree и влитая ветка. Stash `base-crypto before integrating
main` сохранён. Новая ветка `port/tg-mtproto-session` в worktree
`/home/yarik/Projects/ayugram-rs-wt/mtproto-session` создана от этого main.
Текущая незавершённая часть не влита в main.

**Пункт 2:** reqwest для MTProto HTTP заменён ручным POST поверх Tokio byte stream;
NetworkConnector вызывает тот же connect_to_host/SOCKS5/Basic CONNECT, что TCP.
reqwest остаётся только в DoH. Исключение к таблице замен записано в PLAN.md.
Реализованы tolerant status, Content-Length, chunked/trailers/interim response,
EOF framing, pool шести соединений без pipelining, keep-alive, pending/http_wait,
отмена outstanding requests. Заголовки/тело совпали с нативным Qt capture.
В fake TLS добавлен настоящий X25519 через отдельно разрешённый x25519-dalek
2.0.1; fixed private 1..32 даёт ту же точку, что OpenSSL. Старый ClientHello
fixture со всеми полями продолжает совпадать. Digest/NTLM/Negotiate, raw UTF-16
и web-proxy не изменены.

**Эталоном оригинала:** QNetworkAccessManager с настройками из connection_http.cpp
на Qt 6.11.2: sequential=1 connection/2 requests; concurrent=6/8; malformed
HTTP/1.0 с двойным пробелом=8 ошибок Connection closed. Это отдельный Qt harness,
не запуск целого клиента и не доказательство поведения иной версии Qt.
Извлечённый без изменения алгоритма sendSecureRequest/prepareSecurePacket
выполняется с оригинальными SerializedRequest/SHA256/AuthKey/AES-IGE, fixed
entropy/time, отключёнными logs и заменённым network sink. Исходящий packet
совпал; входящий sample проверяет оригинальный prepareAES(false), не выполнение
handleReceived целиком. Стенды/fixtures/manifest сохранены в tests/reference-tools.

**Своими тестами:** HTTP на duplex, keep-alive, cancel, double-space status,
chunked/interim/EOF, повреждённые headers/lengths; X25519/native hello;
MTProto2 конверт с проверками auth_key_id/msg_key/session/length/padding/msg_id
и bad_time, повреждение каждого слова, независимое server-side построение
AES/SHA конверта. TcpConnection::into_split переносит CTR state/частичный frame/
очереди/header, send и receive одновременно проверены для TCP 0/1/D.
Это ещё **не** сценарный сервер полной сессии из задания.

**Живым сервером:** только DH test DC 2 149.154.167.40 (TCP443/HTTP80), оба
успешны. API-запросов/аккаунта/production не было. Пример Instance help.getConfig
ещё отсутствует, getConfig по TCP/HTTP не запускался.

**Только чтением / ограничения:** http_wait соответствует пустоте outstanding
set. Различия Qt idle reconnect/retry/timeout, иные locale/build headers ещё
не установлены запуском; identity/gzip supported, br/zstd/deflate HTTP content
encoding явно отвергаются (другие backend без разрешения не добавлялись).
Прокси/fake TLS живьём не проверены. Нативное decryptSimpleConfig не снято.
В прочитанном ConcurrentSender::senderRequestCancelAll vector сначала создаётся
с size=N, затем в него push_back N id: появляются N нулевых id; этот файл ещё
не портирован, эффект подтверждён только чтением, не отдельным запуском.
Grep AyuGram/ayu в 19 файлах этой части совпадений не дал.

Rust 1.95.0: `cargo fmt --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings` exit 0;
`cargo test --workspace` exit 0: **525 passed, 0 failed, 2 ignored**.
В tg-mtproto 65 обычных тестов; 10 новых, существующий duplex TCP переведён
на одновременные split send/receive. Полный вывод текущих проверок и DH:
`crates/tg-mtproto/docs/verification/session-checks.log`, `session-http.log`.
Новая прямая зависимость только x25519-dalek 2.0.1; const_time_equal в tg-base
использует уже имеющийся RustCrypto crypto-bigint::ctutils. tg-runtime/tg-rpl
не менялись. Cargo.lock не добавлен, push не выполнялся.

**Оставшаяся часть 2, по файлам (та же таблица в README и ledger):**

- session_private.cpp → partial → перенесён только зашифрованный конверт;
  требуется автомат, containers/ack/state/resend/bad_msg/salts/ping/gzip,
  выбор соединений, backoff, временные ключи/bind/PFS/CDN/destroy и base::Timer.
- session_private.h → partial → интерфейс конверта; полный SessionPrivate ещё нет.
- session.cpp → todo → владелец SessionData, задача Tokio, отправка/возврат main queue.
- session.h → todo → Session/SessionData/SessionOptions и совместимый интерфейс.
- mtp_instance.cpp → todo → DC/sessions/keys, migrate/export/import, delayed retries,
  FLOOD_WAIT, invokeAfter/initConnection/layer, cancellation/events и config loading.
- mtp_instance.h → todo → публичный Instance и формат keys-for-write для этапа 3.
- facade.cpp → todo → pause/unpause events через tg-rpl.
- facade.h → todo → shifted DC helpers и facade.
- sender.h → todo → типизированный request/done/fail/send, короткие вызовы.
- mtproto_concurrent_sender.cpp → todo → main dispatch/cancel/handler ownership.
- mtproto_concurrent_sender.h → todo → типизированный concurrent builder.
- config_loader.cpp → todo → DC enumeration/special endpoints и base::Timer.
- config_loader.h → todo → owner/config callback interface.
- special_config_request.cpp → todo → DoH TXT/remote specs/order, RSA/AES simple config.
- special_config_request.h → todo → request owner/cancellation/time handling.
- dedicated_file_loader.cpp → todo → chunk loading/WeakInstance/file lifecycle.
- dedicated_file_loader.h → todo → loader interfaces и typed callbacks.
- proxy_check.cpp → todo → v4/v6 probe lifecycle и callbacks.
- proxy_check.h → todo → checker handles/interface.

Не выполнены обязательные сценарии полного сервера в памяти, base::Timer с
виртуальным временем сессии, native simple config и live Instance getConfig.
Задание целиком не закрыто; этап 3 не начат.

## Этап 2: автомат Session/SessionPrivate — завершён текущий объём

Перенесены только четыре файла: `SourceFiles/mtproto/session.cpp`, `session.h`,
`session_private.cpp`, `session_private.h`; в ledger все четыре **done**.
Instance/sender/concurrent_sender/config_loader/special_config_request/
dedicated_file_loader/proxy_check/facade в этой задаче не начинались. Этап 3
не начинался. Предыдущий раздел «промежуточная фиксация» описывает старый cbb315e,
а не текущее состояние автомата.

Промежуточные коммиты: 012184a (нативный входящий конверт), 8f328e2 (автомат
и первоначальные сценарии), 55e7e83 (аудит контрактов и дополнительные сценарии),
806ccf2 (сохранение Expects), 1e8f027 (снимок состояния и lifetime событий).
Ветка port/tg-mtproto-session влита в main отдельным merge. Пользовательское
прерывание из-за сетевой ошибки модели не потеряло незакоммиченную работу:
перед промежуточным коммитом повторены fmt/clippy и все тесты tg-mtproto.

**Карта готовности:** `crates/tg-mtproto/docs/session-map.md`, 135 строк:
121 определение .cpp, 4 свободных helper, 9 inline SessionData и Default опций.
135 имеют перевод и связанный существующий тест. Три строки дополнительно
затронуты нативными эталонами конверта; это пересекающиеся категории, а не 138
функций. Функций, оставленных только чтением без теста, нет. Настоящие Qt/OS
интеграции, reentrancy и источники platform metadata не выдаются за native запуск.

**Окружение:** SessionEnvironment читает снимок DcOptions, test/destroyer mode,
device/system/language metadata, proxy settings и наличие callback; подписывается
на изменения ключей/options/CDN индивидуальным unbounded каналом без потерь.
InstanceEvent соответствует фактическим вызовам Instance в этих четырёх файлах:
responses/updates/state/reset, запись ключей и key destruction, config/CDN/time
requests, restart timeout/bad configuration. Все уведомления проходят одну FIFO,
затем tg-runtime main queue. Dcenter и общий RequestStore передаются отдельно.
Прямых export authorization/change DC/Config calls здесь нет — методы в трейт
не добавлялись. Реальный Instance реализует контракт в следующей задаче.

Session — задача Tokio LocalSet, снаружи Send SessionHandle. Rc TimerHost
создаётся на потоке задачи. Все таймеры (sender/retry/connect/receive/better/
ping/check/container) — base::Timer. Alarm отделён от протокола: ThreadAlarm
для приложения, ManualClock и явный wake/process в тестах без sleep.
ClientInfo передаёт платформенные значения; смена снимка поддержана командой.
UTC/server time shift пересчитывается для каждого initConnection. Оригинальная
настройка prefer-ipv6 регистрируется через tg-base options Registry.

**Интерфейс соединений:** используются TcpReader/TcpWriter из cbb315e, поэтому
чтение не блокируется ожиданием записи. SessionTransport.submit не ждёт I/O,
next_event cancellation-safe; executor обслуживает candidates/Send/Close/TimedOut
и возвращает packets/errors/состояние/SyncTimeRequest. ConnectTimeout обновляет
fullConnectTimeout при resolving proxy; начальное значение повторяет 4000 *
max(resolvedIPs,1), обычный транспорт — 8000. Реальная factory подключается
будущим Instance; сессия не содержит фиктивного успешного сетевого backend.

**Проверено эталоном оригинала:** отдельно скомпилирован и исполнен входящий
фрагмент handleReceived до dispatcher, с настоящими AuthKey/SHA/AES/constant-time
comparison и проверками length/keyId/msgKey/session/msgId/padding/time. 19 samples,
включая oversized packet через компактное описание 4 194 305 нулевых слов.
Исходящий sendSecureRequest сохранён предыдущим эталоном. Нативный QObject loop,
диспетчер RPC и все состояния автомата C++ целиком не выполнялись.

**Проверено своим сервером/тестами:** независимые MTProto2 и legacy bind key
schedules, настоящий TCP framing/CTR поверх duplex, собственный RSA; permanent
и temporary DH с dh_gen_retry, проверка и расшифровка bind inner, CDN без
permanent/bind. Разрыв после отправки, reconnect/resend, все коды bad_msg
(16/17/18/19/20/32/33/34/35/48/64 и неизвестный), bad_server_salt,
new_session_created, контейнеры, gzip, acks, state request/info, resend request,
пинги и FIFO доставка RPC в main. Проверки повреждений, gzip budget/depth,
received window, seq wrapping, invokeAfter/layer, container cut, key ownership,
destroyer/-404/-444/-429, cancellation/pause, proxy variants, backoff/time
adjustment и всех видов таймеров дополнены тестами прямо через автомат.

Новых тестов 24: **23 сессионных**, **1 native receive fixture** (19 cases внутри).
Rust **1.95.0**, закреплённый nix develop. Команда PORTING.md:

- cargo fmt --check: **exit 0**;
- cargo clippy --workspace --all-targets -- -D warnings: **exit 0**;
- cargo test --workspace: **exit 0**, **549 passed, 0 failed, 2 ignored**.

Полный вывод: `crates/tg-mtproto/docs/verification/session-automaton-checks.log`,
сводка — session-automaton-summary.txt. В tg-mtproto теперь 89 ordinary tests
и два ignored live tests. Новых зависимостей в этой части нет; x25519-dalek
2.0.1 относится к принятому cbb315e. tg-runtime/tg-rpl не изменялись.

**Живой сервер:** в текущей задаче не запускался. Прежний DH TCP/HTTP — история
cbb315e; help.getConfig, аккаунт и production DC не использовались.

**Особенности оригинала и отличия:** LogIdsVector дублирует первый ID;
notifyConnectionInited не сравнивает переданные options; checkSentRequests
планируется в контейнерной, но не одиночной ветке; skip двух выключенных
address/protocol flags пропускает только первый вариант. Сохранено и проверено.
Входящие msgs_state_req/msg_resend_req/future_salts идут через fallback dispatcher
как в оригинале (regular update / игнорирование на иных DC). destroy_auth_key
отправляет Instance, здесь реализован его сессионный lifecycle и уведомления.
Общая SyncTimeRequestDuration в C++ не синхронизирована между сессионными потоками;
в Rust она атомарна. Пустое аутентифицированное тело сохраняет оригинальный
Expects(from < end): tg-base представляет нарушение контракта паникой Rust,
что проверяется на аутентифицированном конверте. QObject
signals/deleteLater заменены FIFO/каналами и владением. Системные значения
ОС/дистрибутива поступают через ClientInfo; будущий слой обновляет их снимок.
Непроверенные настоящие Qt/ОС обстоятельства не считаются native parity proof.

Вопросов, требующих решения пользователя для этого объёма, не осталось.
Следующая задача — только оставшиеся файлы этапа 2 по отдельному поручению.
Cargo.lock не добавлен, push не выполнялся; stash base-crypto сохранён.

**Заключительный аудит интерфейса сессии (1e8f027):** SessionHandle даёт
синхронные get_state/request_state/transport через защищённый снимок, без доступа
к принадлежащему задаче автомату. После stop request_state возвращает 0, как
оригинальный Session без SessionPrivate. ID соединений не переиспользуются при
stop/start; все события транспорта, включая ReceivedSome и SyncTimeRequest,
содержат ID. Поздние события закрытых соединений игнорируются, как отключённые
сигналы QObject. Регрессии проверены lifecycle-тестом. Повторены все три проверки
на окончательном коде: fmt exit 0, clippy -D warnings exit 0, workspace tests
exit 0 (549 passed, 0 failed, 2 ignored); полный лог обновлён. Реестр повторно
сверен: все четыре файла сессии done, файлы следующей задачи остаются todo.


## Этап 2: Instance, Sender и служебные загрузчики (2026-11-12)

Ветка port/tg-mtproto-instance от a600eda; промежуточные коммиты d2bc816,
cd9c34d и f1804a6. Пользовательский main-коммит 33431e2 с Cargo.lock сохранён
и включён в ветку; собственных изменений lock-файла нет. Старые worktree и
влитая ветка mtproto-session удалены. Stash base-crypto сохранён, push не было.

Все 15 файлов текущей задачи отмечены done в ledger. Полная пофайловая таблица
и оговорки находятся в README tg-mtproto. instance-map.md содержит 367 строк:
355 определений функций, 11 =default с языковой заменой и тестом и одно
объявление wipeOutput без определения/вызовов. 366 определений имеют перевод
и связанный тест; decryptSimpleConfig дополнительно проверен нативным эталоном.
Один прототип — только чтение, Rust API для отсутствующей функции не выдуман.
Ссылка на тест проверяет роль функции; это не исполнение всего QObject.

Instance реализует SessionEnvironment; реальным Session передаёт отдельный
мост со снимком метаданных, ключевых событий и FIFO без цикла владения.
RuntimeInstance владеет LocalSet-задачами сессий, ConfigLoader, общим DNS,
резервными HTTPS запросами и base::Timer (delayed/expiry/DNS). Sender выводит
Response из типа запроса, поддерживает done/fail, flood policy, to_dc,
after_request, canWait/override_id и отмену по владельцу. ConcurrentSender
держит слабый Instance и выполняет пользовательский callback через runner.
Пять реальных форм вызова клиента компилируются в tests/sender_calls.rs.

Проверено оригиналом: отдельный CMake вне клона собрал и исполнил целую
оригинальную decryptSimpleConfig, с собственным RSA-2048 ключом и оригинальными
TL writer/reader. Fixture public PEM + ciphertext + expected TL сохранён,
SHA256 a766d08973566c0d14726726c6ff5d0bb7e14d8bd5b58ba3db2783c7c00bfe78.
Исходный клон не изменялся. Повторная сборка и запуск — exit 0.
Это не эталон всего QObject Instance и не запуск резервных интернет-сервисов.

Проверено тестами: реальные Session/SessionPrivate на нескольких DC через
DuplexTransport с независимым серверным шифрованием; migrate/FLOOD_WAIT,
экспорт/импорт гостевой авторизации, upload.getFile (размер и SHA256), owner
lifetime, concurrent callback order, typed API, виртуальные таймеры, пауза,
прокси probe RTT, resolving/cache, конфиг/даты/телефон и fake HTTPS executor.
Logout/destroy, key storage и ветки routing/error policy проверены также
непосредственно через контроллер и его события; это не живой logout/API.
Новых ordinary tests 34. Workspace: 583 passed, 0 failed, 2 ignored;
в tg-mtproto 123 ordinary tests и 2 ignored live DH tests.

Rust 1.95.0, команда из PORTING.md: cargo fmt --check — exit 0;
cargo clippy --workspace --all-targets -- -D warnings — exit 0;
cargo test --workspace — exit 0. Полный вывод и итоговые живые проверки:
crates/tg-mtproto/docs/verification/instance-checks.log,
instance-summary.txt и instance-live.log.

Живой сервер, только тестовые DC, api_id 17349: настоящий Instance выполнил
help.getConfig и help.getNearestDc по TCP и HTTP. Транспорт проверяется
assert по dctransport перед выводом; подписи ранних запусков не считаются
доказательством HTTP. Итог: TCP/HTTP — 9 DC options, country BR,
this_dc 2, nearest_dc 1. Код/QR, аккаунт, production DC и другие прикладные
API-методы не использованы. Служебный DH/PFS выполняется настоящей сессией.
Живой сервер не выявил новой ошибки криптографии конверта. Нашлась ошибка
драйвера/проверки: restart перезагружал опции и снимал ограничение HTTP;
добавлен сохраняемый protocol policy и регрессионный тест. При чтении и
тестах исправлена потеря TLS SyncTimeRequest при снятии кандидатов, добавлен
виртуальный тест. Также проверены удержание after-записей, отсутствие циклов
владения, замена обработчика из callback, canWait/sendDelayed и global pause.
Не выдаём найденное при чтении за находку живого сервера.

Особенности исходника сохранены: main migrate без export, guest auth failures
без logout, первый config request не сохраняется в enumRequest, сравнение
special endpoints игнорирует secret, простой конфиг проверяет expires,
а не будущий date. Контракт при RSA failure представлен expects/panic.
Повторный request ID сохраняет emplace/RequestWrap cancellation.
Определений/маркеров AyuGram/ayu в этих 15 файлах не обнаружено; поведение
проверялось по фактическому коду форка.

Новых зависимостей нет. Пользователь отдельно разрешил существующий reqwest
для резервных HTTPS-конфигов; исключение записано в PLAN.md. HTTP MTProto
по-прежнему ручной AsyncRead/AsyncWrite. tg-runtime и tg-rpl не менялись.
Qt сигналы/потоки заменены Notice/каналами/LocalSet; Core/Lang/Local получают
ConfigApplied/WriteKeys/WriteSettings, их настоящая интеграция — последующие
этапы и только чтение сейчас. WeakInstance Main::Session использует токен
владельца с die/process при смене будущей сессии приложения. Dedicated helper
возвращает FileDescription; владелец создаёт loader с TimerHost. Ожидаемого
SHA256 в исходном loader нет: хеш проверяет стенд, production формат не менялся.
Прокси/fake TLS живьём проверить нечем; реальные Qt/desktop/bootstrap и
резервные Firestore/RemoteConfig не выдаются за проверку запуском.
Вопросов, требующих решения пользователя для этой задачи, не осталось.

## Итог этапа 2

Полный критерий PLAN.md ещё НЕ выполнен. Повторная диагностика 2026-11-13 исчерпала 8 разрешённых попыток: PHONE_CODE_INVALID, независимая проверка пользователя Telethon 1.45 дала тот же отказ; внешняя блокировка для api_id/адреса. Форматные слои tdata и Ayu реализованы отдельно; живой restore остаётся недостигнутым.

- Консольный вход по коду на тестовом DC — НЕ подтверждён: 8 новых номеров на DC1/2/3, документированные 11111/22222/33333 отвергнуты. SMS length=5, next_type=Call, timeout=90; C++ подтвердил байты всех семи пар с полными дампами. Проверены init/layer, маршруты, key/session IDs и preflight; причина не установлена. Логи: crates/tg-mtproto/docs/login-diagnostics/. Прежняя попытка 2026-11-12 — в crates/tg-core/docs/verification/login-test-dc.log.
- Консольный вход по QR на тестовом DC — пример реализован и компилируется, но живой путь не достигнут: первый Instance не авторизован по коду.
- help.getConfig через каждый транспорт — выполнен для TCP и HTTP,
  подтверждён instance-live.log с assert фактического транспорта.
- help.getConfig через каждый прокси — не выполнен: нет живых прокси;
  SOCKS5/Basic CONNECT/fake TLS проверены образцами/в памяти, WEB и расширенные
  схемы HTTP-auth отложены. Нельзя считать этим закрытым весь критерий.

Всего файлов этапа 2: 74; done 49, replaced 1, partial 18, todo 6.
Все partial/todo ниже перечислены пофайлово. Номер после стрелки — этап возврата.
Вход по коду предпринят без успеха; QR не достигнут. Начат только нижний слой этапа 3, без пользовательской tdata, настроек и чатов.

- `SourceFiles/mtproto/connection_abstract.cpp` (partial) → 5 → factory браузерного WEB transport/webview.
- `SourceFiles/mtproto/connection_abstract.h` (partial) → 5 → factory браузерного WEB transport/webview.
- `SourceFiles/mtproto/connection_http.cpp` (partial) → 2 → br/zstd/deflate decoding и native matrix Qt idle/retry/timeout.
- `SourceFiles/mtproto/connection_http.h` (partial) → 2 → br/zstd/deflate decoding и native matrix Qt idle/retry/timeout.
- `SourceFiles/mtproto/details/mtproto_dump_to_text.cpp` (partial) → 2 → эталоны Qt double formatting на границах.
- `SourceFiles/mtproto/details/mtproto_dump_to_text.h` (partial) → 2 → эталоны Qt double formatting на границах.
- `SourceFiles/mtproto/details/mtproto_rsa_public_key.cpp` (partial) → 2 → OAEP и полная матрица malformed PEM/ASN.1.
- `SourceFiles/mtproto/details/mtproto_rsa_public_key.h` (partial) → 2 → OAEP и полная матрица malformed PEM/ASN.1.
- `SourceFiles/mtproto/details/mtproto_tcp_socket.cpp` (partial) → 2 → согласованный backend для Digest/NTLM/Negotiate CONNECT.
- `SourceFiles/mtproto/details/mtproto_tcp_socket.h` (partial) → 2 → согласованный backend для Digest/NTLM/Negotiate CONNECT.
- `SourceFiles/mtproto/details/mtproto_tls_socket.cpp` (partial) → 2 → полная native Qt signal/error matrix; timeout→SyncTimeRequest уже проверен виртуально.
- `SourceFiles/mtproto/details/mtproto_tls_socket.h` (partial) → 2 → полная native Qt signal/error matrix; timeout→SyncTimeRequest уже проверен виртуально.
- `SourceFiles/mtproto/details/mtproto_web_proxy_socket.cpp` (partial) → 5 → настоящий браузерный Transport/webview.
- `SourceFiles/mtproto/details/mtproto_web_proxy_socket.h` (partial) → 5 → настоящий браузерный Transport/webview.
- `SourceFiles/mtproto/mtproto_config.cpp` (partial) → 3 → представление raw QString с непарными UTF-16 без потерь.
- `SourceFiles/mtproto/mtproto_config.h` (partial) → 3 → представление raw QString с непарными UTF-16 без потерь.
- `SourceFiles/mtproto/mtproto_proxy_data.cpp` (partial) → 3 → raw UTF-16 и полная матрица Qt IDNA.
- `SourceFiles/mtproto/mtproto_proxy_data.h` (partial) → 3 → raw UTF-16 и полная матрица Qt IDNA.
- `SourceFiles/mtproto/web_proxy/web_proxy_frame.cpp` (todo) → 5 → webview и браузерный bridge.
- `SourceFiles/mtproto/web_proxy/web_proxy_frame.h` (todo) → 5 → webview и браузерный bridge.
- `SourceFiles/mtproto/web_proxy/web_proxy_transport.cpp` (todo) → 5 → webview и браузерный transport.
- `SourceFiles/mtproto/web_proxy/web_proxy_transport.h` (todo) → 5 → webview и браузерный transport.
- `SourceFiles/mtproto/web_proxy/web_proxy_webview.cpp` (todo) → 5 → оконный слой и backend webview.
- `SourceFiles/mtproto/web_proxy/web_proxy_webview.h` (todo) → 5 → оконный слой и backend webview.


Независимый контроль пользователя: Telethon 1.45, тот же api_id, тестовый DC2,
sentCodeTypeSms длины 5 и PHONE_CODE_INVALID на 22222. Агент эту проверку не
запускал. Вход по коду и QR заблокирован внешней причиной; примеры готовы,
ждут другого api_id или аккаунта. Сквозной restore tdata без повторного входа
ждёт того же. Новые live-попытки запрещены пользователем.

## Тестовый вход и этап 3: ключи и каркас tdata (2026-11-12/13)

Ветка port/tdata от main 7ef2c60; промежуточные коммиты 0d57c13 и 24b2658,
слияние 726725b. Старые worktree mtproto-instance и влитая ветка удалены.
Чужие README/secret-o3-api.jpg из main сохранены; tg-cache и строки lib_storage
не менялись. Cargo.lock обновлён: новая внешняя зависимость md-5 0.11.0 и
внутренние зависимости tg-core на tg-base/tg-mtproto/tg-rpl/tg-runtime.
Test/example зависимости tg-scheme/tg-tl/tokio уже были в workspace.

**Живой вход:** test_dc_login.rs сверяет последовательность с intro_phone,
intro_code, intro_qr и intro_signup. Только Environment::Test, api_id 17349 и
публичный test api_hash из config.h; генерируемый 999662YYYY, код 2 длиной из
sendCode, SignUp Port Test при необходимости. QR: export, accept первым Instance,
updateLoginToken, новый export и повторяемый migrate/import вторым. После успеха
self через users.getUsers и logout. Код/QR UI не переводился.

Первый запуск выявил ошибку примера: пустой device_model ->
CONNECTION_DEVICE_MODEL_EMPTY. Заданы Desktop/Linux. Следующий запуск:

    test phone: 9996620387; initial DC=2
    auth.sendCode: length=5; main DC=2
    test login stopped: Error { code: 400, kind: "PHONE_CODE_INVALID", description: "" }

Отправлялся 22222. Другие коды/номера после отказа не подбирались, FLOOD_WAIT
не обходился. Успешного входа нет: users.getUsers/QR/logout не достигнуты,
ни один аккаунт не авторизован. Production DC не использовались.
В ранних запусках ещё был включён автоматический ConfigLoader help.getConfig —
лишний запрос относительно ограничения задачи. Ошибка явно учтена:
Runtime.set_automatic_config(false) отключает его в обоих новых примерах;
исполняемый тест до любого I/O проверяет подавление и повторное включение.
Повторного живого запуска после PHONE_CODE_INVALID не было. Других исправлений
протокольного автомата по предположению о причине отказа не делалось.
Логи login-*.log в crates/tg-core/docs/verification.

**Форматы:** TDF$ + LE version/length + оригинальный MD5 trailer; выбор s либо
legacy 0/1; временный файл/rename и очередь с coalescing/sync/stop/Drop.
Криптография: modern SHA512(salt+pass+salt) + PBKDF2-HMAC-SHA512 (256 байт,
1/100000), legacy PBKDF2-HMAC-SHA1 (4/4000), SHA1[0..16] и AES-IGE с local v1
schedule false. Общий Qt_5_1 код вынесен из tg-mtproto в tg_base::qt_stream;
старый путь переэкспортирует его. BE integers, nullable QByteArray и raw UTF16
(включая непарные units) реализованы один раз. Старые convenience bytes/string
сохраняют прежнее сведение null к empty; новые nullable_* сохраняют различие.

storage_domain хранит local key/passcode salt, encrypted local key, список и
активный индекс; modern/legacy запуск, смена/проверка код-пароля. Уведомления
синхронны через tg-rpl::EventStream/Producer/Lifetime, Domain живёт на главном
потоке. Main list-write идёт через on_main_local с weak-корнем; готовые бинарные
WriteEntry передаются ObjectOnThread. Таймер map — base::Timer с ManualClock.
main_account хранит wide/narrow authorization и связывает её с настоящим Instance
через Fields/getKeysForWrite/addKeysForDestroy. Это форматный срез, а не готовые
Main::Session/Core/UI модели. Account читает известные map records без открытия
settings/chat/cache файлов и сохраняет payload непрозрачно. Auth dbiUser/dbiKey
и 0x4b, encrypted MTP Config envelope готовы; остальные ReadSetting отложены.

**Карта:** crates/tg-core/docs/tdata-map.md, 133 строки: 128 определений с телом,
5 default. 125 связаны с исполняемыми тестами; 8 Windows-only строк заменены
готовым base и проверены чтением — LockFile/Restart Manager в Linux не запустить.
31 строка дополнительно затронута native execution, категории пересекаются.
Для main_*/storage_account вне текущего объёма пофункционально перечислены
196 определений следующей задачи; нет готовых заглушек Cache/Session/settings.

**Нативный эталон в обе стороны:** отдельный CMake вне клона оригинала,
Qt 6.11.2/OpenSSL 3.6.5/GCC 15.3. Исполнены исходные utility functions,
writeNow/writeHeader/open/path с QSaveFile safe path, MD5 из core/utils.cpp,
AuthKey/SHA/AES, Main authorization serialize/parser, Storage::Domain key/info
writer/parser. Четыре каталога: empty/test-passcode × 1/3 аккаунта, 8 аккаунтных
записей всего, по два текущих DC-ключа и одному destroy-key. Оригинальный writer
и reader совпали; Rust получил те же поля и все 256 байт каждого ключа;
Rust при том же entropy пишет те же имена и байты файлов; оригинальный reader
прочитал Rust-каталоги с теми же ключами/индексами/активным аккаунтом. Exit 0
во всех четырёх вариантах обоих направлений. Fixtures/manifest в tests/fixtures.
Дополнительный оригинальный образец подтверждает null/empty QString/QByteArray,
unpaired UTF16, RGBA, оба PBKDF и narrow -> wide authorization reserialization.

Границы native доказательства: owner/account-start — фасад, не Qt Main::Session;
пустая карта строится обвязкой через оригинальный FileWriteDescriptor, не целым
Storage::Account::writeMap с моделями пользователя. Полный Storage::Account/Cache,
клиент, исходная асинхронная очередь и OS-fallback C++ не выполнялись. Safe path
исполнялся; fallback обвязки аварийно завершает capture, если туда попасть.
Rust очереди, ошибки I/O и suffix selection проверяются отдельно. Не выдаём
эту обвязку за запуск оригинального приложения или его UI/bootstrap.

**Свои тесты:** повреждение каждого байта TDF и все усечения, неверный пароль,
размеры/границы AES и SHA1, s vs 0/1 и порядок mtime без sleep, coalescing,
async writer/sync/stop/Drop и I/O failure, legacy key migration и narrow IDs,
map records, key handoff Instance, Config envelope, код-пароль/rpl lifetime,
виртуальный map timer и main queue. Новых обычных тестов 12: 11 в tg-core,
1 в tg-mtproto. Плюс ручной ignored synthetic exporter выполнен отдельно.
Переиспользованы base FileLock и его тесты двумя процессами; Win запуск недоступен.

**Сквозной живой процесс:** test_dc_tdata.rs компилируется. Он должен войти только
на тестовом DC, записать временную tdata вместе с test Config, запустить себя новым
процессом, выполнить users.getUsers без sendCode/signIn и logout, затем удалить
каталог. Живьём не выполнен: обязательный первый вход отвергнут сервером,
повторных попыток после PHONE_CODE_INVALID не делалось. Успешный restore не
подменён native fixtures. Настоящая tdata пользователя не открывалась/не копировалась.

**Проверки:** Rust 1.95.0, точная команда PORTING.md:
- cargo fmt --check — exit 0;
- cargo clippy --workspace --all-targets -- -D warnings — exit 0;
- cargo test --workspace — exit 0, 595 passed, 0 failed, 3 ignored.
Два ignored — прежний live DH; третий — ручной exporter для native read Rust.
Полный вывод: crates/tg-core/docs/verification/workspace.log, summary.txt,
native-create.log/native-read-rust.log и rustc.txt.

**Особенности оригинала/отличия:** повреждённый modern info после назначения
local key вызывает Failed, затем startFromScratch пытается generateLocalKey
при уже существующем ключе. Контракт сохранён как panic и проверен своим тестом;
C++ аварийный путь подтверждён только чтением. При наличии s оригинал не пытается
читать 0/1, даже если s повреждён; сохранено. Уже прочитанные auth поля остаются
после усечения, как в void-parser C++; Rust дополнительно возвращает Result.
Отрицательный count отвергается вместо опасного reserve(count). QDataStream
Status заменён явным Option/Result с ранним возвратом. Имена временных файлов
отличаются от Qt; финальные байты совпадают. OS failure matrix и Windows не
выдаются за native parity. Внутренний Cache cleanup, канонизация старых map
records и весь UI/self/settings lifetime отложены и не выполняются молча.
Явных AyuGram/ayu маркеров в перечисленных файлах не найдено; kPremiumMaxAccounts
перенесён из фактического форка как 200, без замены значением upstream.

Реестр этой задачи: 13 строк — 6 done (utilities/serialize/domain), 3 replaced
(storage FileLock через base), 4 partial (storage_account.cpp/.h и main_account.cpp,
main_domain.cpp). Основные main headers остаются todo этапа 4.
Отложено по файлам:
- storage_account.cpp/.h → следующий срез этапа 3 → settings/self/drafts/stickers,
  применение и канонизация непустой карты, остальные ReadSetting/readOldMtpData,
  Cache/cleanup через согласованный интерфейс; lib_storage не трогать.
- main_account.cpp → этап 4 (storage hooks — продолжение 3) → Main::Session/Core/UI,
  настоящие владельцы и подписки на сохранение.
- main_domain.cpp → этап 4 (storage hooks — продолжение 3) → UI/активация/окна,
  lifecycle аккаунтов; форматный список и его weak-запись уже готовы.
- storage_file_lock_win.cpp → Windows проверка → LockFile/Restart Manager.
- code/QR и живой restore → продолжение проверки этапов 2/3 → разрешённый
  тестовый сервер должен принять предписанный код; не обходить отказ.

Вопросов, требующих решения для этого среза, нет. Полный критерий этапа 2 не
закрыт из-за отказа входа/непроверенных прокси; критерий этапа 3 также не закрыт:
живого взаимного подключения по tdata ещё нет. Дальнейшие settings/chat/cache
задачи не начинались. Пуш не выполнялся, stash base-crypto сохранён.


## 2026-11-13 — диагностика тестового входа и данные AyuGram

Работа выполнена в `port/ayu-data`, worktree `ayugram-rs-wt/ayu` от ffdb756.
Принятый worktree tdata и ветка port/tdata удалены. Оригинал 0f2a31bb оставался
только для чтения; сборки reference tools находятся вне клона. Пользовательские
`tdata`, `ayu_settings.json`, `ayudata.db` не открывались и не копировались.

### Тестовый вход: восемь попыток, причина PHONE_CODE_INVALID не установлена

Лимит исчерпан: 8 разных номеров, только Test DC 1/2/3 и API 17349/hash из config.h.
FLOOD_WAIT не было. Первая попытка была запущена старым бинарником до окончания
сборки: в лимит включена, доказательным дампом не считается. Остальные семь:

| Попытка | Номер | DC/код | Результат |
|---|---|---|---|
| 1 | 9996624442 | 2 / 22222 | PHONE_CODE_INVALID, старый бинарник |
| 2 | 9996629716 | 2 / 22222 | PHONE_CODE_INVALID |
| 3 | 9996613177 | 1 / 11111 | PHONE_CODE_INVALID |
| 4 | 9996638502 | 3 / 33333 | PHONE_CODE_INVALID |
| 5 | 9996621689 | 2 / 22222 | PHONE_CODE_INVALID, плюс и preflight |
| 6 | 9996624880 | 2 / 22222 | PHONE_CODE_INVALID, suggestMainDcId при forced DC |
| 7 | 9996621711 | 2 / 22222 | PHONE_CODE_INVALID, unforced, маршрут DC1→DC2 |
| 8 | 9996621717 | 2 / 22222 | PHONE_CODE_INVALID, getConfig + nearest/countries, unforced |

Все диагностические sendCode: **sentCodeTypeSms, length=5, next_type=Call,
timeout=90, flags=6**. Ответ целиком выводится dump_to_text; почты/иной фазы в этих
ответах нет. Подробные запросы, ответы, hex, адреса и session/key IDs:
`crates/tg-mtproto/docs/login-diagnostics/`. Официальное правило номера/кода
проверено по https://core.telegram.org/api/auth; изменение правил этим не доказано.

Проверки по заданию:
1. Тип ответа и полный dump получены для попыток 2–8.
2. Нативный C++ lib_tl + оригинальная генерированная scheme сопоставил **все семь
   фактических пар** sendCode/signIn с логами: байты одинаковые, codeSettings flags=0,
   signIn flags=phone_code. Корпус cases.bin/tsv/json и cargo test закрепляют это.
3. invokeWithLayer=229, initConnection API=17349, Desktop/Linux, 7.2.9,
   en/tdesktop/en, params tz_offset=0: исходные вызовы и C++ wrapper сопоставлены.
   Фактические prefix words попыток 3–8 совпали с C++ (в 2 ещё не логировались).
   Desktop — явная модель headless-примера; оригинал может получать модель из DMI.
   Это не перехват трафика запущенного графического оригинала.
4. Подтверждены Test IP 149.154.175.10, 149.154.167.40, 149.154.175.117.
   В 7–8 наблюдается переход main DC1→DC2 на sendCode; завершённый sendCode и signIn
   используют DC2 и одинаковые auth_key_id/session_id. Конкретный промежуточный
   migrate error перехватывается стандартным обработчиком, отдельно не логировался.
5. DC1/11111 и DC3/33333 проверены, ответ тот же.
6. intro_phone/code передают пустой codeSettings, phone/hash/code как в примере;
   sendCode допускает плюс, signIn использует DigitsOnly. Intro запускает nearest
   и countries асинхронно, телефонная форма отменяет ещё ожидающий nearest;
   config_loader также получает getConfig. Попытки 5–8 проверили эти различия;
   в 8 getConfig выполнен первым. Смена порядка результата не изменила.

Восьмая попытка запущена через test_dc_tdata, который теперь использует общий
login_qr после того же входа по коду, затем пишет временную tdata и запускает
дочерний процесс. До QR/записи/нового процесса выполнение НЕ дошло:

    test tdata stopped: Error { code: 400, kind: "PHONE_CODE_INVALID", description: "" }

Протокольных исправлений сессии/Instance по живому серверу не заявляется:
причина не найдена, ошибка порта не исключена. Добавлены только диагностика,
нативный TL-корпус и общий сценарий примеров. Новых live-попыток после восьмой нет.

### Ayu: 115 сохраняемых настроек, восемь таблиц

`ayu-core` содержит модули по исходным путям, типизированные getters/setters и
producer API, per-account ghost, MessageShot, процессное скрытие сообщений,
полные SQL-сущности/запросы и mapper извлечённых значений HistoryItem/PeerData.
Настройки/main-thread используют tg-rpl; SQLite — один владелец с mutex/клонами,
busy_timeout=0, open lifetime до последнего клона. Таймштамп backup — tg_base unixtime.
Глобальный singleton заменён явным владельцем. Реактивность reset и порядок
Effect/save, а также save перед markAsOnline, проверены тестами.

В `crates/ayu-core/docs/ayu-data-map.md` **517 строк функций** (декларации не
дублируются, inline overloads и enum-конвертеры учтены). У 516 строк тестируется
переведённая форматная/скалярная часть; repaintApp — только чтение, нужен оконный
слой этапа 5. Извлечение из объектов Core и применение UI-эффектов явно отложены;
они не выдаются за исполнение Main::Session/QWidget. Явные default/delete special members
заменены владением Rust, без Clone.

Фактических сохраняемых конечных настроек **115**: 88 корневых (включая множество
shadowBanIds), 14 ghost и 13 message shot. Контейнеры дают 117 определений JSON-ключей.
Есть также 8 старых корневых ghost-ключей для миграции и вычисляемое ghostModeActive.
Число «124 сохраняемые настройки» из PLAN.md не совпадает с данным оригиналом;
необнаруженные настройки не выдумывались. Все 115 проверены non-default значениями.

Таблицы: SchemaVersion, DeletedMessage, EditedMessage, DeletedDialog, RegexFilter,
RegexFilterGlobalExclusion, SpyMessageRead, SpyMessageContentsRead. Сохраняются
ограничения, NULL, индексы, порядок столбцов, диапазоны/сортировки запросов и literal
LIKE. Явный replace сохраняет разреженные/максимальные PK; обычный add/insert
игнорирует переданный integer PK, как sqlite_orm. getExcludedByDialogId сохраняет
две отдельные выборки и преобразование filterId в vector<char> под одним mutex.

**Исполнением оригинала подтверждено:**
- JSON defaults/all-changed одинаков байт в байт (4 пробела, Unicode/escaping,
  порядок, double, нет финального LF); C++ прочитал Rust JSON тем же образом,
  что собственный эталон. Конструкторы/конвертеры/load/save/validate берутся из
  оригинала; в headless-обвязке Core::IsAppLaunched=false, edited="edited",
  Native translate недоступен, ghost() выбирает 0. Это не запуск GUI.
- 329 wrong-type/partial-mutation probes, 12 load-образцов (BOM, trailing data,
  malformed JSON, migration, cGhost, validate), 2006 finite-double образцов Grisu2.
- Оригинальный ayu_database.cpp/sqlite_orm создал все 8 таблиц и 12 строк:
  i64/i32 MIN/MAX, разреженные edited fakeId=1/7, Spy fakeId=i64::MAX, NULL/0,
  Unicode/пустые строки и бинарные NUL/0x80/0xff. Rust прочитал и записал ту же
  схему/строки; оригинальный C++ прочитал Rust. Все ORM dumps сравнились побайтно;
  sqlite_master — по полному тексту SQL и именам, не по rootpage.
- 9 вариантов sync_schema: extra/missing nullable/missing required, NOT NULL,
  игнорируемый declared type, V0, future/overflow version, отсутствие id=1.
- Оригинальный getter подтвердил обрезание text на NUL; raw SQLite остаётся полным.

**Своими тестами:** все typed getters/value/changes/setters, SQL-условия/пределы/
сортировки/ошибки/rollback/LIKE, SQLite numeric coercions, legacy filterId TEXT,
параллельные клоны, повреждённая БД/backup, ghost locks/accounts, effects/lifetime/reset,
извлечённый message mapping. Новых обычных тестов **21**: 20 ayu-core, 1 native login TL.

**Только чтением / без запуска в этой среде:** привязка к живым Core/Window/Translator/
Tray/Worker, QWidget repaint и платформенный Native translate; это этапы 4/5.
Оконный/Windows runtime не проверялся. Реальная пользовательская БД не использовалась.

Новые прямые внешние зависимости ayu-core: rusqlite 0.42.0 (bundled), serde 1.0.229,
serde_json 1.0.154 (float_roundtrip); tg-base/tg-rpl — существующие workspace крейты.
libsqlite3-sys 0.40.0 транзитивный, bundled SQLite 3.54.1. Нативная SQLite из Nix —
3.53.3. Cargo.lock обновлён и коммитится. В tg-mtproto новых зависимостей нет.

**Особенности оригинала / отличия:**
- V1 удаляет старые regex-фильтры; sync_schema игнорирует declared type/порядок
  уже существующих столбцов. Поведение сохранено, не канонизируется молча.
- Если нет SchemaVersion(id=1), но есть другие ID, insert исключает integer PK,
  создаёт следующий rowid, UPDATE id=1 не обновляет строку. Этот дефект сохранён.
- std::string читается sqlite_orm до NUL: при повторной записи теряется хвост.
  Это дефект/ограничение оригинала, не скрытое обещание lossless для таких текстов;
  blobs полные. postAuthor/replyMarkupSerialized отсутствуют в ORM и не дописаны.
- Порядок shadowBanIds воспроизводит Linux libstdc++ x86_64 (reserve/rehash), не
  обещается для другого стандартного C++ runtime. Числа печатает перевод Grisu2.
- RegexFilter::toJson ограничен длиной id; C++ без NUL читает за vector и не даёт
  определённого эталона. Нефинитный typed double отвергается явно; C++ мог записать
  null и затем не прочитать его. Rust дополнительно возвращает ошибки I/O/JSON;
  уже выполненные присваивания сохраняются. Default — модель в памяти, путь явный.

Учёт этого среза: 10 строк (одна добавлена для libs/json_ext.hpp), 3 done,
6 partial, 1 replaced. lib_storage и crates/tg-cache не изменялись.
- ayu_settings.cpp/.h (partial) → 4/5 → владелец Core, active account, localized edited,
  применение Effect к filters/translator/tray/worker/window.
- ayu_state.cpp/.h (partial) → 4 → HistoryItem IDs и токен жизненного цикла Session.
- data/messages_storage.cpp/.h (partial) → 4 → извлечение HistoryItem/PeerData,
  настоящий flags/entities mapper; значения/SQL слой уже проверены.
- data/ayu_database.cpp/.h и entities.h — done, libs/json_ext.hpp — replaced.

Проверки Rust 1.95.0: cargo fmt --check — exit 0;
cargo clippy --workspace --all-targets -- -D warnings — exit 0;
cargo test --workspace — exit 0, **616 passed, 0 failed, 3 ignored**.
Логи: crates/ayu-core/docs/verification/workspace.log, summary.txt, native-summary.txt,
native-read-rust.hex; вход — crates/tg-mtproto/docs/login-diagnostics/.

Полный этап 2 и этап 3 не объявлены закрытыми: code/QR/живой restore остаются
неподтверждёнными; Telegram settings/chat/cache из следующего среза не начаты.
Вопросов по формату Ayu, требующих решения пользователя, нет. Причина входа
остаётся открытой; лимит этого задания исчерпан. Пуш не выполняется, stash
base-crypto сохраняется, чужие README/изображение не меняются.

## 13.11.2026: независимые индексы истории и медиа

В main включены существующие коммиты `a0f0f02` (граф и план этапа 4) и
`be00773` (первые storage-модели) из port/core-base. Незакоммиченные lang,
countries, UI/value contracts, runtime и Sender в отдельном рабочем дереве
не переносились и не изменялись; они требуют своего завершения и проверки.

Закончены SparseIdsList, SharedMedia, UserPhotos и Facade: восемь .cpp/.h
отмечены done в ledger. Это независимые индексы в памяти, без подставного
Main::Session. Все 13 типов медиа, peer/topic/monoforum, сортировка/слияние
диапазонов, подсчёт пропусков, query/snapshot и rpl-события доступны в tg-core.
Из первого коммита исправлены ручные u16-маски и BTreeSet: теперь используются
существующие tg-base EnumMask (исходные 32 бита, all) и FlatSet. Сравнение
SharedMediaKey соответствует исходному порядку полей.

Сохранены особенности оригинала: removeOne уменьшает известный счётчик даже
при отсутствии ID; removeAfter удаляет начиная с указанной фотографии;
photo query перезаписывает count-based skippedBefore. Правка Ayu для обратного
диапазона сохраняет применение count и событие, даже если сами ID не добавлены.

Новый native_storage_trace компилирует неизменённые четыре исходных .cpp,
настоящие lib_rpl/base::flat_set и Qt Core. MsgId/PeerId/UserId/PhotoId в
headless-префиксе заменены соответствующими целыми значениями; оконный клиент
и платформенный bootstrap не собираются. На 7031 детерминированной операции
сравниваются каждое событие, завершение producer, query/snapshot/empty,
countAfter и число вызовов предиката. Вход и ответы с SHA256 сохранены в
tests/fixtures/core-base; воспроизведение описано там же. Ещё три Rust-теста
проверяют чтение при подписке, известный нулевой count, завершение без значения
и отписку с чтением snapshot из обработчика. Всего в новом срезе 14 тестов:
10 storage_lists, 3 cloud_blob и один native trace.

CloudBlob оставлен partial: распаковка ZIP выполняется, но BlobLoader,
StateDescription, поле Progress::percent и Session/UI binding ещё отложены.
Тесты ZIP — собственные, без заявления C++-паритета распаковки. Новых внешних
зависимостей и изменений Cargo.lock в этом завершении нет.

Этапы 2/3/4 целиком не закрыты: живой вход/QR/restore не подтверждены,
Telegram settings/chat persistence/cache остаются дальше. Новых попыток
тестового входа не выполнялось. Реальные tdata/ayudata пользователя не читались.
Карта функций: crates/tg-core/docs/storage-lists-map.md.

Целевой репозиторий по уточнению пользователя:
https://github.com/B2B-AI-SaaS-Startup-LLC/GramTele (remote gramtele).
remote.pushDefault установлен в gramtele; прежний origin сохранён.

Проверки Rust 1.95.0 — PASS: cargo fmt --check;
cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace:
630 passed, 0 failed, 3 ignored. После окончательного перехода на FlatSet/EnumMask
повторно прошли fmt, clippy всего workspace и cargo test -p tg-core:
25 passed, 0 failed, 1 ignored (ручной exporter). Новых ignored нет.
Краткий протокол — crates/tg-core/docs/verification/storage-lists.txt;
полные локальные логи — /tmp/ayugram-codex-storage-workspace.log и
/tmp/ayugram-codex-storage-final.log.

## 13.11.2026: работа разделена, файловый слой tg-cache

Пользователь поручил вести работу параллельно с Claude на другой машине.
Распределение — docs/parallel-work.md. Для Claude сохранён WIP lang/countries
в handoff/claude-lang-countries, готовое задание — docs/tasks/claude-lang-countries.md.
Снимок исходной незавершённой работы не влит в main; обнаруженные два падения
его language tests записаны в handoff. Исходный грязный worktree сохранён.

Codex в port/cache-file-format перенёс storage_encryption и
storage_encrypted_file в tg-cache: AES-CTR/key derivation, TDEF, header,
read/write/padding, seek/offset/size, FileLock и Move. Native оригинал читает
Rust файлы обоих размеров ключа и пересоздаёт их побайтно идентично. Обратное
чтение проверяется fixtures, включая повреждения/обрезания/формат/checksum.
Перенесены обычные секции исходных тестов и POSIX two-process takeover.
Реальные данные пользователя не использовались, живых входов не было.

PASS Rust 1.95.0: fmt, clippy --workspace --all-targets -D warnings,
cargo test --workspace — 643 passed, 0 failed, 3 прежних ignored;
финальный tg-cache — 13 passed, 0 failed, 0 ignored. Native: 30 CTR vectors,
22 file probes, 8 Move cases, cross-device Move и byte-exact Rust → C++ rewrite.
Отчёт — docs/handoffs/codex-cache-file.md, карта — crates/tg-cache/docs/port-map.md.

Две строки storage_encryption — done. storage_encrypted_file.cpp/.h и
storage_encrypted_file_tests.cpp — partial: Windows/macOS runtime и ошибка
диска/буферизация QFile остаются непроверенными; std::fs не эмулирует QFile
buffering. Остальные Cache/binlog/database/cleaner/compactor — todo.
Этап 3 полностью не закрыт. Версии зависимостей не обновлялись: tg-cache
использует уже имеющиеся aes/ctr/nix/tg-base. Cargo.lock меняет только
зависимости пакета tg-cache.

## 13.11.2026: ID ядра после lang/countries

По просьбе пользователя продолжен независимый блок ядра после области Claude.
В port/data-ids добавлены data_peer_id и скалярная часть data_msg_id:
типизированные UserId/ChatId/ChannelId/FakeChatId/PeerId, signed MsgId,
FullMsgId/FullStoryId/GlobalMsgId и MsgRange, константы и client/story conversions.
TL Peer и modern/legacy storage serialization сверены с настоящим C++.
Сохранены marker bit 55, знаковые MTPint conversions, верхние биты и исходный
fallback peerUser(0); недопустимые значения не канонизируются молча.

PASS Rust 1.95.0: fmt, clippy --workspace --all-targets -D warnings,
cargo test --workspace — 651 passed, 0 failed, 3 прежних ignored.
Новых проверок восемь: 4 обычных и 4 compile-fail. Native corpus — 1792 операции
(1236 Peer, 549 Msg, 6 индексов, 1 таблица констант), включая exact TL/Qt bytes.
В native используются неизменённые strong ID headers/data_peer_id.cpp и
извлечённый без правки generated Peer code; целочисленных замен ID нет.
Полные оригинальные .cpp/.h прочитаны; GUI/Session runtime не запускался.

data_peer_id.cpp/.h — done; data_msg_id.h — partial. MessageHighlightId,
FullReplyTo и SuggestOptions ждут TextWithEntities/CreditsAmount от Claude,
заглушек для них нет. Его lang/countries/runtime/Sender/UI/codegen не менялись.
tg-scheme перенесён из dev в обычные зависимости tg-core; Cargo.lock и версии
не менялись. Добавлен только экспорт data в корневой lib.rs.

Карта: crates/tg-core/docs/data-ids-map.md; отчёт:
docs/handoffs/codex-data-ids.md; протокол:
crates/tg-core/docs/verification/data-ids.txt. Границы параллельной работы
обновлены в docs/parallel-work.md. Raw storage-list adapters и MTProto packet
MsgId=u64 остаются прежними: это разные пространства ID. Session/stores и
полный этап 4 ещё не закрыты, новых живых входов не выполнялось.


### tg-cache: оставшаяся база медиа (port/cache-database)

Срез от main 20695cb завершает все 24 файла lib_storage (4 598 строк):
23 done, storage_pch.h replaced импортами Rust, partial/todo = 0.
Предыдущие partial File.h/.cpp и encrypted_file_tests.cpp закрыты буферизацией
QFile, native short-write/ENOSPC/flush и переносом процессных секций без sleep.

Database/Databases, явный LE binlog codec, все Store/MultiStore/MultiRemove/
MultiAccess, 7-байтный PlaceId, восстановление, delayed access/remove,
pruning/лимиты, compactor/catch-up/атомарная замена и cleaner/clear_legacy
используют реальную очередь tg-runtime и ConcurrentTimer. Stats — tg-rpl на main;
Main::Session и Data не имитируются. Область Claude не изменялась.

Карта: crates/tg-cache/docs/port-map.md — 252 строки, все с реализацией и
исполняемым тестом, 0 отложенных. Все 20 исторических SECTION fe618bd652 + init timers
перенесены как сценарии для текущего ccdc725; в native старый .cpp не включён.
Большой тест на 102 400 записей включён. Сохранены отличия текущего API и
порядок prune/touch на общей границе таймеров.

Native CMake вне оригинала: 10 баз, 284 файла / 130 848 байт совпадают целиком;
двусторонние чтение/значения/теги и дописанные Access (135 360 байт) совпадают;
22 recovery-пробы, 72 файла / 14 797 байт, QFile faults и symlink-cleaner PASS.
Manifest с исходными SHA256 и commits сохранён в tests/database-reference.
Реальная tdata не открывалась. Windows/macOS runtime, power loss и произвольные
внешние гонки ФС не запускались; границы и особенности оригинала перечислены
в docs/handoffs/codex-cache-database.md и карте сценариев.

Rust 1.95.0 / nix develop: fmt --check PASS; clippy --workspace --all-targets
-D warnings PASS; test --workspace 678 passed, 0 failed, 3 прежних ignored.
tg-cache: 40 тестов, +27 к предыдущим 13, новых ignored нет.
Новых Rust-пакетов/версий нет: добавлены workspace tg-runtime/tg-rpl и feature
resource у прежнего nix для RLIMIT_FSIZE. Интеграция — main и ветка в gramtele,
без force push; подробный поимённый учёт 24 файлов — в handoff.
