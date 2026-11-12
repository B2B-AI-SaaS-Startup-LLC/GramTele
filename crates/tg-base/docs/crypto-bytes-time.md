# Байты, криптография, ZIP и время

Исходник: AyuGram Desktop 7.2.9,
`0f2a31bb420de98f331990432265b131b8d836b5`, каталог
`Telegram/lib_base/base`. Оригинал не изменён и не собирался.
Rust-код этой части находится в отдельных одноимённых модулях `tg-base`.
В крейте запрещён `unsafe`. Внешние safe API могут использовать FFI внутри
своих реализаций (getrandom, nix, file-guard, restart-manager).

## Решение по каждому файлу

| Файл оригинала | Решение и предел проверки |
|---|---|
| `bytes.h/.cpp` | Срезы/Vec/массивы; copy, memmove через copy_within, сравнение по размеру, concatenate, заполнение случайными байтами. Native POD-представление — bytemuck. |
| `openssl_help.h` | BigNum, SHA-1/256/512, части SHA, запись SHA в буфер, PBKDF2-HMAC-SHA512, HMAC-SHA256. BN_CTX заменён управлением памятью внутри RustCrypto. raw/takeRaw — FFI-интерфейс OpenSSL, в Rust отсутствует; его потребитель RSA ещё не портирован. |
| `zlib_help.h` | In-memory ZIP reader/writer с кодами ошибок minizip; stored и deflate, CRC, имена/extra/comment, лимит распакованного размера, чтение ZIP64. Статус partial: ZIP64 writer (>4 GiB/65534 записей) не реализован; deflate не обещает байтовый паритет zlib. |
| `crc32hash.h/.cpp` | Табличный CRC-32/ISO-HDLC; результат i32, та же интерпретация старшего бита. |
| `random.h/.cpp` | getrandom ОС, POD-значения, исходный rejection sampling и BufferedRandom с копированием в RandomIndex. Статус partial: RandomAddSeed явно отбрасывает вход, поскольку у ОС нет RAND_seed; собственного DRBG не добавлено. |
| `unixtime.h/.cpp` | Атомарные сдвиги серверного/HTTP-времени, первый значимый update, force, parse/serialize, генератор ID, updates через tg-runtime и tg-rpl. |
| `parse_helper.h/.cpp` | Побайтовый stripComments с исходными особенностями, skipWhitespaces и readName. |
| `qthelp_url.h/.cpp` | Percent encoding/decoding, разбор параметров, протоколы, проверка URL, доменные выражения; отрицательный lookbehind вынесен в проверку предыдущего символа. |
| `qthelp_regex.h` | Перевод поддерживаемых выражений Qt в regex, флаги и группы. Статус partial: произвольный PCRE не поддерживается, см. ниже. |
| `base_file_utilities.h/.cpp` | Очистка имён, включая Windows .lnk/.scf и CON/COM/LPT. Статус partial: RegisterBundledResources (Qt .rcc) отложен до генерации ресурсов/графики; замены, выдающей фиктивный успех, нет. |
| `file_lock.h`, `file_lock_posix.cpp` | POSIX fcntl record lock [0,4), поиск PID владельца, SIGTERM/после 50 попыток SIGKILL, unlock при drop. Проверены двумя настоящими процессами. |
| `file_lock_win.cpp` | cfg(windows), file-guard LockFileEx с немедленным отказом, Restart Manager через restart-manager, без unsafe в порте. Статус partial до сборки и проверки Windows; в Linux эта ветка не компилировалась и не запускалась. |

Старые заготовки перечитаны, а не приняты как готовые. Добавлен пропущенный
memmove. Исправлены тесты времени (общее состояние) и блокировки: ребёнка
нужно reap-ить параллельно, иначе оригинальный KillProcess ждёт ESRCH у zombie
бесконечно. Этот дефект оригинального алгоритма не изменялся.

## Примитивы и версии

Версии фактически разрешены Cargo при проверках; `Cargo.lock` не коммитится.
Ограничения диапазонов заданы в Cargo.toml, поэтому последующее разрешение
зависимостей может выбрать более новый совместимый patch.

