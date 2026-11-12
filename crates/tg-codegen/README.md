# tg-codegen: стили, строки и эмодзи

Порт 51 файла `Telegram/codegen/codegen/{common,style,lang,emoji}` из
AyuGram Desktop `0f2a31bb420de98f331990432265b131b8d836b5`. Обработка отделена
от Rust-выдачи; Qt, C++ compiler и оригинальный клон для обычной сборки не нужны.
Крейт не меняет tg-ui/tg-core: требования к их будущему API перечислены ниже.

## Вызов из build.rs

Добавить `tg-codegen` в build-dependencies. В примерах `resources` — каталог
с подкаталогами `SourceFiles`, `lib_ui`, `Resources` исходных ресурсов Telegram.
Функции возвращают текст и данные, не запускают компилятор и не ищут оригинальный
клон по абсолютному пути. Ошибка — `common::Error { file, line, code, message }`;
Display даёт `file(line): error N: message`.

```rust,ignore
let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
let includes = vec![resources.join("SourceFiles"), resources.join("lib_ui"),
                    resources.join("Resources")];
// inputs: список .style; includes рекурсивно разбираются с проверкой циклов.
let styles = tg_codegen::style::processor::process(&inputs, includes)?;
for (name, rust) in styles { std::fs::write(out.join(name), rust)?; }
// Добавить сгенерированные модули в общий parent module (palette, style_basic, ...).

let lang = tg_codegen::lang::processor::process(
    &std::fs::read(resources.join("Resources/langs/lang.strings"))?,
    "Resources/langs/lang.strings", &previous_tags, &previous_indices)?;
std::fs::write(out.join("lang_auto.rs"), lang.rust)?;
// Сохранить lang.tags/lang.indices для следующей генерации, если нужны стабильные slots.

let emoji = tg_codegen::emoji::processor::process(
    &std::fs::read(resources.join("lib_ui/emoji.txt"))?,
    &std::fs::read(resources.join("lib_ui/emoji_suggestions/emoji_autocomplete.json"))?,
    "emoji_autocomplete.json", &[])?;
std::fs::write(out.join("emoji.rs"), emoji)?;
```

`previous_tags`/`previous_indices` — явное состояние build, не скрытая глобальная
таблица. Пустые строки означают чистую генерацию. Для чтения без генерации есть
`lang::parse`, `style::Parser`, `emoji::data_read::read_data` и отдельные обработчики.
Build.rs должен выдать `cargo:rerun-if-changed` для входов, include-ов, иконок и state
файлов; список разрешённых style-модулей доступен в `Parser::cache`.

## CLI

```text
codegen_style -ISourceFiles -Ilib_ui -IResources -oOUT/styles -tOUT/styles/stamp FILE.style ...
codegen_style -Ilib_ui -oOUT/styles -tOUT/styles/palette lib_ui/ui/colors.palette
codegen_lang -oOUT Resources/langs/lang.strings
codegen_emoji -oOUT lib_ui/emoji.txt lib_ui/emoji_suggestions/emoji_autocomplete.json [old-emoji.txt ...]
```

Короткие параметры принимают и слитную, и раздельную запись, как оригинал.
`-w` принимается и не меняет cwd: оригинальный logging тоже лишь сохранял значение.
Style допускает несколько файлов, `-I`, `-o`, `-t`, `-w`, `--render-svg in out [size]`;
lang — один файл, `-o`, `-s`, `-w`, `--subsets-only`; emoji — `-o`, `--images mode`
и дополнительные старые таблицы. Выходы `.rs`; state `.tags`/`.indices` и stamp `1`
сохраняют текстовый формат. Неизменённые `.rs` не переписываются; stamp обновляется.

