# tg-mtproto: транспорт, ключи и автомат сессии

Перенос транспорта и автомата сессии этапа 2. Создание постоянного auth key проверено на
тестовом DC через TCP и ручной HTTP. Для fake TLS используется настоящий X25519
из x25519-dalek с подставляемой случайностью HelloEntropy. WEB требует браузерный
Transport. Session/SessionPrivate реализованы отдельно от Instance. Сам Instance,
sender и загрузчики остаются следующей задачей; живой help.getConfig здесь не выполнялся.

## Соединение и время

В C++ QObject сокета и соединения живут в потоке сессии. В Rust один владелец
держит mutable protocol state в задаче Tokio; AsyncRead + AsyncWrite — граница
потока байтов. TcpCodec, KeyExchange, DomainResolver и ResolvingConnection
не открывают сеть и не получают время самостоятельно. Их входы — байты,
Random и Time/Clock. Тесты дают duplex stream и виртуальные миллисекунды;
sleep и реальная сеть в обычных Rust tests не используются.

TcpConnection/Socks/CONNECT/TlsSocket — асинхронные адаптеры к такому потоку.
Protocol 0 — abridged, 1 — abridged с SHA256(source+secret), D — padded
intermediate. LE задан явно, даже когда C++ использовал reinterpret_cast
32-bit слов. Обфусцированный prefix отправляется вместе с первым frame;
CTR уже продвинут на 64 байта. Для fake TLS надо создавать TcpConnection
через `new_with_nonce_filter(..., |_| true)`: оригинальный TlsSocket принимает
любой nonce; TCP и WEB фильтруют запрещённые первые байты.

SocketEvent соответствует connected/disconnected/readyRead/error/syncTimeRequests;
Event соединения — receivedData/receivedSome/connected/disconnected/error/syncTimeRequest.
ConnectionState хранит очередь пакетов и событий. `receive_into_queue()` наполняет
очередь, `receive()` потребляет один пакет. Result сообщает сетевую ошибку;
владелец решает, когда отменять соединение. `dispatch_events_on_main` отправляет
пачку через FIFO tg-runtime. Обратный вызов исполняется после возврата в очередь,
а не синхронно внутри Qt signal. Удаление владельца/отмена future заменяет deleteLater.

HTTP реализован вручную поверх AsyncRead + AsyncWrite, POST `/api`, порт 80,
Content-Type `application/x-www-form-urlencoded`, тело без двух reserved TCP words.
`NetworkConnector` вызывает тот же `connect_to_host`, что TCP: SOCKS5 и Basic
HTTP CONNECT доступны без reqwest. `HttpConnector` подставляет duplex в тестах.
Клоны HttpConnection — handles одного manager: максимум шесть одновременно
выполняющихся запросов, idle keep-alive pool, pending count, need_http_wait и общая
отмена watch channel. Один POST занимает соединение до ответа, pipelining нет.
`probe()` выполняет оригинальный fake req_pq и фиксирует ping/connected.
Разбор HTTP допускает несколько пробелов в status, Content-Length, chunked/trailers,
промежуточные 1xx и тело до EOF; заголовки ограничены 64 KiB, тело — 64 MiB.
Поддержаны identity и gzip Content-Encoding; br/zstd/deflate дают явную ошибку.
Заголовки по умолчанию сняты с Qt 6.11.2 на Linux: Accept-Encoding
`zstd, br, gzip, deflate`, Accept-Language `en,*`, User-Agent `Mozilla/5.0`.
Это зависимые от сборки Qt/локали значения; `HttpHeaders` позволяет передать их
явно. Для полного автоматического HTTP content decoding нужны дополнительные
backend br/zstd (не выбирались). Живой Telegram отвечает без content encoding.
reqwest остался только у DNS-over-HTTPS; исключение записано в PLAN.md.

Нативный Qt harness `tests/reference-tools/http.cpp` подтвердил: два последовательных
POST используют одно keep-alive соединение; восемь одновременных POST используют
шесть соединений; нормальный HTTP/1.0 ответ принимается. С двойным пробелом после
HTTP/1.0 эта конкретная Qt 6.11.2 тоже сообщает `Connection closed` (8/8).
Это проверка QNetworkAccessManager с теми же настройками, а не запуск полного
клиента. Поведение иной версии/сборки Qt, системной proxy/locale, сетевых ошибок
и автоматических повторов после закрытия idle соединения не установлено запуском.
Семантика `http_wait` прочитана в connection_http.cpp: using=true,
need=true при пустом наборе outstanding requests; генерация http_wait — сессия.

