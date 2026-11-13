# Задание Claude: завершить lang/countries

Продолжи полный порт AyuGram Desktop на Rust в репозитории
https://github.com/B2B-AI-SaaS-Startup-LLC/GramTele.
Параллельно Codex делает файловый слой tg-cache; его файлы не трогай.

## Начальное состояние

1. Клонируй организационный репозиторий и создай свою ветку:

   ```bash
   git fetch origin
   git switch -c port/lang-countries origin/handoff/claude-lang-countries
   ```

2. Прочитай PLAN.md, PORTING.md, docs/stage4-plan.md,
   docs/parallel-work.md и docs/handoffs/claude-wip-snapshot.md.
   Rust 1.95.0; Nix закреплён в flake.lock. Оригинал AyuGramDesktop —
   commit 0f2a31bb420de98f331990432265b131b8d836b5 (7.2.9).
   Оригинал только читать; полный Qt-клиент не собирать. Если клона нет,
   получи оригинал отдельно и настрой пути harness под эту машину.

3. В ветке уже сохранены незакоммиченные наработки прежнего Claude.
   Это WIP без гарантии сборки или паритета. Их нужно проверить и завершить;
   нельзя считать наличие файла доказательством готовности.

## Твоя область

- crates/tg-core/src/lang/**: ключи/теги, значения/фразы, plural rules,
  языковые файлы и сериализация пакетов, cloud manager и локализация;
- crates/tg-core/src/countries/**: instance и manager по оригиналу;
- crates/tg-core/src/core/credits_amount.rs и необходимый mod.rs;
- начатые value-контракты crates/tg-ui/src/text/** и effects/**;
- crates/tg-runtime/src/main_local.rs и его экспорт;
- необходимые изменения crates/tg-mtproto/src/sender.rs и mtp_instance.rs;
- crates/tg-codegen/src/lang/generator.rs, tg-core/build.rs, экспорты и
  Cargo.toml, необходимые для этих модулей;
- относящиеся к задаче тесты, fixtures и native targets core-base.

Заверши независимую headless-часть этого среза. Отложенные Main::Session,
переводчики с настоящим UI/платформой, layout/paint и окно не подменяй заглушками:
перечисли конкретные функции, причины и следующий этап в карте. Код главы
lang/countries должен быть экспортирован и использовать готовые workspace API.

## Что обязательно проверить

- Каждую функцию сверять с полным оригиналом и местами использования,
  включая патчи Ayu. Не улучшать поведение до паритета.
- QString/QDataStream и UTF-16 offsets сохранять, включая суррогаты и длины.
  Native fixtures для plural rules, lang pack и countries уже частично есть:
  проверить происхождение, добавить недостающие случаи, не ограничиваться roundtrip.
- Cloud manager/countries manager: реальные TL-запросы, порядок callback,
  отмена и повторение через существующий Sender и локальный тестовый сервер.
  Не выполнять живую авторизацию: PHONE_CODE_INVALID и прежний лимит попыток
  остаются отдельной задачей.
- MainLocal и local callbacks Sender: выполнение и уничтожение на главном
  потоке, отмена, drop, неверный поток. Не обходить ограничения unsafe/Send.
  Диагностический public request_body из WIP отдельно оценить; не оставлять
  лишнюю диагностическую поверхность ради удобства тестов.
- Генератор языка: реальные lang.strings, native reference, все формы/теги и
  использование сгенерированных фраз. Не сломать прежние тесты tg-codegen.

## Границы и сдача

Не менять crates/tg-cache/**, storage-файлы tg-core, PLAN.md, PORTING.md,
корневые README/изображение, PROGRESS.md и ledger/files.csv. При необходимости
общего контракта сначала описать фактическую зависимость; не переделывать чужую
подсистему. Cargo.lock менять лишь по необходимым зависимостям этой задачи.

Добавь crates/tg-core/docs/lang-countries-map.md: функция оригинала → реализация
→ конкретный тест/native sample, либо явно отложенная функция и причина.
Итоговый отчёт — docs/handoffs/claude-lang-countries.md: что готово, проверено,
что осталось, зависимости и предлагаемые статусы ledger по исходным путям.

Проверки:

```bash
nix develop --command bash -euc 'cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace'
```

Если Nix недоступен, установить ровно Rust 1.95.0 с rustfmt/clippy и выполнить
те же Cargo-проверки; способ и ограничения записать в отчёт. Отдельно выполнить
native references. Нельзя заявлять PASS для неисполненных проверок.

Сделай осмысленные коммиты без assistant attribution trailers и запушь
port/lang-countries в B2B-AI-SaaS-Startup-LLC/GramTele. Не пушить в main,
не делать force push, не вливать работу Codex самостоятельно. Передай Ярику
SHA и краткий отчёт; интеграцию и общий реестр обновит Codex.