`--subsets-only` принят как устаревший для Rust режим: сообщает причину и не создаёт
C++ headers/cache. Это оптимизация компиляции C++ phrase-specializations, а не данные
языкового пакета; Rust выдача использует один Phrase и не сканирует C++ исходники.
`--render-svg` и непустой `--images` завершаются явной ошибкой 910, потому что графический
backend отложен до этапа 5. Готовые emoji_*.webp остаются ресурсами клиента.

## Настоящие входы и AyuGram

CMake: `lib_ui/cmake/generate_styles.cmake`, `generate_emoji.cmake`,
`cmake/generate_lang.cmake`, вызовы в `lib_ui/CMakeLists.txt`, `cmake/td_ui.cmake`
и `cmake/td_lang.cmake`. Проверены **все 76 .style**, `ui/colors.palette`,
**9 193 строки lang.strings**, emoji.txt и emoji_autocomplete.json.
AyuGram добавляет `ayu/ui/ayu_icons.style`, `ayu/ui/ayu_styles.style`,
`ayu/ui/settings/ayu_settings.style` и блок `// AyuGram keys generated` в lang.strings.
Они включены в тесты. Сохранена политика `getWideMultiplied` из generator.cpp:
msgMaxWidth/historyGroupWidthMax/msgServicePhotoWidth/maxMediaSize/maxWallPaperWidth/
maxWallPaperHeight — factor 1.0, maxGifSize/maxStickerSize/maxAnimatedEmojiSize — 0.6,
maxVideoMessageSize — 0.75. Она применяется после ConvertScale и не переопределяет alias.

Тестовые снимки настоящих входов и реально используемых иконок находятся в
`tests/fixtures/Telegram`; SHA256 каждого файла — `tests/fixtures/SHA256.json`.
Они не являются эталоном вывода. Отдельные native эталоны — `tests/fixtures/reference`.

## C++ → Rust на примерах клиента

| Оригинальная выдача / вход | Выдача Rust |
|---|---|
| `style::TextStyle` из `ui/basic.style` | `style_basic::TextStyle`, поля в исходном порядке |
| `st::normalFont` из `normalFont: font(fsize)` | `style_basic::normalFont() -> tg_ui::style::Font` |
| `st::fsize`, `13px` | `fsize() -> i32`, через `tg_ui::style::convert_scale(13)` |
| `st::windowBg` из colors.palette | `palette::windowBg()`, Color::palette(2) |
| `windowFgOver: windowFg` | alias и fallback index 3, без собственного нового RGB |
| `style::palette_data::Checksum()` | `palette::PALETTE_CHECKSUM: i32`, **-1755758547** |
| `internal::EnsureContrast` | `CONTRAST_PAIRS`, применяет будущий palette runtime |
| `AyuUiSettings::getWideMultiplied` | `tg_ui::style::wide_multiplied(value, factor)` |
| `style::point/size/margins` | Point/Size/Margins::new с ConvertScale каждого компонента |
| `FontFlags` | flags u8: bold/semibold=1, italic=2, underline=4 |
| `MonoIcon` / `icon("/menu/article_rich-24x24", color)` | IconSource: исходные bytes, modifiers, color, padding, size |
| `tr::lng_cancel` из lang.strings | `key_lng_cancel: Phrase`, числовой base сохраняется |
| `tr::ayu_AyuPreferences` | `key_ayu_AyuPreferences`, значение AyuGram Preferences |
| `lngtag_count`, `lt_count_short`, `lt_count_decimal` | TAG_COUNT и COUNT_MODES: index, index+1000, index+2000 |
| `{user}` в переводе | UTF-16 `[0x10,0x20,0x20+tag_index,0x10]` |
| `key#one`, `key#other` | шесть соседних slots `__plural0..5`: zero/one/two/few/many/other |
| `DefaultData`/`Offsets` | Entry.value: &'static [u16], Entry.index; holes сохраняются |
| `Ui::Emoji::Data` и IdData | EMOJI и EMOJI_INIT, ID как UTF-16, те же флаги и original+1 |
| `SectionData` | CATEGORIES, включая невидимую other-категорию |
| `FindIndex` с деревом if/switch | ID_MAP в обратном UTF-16 порядке; поиск делает будущий runtime |
| `:-)` → улыбающийся emoji | EMOTICONS: та же пара строки и индекса |
| `:like:`, `:dislike:`, `:hmm:`, `:party:` | добавлены в REPLACEMENTS как в replaces.cpp |
| ReplacementData/WordLengths/Init/Indices | REPLACEMENT_DATA/WORD_LENGTHS/REPLACEMENT_INIT/BY_CHAR |
| ReplacementChecksums | Replacement.checksum: CRC32 именно UTF-16LE bytes |

