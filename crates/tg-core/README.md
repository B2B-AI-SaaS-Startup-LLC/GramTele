# tg-core: ключи и нижний слой tdata

Срез от 13.11.2026 также включает независимые индексы истории и медиа:
SparseIdsList, SharedMedia, UserPhotos и Facade. Их алгоритмы и события
сверены с оригинальным C++ на 7031 операции; карта —
[docs/storage-lists-map.md](docs/storage-lists-map.md). Эти индексы живут
в памяти; сохранение данных чатов, настройки Telegram и подключение к
Main::Session остаются следующими срезами. Из CloudBlob перенесена только
распаковка ZIP; загрузчик и UI-описания состояния отложены.

Реализован первый срез этапа 3: TDF$ envelope, локальное шифрование, код-пароль,
key_datas, индексы аккаунтов, map envelope и MTP authorization. Настройки, данные
чатов и Storage::Cache не читаются. Это библиотека форматов; Main::Session/Core/UI
и запуск аккаунтных моделей не представлены заглушками. После чтения вызывающий
код передаёт настоящему Instance полученные Fields.

## Чтение и запись

```rust,ignore
use tg_core::storage::storage_domain::Domain;
use tg_mtproto::mtproto_dc_options::Environment;

let mut domain = Domain::new(path_to_synthetic_tdata, "data");
domain.start_modern(passcode_bytes)?;
for (index, authorization) in &domain.owner.accounts {
    let fields = authorization.fields(Environment::Test);
    // RuntimeInstance::new(Mode::Normal, fields, client_info, transport_policy)
    // После создания destroyer: authorization.apply_keys_to_destroy(&destroyer).
}
```

`start_modern` — явное чтение без автоматического reset. `start` сохраняет
оригинальный выбор modern/legacy/from-scratch и различает IncorrectPasscode и
IncorrectPasscodeLegacy. Как в исходнике, корректный ключ с повреждённым info
может привести в start к нарушению предусловия generateLocalKey; отдельный
тест сохраняет этот контракт как panic. Не считайте start универсальным
неразрушающим просмотрщиком повреждённого хранилища.

Account::read_map_with восстанавливает local key из legacy-карты, если ключ
ещё не задан; read_mtp_data понимает modern block 0x4b и legacy dbiUser/dbiKey.
Main authorization читает старый narrow user ID и современный wide tag,
главный DC, текущие ключи и ключи на уничтожение. После ошибки чтения уже
назначенные поля сохраняются, как у void-парсера C++; дополнительно возвращается
Result. Отрицательный count отвергается вместо опасного reserve(count).

Для записи используются FileWriteDescriptor/EncryptedDescriptor и WriteManager.
`finish(manager, sync)` явно передаёт запись заданному manager; Drop незавершённого
FileWriteDescriptor отправляет её глобальному AsyncWriteManager, как деструктор C++.
AsyncWriteManager работает на tg-runtime::ObjectOnThread, объединяет записи по
имени, sync/stop ставят синхронный барьер. Глобальные sync()/finish() соответствуют
Storage::details::Sync/Finish. finish завершает глобальный manager один раз.
Ошибки I/O доступны как Result; асинхронная запись сообщает их в stderr.

WriteManager пишет временный файл в том же каталоге, flush/sync_all и rename в
суффикс s; при неудаче используется 0. Имена временных файлов отличаются от Qt,
финальные байты совпадают. При наличии s legacy 0/1 не рассматриваются даже при
повреждении s. Без s выбирается более новый 0/1, с удалением альтернативы после
успешного чтения — это поведение исходника сохранено.

localPasscodeChanged возвращает tg-rpl::Producer с Lifetime-подпиской и синхронной
доставкой, как rpl::event_stream в оригинале. Domain остаётся на главном потоке;
фоновая очередь получает только готовые WriteEntry. Главная weak-задача списка
использует on_main_local; Rc/RefCell здесь один корневой контейнер, не граф аккаунтов.

## C++ → Rust

