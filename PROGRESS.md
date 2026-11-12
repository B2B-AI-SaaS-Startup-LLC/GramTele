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
