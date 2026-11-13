# Codex: следующий блок ядра — ID

13.11.2026, ветка port/data-ids от main 0f810b7. По просьбе пользователя
начата независимая часть после lang/countries. Результат Claude ещё не
поступил; его WIP, файлы и известные два language test failures не менялись.

## Сделано

crates/tg-core/src/data/data_peer_id.rs:
BareId, ChatIdType и отдельные UserId/ChatId/ChannelId/FakeChatId, PeerId,
конструкторы/классификация/извлечение, peerFrom/ToMTP и modern/legacy storage
serialization. Сохранены старые type masks, marker bit 55, знаковое расширение
MTPint, верхние биты modern значения, отсутствие очистки bare при конструировании
и fallback peerUser(0). Rust Hash используется для контейнеров, не как wire ABI.

crates/tg-core/src/data/data_msg_id.rs:
MsgId, диапазоны/специальные константы, client/story index conversions,
FullMsgId/FullStoryId/GlobalMsgId, MsgRange, сравнения, bool/!/prefix/postfix
и арифметика определённого исходником диапазона. Signed overflow, неопределённый
в C++, явно отвергается Rust checked arithmetic. story_id_to_msg_id сохраняет
исходное предусловие только >=0, без нового ограничения ServerMaxStoryId.

Оба модуля экспортированы. Нет фиктивных PeerData, Session, History, Store или
копий TextWithEntities/CreditsAmount. tg-scheme лишь перенесён из dev в обычные
dependencies; новые пакеты и изменения Cargo.lock не нужны.

## Проверка

PASS Rust 1.95.0: fmt, clippy --workspace --all-targets с -D warnings и
cargo test --workspace: 651 passed, 0 failed, 3 прежних ignored (111 suites).
PASS: четыре обычных теста нового среза и четыре compile-fail doctests.
PASS: 1792 операции с настоящими C++ strong IDs и исходными алгоритмами,
включая exact boxed TL input/output и Qt_5_1 big-endian сохранение PeerId.
Состав: 1236 Peer, 549 Msg, 6 client/story conversions и одна таблица констант.
Покрыты все 16 legacy tags, 256 raw type bytes, high/reserved bits, отрицательные
MTPint, i64 MIN/MAX, zero/nonzero полных ключей, prefix/postfix и ordering.
Compile-fail проверяет UserId/ChatId mix, reserved SHIFT, integer ordering и
удалённый ChannelId-конструктор FullMsgId.

Native target компилирует неизменённые data_peer_id.cpp/.h и data_msg_id.h.
Для TL из generated scheme.cpp извлечены без правки Peer methods и три creator
methods; весь scheme.h настоящий. ID-типы не заменяются целочисленными stub.
QtGui value header включён оригиналом, но GUI runtime и приложение не запускаются;
target линкуется с Qt Core. Исходный клон остаётся чистым. Сетевых запросов и
пользовательских tdata/аккаунтов в этом срезе нет.

Карта: crates/tg-core/docs/data-ids-map.md; fixtures и SHA256:
crates/tg-core/tests/fixtures/data-ids; воспроизведение:
crates/tg-core/tests/reference-tools/data-ids/README.md.
Полные локальные логи: /tmp/ayugram-data-ids-targeted.log и
/tmp/ayugram-data-ids-workspace.log.

## Отложенное и интеграция

data_peer_id.cpp/.h — done; data_msg_id.h — partial:
MessageHighlightId, FullReplyTo, SuggestOptions ждут принятых фактических
TextWithEntities и CreditsAmount от Claude. Их определения не добавлены
заглушками. После принятия его ветки перенести оставшиеся части с native
проверкой quote/entity/price поведения.

Raw storage-list i64/u64 API и MTProto packet MsgId=u64 не мигрировались.
Ключ клиентского сообщения — отдельный signed MsgId; нельзя заменять им
криптографический mtpMsgId. Session/store модели остаются следующей работой.

Минимальные общие точки с веткой Claude: Cargo.toml tg-core должен содержать
tg-scheme ровно один раз в обычных dependencies; lib.rs должен совместить
pub mod data с его core/countries/lang и extern crate self. Саму область
Claude этот срез не редактирует. docs/parallel-work.md дополнен новым scope.