| Роль | Крейт и проверенная версия |
|---|---|
| Все большие числа, знаковая оболочка, модульная арифметика | crypto-bigint **0.7.11**, alloc + zeroize |
| Проверка простоты | crypto-primes **0.7.2**, тот же BoxedUint; второго bigint-крейта нет |
| SHA-1 | sha1 **0.11.0** |
| SHA-256, SHA-512 | sha2 **0.11.1** |
| HMAC | hmac **0.13.0** |
| PBKDF2 | pbkdf2 **0.13.0** |
| Сжатие ZIP/DEFLATE | flate2 **1.1.11**, default-features=false, rust_backend; miniz_oxide **0.9.1** |
| Криптографическая случайность ОС | getrandom **0.4.3**, sys_rng |
| Байтовые POD-представления | bytemuck **1.25.2** |
| Очистка BigNum при Drop | zeroize **1.9.1** |
| Регулярные выражения | regex **1.13.1** |
| POSIX locks/signals | nix **0.31.3**, fs + signal |
| Windows byte-range lock | file-guard **0.2.0** |
| Windows Restart Manager | restart-manager **0.1.0** |
| Проверки AES-блока, CTR, IGE (dev-dependencies) | aes **0.9.4**, ctr **0.10.1**, ige **0.2.1** |
| Запись эталонных hex-векторов | hex-literal **1.1.0** (dev) |

AES/RSA не являются функциями openssl_help.h: этот заголовок также включает
OpenSSL C-заголовки, а прикладные вызовы находятся в других каталогах.
AES здесь проверен как выбранный примитив, прикладные обёртки не переносились.

## Как писали в C++ → как писать в Rust

Имена в примерах соответствуют реальным местам клиента; это карта перевода,
а не утверждение, что сами указанные файлы клиента уже портированы.

| C++ и место | Rust |
|---|---|
| `bytes::make_span(buffer)` | `&buffer[..]` / `&mut buffer[..]`; для POD `bytemuck::cast_slice(&buffer)` |
| `bytes::object_as_span(&nonce)` | `bytemuck::bytes_of(&nonce)` (native endian, только Pod) |
| `bytes::copy(key, openssl::Sha256(payload))`, `mtproto/connection_tcp.cpp` | `bytes::copy(&mut key, &openssl_help::sha256(&payload))` |
| `bytes::move(_full, _part)`, `lib_storage/.../storage_cache_binlog_reader.cpp` | `bytes::move_bytes(&mut full, part_start..part_end, 0)` |
| `BigNum(primeBytes)` / `getBytes()`, `mtproto_dh_utils.cpp` | `BigNum::from_bytes(prime_bytes)` / `n.get_bytes()`; big endian, минимальная длина |
| `BigNum::ModExp(g, BigNum(randomPower), prime)` | `BigNum::mod_exp(&g, &BigNum::from_bytes(&random_power), &prime)` |
| `BigNum(prime).subWord(1).divWord(2).isPrime(context)` | `let mut q = prime.clone(); q.sub_word(1).div_word(2, None); q.is_prime()` |
| `BigNum::ModSub(B, kg_x, p, context)`, `core_cloud_password.cpp` | `BigNum::mod_sub(&b, &kg_x, &p)` |
| `Sha256(salt1, password, salt1)` | `sha256_parts(&[salt1, password, salt1])` |
| `Pbkdf2Sha512(hash2, salt1, kIterations)` | `pbkdf2_sha512(&hash2, salt1, iterations)` (64 байта) |
| `Sha512(s, passcode, s)`, `storage_file_utilities.cpp` | `sha512_parts(&[s, passcode, s])` |
| `PKCS5_PBKDF2_HMAC(..., EVP_sha512(), key.size(), ...)`, там же | `pbkdf2::pbkdf2_hmac::<sha2::Sha512>(hash, salt, iterations, &mut key)`; key здесь **256** байт, не обёртка на 64 |
| `PKCS5_PBKDF2_HMAC_SHA1(...)`, legacy local key | `pbkdf2::pbkdf2_hmac::<sha1::Sha1>(passcode, salt, iterations, &mut key)` |
| `HmacSha256(key, fulldata)`, `mtproto_tls_socket.cpp` | `hmac_sha256(key, fulldata)` |
| `base::RandomIndex(count, buffered)` | `random::random_index_buffered(count, &buffered)`; исходное копирование сохранено |
| `unixtime::update(serverTime, force)` | `unixtime::update(server_time, force)`; уведомление доставляется через очередь главного потока |
| `unixtime::updates()` | `unixtime::updates()`; подписка с главного потока после init_main_queue |
| `unixtime::parse(id)` / `serialize(date)` | `parse(id)` → `Option<SystemTime>` / `serialize(date)` |
| `zlib::FileToRead(content).readFileContent(...)` | `let mut zip = FileToRead::new(content); zip.read_file_content(name, CASE_INSENSITIVE, limit)` |
| `FileToWrite::openNewFile(...); writeInFile(...); closeFile(); close()` | `open_new_file(...); write_in_file(...); close_file(); close()`; result после close |
| `qthelp::regex_match(pattern, subject, options)` | `qthelp_regex::regex_match(pattern, subject, options)`; offsets в UTF-8 байтах |
| `match->captured(name)` | `matched.captured_name(name)` |
| `FileNameFromUserString(name)` | `base_file_utilities::file_name_from_user_string(name)` |
| `FileLock::lock(file, mode)` | `lock.lock(&mut Option<File>, path, &OpenOptions)`; файл и guard сохраняются до окончания использования |

