# Native MTProto reference

Исходник — read-only AyuGramDesktop 0f2a31bb420de98f331990432265b131b8d836b5.
Стенды собраны GCC 15.3 / Qt 6.11.2 / OpenSSL 3.6.5 на x86_64 Linux,
в `/tmp/ayugram-mtproto-reference`, вне клона оригинала. Сборка — только
нужных частей, не Telegram Desktop целиком.

```bash
nix-shell -p python3 ripgrep cmake ninja qt6.qtbase microsoft-gsl range-v3 openssl --run \
 'crates/tg-mtproto/tests/reference-tools/capture.sh /home/yarik/Projects/AyuGramDesktop /tmp/mtp-native /tmp/ayugram-tl-reference'
```

Третий аргумент — каталог native TL reference из tg-scheme; используются его
`gen/scheme.h` и уже скомпилированный `scheme.cpp.o`. Свежая сборка TL описана в
`crates/tg-scheme/tests/reference-tools/README.md`. Сохранённые fixture не требуют
C++, Qt, OpenSSL, Nix или доступа к оригиналу при обычном cargo test.

- `crypto.cpp`: неизменённые mtproto_auth_key.cpp, mtproto_rsa_public_key.cpp,
  mtproto_dc_options.cpp, mtproto_config.cpp. Logging отключён в prefix.hpp,
  неиспользуемые функции Integration удаляет linker GC; алгоритмы не подменены.
  `crypto.bin`: 8 bytes keyId; 4 × (32 key + 32 IV), v2 send/receive и v1
  send/receive; 8 bytes RSA fingerprint; 256 bytes RSA_NO_PADDING result;
  далее u32 LE length + Qt bytes для DC/options config, production/test.
- `ige.bin`: AES256-IGE ciphertext для plaintext 0..63, key 0..31, IV 32..63;
  вычислен исходным aesIgeEncryptRaw. Rust отдельно проверяет encrypt и decrypt.
- `storage-custom.bin`: для каждого environment два Config: все поля с разными
  значениями и строками с emoji, затем пять null QString. Методы C++ доступны
  через настоящий mutable ConfigFields (const_cast снимает только const view).
  Это проверяет порядок полей, BE числа и UTF-16, не только значения по умолчанию.
- `extract_tcp.py`: извлекает оригинальные Protocol versions 0/1/D,
  GoodStartNonce и prepareConnectionStartPrefix. Меняются имена внешней обвязки,
  не алгоритмы. Исходные bytes.cpp/random.cpp выполняются; RAND_bytes заменён
  linker hook на байты 1,2,3,... . Для каждой версии: u32 LE size, 64-byte
  prefix и первый зашифрованный frame, включая state после первых 64 байтов CTR.
- `extract_tls.py`: оригинальные PrepareClientHelloRules и Generator. Источники
  entropy/time фиксированы: RAND_bytes — счётчик, HTTP time=1700000000,
  permutation — reverse, X25519 keygen — OpenSSL с private bytes 1..32.
  Вычисление публичной точки выполняет OpenSSL; Rust проверяет эту точку отдельным вызовом x25519-dalek для private 1..32;
  default HelloEntropy теперь также вычисляет настоящий X25519. Fixture проверяет
  и формат hello, и совпадение публичной точки с OpenSSL. Сохраняются public point 32, u32 LE size, hello bytes, digest 32.
- `socks.cpp` и `connect.cpp`: тот же QTcpSocket/QNetworkProxy, который вызывает
  оригинальный TcpSocket, подключён к локальному тестовому QTcpServer. Сняты
  greeting, RFC1929 credentials, CONNECT и HTTP Basic challenge/retry. Интернет
  не используется. Первая HTTP CONNECT попытка у Qt без Authorization.
- `proxy.cpp`: включает оригинальный core_settings_proxy.cpp и вызывает его
  SerializeProxyData, проверяя null/empty QString. Он не портируется на этом
  этапе; нужен только как эталон настоящего формата proxy record.

Остальные fixtures: `true.gz` записан Python 3.12 gzip/zlib с mtime=0;
`test-server-rsa.bin` содержит n/e/d отдельного 2048-bit RSA ключа тестового
сервера, созданного OpenSSL genpkey. Это исключительно тестовые данные,
не ключ Telegram или пользователя. Формат: три u32 LE length + BE integer.

Границы: native RSA образец — public raw operation и fingerprint; OAEP не снят.
DC options native образцы — встроенные адреса; дополнительные фильтры, CDN
таблицы и испорченные данные проверены своими тестами лишь в перечисленном
объёме. Никакого утверждения о native паритете всех ветвей этих классов нет.
Сверка source/fixture SHA-256 — `tests/fixtures/manifest.json`.