Clock::now даёт monotonic_ms и synchronized unix_seconds; next_message_id
предоставляет ID (в RuntimeClock — готовый tg_base::unixtime::mtproto_msg_id,
включая перенастройку при сдвиге времени). update_unix_time
получает время после проверки SHA1 server_DH_inner_data. RuntimeClock соединяет
это с tg_runtime::now и tg_base::unixtime::update. Автомат не использует
Tokio sleep: DNS stagger и IP timeout — next_deadline/poll(now). Сессионные
restart/ack/ping timers реализованы в SessionPrivate поверх base::Timer. Существующего tg-runtime достаточно
для clocks, main dispatch и фоновых очередей; его blocking executor не используется
для асинхронного сетевого I/O. tg-runtime и tg-rpl не изменены.

## C++ → Rust (места из session_private.cpp)

| C++ | Rust |
|---|---|
| `BareDcId(_shiftedDcId)` | `core_types::bare_dc_id(shifted_dc_id)` |
| `MTP_bool(true)` / `mtpIsTrue(...)` | `type_utils::mtp_bool(true)` / обычный bool |
| `AbstractConnection::Create(... Tcp ...)` | socket adapter + `TcpConnection::new(stream, protocol, dc_id, random).await` |
| `connectToServer(ip, port, secret, id, forFiles)` | `mtproto_tcp_socket::connect_to_host(...)`, затем Protocol::from_secret и TcpConnection |
| `ProtocolDcDebugId(getProtocolDcId())` | `protocol_dc_debug_id(id)` |
| `prepareNotSecurePacket(req, newId)` | `prepare_not_secure_packet(&req, msg_id, &mut random)` |
| `prepareSecurePacket(keyId, msgKey, fullSize)` | `prepare_secure_packet(key_id, &msg_key, words)` |
| `_connection->received().front()` / pop | `connection.state.received.pop_front()` |
| `connect(... receivedData ...)` | drain Event::ReceivedData или `receive().await` |
| `connected` / `pingTime` | `probe(...).await`, state.connected/state.ping_time |
| `sentEncryptedWithKeyId` | `state.sent_encrypted_with_key_id` |
| `_encryptionKey->partForMsgKey(false)` | `key.part_for_msg_key(false)` |
| `prepareAES(msgKey, ..., false)` | `key.prepare_aes(&msg_key, false)` |
| `aesIgeDecrypt(...)` | derive key/iv, затем `aes_ige_decrypt_raw(&mut bytes, &key, &iv)` |
| `SerializedRequest::Serialize(MTPPing(...))` | `SerializedRequest::serialize(&Ping::new(id), now_ms)` |
| `request.setMsgId / setSeqNo / addPadding` | `set_msg_id / set_seq_no / add_padding(false, random)` |
| maps, держащие shared SerializedRequest | один RequestStore; копируются RequestHandle, мутации общие через store.get_mut |
| `request->after` | `Option<RequestHandle>`, владелец сессии держит зависимые requests живыми |
| `registerMsgId(msgId, needAck)` | `ReceivedIdsManager::register_msg_id`; shrink вызывается отдельно |
| `BoundKeyCreator::start / bind / restartBinder` | те же действия через owner, DcKeyCreator::receive выдаёт CreationStep |
| `temporaryKey->setExpiresAt(now + 86400 + 30)` | до публикации Arc: `Arc::get_mut(&mut result.temporary_key).unwrap().set_expires_at(...)` |
| `prepareBindRequest(key, sessionId)` | `prepare_bind_request(key, session_id, msg_id, now_ms, random)` |
| `handleBindResponse(response)` | DcKeyBindState::Success/Failed/DefinitelyDestroyed |
| `DumpToText(from, end)` | `mtproto_dump_to_text::dump_to_text(&words)` |
| `dcOptions.lookup(dc, type, throughProxy)` | `lookup`, адрес × протокол в том же порядке |

RequestStore не копирует RequestData при копировании handle. SessionData хранит
идентификаторы общего хранилища; служебные записи освобождаются по завершении
и при drop. Будущий Instance освобождает API requests после обработки зависимостей
и callbacks; самостоятельное клонирование буфера не заменяет C++ shared_ptr.

## Криптография и сериализация

BigNum/SHA1/SHA256/HMAC берутся из готового tg_base::openssl_help.
DH/RSA числа — BE; keyId/fingerprint/nonce/msgId и слова TL — LE;
QDataStream Qt_5_1 в tdata — BE числа и byte length + UTF-16 BE строки.
QString null и empty различаются: string_nulls сохраняет это при чтении/записи.
Непарные UTF-16 суррогаты отклоняются: String их не представляет. Для полного
паритета повреждённой/произвольной tdata нужен отдельный raw QString тип на этапе 3.