| Оригинал | Порт |
|---|---|
| ToFilePart(key) | to_file_part(key), младшая hex-тетрада первой |
| HashMd5 + TdfMagic | RustCrypto md-5 + TDF$ + LE AppVersion + digest |
| CreateLocalKey(pass, salt) | create_local_key, SHA512(salt+pass+salt), PBKDF2-SHA512 256 байт |
| CreateLegacyLocalKey | PBKDF2-SHA1, 4/4000 итераций |
| PrepareEncrypted / DecryptLocal | LE length, случайное дополнение, SHA1[0..16], AES-IGE v1 false |
| FileWriteDescriptor::~FileWriteDescriptor | Drop → глобальная фоновая очередь |
| EncryptedDescriptor::stream << bytes | descriptor.stream.bytes(bytes) |
| QDataStream Qt_5_1 | tg_base::qt_stream, единая реализация для tg-core и tg-mtproto |
| null / empty QByteArray | nullable_bytes(None) / nullable_bytes(Some(&[])) |
| QString с непарными UTF-16 | nullable_utf16(Some(&units)); String не навязывается |
| serializeMtpAuthorization | Authorization::serialize_mtp_authorization / from_instance |
| setMtpAuthorization | Authorization::set_mtp_authorization → fields(environment) |
| getKeysForWrite / addKeysForDestroy | from_instance / apply_keys_to_destroy |
| ComposeDataString("data", 2) | compose_data_string("data", 2) == "data#3" |
| Domain::writeAccounts | count + индексы в порядке владельца + active index |
| writeMapDelayed / writeMapQueued | AccountWrites, TimerHost и главная очередь |
| Main::Domain::scheduleWriteAccounts | weak-задача tg-runtime, объединение записей |
| Storage::FileLock | переэкспорт готового tg_base::file_lock, тот же диапазон [0,4) |

Writer/Reader используют BE для QDataStream и raw u16 для QString. Convenience
bytes()/string() по-прежнему сводят null к пустому значению; nullable_* сохраняют
различие. Ошибка потока выражается Option/Result и ранним возвратом вместо
неявного sticky QDataStream::Status. Поддерживается Qt_5_1 формат длины u32;
Qt6 extended lengths не являются форматом этого читателя.

## Проверка оригиналом

Отдельный CMake в /tmp, исходный клон только читался. Из него извлекаются тела
utility functions, включая writeNow/writeHeader/open/path, MD5 из core/utils.cpp,
Main::Account serializeMtpAuthorization/setMtpAuthorization и Storage::Domain
encryptLocalKey/startModern/writeAccounts. AuthKey/SHA/AES — настоящие исходные
файлы и OpenSSL. RAND_bytes заменён счётчиком, crl::now=0.

Четыре каталога в tests/fixtures/tdata: без пароля/с test-passcode, один/три
аккаунта. У каждого аккаунта ключи DC2/DC4 и ключ DC1 на уничтожение; local key,
соли и дополнение фиксированы. Проверено:

- оригинальный writer → оригинальный reader;
- оригинальный каталог → Rust: те же user/DC/все 256 байт каждого ключа;
- Rust writer при том же entropy → полностью те же имена и байты файлов;
- каталог Rust → оригинальный reader: те же аккаунты, ключи и активный индекс.

Дополнительный нативный образец qt-pbkdf.bin подтверждает null/empty QByteArray
и QString, непарные UTF-16, цвет, оба PBKDF на 256 байт, narrow authorization и
её повторную wide-сериализацию оригиналом. SHA256 всех fixtures — manifest.json.

Границы эталона существенны: Main owner/start callbacks заменены обвязкой;
полные Main::Session/Account, Storage::Account с Cache/settings и весь клиент не
собирались. Пустой map payload создаёт обвязка через оригинальный FileWriteDescriptor,
а не весь UI-зависимый writeMap. Async scheduling и OS fallback исходника в native
не выполнялись: fallback-заглушки аварийно завершают harness, если туда попасть.
Rust очереди/ошибки/suffix selection проверяются своими тестами. Для Config
используется уже проверенный на этапе 2 tg-mtproto; здесь добавлен тест encrypted
config envelope. Это не native-эталон целого приложения или живой desktop-сессии.

Карта: [docs/tdata-map.md](docs/tdata-map.md), 133 строки: 125 связаны с тестами,
8 Windows-only — замена готовым base и проверка чтением; 31 native overlap.
Windows LockFile/Restart Manager в Linux не выполнялись.

## Живые примеры

```bash
nix develop --command cargo run -p tg-mtproto --example test_dc_login
nix develop --command cargo run -p tg-core --example test_dc_tdata
```