## HTTP QNetworkAccessManager

`http.cpp` воспроизводит настройки original connection_http.cpp (ContentLength,
ContentType, manager.post) на локальном QTcpServer. Запуск native_http OUTPUT MODE,
MODE = sequential / concurrent / malformed. Host с ephemeral port нормализован
до example.com; формат OUTPUT — повторяющиеся u32 LE size + raw request.
`http-post.bin` — два последовательных POST по одному keep-alive соединению.
Результаты Qt 6.11.2: sequential 1 connection/2 requests/0 errors; concurrent
6 connections/8 requests/0 errors; malformed 8 connections/8 requests/8 errors
(Connection closed). Последний режим отвечает HTTP/1.0 с двойным пробелом, как
test DC. Эти результаты не означают запуск целого клиента с его сборкой Qt.

## Зашифрованный конверт SessionPrivate

`extract_session.py TELEGRAM_ROOT OUTPUT.cpp` извлекает sendSecureRequest и
prepareSecurePacket из оригинала без перевода алгоритма. Обвязка заменяет сетевой
sink записью packet, logging выключен, RAND_bytes — счётчик 0..255, crl::now=0.
Оригинальные SerializedRequest::addPadding/AuthKey/SHA256/AES-IGE компилируются и
выполняются. `native_session OUTPUT.bin`: outgoing и incoming records, каждый
u32 LE length + packet с transport prefix. Incoming строится отдельной обвязкой
через оригинальный prepareAES(false), а не вызовом handleReceived. Поэтому
эталон подтверждает исходящую функцию и входящие криптопримитивы, а не все
проверки исходного получателя. Параметры зафиксированы в README крейта.

Для target native_session нужны SCHEME_OBJECT и SCHEME_INCLUDE от native TL
build, как для native_tls; сгенерированный OUTPUT.cpp положить в REFERENCE_DIR/session.cpp.

`extract_session_receive.py TELEGRAM_ROOT OUTPUT.cpp` извлекает входную часть
handleReceived до `wasConnected`: original размер/auth_key_id, AES decrypt,
ConstTimeIsDifferent/SHA256, msg_len/padding/session/msg_id и badTime.
Network queue и restart заменены sink-обвязкой, DebugEnabled=false, unix=1700000000.
Метод исполняется для 19 входов: valid, auth_key_id, msg_key, session, msg_id low
0/2, невыравненная/длинная/переполненная длина, padding>1024, границы времени,
0..3 transport-padding words и truncated/oversized packet. Диспетчер RPC, salt mutation,
окно дубликатов и callbacks не заменяются этими образцами: они проверяются отдельно.
`native_session_receive OUTPUT.bin`: header[5] u32 LE = case, packet words,
accepted, badTime, decrypted words; затем packet и decrypted header+body.

Случай 18 хранит packet words=0 как компактное описание входа из 4 194 305 нулевых слов; C++ выполняет его целиком, Rust восстанавливает его перед проверкой. Это проверка верхней границы размера без 16 MiB нулей в fixture.

## Простой резервный конфиг

`extract_simple_config.py TELEGRAM_ROOT REFERENCE_DIR/simple_config.cpp`
извлекает целиком decryptSimpleConfig. Меняются имя класса, публичный ключ
(собственный тестовый) и добавляется диагностика перед return false. Алгоритм
RSA/AES-CBC/SHA256 и оригинальный TL reader остаются исходными. Оригинальный
TL writer независимо строит boxed help.configSimple. Обвязка генерирует RSA-2048,
подписывает payload private RSA NO_PADDING и проверяет исходным методом; Qt
writer повторно пишет тот же TL. Target `native_simple_config` собирается в
отдельном каталоге с теми же TELEGRAM_ROOT, REFERENCE_DIR, SCHEME_INCLUDE и
SCHEME_OBJECT, что native_tls. Нужны Qt Core/Gui/Network и OpenSSL в Nix.

```bash
cmake --build /tmp/ayugram-instance-reference --target native_simple_config
/tmp/ayugram-instance-reference/native_simple_config /tmp/simple-config.bin
```

Аргумент — выходной файл: три записи u32 LE length + bytes (public PEM,
base64 ciphertext, expected boxed TL). Приватный ключ не сохраняется. При каждом
создании берётся новый ключ, поэтому повторный запуск проверяет алгоритм, но
не воспроизводит байты старого fixture. Зафиксированный fixture и его SHA256 —
`tests/fixtures/simple-config.bin` и `.json`; Rust проверяет именно этот образец.
В этой задаче повторная сборка и запуск завершились с exit 0. Полный QObject
Instance и сетевые Firestore/RemoteConfig в этом эталоне не исполняются.