Секретный показатель всегда имеет исходную фиксированную длину 256 байтов;
BigNum использует constant-time Montgomery pow для нечётного модуля, не другой
big integer crate. AES256-IGE — ige/aes; CTR — Ctr128BE (как OpenSSL ctr128).
Есть v2 send x=0/receive x=8 и v1 для local/bind. Random передаётся явно;
системный provider пользуется уже готовым tg-base random. Приватный auth key
очищается zeroize при drop.

RSA поддерживает PKCS#1 и SPKI PEM, публичную raw операцию с 256-byte блоком
и OAEP SHA1/MGF1 с пустым label. Raw padding не добавляется. Некорректные/нулевые
компоненты отвергаются строже OpenSSL; OAEP пока проверен чтением реализации,
не native образцом. Встроенные PEM и production/test DC адреса перенесены буквально.

Для JSON использован существующий serde_json через `tg_base::json`; для gzip
добавлен `tg_base::zlib_help::inflate_gzip`, использующий уже выбранный flate2/miniz_oxide.
Нового JSON/zlib backend и прямых новых зависимостей этих типов нет.

| Новая прямая зависимость tg-mtproto | Проверенная версия | Назначение |
|---|---|---|
| aes | 0.9.4 | AES256 |
| ctr | 0.10.1 | поток CTR |
| ige | 0.2.1 | IGE |
| reqwest | 0.12.28 | только DoH; rustls-tls/json features |
| x25519-dalek | 2.0.1 | настоящий public key fake TLS; разрешён отдельно |
| tokio | 1.54.1 | I/O, futures, watch, задачи |
| zeroize | 1.9.1 | очистка секретов; уже был в tg-base |
| tg-base / tg-runtime / tg-scheme / tg-tl | workspace | готовые базовые примитивы и TL |

Cargo.lock не коммитится; в таблице версии фактического проверенного разрешения.

## Проверка и границы

Эталон оригинала: AuthKey keyId, все четыре key schedules, RSA fingerprint/raw,
AES-IGE encrypt/decrypt, DC/options config (production/test), Config с каждым непустым полем отличным
от defaults, emoji UTF-16 и null strings, proxy record null/empty, все три
TCP prefixes + first frames, fake TLS hello со всеми GREASE/extensions.
Дополнительно нативные Qt SOCKS5 и HTTP CONNECT Basic образцы. Способ переснять,
форматы и источники — [tests/reference-tools](tests/reference-tools/README.md).

Свои tests: сервер с собственным RSA private key проверяет RSA_PAD, SHA256,
IGE, DH и совпадение auth key, успешный ответ и retry. Отдельные tests проверяют
плохие nonce/hashes/stages/DH lengths/prime/g_a, fail и retry limit. Это не
доказательство всех теоретических ошибок криптографических backend: например,
ошибка RSA_public_encrypt на корректном 2048-bit ключе не воспроизводилась.
Есть duplex TCP 0/1/D, fake TLS, SOCKS/CONNECT; виртуальное время resolver/DoH,
потоки Dcenter, bind decrypt, request padding/identity, dump primitives/vector/
gzip/layer и CRC/budget ошибки gzip. Непустой g_b/secret power и порядок bytes
проверены успешным обменом, а не одним roundtrip одного codec.

Живой сервер: тестовый DC 2, 149.154.167.40, TCP:443 и HTTP:80 — DH success.
Ручной HTTP принимает настоящий `HTTP/1.0  200 OK`, ранее отвергавшийся reqwest.
Вывод текущего запуска — [session-http.log](docs/verification/session-http.log).
Предыдущий отказ сохранён как исторический [live-test-dc.log](docs/verification/live-test-dc.log).
Аккаунт/api_id не использовались, API requests не посылались, production DC не трогались.

Запуск вручную: `nix develop --command cargo test -p tg-mtproto --test live_test_dc -- --ignored --nocapture`.
Прокси/fake TLS в интернете не проверены: только fixtures, duplex и чтение.
OAEP, все варианты ASN.1/Qt IDNA и JSON numerics на экстремальных значениях
не имеют native parity proof. QObject reentrancy заменена очередью событий.

Различия и сохранённые особенности оригинала:

- FactorizePQ для пустого/0/1 pq всё равно возвращает p=1 и q=0/1, каждый
  padded до 4 BE bytes. Сохранено и проверено; в большом пути padding минимальный.
- В dc_options секрет может быть принят validate_secret и записан, но затем
  отклонён собственным reader из-за лимита 32 байта. Сохранено и проверено.