## Большие числа, порядок байтов и постоянное время

Проверены вызовы в `SourceFiles/mtproto`, `storage`, `core`, `passport`,
`calls`, а также `lib_storage`.

- `BN_bin2bn` принимает беззнаковую big-endian величину. Вход любой длины,
  ведущие нули допустимы, пустой вход равен нулю. `get_bytes()` возвращает
  минимальную big-endian абсолютную величину; ноль и failed дают пустой Vec.
  Знак существует отдельно и не сериализуется, как у BN_bn2bin.
- DH (`mtproto_dh_utils.cpp`) проверяет 2048 бит p, 1984 бита обеих границ
  modexp и не более 256 байт. Секретный показатель — 256 случайных байт.
  CreateAuthKey возвращает минимальную величину; дополнение при создании
  AuthKey находится за границей данного модуля.
- SRP (`core_cloud_password.cpp`) дополняет p, g, A, B, S **слева нулями до
  256 байт**, перед хешированием. Это обязанность NumBytesForHash, не BigNum.
  Вход больше 256 байт нарушает precondition исходных span-операций.
- RSA (`mtproto_rsa_public_key.cpp`) передаёт 256 байт без дополнения,
  возвращает блок ширины модуля. Raw RSA должен отклонять m >= n и неверную
  длину; простое mod_exp этого намеренно не делает. OAEP — отдельный вызов.
- Для нечётного модуля используется BoxedMontyForm::pow, с числом шагов по
  публичной precision показателя, без bit_width_vartime. Это не лестница:
  реализация crate использует свой оконный Montgomery алгоритм.
  Для чётного модуля — оба результата каждого шага и CtSelect, цикл также
  по precision. Ранние отказы зависят от failed/знака/нулевого модуля.
- Весь BigNum не объявляется constant-time: сравнения, сериализация,
  проверка простоты, знаковая арифметика и ветвление по модулю имеют другой
  контракт. Проверки времени/BINSEC здесь не запускались; отсутствие
  ветвлений по битам показателя проверено чтением кода и API RustCrypto.
- BigNum при Drop очищает magnitude через zeroize, как BN_clear_free.
  BN_CTX, raw и takeRaw не моделируются OpenSSL-совместимыми указателями.
- isPrime применяет BPSW плюс 64 случайных Miller–Rabin раунда; исходный
  BN_is_prime_ex использует 64 раунда. Это явно отличающаяся реализация,
  не обещание идентичных случайных оснований/траектории для псевдопростых.

## ZIP, время и регулярные выражения

ZIP читает central/local headers, CRC, stored/deflate и ZIP64 размеры/EOCD.
Чтение проверено архивами **reference zlib/minizip 1.3.2** и CPython 3.12.14.
Полные байты stored writer сверены с minizip. Deflate miniz_oxide может
отличаться от zlib, оставаясь совместимым. Обнаружение text/internal_fa
приближает zlib по всему входу, а оригинал смотрит литералы первого блока;
это тоже может менять заголовок. Для произвольного deflated ZIP байтовый
паритет **не заявляется**. Writer ZIP64 и split archives не реализованы.
Чтение зашифрованных записей без пароля не поддерживается, как и у вызовов
исходного wrapper без password. Для многодисковых архивов возвращается ошибка.

