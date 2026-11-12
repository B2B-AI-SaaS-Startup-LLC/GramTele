# ayu-core: настройки и база AyuGram

Порт `SourceFiles/ayu/{ayu_settings,ayu_state,data/ayu_database,data/entities,
data/messages_storage}` из AyuGram Desktop 7.2.9, commit
`0f2a31bb420de98f331990432265b131b8d836b5`. Авторы оригинала — Radolyn и
участники AyuGram. [Карта функций и настроек](docs/ayu-data-map.md).

Реализованы **115 сохраняемых конечных настроек**: 88 корневых, 14 ghost,
13 снимка сообщения. Два контейнера добавляют ещё два определения ключей JSON.
В PLAN.md указано 124; дополнительно в коде есть 8 старых корневых ghost-ключей
для миграции и вычисляемое ghostModeActive, но это не дополнительные сохраняемые
конечные настройки. Все 115 значений проверены изменёнными относительно defaults.

Реализованы восемь таблиц `ayudata.db`, обе явные индексации и миграция V1.
SQLite из вложенного `libs/sqlite` не копируется: используется rusqlite.
Никакие пользовательские файлы не открываются автоматически. `Default` — модель
в памяти; для сохранения задаётся путь через `with_path` / `initialize`.

```rust,no_run
use ayu_core::{ayu_settings::AyuSettings, data::ayu_database::AyuDatabase};
# fn run() -> Result<(), Box<dyn std::error::Error>> {
let mut settings = AyuSettings::with_path("/explicit/synthetic/tdata/ayu_settings.json");
// load возвращает диагностику; уже выполненные присваивания сохраняются,
// как при пойманном исключении C++.
let _diagnostic = settings.load(false, Some(42));
settings.set_use_global_ghost_mode(false)?;
settings.set_send_read_messages(42, false)?;
let lifetime = tg_rpl::Lifetime::new();
settings.ghost(42).send_read_messages_value()
    .on_next(|enabled| println!("read messages: {enabled}"), &lifetime);
let database = AyuDatabase::initialize("/explicit/synthetic/tdata/ayudata.db")?;
assert!(!database.has_deleted_messages(42, -100, 0));
# Ok(()) }
```

| C++ | Rust |
|---|---|
| `AyuSettings::getInstance().saveDeletedMessages()` | `settings.save_deleted_messages()` |
| `setSaveDeletedMessages(false)` | `settings.set_save_deleted_messages(false)?` |
| `AyuSettings::ghost(session).sendReadMessages()` | `settings.ghost(extracted_user_id).send_read_messages()` |
| `ghost(session).setSendReadMessages(false)` | `settings.set_send_read_messages(extracted_user_id, false)?` |
| `sendReadMessagesValue() \| rpl::on_next(..., lifetime)` | `.send_read_messages_value().on_next(..., &lifetime)` |
| `setGhostModeEnabled(true)` | `settings.set_ghost_mode_enabled(user_id, true, active_session_id)?` |
| `messageShotSettings().setEmbeddedTheme(type, accent)` | `settings.set_embedded_theme(kind, accent)?` |
| `AyuDatabase::getDeletedMessages(user, dialog, topic, min, max, limit, query)` | `database.get_deleted_messages(user, dialog, topic, min, max, limit, query)` |
| `AyuMessages::map(item, result)` | `messages_storage::map(extracted_values, unix_time)` |
| `RegexFilter::dialogId == std::nullopt` | `filter.dialog_id == None` |
| `std::vector<char>` | `Vec<u8>` с теми же байтами |
| `std::string` в SQL | `SqlText(Vec<u8>)`, включая невалидный UTF-8 |

Настройки и tg-rpl остаются на главном потоке. Единственного владельца передаёт
будущий Core; глобального mutable singleton нет. Сеттеры ghost/shot проходят
через владельца настроек, чтобы сохранить файл без скрытых глобальных ссылок.
`reset` сохраняет подписчиков существующих variable. `shadowBanIds` — обычное
множество, без rpl producer, как в оригинале. Активность ghost — distinct поток
выражения пяти пар значений/блокировок.