- Dcenter при несовпадении persistent key в release_on_done оставляет флаг
  creating установленным до release_on_fail. Поведение сохранено; вывод по чтению.
- Core dump пропускает legacy layer1 в поиске и имеет current-layer fallback,
  до которого обычная generated serializer map не доходит: invokeWithLayer
  печатается как структура. Сохранено; проверены оба пути.
- Dump gzip ограничен 32 MiB и глубиной 64; оригинальный logging decoder
  не имеет этих лимитов; неполный gzip также отклоняется. Double formatting реализован с шестью значащими цифрами,
  все пограничные Qt случаи ещё не сверены. Крипто/сетевые байты это не меняет.
- Только plain/Basic HTTP CONNECT; Digest/NTLM/Negotiate дают явную ошибку.
- Fake TLS: default HelloEntropy генерирует private bytes и вычисляет настоящий
  X25519 public key, private временный буфер очищается. Public key для private
  1..32 совпадает с OpenSSL fixture; весь ClientHello тоже продолжает совпадать.
  Live MTProxy/fake TLS не проверены.
- WEB socket реализует поток и flow-control через WebTransport. Реальный
  web_proxy/Transport + webview вне этой части и требует оконного/браузерного слоя.

Решение по каждому из 49 исходных файлов и возврат к partial — ниже.

