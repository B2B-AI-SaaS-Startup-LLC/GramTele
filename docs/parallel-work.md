# Параллельная работа: Claude и Codex

Репозиторий: https://github.com/B2B-AI-SaaS-Startup-LLC/GramTele.
Разделение согласовано пользователем 13.11.2026. Правила PLAN.md и PORTING.md
действуют для обеих веток; это собственный порт, без TDLib/grammers.

| Исполнитель | Ветка | Задача и файлы |
|---|---|---|
| Claude на другой машине | port/lang-countries от handoff/claude-lang-countries | Незавершённые lang/countries, credits_amount и необходимые им value types tg-ui, MainLocal/runtime, Sender и генератор языка |
| Codex здесь | port/cache-file-format | tg-cache: storage_encryption, storage_encrypted_file и перенесённые тесты; native-паритет шифрования и файлового формата |
| Codex: следующий блок ядра | port/data-ids | tg-core/src/data/data_peer_id и скалярная часть data_msg_id: типизированные ID, TL и legacy/modern serialization |

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
