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
| TL: каждый тип против эталонного C++ исполнения | **Полностью не подтверждён** | all_definitions проходит всю схему и optional modes, но часть образцов построена по общей metadata парсера; это не native C++ oracle каждого типа. Независимый запуск оригинального TL writer на всех типах ещё нужен. |
| tg-runtime | Переносимая часть да; есть partial | 56 tests и workspace checks; остаются reactive bridges и lifetime потока, см. список. |
| tg-codegen common/style/lang/emoji | Переносимая обработка и Rust-выдача да; графика отложена | Все настоящие входы разобраны, вся выдача компилируется; native эталоны таблиц/значений совпали, 33 теста. Image backend/sprites — этап 5. |
| Обязательные проверки на едином Rust | Да | Rust 1.95.0 закреплён для nix-shell/nix develop/CI; все три команды exit 0, 457 тестов. |

**Этап 1 нельзя объявить безусловно закрытым целиком:** переносимые части и их
проверки готовы, но строгий native TL criterion для каждого типа не доказан,
а отложенные файлы остаются в реестре. Следующий MTProto этап в этой работе
не начинался. Ниже полный список всех файлов с stage=1 и status=partial/todo
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