| Исходный файл | Статус | Rust модуль / решение и граница |
|---|---|---|
| `connection_abstract.cpp` | partial | `connection_abstract.rs` — Clock/Exchange/events/packet helpers/runtime adapter; единая factory для всех backend зависит от partial TLS/WEB/HTTP; этап 2. |
| `connection_abstract.h` | partial | `connection_abstract.rs` — Clock/Exchange/events/packet helpers/runtime adapter; единая factory для всех backend зависит от partial TLS/WEB/HTTP; этап 2. |
| `connection_http.cpp` | partial | `connection_http.rs` — Ручной Tokio HTTP, native Qt POST, duplex tests, live DH; partial: br/zstd/deflate content decoding и все Qt retry/timeout ветви не воспроизведены; этап 2. |
| `connection_http.h` | partial | `connection_http.rs` — Ручной Tokio HTTP, native Qt POST, duplex tests, live DH; partial: br/zstd/deflate content decoding и все Qt retry/timeout ветви не воспроизведены; этап 2. |
| `connection_resolving.cpp` | done | `connection_resolving.rs` — IP fallback/deadline/domain changes как actions владельцу; virtual tests, Qt forwarding заменён owner contract. |
| `connection_resolving.h` | done | `connection_resolving.rs` — IP fallback/deadline/domain changes как actions владельцу; virtual tests, Qt forwarding заменён owner contract. |
| `connection_tcp.cpp` | done | `connection_tcp.rs` — 0/1/D, prefix/CTR, stream/probe/queues; native fixtures, duplex и live test DC. |
| `connection_tcp.h` | done | `connection_tcp.rs` — 0/1/D, prefix/CTR, stream/probe/queues; native fixtures, duplex и live test DC. |
| `core_types.h` | done | `core_types.rs` — Типы/ID/сдвиги; TL codec из tg-tl, явный LE; native TL и TCP/DH. |
| `details/mtproto_abstract_socket.cpp` | done | `details/mtproto_abstract_socket.rs` — AsyncRead/AsyncWrite, SocketEvent и выбор типа вместо QObject. |
| `details/mtproto_abstract_socket.h` | done | `details/mtproto_abstract_socket.rs` — AsyncRead/AsyncWrite, SocketEvent и выбор типа вместо QObject. |
| `details/mtproto_bound_key_creator.cpp` | done | `details/mtproto_bound_key_creator.rs` — Owner exchange/binder, stop/restart, error classification; проверка по чтению и bind tests. |
| `details/mtproto_bound_key_creator.h` | done | `details/mtproto_bound_key_creator.rs` — Owner exchange/binder, stop/restart, error classification; проверка по чтению и bind tests. |
| `details/mtproto_dc_key_binder.cpp` | done | `details/mtproto_dc_key_binder.rs` — Bind inner, v1 шифрование и результаты; независимая расшифровка в test. |
| `details/mtproto_dc_key_binder.h` | done | `details/mtproto_dc_key_binder.rs` — Bind inner, v1 шифрование и результаты; независимая расшифровка в test. |
| `details/mtproto_dc_key_creator.cpp` | done | `details/mtproto_dc_key_creator.rs` — Автоматы одиночного/постоянного+временного обмена, RSA_PAD, retry, sync time; memory server и отказные tests, TCP live. |
| `details/mtproto_dc_key_creator.h` | done | `details/mtproto_dc_key_creator.rs` — Автоматы одиночного/постоянного+временного обмена, RSA_PAD, retry, sync time; memory server и отказные tests, TCP live. |
| `details/mtproto_dcenter.cpp` | done | `details/mtproto_dcenter.rs` — Mutex/Arc вместо Qt lock; exclusion, identity, destroy; многопоточный test. |
| `details/mtproto_dcenter.h` | done | `details/mtproto_dcenter.rs` — Mutex/Arc вместо Qt lock; exclusion, identity, destroy; многопоточный test. |
| `details/mtproto_domain_resolver.cpp` | done | `details/mtproto_domain_resolver.rs` — DoH request specs/reqwest, JSON, stagger/cache/cancel owner; virtual tests. Живой DoH не запускался. |
| `details/mtproto_domain_resolver.h` | done | `details/mtproto_domain_resolver.rs` — DoH request specs/reqwest, JSON, stagger/cache/cancel owner; virtual tests. Живой DoH не запускался. |
| `details/mtproto_dump_to_text.cpp` | partial | `details/mtproto_dump_to_text.rs` — Примитивы/vector/gzip/layers работают; все Qt double edge cases ещё не сверены. Возврат этап 2. |
| `details/mtproto_dump_to_text.h` | partial | `details/mtproto_dump_to_text.rs` — Примитивы/vector/gzip/layers работают; все Qt double edge cases ещё не сверены. Возврат этап 2. |
| `details/mtproto_received_ids_manager.cpp` | done | `details/mtproto_received_ids_manager.rs` — BTreeMap, duplicate/too-old, explicit shrink; tests. |
| `details/mtproto_received_ids_manager.h` | done | `details/mtproto_received_ids_manager.rs` — BTreeMap, duplicate/too-old, explicit shrink; tests. |
| `details/mtproto_rsa_public_key.cpp` | partial | `details/mtproto_rsa_public_key.rs` — Native fingerprint/raw; strict malformed PEM/zero components, OAEP ещё только чтение; возврат этап 2. |
| `details/mtproto_rsa_public_key.h` | partial | `details/mtproto_rsa_public_key.rs` — Native fingerprint/raw; strict malformed PEM/zero components, OAEP ещё только чтение; возврат этап 2. |
| `details/mtproto_serialized_request.cpp` | done | `details/mtproto_serialized_request.rs` — Header/body/padding/ack; RequestStore + handle identity; tests. |
| `details/mtproto_serialized_request.h` | done | `details/mtproto_serialized_request.rs` — Header/body/padding/ack; RequestStore + handle identity; tests. |
| `details/mtproto_tcp_socket.cpp` | partial | `details/mtproto_tcp_socket.rs` — Tokio, SOCKS5 и Basic CONNECT; native Qt samples. Digest/NTLM/Negotiate отсутствуют; возврат этап 2 с выбором backend. |
| `details/mtproto_tcp_socket.h` | partial | `details/mtproto_tcp_socket.rs` — Tokio, SOCKS5 и Basic CONNECT; native Qt samples. Digest/NTLM/Negotiate отсутствуют; возврат этап 2 с выбором backend. |
| `details/mtproto_tls_socket.cpp` | partial | `details/mtproto_tls_socket.rs` — Native hello/X25519, duplex record stream; timeout→sync owner integration требует сессии; этап 2. |
| `details/mtproto_tls_socket.h` | partial | `details/mtproto_tls_socket.rs` — Native hello/X25519, duplex record stream; timeout→sync owner integration требует сессии; этап 2. |
| `details/mtproto_web_proxy_socket.cpp` | partial | `details/mtproto_web_proxy_socket.rs` — Поток/window/Drop через WebTransport; настоящий browser Transport/webview — этап 5. |
| `details/mtproto_web_proxy_socket.h` | partial | `details/mtproto_web_proxy_socket.rs` — Поток/window/Drop через WebTransport; настоящий browser Transport/webview — этап 5. |
| `mtproto_auth_key.cpp` | done | `mtproto_auth_key.rs` — Native keyId, v1/v2 оба направления, AES-IGE; CTR в native TCP. |
| `mtproto_auth_key.h` | done | `mtproto_auth_key.rs` — Native keyId, v1/v2 оба направления, AES-IGE; CTR в native TCP. |
| `mtproto_config.cpp` | partial | `mtproto_config.rs` — Native все поля/default/custom/UTF16/null, apply/legacy tails; непарные UTF-16 требуют raw QString на этапе 3. |
| `mtproto_config.h` | partial | `mtproto_config.rs` — Native все поля/default/custom/UTF16/null, apply/legacy tails; непарные UTF-16 требуют raw QString на этапе 3. |
| `mtproto_dc_options.cpp` | done | `mtproto_dc_options.rs` — Встроенные DC/RSA, lookup/change/CDN/persistence/debug file; native defaults и tests, сложные CDN cases — только чтение. |
| `mtproto_dc_options.h` | done | `mtproto_dc_options.rs` — Встроенные DC/RSA, lookup/change/CDN/persistence/debug file; native defaults и tests, сложные CDN cases — только чтение. |
| `mtproto_dh_utils.cpp` | done | `mtproto_dh_utils.rs` — BigNum, все g/prime/ga bounds, fixed 256-byte secret power; свои tests и live DH. |
| `mtproto_dh_utils.h` | done | `mtproto_dh_utils.rs` — BigNum, все g/prime/ga bounds, fixed 256-byte secret power; свои tests и live DH. |
| `mtproto_pch.h` | replaced | `lib.rs` — PCH заменён Rust imports и модульной компиляцией. |
| `mtproto_proxy_data.cpp` | partial | `mtproto_proxy_data.rs` — Links/secrets/WEB capability/proxy records; native null/empty и tests. Все Qt IDNA cases и raw UTF-16 требуют этапа 3. |
| `mtproto_proxy_data.h` | partial | `mtproto_proxy_data.rs` — Links/secrets/WEB capability/proxy records; native null/empty и tests. Все Qt IDNA cases и raw UTF-16 требуют этапа 3. |
| `mtproto_response.cpp` | done | `mtproto_response.rs` — RPC parsing через готовый qthelp_regex, local/flood/temporary; tests. |
| `mtproto_response.h` | done | `mtproto_response.rs` — RPC parsing через готовый qthelp_regex, local/flood/temporary; tests. |
| `type_utils.h` | done | `type_utils.rs` — Bool языка и те же thin helpers; используется schema/response. |