Только тестовые DC, api_id 17349 и test api_hash из config.h. Телефон не
принимается снаружи: генерируется 999662YYYY, код — 2 длиной из sendCode.
SignUp — Port Test. QR: export → accept первым Instance → updateLoginToken →
повторный export → при необходимости setMainDcId/import (включая цепочку миграций).
После успеха users.getUsers(inputUserSelf), затем logout. FLOOD_WAIT не обходится.

Runtime.set_automatic_config(false) отключает фоновые help.getConfig в этих
примерах; обычный Runtime сохраняет прежнее поведение. Исполняемый тест проверяет
подавление до любого I/O и повторное включение загрузчика. В первых двух живых
попытках этот режим ещё не был добавлен: автоматический help.getConfig мог
выполняться; это лишний запрос относительно ограничения задачи.

Фактический запуск: первая попытка отвергла пустой device_model, пример исправлен
(Desktop/Linux). Затем номер 9996620387 получил sendCode length=5, но signIn
с 22222 вернул PHONE_CODE_INVALID. После отказа другие коды/номера не пробовались.
QR не дошёл до авторизации, users/getUsers/logout не потребовались: ни одного
Instance авторизовать не удалось. Нет вывода успешного QR или восстановления.
Логи — docs/verification/login-*.log. Новой ошибки автомата сессии/Instance
кроме ограничений драйвера этим запуском не подтверждено.

Сквозной test_dc_tdata пишет только собственный временный каталог с test Config,
запускает себя новым процессом, открывает ключи и вызывает users.getUsers без
sendCode/signIn, затем logout и удаляет каталог. Он компилируется, но живьём не
выполнен: отсутствует успешно авторизованный тестовый аккаунт, повторный подбор
после PHONE_CODE_INVALID не делался. Настоящая tdata пользователя не открывалась.

## Отложенное

| Файл/часть | Возврат | Что нужно |
|---|---|---|
| storage_account.*: применение и каноническая перезапись непустой карты | 3, следующая задача | модели settings/chat/self; сейчас известные records сохраняются непрозрачно |
| storage_account.*: остальные ReadSetting и readOldMtpData | 3, следующая задача | старые общие settings/config блоки и Local; auth dbiUser/dbiKey/0x4b готовы |
| storage_account.*: настройки, черновики, стикеры, кэш, cleanup | 3, следующая задача | Storage::Cache через согласованный интерфейс; tg-cache не менялся |
| main_account.cpp/.h вне authorization record | 4 (storage hooks — продолжение 3) | Main::Session/Core/UI lifecycle, подписки на сохранение |
| main_domain.cpp/.h вне account-list record | 4 (storage hooks — продолжение 3) | владельцы аккаунтов, окна/порядок/активация приложения |
| storage_file_lock_win.cpp | Windows-проверка | запуск LockFile/Restart Manager на Windows |
| Живой QR и восстановление tdata | 2/3 | успешная разрешённая авторизация тестового номера |

Полный пофункциональный список следующей задачи находится в карте. В ней нет
готовых заглушек Cache, Session или настроек. В отмеченном объёме механизм
код-пароля и файлов работает независимо от оконного слоя.

Новая внешняя зависимость — md-5 0.11.0 (RustCrypto). Остальное — существующие
tg-base/tg-mtproto/tg-rpl/tg-runtime, RustCrypto AES/IGE через AuthKey; для examples/tests
используются уже имеющиеся tg-scheme/tg-tl/tokio. Cargo.lock обновлён вместе с кодом.

## Итоговые проверки

Rust 1.95.0 (закреплённый Nix), 2026-11-13: fmt exit 0, clippy workspace/all-targets
с -D warnings exit 0, cargo test workspace exit 0: **595 passed, 0 failed,
3 ignored**. Новых обычных тестов 12 (11 tg-core и один режим драйвера MTProto).
Два ignored относятся к прежнему живому DH, третий — ручной synthetic exporter,
который был выполнен отдельно для обратного native-чтения. Полный вывод:
[docs/verification/workspace.log](docs/verification/workspace.log).

Повторная диагностика PHONE_CODE_INVALID: см. ../tg-mtproto/docs/login-diagnostics.
Восемь разрешённых попыток исчерпаны, причина не установлена. test_dc_tdata
теперь включает общий QR-путь после того же входа по коду; живой restore по-прежнему
не выполнен, так как signIn вернул PHONE_CODE_INVALID до записи временной tdata.