## Требования к будущим крейтам

`tg_ui::style`: Color::{palette(usize),named(&str),rgba([u8;4])}, PaletteColor
{name:&'static str,rgba:[u8;4],fallback:i32}; Point/Size::new(i32,i32),
Margins::new(i32,i32,i32,i32), Align::named(&str), Font::new(&str,i32,u8),
Icon::from_sources(Vec<IconSource>), convert_scale(i32)->i32 и
wide_multiplied(i32,f64)->i32. Все типы Clone и Default для агрегатов.
Color::named("windowFg") воспроизводит цвет неинициализированного поля;
Align default=topleft, font field defaults исходного Value: family="", size=13, flags=0.

IconSource: bytes/bytes_2x/bytes_3x: &'static [u8], format: "svg"/"png"/"size",
modifiers: Vec<&'static str>, color:Color, padding:[i32;4], size:[i32;2].
SVG/PNG payload копируется без изменения. Будущий backend обязан применять modifiers,
масштаб, padding и tint, поддержать SVG size override. Palette runtime должен загрузить
цвета в исходном порядке, применять fallback, затем CONTRAST_PAIRS; CRC считается
по исходным значениям **до** EnsureContrast.

`tg_core::lang`: Entry {key:&'static str,index:u16,value:&'static[u16],
tags:&'static[u16],plural_base:&'static str}; Phrase::new(index:u16,
tags:&'static[u16],plural:bool) — const fn. Будущий runtime реализует lookup,
подстановку тегов, выбор plural, текущую строку и producer через tg-rpl.
Это замена C++ phrase-specializations, а не готовая локализация приложения.

`tg_ui::emoji`: Emoji {id:&'static[u16],postfixed:bool,variated:bool,colored:bool};
Replacement {id:&'static[u16],replacement:&'static str,words:&'static[&'static str],checksum:u32}.
Runtime использует EMOJI_INIT для original/variation links, CATEGORIES, ID_MAP,
POSTFIX_REQUIRED, EMOTICONS и BY_CHAR для поиска с longest-prefix и postfix semantics.
Индексы цветов и порядок packed UTF-16 соответствуют исходнику.

Минимальные compile definitions находятся **только** в tests/support/contracts.rs;
это не runtime и они не проверяют рендеринг. cargo test запускает rustc для всей
выдачи настоящих входов с этими определениями. В tg-ui/tg-core заглушек нет.

## Диск, сеть и порядок

Palette checksum исходник строит из строки
`&transparent:{ 255, 255, 255, 0 }&white:{ 255, 255, 255, 255 }`, затем для каждой
переменной `&name:{ r, g, b, a }`. Пробелы и порядок сохранены; CRC берётся из
`tg_base::crc32hash`. В themes/window_theme.cpp CRC входит в проверку кэша тем,
поэтому изменение этой строки нарушило бы совместимость. Проверены все 588 цветов,
имена, индексы и fallback, не только итоговая CRC.

Lang::Instance::serialize/fillFromSerialized (SourceFiles/lang/lang_instance.cpp)
сохраняет пары QByteArray **key/value**, а cloud MTP langPackString/Pluralized/Deleted
передаёт **имена ключей**, не сгенерированные индексы. Ordinal key/tag numbers —
внутренние runtime indices; теги-последовательности обрабатываются при чтении текста.
State .indices/.tags — файлы сборки, не tdata. Старые slots сохраняются и при удалении
ключей; шесть plural slots остаются соседними. Этот вывод о диске/сети проверен чтением
клиентских call sites; сам lang runtime/tdata ещё не портирован.

Emoji data_old — две неизменённые таблицы packed UTF-16. Проверка истории и aliases
перенесена: в реальном наборе 3 736 emoji и 3 767 ID-map entries. Эти aliases нужны
для сохранённых старых идентификаторов. Сам читатель старых настроек/legacy numeric
key conversion находится вне codegen и относится к этапам 3/4/5. Эталон проверяет
порядок ID, flags, variation parents, sections, aliases и подсказки.

## Иконки и отложенная графика

Обычная SVG-ветвь оригинала проверяет QSvgRenderer::isValid, встраивает `SVG:` + bytes
и size override; рисование происходит во время работы клиента. PNG-ветвь читает 1x/2x/3x,
проверяет размеры, применяет invert/mirror/rotate/resize и компонует три изображения
в один PNG. Отдельный --render-svg рисует preview на чёрном фоне в PNG (512 по умолчанию).

Порт сохраняет исходные SVG и все три PNG вместе с параметрами. Компоновка PNG и
pixel transforms пока переданы контракту Icon::from_sources этапа 5; PNG-кодирование
и пиксели не объявляются совпавшими с QImage. Валидация SVG полноценным парсером
и --render-svg также отложены. tiny-skia задан PLAN.md как rasterizer, но он не
разбирает SVG. Варианты — согласовать usvg/resvg + tiny-skia либо внешний SVG backend
за FFI; новый SVG-крейт в этой задаче не выбран. Это явные partial участки generator
и parsed_file, а не рабочие image stubs.

Emoji sprites через QPainter/QFontDatabase/WebP отложены до 5: использовать уже готовые
Resources/emoji_*.webp. Они не нужны для генерации таблиц. Байты спрайтов не пересоздавались.

## Эталоны и границы проверки

Обычная CMake-конфигурация оригинала остановилась: в клоне нет cmake/version.cmake и
validate_special_target.cmake. После этого **собраны и запущены все три оригинальных
codegen_style/codegen_lang/codegen_emoji** из неизменённых .cpp/.h через отдельный
CMake в /tmp/ayugram-codegen-reference/standalone, Nix GCC 15.3.0, C++23, Qt 6.11.2.
Префиксный заголовок добавляет только стандартные/Qt includes и числовые typedefs,
заменяя build PCH. Не добавлялись замены алгоритмов или runtime stubs оригинала.
Клон остался чистым; сборка/выводы находятся вне него.

Сохранены native эталоны и SHA256 всех C++ outputs. extract.py извлекает числа из C++
массивов; два небольших dump executable сериализуют результат **оригинальных**
парсеров в JSON. Они не являются эталонной сборкой всего приложения.
В tests/reference-tools лежат CMake, prefix, dump helpers и extraction script.

Проверено эталоном оригинала:

- все 588 palette rows, индексы, fallback и CRC -1755758547;
- все 10 997 lang slots, 175 tags, .indices/.tags, 249 625 UTF-16 units и 10 998 offsets;
- все структуры, поля, значения, aliases/copies и inheritance status в 76 стилях;
- emoji IdData/Data/SectionData, variation parents, исторический ID map и emoticons;
- все 2 018 подсказок: UTF-16 payload, words/lengths, char indices, init data, CRC32;
- выбранные сообщения ошибок Qt JSON, включая разрешённый Qt неизвестный escape.

Своими тестами: малые положительные/ошибочные образцы, порядок, UTF-16, CRC-строка,
сохранение slots и holes, C++ дефект plural group в конце файла, CLI errors,
Ayu wide hooks, детерминированность текста и компиляция всей Rust-выдачи.

Только чтением: tdata/cloud-language call sites; будущая связь с tg-rpl/UI, SVG/PNG
renderer и sprites. Rust getter возвращает значение вместо ссылки C++ и читает scale
при вызове вместо C++ init-фазы; будущий runtime должен фиксировать scale/palette для
одной UI generation. Код не сохраняет исходное switch-tree устройство поиска: данные
те же, lookup будет в runtime.

Ошибки выдаются как fail-fast Result; CLI печатает Display, вместо C++ ostream chains.
Qt JSON reader хранит UTF-16 и сохраняет порядок QString, lone surrogates,
unknown escapes и предел 1024. Выбранные syntax diagnostics сняты с native Qt;
в отличие от serde_json::Value не теряются непредставимые в String code units.
Для безопасной рекурсии reader использует отдельный поток со stack 16 МиБ;
парсер не зависит от размера stack вызывающего build.rs. Сеть и desktop session
для генерации не нужны.

## Зависимости

Новая direct dependency tg-codegen: tg-base (workspace 0.0.0, CRC32).
Dev dependency: serde_json="1" (при проверке 1.0.154, тестовые эталоны).
Совместимость QJsonDocument потребовала собственного небольшого reader: serde_json::Value
не представляет lone UTF-16 surrogates, а исходный генератор принимает их.
std выполняет файловый ввод-вывод, форматирование Rust, ordered maps и CLI parsing.
Qt нужен лишь для необязательного воспроизведения native эталонов. Новых SVG/PNG/
WebP крейтов нет. Cargo.lock не добавляется по правилам задачи.

## Решение по каждому из 51 файлов

Пути ниже относительны Telegram/codegen/codegen. `todo` означает отложенный файл,
`partial` — конкретный остаток в последнем столбце; этап возврата отличен от stage=1
принадлежности исходного codegen в ledger. Заголовок и .cpp перечислены отдельно.

| Файл | Статус | Этап возврата | Решение / что требуется |
|---|---|---:|---|
| `common/basic_tokenized_file.cpp` | done | — | Токены, numeric merge, escapes, отложенная лексическая ошибка, file/line/code; малые tests и native style/lang IR. |
| `common/basic_tokenized_file.h` | done | — | Токены, numeric merge, escapes, отложенная лексическая ошибка, file/line/code; малые tests и native style/lang IR. |
| `common/checked_utf8_string.cpp` | done | — | str::from_utf8 + отказ от буквального U+FFFD, как CheckedUtf8String. |
| `common/checked_utf8_string.h` | done | — | str::from_utf8 + отказ от буквального U+FFFD, как CheckedUtf8String. |
| `common/clean_file.cpp` | done | — | C-style comments, CRLF, single-line descriptions и размер 10 МиБ; tested. |
| `common/clean_file.h` | done | — | C-style comments, CRLF, single-line descriptions и размер 10 МиБ; tested. |
| `common/clean_file_reader.h` | replaced | — | Заменён безопасными byte slices и индексами TokenizedFile; сырые указатели не нужны. |
| `common/const_utf8_string.h` | replaced | — | Заменён borrowed &[u8]/&str; ownership/lifetime проверяет Rust. |
| `common/cpp_file.cpp` | done | — | String-выдача Rust и common::cpp_file write/touch_timestamp; unchanged bytes не переписываются. |
| `common/cpp_file.h` | done | — | String-выдача Rust и common::cpp_file write/touch_timestamp; unchanged bytes не переписываются. |
| `common/logging.cpp` | done | — | Error/Display и eprintln: filename(line): error N; -w принят, cwd не меняется. |
| `common/logging.h` | done | — | Error/Display и eprintln: filename(line): error N; -w принят, cwd не меняется. |
| `emoji/data.cpp` | done | — | Тоны, двойные тоны, postfix, aliases и old checks; 3736 emoji/3767 map entries совпали с native. |
| `emoji/data.h` | done | — | Тоны, двойные тоны, postfix, aliases и old checks; 3736 emoji/3767 map entries совпали с native. |
| `emoji/data_old.cpp` | done | — | Две исторические таблицы packed UTF-16 перенесены без изменения; native old alias map проверен целиком. |
| `emoji/data_old.h` | done | — | Две исторические таблицы packed UTF-16 перенесены без изменения; native old alias map проверен целиком. |
| `emoji/data_read.cpp` | done | — | Разделы/категории, single/double-colored samples, packed surrogate IDs; native таблицы. |
| `emoji/data_read.h` | done | — | Разделы/категории, single/double-colored samples, packed surrogate IDs; native таблицы. |
| `emoji/generator.cpp` | partial | 5 | Все таблицы Rust сверены с native C++ arrays; writeImages/generateImage/QPainter/WebP sprites отложены, --images error 910. |
| `emoji/generator.h` | partial | 5 | Все таблицы Rust сверены с native C++ arrays; writeImages/generateImage/QPainter/WebP sprites отложены, --images error 910. |
| `emoji/main.cpp` | done | — | Тонкий bin codegen_emoji; Rust table output, stamp; графический режим явно отложен. |
| `emoji/options.cpp` | done | — | -o, --images, data/replaces/old paths, original defaults/errors. |
| `emoji/options.h` | done | — | -o, --images, data/replaces/old paths, original defaults/errors. |
| `emoji/replaces.cpp` | done | — | Replacements/конверсии/UTF-16 CRC совпали с native; Qt-compatible UTF-16 JSON, порядок QString, nesting 1024 и диагностика проверены. |
| `emoji/replaces.h` | done | — | Replacements/конверсии/UTF-16 CRC совпали с native; Qt-compatible UTF-16 JSON, порядок QString, nesting 1024 и диагностика проверены. |
| `lang/generator.cpp` | done | — | Rust Entry/Phrase/TAGS/COUNT_MODES; .tags/.indices state, holes и лимиты; native все значения/индексы. |
| `lang/generator.h` | done | — | Rust Entry/Phrase/TAGS/COUNT_MODES; .tags/.indices state, holes и лимиты; native все значения/индексы. |
| `lang/main.cpp` | done | — | Тонкий bin codegen_lang; читает/пишет state и stamp; tests. |
| `lang/options.cpp` | done | — | Один input, -o/-w/-s/--subsets-only; original defaults/errors. |
| `lang/options.h` | done | — | Один input, -o/-w/-s/--subsets-only; original defaults/errors. |
| `lang/parsed_file.cpp` | done | — | Ключи, теги, legacy numeric tags, 6 plural slots и исходный strict assertion; native DefaultData/Offsets. |
| `lang/parsed_file.h` | done | — | Ключи, теги, legacy numeric tags, 6 plural slots и исходный strict assertion; native DefaultData/Offsets. |
| `lang/processor.cpp` | done | — | Чистый input+state → Generated Rust+state; filesystem отделён в CLI. |
| `lang/processor.h` | done | — | Чистый input+state → Generated Rust+state; filesystem отделён в CLI. |
| `lang/subsets.cpp` | replaced | — | C++ scan/cache/headers не нужны одному Rust Phrase; --subsets-only принят как obsolete no-output mode с пояснением. |
| `lang/subsets.h` | replaced | — | C++ scan/cache/headers не нужны одному Rust Phrase; --subsets-only принят как obsolete no-output mode с пояснением. |
| `style/generator.cpp` | partial | 5 | Rust structs/values/palette, scale и Ayu wide policy; PNG composition/transforms и SVG validation требуют графического backend. |
| `style/generator.h` | partial | 5 | Rust structs/values/palette, scale и Ayu wide policy; PNG composition/transforms и SVG validation требуют графического backend. |
| `style/main.cpp` | done | — | Тонкий bin codegen_style; параметры оригинала, Rust outputs, явный deferred render-svg. |
| `style/module.cpp` | done | — | Порядок includes/structs/variables, поиск и запрет дубликатов; полный native IR 76 стилей. |
| `style/module.h` | done | — | Порядок includes/structs/variables, поиск и запрет дубликатов; полный native IR 76 стилей. |
| `style/options.cpp` | done | — | -I/-o/-t/-w/--render-svg и несколько inputs, оригинальные defaults/errors; CLI tests. |
| `style/options.h` | done | — | -I/-o/-t/-w/--render-svg и несколько inputs, оригинальные defaults/errors; CLI tests. |
| `style/parsed_file.cpp` | partial | 5 | Обработка всех реальных стилей и модификаторов проверена native IR; QImage GetModifier pixel transforms переданы будущему IconSource backend. |
| `style/parsed_file.h` | partial | 5 | Обработка всех реальных стилей и модификаторов проверена native IR; QImage GetModifier pixel transforms переданы будущему IconSource backend. |
| `style/processor.cpp` | done | — | Разбор include graph и детерминированная выдача .rs; все 77 модулей компилируются. |
| `style/processor.h` | done | — | Разбор include graph и детерминированная выдача .rs; все 77 модулей компилируются. |
| `style/render_svg.cpp` | todo | 5 | Отложен: SVG parser/backend ещё не выбран; tiny-skia — только rasterizer. --render-svg явно возвращает error 910. |
| `style/render_svg.h` | todo | 5 | Отложен: SVG parser/backend ещё не выбран; tiny-skia — только rasterizer. --render-svg явно возвращает error 910. |
| `style/structure_types.cpp` | done | — | Типизированные Value/Data/Field/Status; исходные defaults, копии и inheritance; native IR. |
| `style/structure_types.h` | done | — | Типизированные Value/Data/Field/Status; исходные defaults, копии и inheritance; native IR. |

## Проверки

На закреплённом Rust 1.95.0 выполнена одна команда из PORTING.md:

```text
cargo fmt --check                                      exit 0, пустой вывод
cargo clippy --workspace --all-targets -- -D warnings  exit 0, без Rust warnings
cargo test --workspace                                 exit 0
457 passed; 0 failed; 0 ignored
```

tg-codegen: **33 теста** (24 формата/ошибки/границы, 5 native reference,
3 настоящие input sets с компиляцией выдачи, 1 CLI с двумя независимыми запусками).
Дочерний helper single_instance соседнего крейта повторно в 457 не посчитан.
Локальный полный лог: /tmp/codegen-verified-checks.log.

## Время генерации и компиляции выдачи

Замер 2026-11-12, x86_64 Linux, закреплённый rustc/cargo 1.95.0 через Nix.
Три набора выполнялись последовательно (`--test-threads=1`), без других сборок.
В «генерацию» включены чтение настоящих входов, разбор/обработка и первый текст
выдачи; style включает также повторную генерацию каждого модуля для проверки
детерминированности. Compile — отдельный `rustc --edition=2021 --crate-type=lib`
всего результата с тестовыми contracts, без оптимизации и без tg-ui/tg-core runtime.
Это время текущей тестовой выдачи, не прогноз сборки будущего приложения.

| Набор | Разбор и генерация | Компиляция выдачи |
|---|---:|---:|
| Все 76 стилей + palette (77 модулей) | 8.711 с | 44.706 с |
| lang.strings (10 997 entries) | 1.395 с | 9.122 с |
| emoji.txt + suggestions (3 736 / 2 018) | 0.293 с | 2.682 с |

Команда: `nix develop --command cargo test -p tg-codegen --test real_inputs -- --nocapture --test-threads=1`.
Лог: /tmp/codegen-benchmark-final.log. Полная обязательная проверка — команда из PORTING.md.