## Отложено

| Файл | Возврат | Нужно |
|---|---|---|
| connection_abstract.* | 2 | Общая factory после решений по HTTP/TLS/WEB; не обещает подключить любой backend сейчас. |
| connection_http.* | 2 | br/zstd/deflate content decoding, точные Qt idle retry/timeout branches; malformed status и SOCKS/CONNECT уже работают. |
| details/mtproto_tcp_socket.* | 2 | Согласовать и проверить Digest/NTLM/Negotiate, если они нужны вместо Basic. |
| details/mtproto_tls_socket.* | 2 | Связать deadline owner с SyncTimeRequest; X25519 уже реализован. |
| details/mtproto_web_proxy_socket.* | 5 | web_proxy/Transport и webview, браузерная сессия/IPC. |
| details/mtproto_rsa_public_key.* | 2 | Native OAEP vectors и malformed PEM/ASN.1 compatibility. |
| details/mtproto_dump_to_text.* | 2 | Эталон Qt double formatting на границах. |
| mtproto_proxy_data.*, mtproto_config.* | 3 | Raw UTF-16 QString, полный IDNA native matrix; значения String с непарным суррогатом сейчас отвергаются. |
| mtp_instance*, facade*, sender*, mtproto_concurrent_sender*, config_loader*, special_config_request*, dedicated_file_loader*, proxy_check* | 2, часть 2 | Это не начато; использовать описанные packet/event/key interfaces. |

Проверенное только чтением явно отмечено выше: часть lifecycle wrappers и ошибок,
экстремальные ASN.1/IDNA/JSON случаи, некоторые CDN/конфигурационные ветви. Не все
49 файлов имеют статус done. Поэтому эта часть этапа 2 целиком не объявляется готовой.

Поиск `AyuGram|ayu` (также без учёта регистра) во всём каталоге mtproto
не дал совпадений. Отдельных помеченных патчей в этих файлах не найдено.
Присутствующий в исходной точке WEB bridge (host/path/link secret/capability)
перенесён в своей переносимой части, а не исключён из-за отсутствия маркера.

Негативные счётчики и некорректные UTF-16 поля отвергаются раньше; состояние
объекта после неудачного чтения повреждённых данных может отличаться от C++.
Успешные valid-data native сравнения не доказывают равенства всех error paths.

## Автомат сессии без Instance

[Карта функций](docs/session-map.md) содержит 135 строк: 121 определение из .cpp,
четыре свободных helper, девять inline методов SessionData и Default SessionOptions.
Все имеют перевод и тест. Нативные образцы затрагивают три строки (шифрование,
проверка входящего конверта и сравнение msg_key); это не native proof всего автомата.