Побочные эффекты UI выдаются через `effects()` в исходном порядке: смена значения,
эффекты, save; у `setGhostModeEnabled` markAsOnline идёт **после** save.
`with_defaults(edited_mark, native_translation_available)` принимает уже полученные
от ядра локализованную строку и доступность системного переводчика.
Main::Session/HistoryItem/PeerData не подменены фиктивными типами: передаются ID,
флаги, текст, сериализованные entities и даты. При подключении mapper к Core нужно
сохранить оба вызова времени оригинала: fallback editDate извлекается отдельно,
entityCreateDate передаётся как unix_time.

Соединение SQLite живёт до последнего клона `AyuDatabase`; `Arc<Mutex<Connection>>`
сериализует операции этого владельца, как databaseMutex/open_forever. Busy timeout
равен нулю, а не стандартным пяти секундам rusqlite. Диапазоны исключительные,
`topicId=0` — wildcard, сортировка правок по fakeId, удалённых — по messageId;
LIKE экранирует `%`, `_`, `\`. Удаление фильтра не удаляет его исключения.
Низкоуровневые insert/initialize возвращают Result; публичные query/mutation
методы сохраняют пустые/false/0 результаты C++ при ошибках. Диагностика Rust
отличается от Qt LOG, ошибка save дополнительно возвращается вызывающему коду.

## Нативный эталон

[reference-tools](tests/reference-tools) собирается отдельным CMake **вне клона**.
`database.cpp` включает настоящий `ayu_database.cpp`, его sqlite_orm, сущности,
миграции и запросы. Подменены только LOG и источник времени (1700000000).
Settings harness берёт настоящие заголовки (меняется только private/public в
копии каталога сборки) и неизменённые тела конструкторов, JSON-конвертеров,
load/save/validate/ghost(userId). Core::IsAppLaunched=false, Native-переводчик
недоступен; UI не линкуется. `ghost()` в headless-обвязке выбирает аккаунт 0.
Это проверка формата, не запуск графического приложения.

Проверено исполнением оригинала:

- defaults и все non-default значения JSON: Rust пишет те же байты, включая
  четыре пробела, Unicode, escaping, числа и отсутствие последнего LF;
- ошибочные типы, enum fallback, неизвестные ключи, частичные присваивания,
  legacy theme и аккаунтные ID; отдельные load-образцы с BOM, мусором после
  первого JSON, повреждением, migration и cGhost;
- Grisu2: фиксированная последовательность double, включая граничные bit patterns;
- все восемь таблиц, имена/порядок столбцов, NULL, два индекса, sqlite_master,
  строки Unicode/пустые строки и blobs с NUL/0x80/0xff;
- sync_schema: лишний/недостающий nullable/required столбец, изменение NOT NULL,
  игнорируемое изменение declared type, V0, будущая/переполненная версия,
  отсутствие строки версии с id=1;
- Rust читает C++-файлы; C++ читает Rust-файлы. Полные ORM dumps всех строк
  сравниваются в обе стороны. Для JSON сравнивается повторное чтение C++.

Своими тестами: каждый typed getter/value/changes и setter, исключения из условий
SQL, literal LIKE, replace/update/delete, ошибки/rollback, повреждённая БД,
параллельные клоны владельца, ghost locks, эффекты и lifetime/reset.
Оконные эффекты, Main::Session и платформенный Native translate запуском здесь
не проверены. Защита от всех возможных OS I/O failures не заявляется.

Для повторения нужен Nix shell с cmake, ninja, Qt6 Core, microsoft-gsl, range-v3,
sqlite, python3, pkg-config и C++ compiler из закреплённого nixpkgs:

```sh
cmake -S crates/ayu-core/tests/reference-tools -B /tmp/ayu-native-fresh -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DSQLITE_LIBRARY="$(pkg-config --variable=libdir sqlite3)/libsqlite3.so"
cmake --build /tmp/ayu-native-fresh -j2
mkdir -p /tmp/ayu-original-fresh
/tmp/ayu-native-fresh/native_ayu_database create /tmp/ayu-original-fresh
/tmp/ayu-native-fresh/native_ayu_settings default /tmp/ayu-default-fresh.json
/tmp/ayu-native-fresh/native_ayu_settings changed /tmp/ayu-changed-fresh.json
# Выбрать новый каталог, созданный только для синтетического эталона:
AYUGRAM_AYU_RUST_REFERENCE_DIR=/tmp/ayu-rust-fresh cargo test -p ayu-core native_
/tmp/ayu-native-fresh/native_ayu_database read /tmp/ayu-rust-fresh
/tmp/ayu-native-fresh/native_ayu_settings read /tmp/ayu-rust-fresh/settings.json
```

Инструменты и результаты запуска — в [verification](docs/verification).
Нативная сборка не нужна для обычного cargo test: образцы включены в репозиторий.

## Особенности оригинала и отличия

- sync_schema сравнивает NOT NULL/default/PK/generated, **не declared type и не
  порядок существующих столбцов**. Это сохранено; совместимый старый файл не
  переписывается ради косметической канонизации. V1 удаляет старые regex-фильтры.
- При отсутствии SchemaVersion(id=1), но наличии других ID, sqlite_orm insert
  исключает integer PK и создаёт следующий rowid; UPDATE id=1 ничего не меняет.
  Такое поведение оригинала сохранено и проверено нативным образцом.
- sqlite_orm читает std::string как C-string и обрезает внутренний NUL. Сохранено:
  raw байты БД при чтении не меняются, но повторная запись такой текстовой строки
  теряет хвост, как в C++. Это ограничение требования «без потерь» у оригинала;
  двоичные поля Vec<char> читаются полностью. SqlText позволяет не терять UTF-8-байты.
- postAuthor/replyMarkupSerialized присутствуют в сущности, но отсутствуют в ORM
  обеих таблиц сообщений. Порт не добавляет столбцы и не сохраняет эти поля.
- Порядок shadowBanIds воспроизводит Linux libstdc++ x86_64, включая reserve и
  rehash. C++ не задаёт переносимый порядок unordered_set: побайтовая гарантия
  для иного стандартного C++ runtime не заявляется.
- RegexFilter::toJson в C++ читает id.data() до NUL без ограничения длиной vector.
  Rust ограничен длиной: поведение без завершающего NUL определено безопасно,
  а за пределами vector у C++ нет определённого эталона.
- Rust строки UTF-8; несуществующие Qt-объекты не моделируются. Нефинитный double
  в typed setter отвергается явно (C++ мог сохранить null и затем отвергнуть его
  при load). Чтение null и всех проверенных допустимых чисел совпадает.

## Отложено

| Файл | Этап | Что нужно |
|---|---|---|
| ayu_settings.cpp/.h | 4, 5 | Владелец Core, получение активного аккаунта/локализованного edited, применение Effect к filters/translator/tray/worker/window |
| ayu_state.cpp/.h | 4 | Извлечение ID из HistoryItem, привязка токена Main::Session к отключению ghost |
| data/messages_storage.cpp/.h | 4 | Извлечение значений из HistoryItem/PeerData и реальный mapper flags/entities; форматный слой уже готов |

rusqlite/serde заменяют sqlite_orm/nlohmann в production. Вложенные sqlite и json
остаются зависимостями только нативного reference harness; unsafe в ayu-core запрещён.

## Зависимости

| Крейт | Версия | Роль |
|---|---|---|
| rusqlite | 0.42.0 | SQLite API, bundled |
| libsqlite3-sys | 0.40.0 | транзитивная FFI/SQLite |
| serde | 1.0.229 | сериализация значений |
| serde_json | 1.0.154 | JSON, float_roundtrip |
| tg-base | 0.0.0 | существующий workspace: unixtime и журнал |
| tg-rpl | 0.0.0 | существующий workspace: variable/producer/lifetime |