Время: update без force применяется только один раз, если разница с текущим
сдвигом >=3 секунд; force может применять последующие значения. Все сдвиги
атомарны. HTTP shift/valid сбрасываются при принятом server update, даже
если итоговый сдвиг не изменился. Генератор ID использует Instant,
множитель 0xffff0000/1e9 и AtomicU32 +=4. Порядок/гонки между конкурентным
update и next сохранены; строгая глобальная монотонность при смене времени
не обещается. parse/serialize используют SystemTime вместо Qt QDateTime;
таймзона представления относится к будущему интерфейсу.

HTTP Date **парсится не в unixtime**: `mtproto/special_config_request.cpp`,
ParseHttpDate, выражение для `Wed, 10 Jul 2019 14:33:38 GMT`, таблица месяцев
Jan..Dec. Затем вызывается http_update(parsed.toSecsSinceEpoch()). Сам этот
каталог по заданию не портировался. Нельзя молча заменить его парсером,
принимающим другой набор HTTP-дат, при начале этапа 2.

Аудит PCRE: шаблоны qthelp::regex_match (local_url_handlers.cpp,
core/click_handler_types.cpp, core/application.cpp, ayu/ayu_url_handlers.cpp)
покрыты таблицей compile-тестов. В qthelp_url есть отрицательный lookbehind,
который эмулируется проверкой предыдущего символа и повторным поиском.
В клиенте также есть **прямые QRegularExpression** с lookahead/lookbehind:
`history/history_item.cpp`, `history/view/media/history_view_media.cpp`,
`ayu/ui/boxes/plugin_info_box.cpp`. Их нельзя переносить напрямую в regex.
Обратных ссылок в проверенных вызовах wrapper не обнаружено. regex не
поддерживает lookaround, backrefs, atomic groups, possessive quantifiers,
\\Z/\\G/\\K. Такие паттерны не компилируются; full PCRE-паритет не заявляется.
Дополнительно `$` в PCRE может совпасть перед конечным LF, а regex требует
конца; позиции групп возвращаются в байтах UTF-8 вместо кодовых единиц UTF-16.
Эти отличия — причина partial в реестре, а не признание «эквивалентной замены».

## Что проверено тестами

- SHA-1/256/512: abc и миллион a, ожидаемые FIPS значения; parts и To.
- HMAC-SHA1: RFC 2202 case 1; HMAC-SHA256: RFC 4231 cases 1, 2, 6,
  включая ключ длиннее блока хеша.
- PBKDF2-HMAC-SHA1: RFC 6070, c=1,2,4096. SHA512: password/salt c=1,2
  и Wycheproof tcId=3 (65 байт, два блока), включая wrapper на 64 байта.
- AES-128/192/256: FIPS 197; AES-256 CTR: четыре блока SP 800-38A,
  вызовы по семь байт проверяют состояние внутри блока. IGE: опубликованный
  OpenSSL-вектор 1 с AES-128. Это проверки примитивов, не MTProto-фрейминга.
- CRC32: ISO-HDLC check 123456789 и дополнительные фиксированные значения.
- BigNum: signed add/sub/mul/div, нормализация BE, сравнения, failed,
  mod add/sub/mul/inverse/gcd, нечётный/чётный модуль, нулевой показатель,
  RSA n=3233,e=17,d=2753,m=65 → c=2790; safe prime Telegram 2048 бит.
- Inflate: фиксированный поток reference zlib; чтение ZIP stored/deflated,
  minizip и ZIP64; CRC-ошибка, лимит размера, sticky errors; stored writer
  сравнивается побайтно с minizip, а не только круговым преобразованием.
- FileLock: второй процесс действительно держит fcntl-диапазон; виден его
  PID; lock другого процесса завершает его SIGTERM и захватывает диапазон.
- unixtime: сдвиги, HTTP validity, parse/serialize, ID; отдельный процесс
  проверяет worker update и доставку уведомления на главном потоке.
- Имя файла, байты/memmove, URL, percent decoding, regex и stripComments:
  фиксированные примеры и границы по прочитанному исходнику.

Только чтением проверены: call-site дополнения SRP/DH/RSA, свойства timing
алгоритмов RustCrypto, Windows API и Restart Manager, HTTP Date parser,
Qt Unicode/UTF-16 различия, альтернативные ветки исходника за пределами base.
Windows target не собирался и не запускался. Нет проверки открытия реальной
tdata, TLS handshake, SRP/MTProto или совместимости всего клиента: этап 2
не начинался. Ни пользовательские tdata, ни аккаунты не использовались.