`Session<C,R>` владеет SessionPrivate и работает как задача Tokio LocalSet:
`base::Timer`/TimerHost содержат Rc и привязаны к потоку. Снаружи используется
Send-ручка SessionHandle, команды и снимки SessionData. Для отдельного рабочего
потока LocalSet и Session создаются внутри него; готовый Session нельзя передавать
через tokio::spawn. Обработка I/O идёт в независимом исполнителе SessionTransport.
И Query-состояние, и подготовленные запросы доступны через защищённые данные.
Все Callback/Update/state/key события проходят общую FIFO, затем tg-runtime
on_main. Фактические done/fail вызовы выполнит будущий Instance на главном потоке.
В этой задаче сессия доставляет Response, не разбирает тип ответа API.

Трейт `SessionEnvironment` содержит только зависимости из четырёх исходных файлов:

| C++ источник | Контракт Rust |
|---|---|
| `_instance->dcOptions()` (dcType/lookup/keys/hasCDNKeys) | `dc_options() -> DcOptions` — согласованный снимок |
| `isTestMode`, `isKeysDestroyer` | `is_test_mode`, `is_keys_destroyer` |
| `deviceModel`, `systemVersion` | `device_model`, `system_version` |
| `systemLangCode`, `cloudLangCode`, `langPackName` | одноимённые snake_case getters |
| `Core::App().settings().proxy()` в refreshOptions | `proxy_settings() -> ProxySettings` |
| `hasCallback(requestId)` | `has_callback` — потокобезопасное чтение |
| dcTemporaryKeyChanged, dcOptions.changed, cdnConfigChanged | `subscribe() -> UnboundedReceiver<EnvironmentChange>` |
| processCallback/processUpdate, onStateChange/onSessionReset | `emit(Callback/Update/StateChange/SessionReset)` |
| dcPersistentKeyChanged/dcTemporaryKeyChanged | `emit(DcPersistentKeyChanged/DcTemporaryKeyChanged)` |
| syncHttpUnixtime/requestConfig/requestConfigIfOld/requestCDNConfig | соответствующие варианты `InstanceEvent` |
| restartedByTimeout/badConfigurationError/keyWasPossiblyDestroyed/keyDestroyedOnServer | соответствующие варианты `InstanceEvent` |

Dcenter и общий RequestStore передаются конструктору отдельно, как dc аргумент
Session и shared SerializedRequest оригинала. Запись ключей остаётся обязанностью
получателя уведомлений. Экспорт авторизации, смена DC и чтение Config не вызываются
непосредственно в этих четырёх файлах, поэтому методов для них в трейте нет.
Будущий Instance подключит свои tg-rpl streams к индивидуальным unbounded каналам
подписок; замена их ограниченным broadcast с потерей событий недопустима.

`ClientInfo` передаёт фактический api_id, версию/архитектуру/дистрибутив,
локальный UTC offset и prefer-ipv6 из платформенного/настроечного слоя.
Default — образец для тестового сервера (17349), не конфигурация будущего входа
в production. compute_app_version сохраняет алгоритм C++; ОС/KSandbox значения
приходят от вызывающего кода. При смене timezone/настроек используется команда
`SessionCommand::ClientInfo` перед переинициализацией. Сдвиг серверного времени
вычисляется заново для каждого initConnection через SessionClock, не фиксируется
при старте. `register_options(registry)` регистрирует оригинальный prefer-ipv6
с теми же name/description/default; его значение включается в ClientInfo.

### Транспорт и таймеры

`SessionTransport::submit(action)` не ждёт I/O и обслуживает Connect/Send/Close/
TimedOut. `next_event()` возвращает Connected/Packet/ReceivedSome/Error/
Disconnected/SyncTimeRequest. Оно должно быть безопасно при отмене future.
Для TCP используются уже разделённые TcpReader/TcpWriter; один writer последовательно
дописывает кадры до конца, reader принимает данные независимо от очереди отправки.
HTTP executor использует отдельные outstanding POST и сообщает need_http_wait.
Обновление fullConnectTimeout при разрешении прокси передаётся ConnectTimeout;
исходное значение — 4000 * max(resolvedIPs,1), либо 8000 для обычного транспорта.
Исполнитель реальных соединений подключит будущий Instance: здесь интерфейс
обязателен к реализации, фиктивного сетевого backend с успешным результатом нет.

Все retry/connect/receive/better/ping/checkSent/container/sender таймеры —
base::Timer. Источник пробуждения задаётся отдельно через TimerHost::Alarm.
Для реального времени можно установить ThreadAlarm, будящий Session::wakeup();
для тестов ManualClock и явный wake/process, без sleep.

