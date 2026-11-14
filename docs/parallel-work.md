# Параллельная работа: Claude и Codex

Репозиторий: https://github.com/B2B-AI-SaaS-Startup-LLC/GramTele.
Разделение согласовано пользователем 13.11.2026. Правила PLAN.md и PORTING.md
действуют для обеих веток; это собственный порт, без TDLib/grammers.

| Исполнитель | Ветка | Задача и файлы |
|---|---|---|
| Claude на другой машине | port/lang-countries от handoff/claude-lang-countries | Незавершённые lang/countries, credits_amount и необходимые им value types tg-ui, MainLocal/runtime, Sender и генератор языка |
| Codex: завершено | port/cache-file-format | tg-cache: storage_encryption, storage_encrypted_file и перенесённые тесты; native-паритет шифрования и файлового формата |
| Codex: принят пользователем, main 20695cb | port/data-ids | tg-core/src/data/data_peer_id и скалярная часть data_msg_id: типизированные ID, TL и legacy/modern serialization |
| Codex: завершено и проверено | port/cache-database от main 20695cb; worktree cache-database | Остаток tg-cache/lib_storage, 24 файла; native в обе стороны, 252 строки карты, 40 тестов; интеграция main после проверок |
| Codex: завершено и проверено | port/data-changes от main be0473f; worktree data-changes | SessionScope/typed keys, data_changes.h/.cpp; 136 строк карты, 25 tests, native O0/O2; handoff codex-data-changes |

Claude получает [отдельное задание](tasks/claude-lang-countries.md) и WIP-снимок
прежнего рабочего дерева. WIP не считается проверенным результатом и не вливается
в main до завершения. Исходное грязное дерево port/core-base сохраняется целиком.

Claude не меняет crates/tg-cache и файлы data-ids. Codex не меняет lang/countries,
credits_amount, tg-ui, tg-codegen, tg-runtime и tg-mtproto.
После просьбы пользователя продолжить блок после Claude добавлен срез data-ids.
Для него Codex добавляет только pub mod data в tg-core/src/lib.rs и переносит
существующую зависимость tg-scheme из dev в обычные зависимости tg-core.
Изменения экспорта/manifest при интеграции совмещаются с добавлениями Claude.
MessageHighlightId, FullReplyTo и SuggestOptions отложены до принятия его
TextWithEntities/CreditsAmount; их заглушек или копий в data-ids нет.
Тестовые native targets хранятся в соответствующем крейте, поэтому CMake-файлы
друг друга не затрагиваются. Не обновлять версии общих зависимостей.

Ветки пушатся в организационный репозиторий, без force push и без прямого пуша
Claude в main. Интеграцию в main выполняет Codex после проверки готового коммита.
Правки Cargo.lock — только необходимые своей задаче; при объединении проверять
сохранение обеих групп зависимостей, не откатывать весь lock из одной ветки.

Чтобы не конфликтовать в общих отчётах, Claude пишет
docs/handoffs/claude-lang-countries.md и свою карту функций; Codex —
docs/handoffs/codex-cache-file.md и карту tg-cache. Корневые PROGRESS.md и
ledger/files.csv обновляет Codex при интеграции по фактическим результатам.


Завершение cache-database: [handoff](handoffs/codex-cache-database.md),
[карта функций](../crates/tg-cache/docs/port-map.md).
Все три проверки workspace PASS, 678 passed / 0 failed / 3 прежних ignored.
Существующие файлы tg-runtime/tg-ui/tg-codegen/tg-mtproto и lang/countries
не менялись: Stats bridge локален tg-cache. Общий lock получил только две
существующие workspace-зависимости. Пушатся main и port/cache-database в gramtele,
без force push. Для дальнейшей работы не повторять завершённый cache-database;
ожидать проверенный коммит Claude для lang/countries и зависимых value types.

Срез data-changes: [handoff](handoffs/codex-data-changes.md),
[карта](../crates/tg-core/docs/data-changes-map.md). Код ограничен tg-core;
Claude files и Cargo.lock не меняются. Native compile не подменяет Main lifecycle:
живой Session/main guard остаётся own tests + чтение до пакета29. Изменяемые ID
media/topics/messages и pointer-order→key-order явно задокументированы как границы
будущего пакета моделей. Все три проверки PASS: 702 workspace tests + 1 дочерний процессный,
0 failed / 3 прежних ignored. Интеграция main + port/data-changes без force push.

Срез port/data-peer от main959488b, worktree data-peer:
[handoff](handoffs/codex-data-peer.md),
[карта](../crates/tg-core/docs/data-peer-map.md). Общая модель PeerData и Peers
component только в tg-core, без fake subtype records. Concrete User/Chat/Channel
и owned enum — 02/03; UI5/6, full owners29 и Unicode text backend5 остаются partial.
357 строк карты (300 done/57 partial), native 4247 строк, 31 новых tests.
Cargo manifests/lock и области Claude не менялись.

По указанию пользователя параллельные Codex worktrees port/tdata-settings
(storage settings scheme/localstorage), port/base-linux (lib_base/platform/linux)
и docs-only stage4 audit зарезервированы за другими агентами. В data-peer их файлы
не меняются, ветки не интегрируются. Их интеграция в main — только отдельным
следующим заданием пользователя. Текущая интеграция касается только port/data-peer.

Пакет03 port/data-chat-channel от main1038c32, worktree data-chat-channel:
[handoff](handoffs/codex-data-chat-channel.md),
[карта](../crates/tg-core/docs/data-chat-channel-map.md). Concrete Chat/Channel,
sole Peers owner и released-borrow phases, rights/participants/TL scalar updates.
337 строк карты (287 done/50 partial), native481 строк,19 новых tests.
Полные UserData02/Session13/14/29/History18 и внешний UI/API apply не подменяются.
Native pure proof отделён от own tests+чтение полного состояния. Cargo manifests/lock,
Claude files и чужие ветки не менялись. Интегрируется только port/data-chat-channel.
Сообщённую пользователем гонку ObjectAsync::drop не чинить: tg-runtime у Claude.

Проверки data-chat-channel: fmt/clippy/test workspace PASS,769 успешных результатов,
0 failed/3 прежних ignored. Интеграция своей ветки после проверки,без force push.

Пакет04 port/data-media от main0c38855, worktree data-media:
[handoff](handoffs/codex-data-media.md), [карта](../crates/tg-core/docs/data-media-map.md).
Реализованы Photos/Documents owner/convert/indices и model/pure phases, byte
leases, TL, filename/MIME contract, cache queue adapter. Wire PhotoKey/DocumentKey
остаются lookup selectors; pointers представлены отдельными stable RecordKey,
без media generation. Это уточнение эскиза stage4-plan после чтения collision
бodies; reserved docs-only audit file не менялся. Lookup-key и mutable wire id
могут различаться ровно как C++. Source repeated conversion quirk сохранён.
307 строк188 done/119 partial/0 pending; native1706,33 targeted tests/clippy PASS.
Финальные fmt/clippy/test workspace PASS после merge уже принятого main750204e:
809 successful (808+1 child),0 failed,3 existing ignored. Интеграция своей ветки
в main обычным fast-forward, push main+port/data-media без force. Не интегрировать private port/localstorage:
он у Claude; уже принятый main750204e сохранить. Области Claude и чужие файлы
не менялись. Известная гонка ObjectAsync::drop остаётся владельцу tg-runtime.

Предварительная полная проверка на базе0c38855: fmt --check PASS, clippy --workspace --all-targets -D warnings PASS, test --workspace PASS (802 успешных результата,0 failed,3 прежних ignored). После совмещения с уже принятым main750204e проверки повторяются.
