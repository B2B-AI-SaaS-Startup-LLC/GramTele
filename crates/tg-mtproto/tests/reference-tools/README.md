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
  Вычисление публичной точки выполняет OpenSSL; Rust получает готовую точку как
  часть входа HelloEntropy. Fixture проверяет формат hello, а не реализацию X25519
  в порте. Сохраняются public point 32, u32 LE size, hello bytes, digest 32.
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