## Что осталось в SourceFiles/mtproto и других каталогах

`mtproto_auth_key.cpp/.h`: AES-256 IGE/CTR, 32-байтовый IV IGE (две половины
в порядке OpenSSL), CTR ivec/ecount/num, derive key/IV для старого и нового
MTProto; len IGE кратен 16, CTR допускает любой хвост и сохраняет состояние.
`mtproto_dh_utils.cpp`: проверки DH и дополнение результата.
`mtproto_rsa_public_key.cpp`: PEM PKCS#1/SPKI, raw RSA, OAEP SHA1/MGF1,
проверки размера/диапазона и fingerprint (SHA1 TL-строк n/e).
`mtproto_dc_key_creator.cpp`: RSA_PAD, nonces, временные AES ключи, framing.
`mtproto_tls_socket.cpp`: X25519 keygen, raw public key 32 байта, fake TLS.
`special_config_request.cpp`: разбор HTTP Date.

В `storage` остались 256-байтовые PBKDF2 local keys, legacy SHA1 derivation,
tdata framing/шифрование/хеши; в passport AES-CBC с проверкой кратности 16,
в core RSA SHA1/Ed25519/ECDSA verification обновлений; calls использует DH
и SHA256 отпечатки. Для них ещё нужны соответствующие обёртки/FFI по плану;
эта работа их не заменяет.

## Источники эталонов и контрактов

- [FIPS 197](https://csrc.nist.gov/pubs/fips/197/final),
  [SP 800-38A](https://csrc.nist.gov/pubs/sp/800/38/a/final),
  [FIPS 180-4](https://csrc.nist.gov/pubs/fips/180-4/upd1/final).
- [RFC 2202](https://www.rfc-editor.org/rfc/rfc2202),
  [RFC 4231](https://www.rfc-editor.org/rfc/rfc4231),
  [RFC 6070](https://www.rfc-editor.org/rfc/rfc6070).
- [Wycheproof PBKDF2-SHA512](https://github.com/C2SP/wycheproof/blob/main/testvectors_v1/pbkdf2_hmacsha512_test.json),
  [RustCrypto PBKDF2 tests](https://github.com/RustCrypto/KDFs/blob/master/pbkdf2/tests/pbkdf2.rs).
- [OpenSSL IGE vectors](https://www.links.org/files/openssl-ige.pdf).
- [crypto-bigint](https://docs.rs/crypto-bigint/0.7.11/crypto_bigint/),
  [Restart Manager safe API](https://docs.rs/restart-manager/0.1.0/restart_manager/).
- `tests/fixtures/generate.py` документирует генерацию несекретных ZIP/zlib
  образцов; сами образцы сохранены, Python/minizip для тестов не нужны.

При проверке заготовки также исправлено округление serialize до 1970 года:
Qt делит toMSecsSinceEpoch() на 1000 с усечением к нулю, а заготовка ошибочно
использовала floor. Теперь -1500 мс дают -1 секунду; это закреплено тестом.
Контракт сверён с [исходником Qt 6.8](https://code.qt.io/cgit/qt/qtbase.git/tree/src/corelib/time/qdatetime.cpp?h=6.8).

Дополнение DH-ключа проверено также в `AuthKey::FillData`: если величина
короче 256 байт, нули ставятся слева, длиннее 256 — Assert. В `calls_call.cpp`
вызывается тот же FillData. `AuthKey::countKeyId` читает последние 8 байт SHA1
как native uint64 (на целевых little-endian системах — little endian);
этот идентификатор нельзя получать из digest как big-endian BigNum.

## Обязательные проверки после интеграции main

Команды выполнены в `ayugram-rs-wt/base-crypto` через Nix на Linux.

```text
cargo fmt --check
  exit 0, вывода нет
cargo clippy --workspace --all-targets -- -D warnings
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.45s
  exit 0
cargo test --workspace
  Finished `test` profile [unoptimized + debuginfo] target(s) in 4.60s
  349 passed; 0 failed (сумма unit/integration/doc тестов workspace)
  exit 0
```

Включены прежние тесты tg-base, tg-runtime, tg-rpl и TL. Полный журнал
последнего запуска: `/tmp/base-crypto-release-checks.log` (вне репозитория).
Файлы соседних worktree base-runtime/codegen не изменялись.