```rust,ignore
let timer_clock = Arc::new(tg_base::timer::SystemClock);
let host = tg_base::timer::TimerHost::new(timer_clock.clone());
let (actor, handle) = Session::new(dc_id, dc, environment, requests,
    SessionParameters { clock: RuntimeClock, random: SystemRandom, host, info });
let wake = actor.wakeup();
actor.set_alarm(Rc::new(tg_base::timer::ThreadAlarm::new(timer_clock,
    move || wake.notify_one())));
// Внутри LocalSet рабочего потока; main queue исполняется главным потоком.
tokio::task::spawn_local(actor.run(transport));
```

### Проверки и особенности оригинала

Сервер в памяти использует настоящий TCP framing/CTR на duplex, собственный RSA,
самостоятельные MTProto2 и v1 bind key schedules по описанию
[MTProto](https://core.telegram.org/mtproto/description) и
[создания ключа](https://core.telegram.org/mtproto/auth_key). Клиентский
encoder/decrypt/prepare_aes сервером не вызываются: общие только криптопримитивы.
Проверены permanent+temporary DH, dh_gen_retry, bind с расшифровкой внутреннего
сообщения, CDN без permanent/bind, разрыв после запроса, повтор, все коды bad_msg,
bad_server_salt, new_session_created, контейнеры, gzip, msgs_ack, msgs_state_req/
state_info, msg_resend_req и возврат RPC в главный поток. Остальные проверки
границ/повреждений/таймеров идут прямо через автомат и независимые серверные байты.
Нативная входная проверка — 19 случаев из handleReceived до dispatcher; исходящий
sendSecureRequest снят отдельно. Fixtures, CMake и extract scripts сохранены.

Сохранены особенности:

- notifyConnectionInited сравнивает current с текущими полями, а не переданными
  опциями; в последовательном исполнении всегда помечает живой DC inited.
- В LogIdsVector первый ID выводится дважды.
- Одиночная отправка запроса не ставит checkSentRequests timer; он ставится в
  контейнерной ветке, как в C++. Receive timeout при этом действует.
- При выключении обоих address/protocol флагов skip-логика исключает только
  первый вариант; воспроизведено тестом.
- Входящие msgs_state_req, msg_resend_req и future_salts не имеют отдельных
  ветвей в исходном dispatcher: regular DC передаёт их как update, остальные
  типы DC игнорируют. Новые обработчики сверх оригинала не придуманы.
- destroy_auth_key отправляет Instance, не SessionPrivate: здесь проверены
  destroyer mode, -404 и уведомления KeyWasPossiblyDestroyed/KeyDestroyedOnServer.

Отличия: QObject signals/deleteLater заменены командами и владением; общая
SyncTimeRequestDuration атомарна, тогда как в C++ она читается/пишется из разных
сессионных потоков без синхронизации. Пустое аутентифицированное тело попадает в контракт
Expects(from < end), как в C++; Rust проверяет его через tg_base::expects! (panic).
Этот случай проверен перехватом panic после обработки настоящего конверта.
Константное сравнение использует RustCrypto ctutils. Время/энтропия/платформенные
значения передаются явно. Реальные Qt reentrancy, системные события ОС и упаковка
Mac/Windows не запускались; формат их переданных значений проверен unit tests.
Живой сервер в этом задании не использовался, help.getConfig и аккаунт не запускались.

Пофайловый статус текущей и следующей задач:

| Файл оригинала | Статус | Сделано / требуется |
|---|---|---|
| `session.cpp` | done | Автомат, окружение, base::Timer, проверки; 135 функций в docs/session-map.md. |
| `session.h` | done | Автомат, окружение, base::Timer, проверки; 135 функций в docs/session-map.md. |
| `session_private.cpp` | done | Автомат, окружение, base::Timer, проверки; 135 функций в docs/session-map.md. |
| `session_private.h` | done | Автомат, окружение, base::Timer, проверки; 135 функций в docs/session-map.md. |
| `mtp_instance.cpp` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `mtp_instance.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `facade.cpp` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `facade.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `sender.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `mtproto_concurrent_sender.cpp` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `mtproto_concurrent_sender.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `config_loader.cpp` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `config_loader.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `special_config_request.cpp` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `special_config_request.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `dedicated_file_loader.cpp` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `dedicated_file_loader.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `proxy_check.cpp` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
| `proxy_check.h` | todo | Реализация ещё не начата; требуется перенос по функциям и интеграция с сессией/Instance, этап 2. |
