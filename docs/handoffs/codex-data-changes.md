# Codex: контракты хранилищ и Data::Changes

Ветка `port/data-changes` от `main be0473f`, отдельный worktree
`ayugram-rs-wt/data-changes`. Оригинал `0f2a31bb420de98f331990432265b131b8d836b5`
только читался. Интеграция и пуш main/ветки в gramtele — без force push.
Область Claude (lang/countries, credits_amount, tg-ui, tg-codegen, tg-runtime,
tg-mtproto) и Cargo.lock не меняются. Новых зависимостей нет.

Сделаны SessionScope/SessionScopeId, PeerKey/UserKey/ChatKey/ChannelKey,
MessageKey с generation и identity index MessageKeys, DocumentKey/PhotoKey,
StoryKey/TopicKey/SublistKey/FolderKey и четыре настоящих варианта EntryKey.
Generation только у messages. Scope завершается с owner; queued jobs не держат
его живым. Ключи — значения, не Rc на каждой записи графа.
Настоящие модели Peer/History/Main::Session, ensure/loaded lookup не имитируются.

Data::Changes перенесён целиком: 7 полных наборов flags/update payloads,
NameUpdate, admin/rank payloads (UTF-16 сохранён), все overloads/filter/flagsValue,
realtime-before-enqueue, repeated OR, dropScheduled, remove, snapshot flush,
порядок категорий, реентерабельность и SessionScope-guard main queue.
Ни один mutable RefCell borrow менеджера не держится через fire.
Ручной flush не отменяет старую queued task: она может доставить следующую
порцию. Это исходное поведение, а не generation-token scheduler.

## Учёт

| Исходный файл | Статус | Реализация / остаток |
|---|---|---|
| data/data_changes.h, 483 LOC | done | data_changes.rs, все declarations/flags/payloads |
| data/data_changes.cpp, 394 LOC | done | все методы Changes и Manager |
| data/data_chat_participant_status.h, 287 LOC | partial | enum ChatAdminRight/ChatAdminRights; остальные rights/restriction/permission методы → пакет 03 |
| data/data_types.h, 418 LOC | partial | FolderId/PhotoId/DocumentId; остальные типы → пакет 16 |

[Карта](../../crates/tg-core/docs/data-changes-map.md): **136 строк**, **0 без теста**,
**0 partial строк**. Включены **59 исходных функций**: 49 Changes + 8 Manager +
CountBit + NameUpdate ctor; остальное — value contracts и Rust операции ключей.
Вспомогательные заголовки partial честно привязаны к будущим пакетам; карта не
объявляет весь data_session.h переведённым. Новые тесты: **25**, без ignored/sleep.

## Native и расхождения

[Отдельная цель](../../crates/tg-core/tests/reference-tools/data-changes/README.md)
собрана в `/tmp/ayugram-data-changes-native`: неизменённый настоящий
`data_changes.cpp` с настоящими headers и lib_base/lib_rpl/lib_crl, Qt6 Core,
GCC15.3/x86_64. O0/O2 совпали; Rust воспроизводит три CSV, всего **600 строк**:
148 enum values, 242 shift/mixed-mask trace, 210 manager/category-order trace.
Сравнены все 7 Managers: realtime, повторные маски, postponed/filter/flagsValue,
dropScheduled с OR и erase до callback, drop, snapshot, вложенный flush из
realtime и postponed callbacks; отдельно настоящий Changes constructor/общий flush.

Нативная цель не создаёт fake Peer/History/Session: pointers служат непрозрачными
синтетическими tokens и не разыменовываются. Для общего flush pending ставится
через настоящие private Managers, notify — тестом. Поэтому привязка к живому
Main::Session, `session()` и `scheduleNotifications()` — **own tests + чтение**;
полный native lifecycle требует пакета **29** с Account/API/storage/Core owners.
Весь cpp компилируется, но GC при линковке не означает выполнение этих bindings.

Особенности оригинала:

- `1U << i` в Peer realtime — UB начиная с 32. Native O0/O2 показывает wrap32:
  high-only bits 32..57 не доставляются realtime, low bits 0..25 дополнительно
  вызывают streams 32..57. Все input flags остаются в payload; postponed — 64-bit.
  Rust переносит **измеренное поведение** с wrapping_shl32, тестом и source link.
  Другие C++ компиляторы/архитектуры не проверены.
- Порядок категорий сохранён. **Внутри категории есть явное отличие**: source
  flat_map сортирует pointer address, Rust — typed scoped key. Native a/b tokens
  выбраны по возрастанию адресов и нормализованы; это не доказывает равенства с
  произвольным allocator order. Скрытого slot/address поля к tuple keys не добавлено.
- C++ указатели заменены scope keys; несовпадение scope даёт assert, и use после
  завершения scope запрещён. Исходный Main::Session& accessor становится scope ID.
  Два overloads получают `_for` имена. Qt rank/QChar — owned UTF-16 values.

## Обязательства будущего пакета моделей

1. Фото/документы (03/04 + Session13/14): `photoConvert/documentConvert` меняют ID
   **того же объекта**. При существующем destination два pointer identities могут
   иметь одинаковый id и старый map key остаётся. `(scope,id)` их не различает:
   это design gate, не решённый тайной alias-таблицей или generation media.
   Нужно учесть secondary references, cache move, saved GIFs и media leases.
2. Сообщения (13/14,18..20): local→server remap переносит allocation; wire key,
   generation, dependencies/non-channel index/pending/subscriptions должны быть
   согласованы. Нельзя инвалидировать ключ раньше Destroyed callback; текущий
   тест это закрепляет. Фиктивного ремапа без HistoryItem нет.
3. Forum (08) меняет rootId существующего topic; Topic/SavedSublist (08/12)
   могут удаляться/создаваться с тем же tuple. Пользователь разрешил generation
   только messages: future record lifetimes обязаны завершать старые подписки,
   а ID remap обновлять все references в исходных точках уведомления.
4. Полный native Main lifecycle и binding SessionScope — пакет29. Ранние модели
   должны отдельно решить, нужен ли им исходный allocator order внутри категории.

## Проверки

Rust1.95.0, обязательная команда из PORTING.md завершилась с exit 0:

```text
nix develop --command bash -euc 'cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace'
cargo fmt --check: PASS
cargo clippy --workspace --all-targets -- -D warnings: PASS (2m04s)
cargo test --workspace: PASS (build 4m16s)
702 workspace tests passed / 0 failed / 3 existing ignored
+1 successful child-process test = 703 successful test results in the full log
```

tg-core: 25 новых Changes tests, в том числе три fixture comparisons; все PASS.
Native O0/O2 → CSV сравнение и manifest SHA256: PASS. Все 136 ссылок карты
на функции тестов проверены, исходных функций Changes/Manager в .cpp 49+8;
вместе с двумя inline функциями заголовка —59. Прежние ignored не расширены.
Полный локальный лог: `/tmp/data-changes-workspace-checks.log`.

Пользовательская tdata, аккаунты, сообщения и ключи не открывались.
