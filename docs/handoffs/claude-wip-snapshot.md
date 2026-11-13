# WIP-снимок для Claude, 13.11.2026

Ветка handoff/claude-lang-countries начинается с main 82200d4 и содержит
снимок незакоммиченных наработок из port/core-base (be00773). Исходное рабочее
дерево на машине Ярика не изменялось. Файлы и SHA256 перечислены в
claude-wip-manifest.json.

Скопированы lang/countries, credits_amount, text/effects value types,
MainLocal/runtime, local callbacks Sender, диагностический request_body,
генератор/сборка языка, относящиеся fixtures и настройки зависимостей.
Native pack/countries targets добавлены к текущему CMake; target проверки
storage trace из main сохранён.

Это архивирование начатой работы: сборка, тесты и семантический паритет
данного WIP ещё не подтверждены. Существующие тесты main прошли до добавления
этого снимка. Имена и наличие функций не означают статус done.

Не перенесены старые правки storage_sparse_ids_list, cloud_blob и storage_lists
из грязного дерева: эти модули уже завершены в main 819d7f5 с FlatSet/EnumMask.
Нельзя возвращать их старые версии. Настоящая пользовательская tdata и
учётные данные не включены.

Текущее задание: ../tasks/claude-lang-countries.md.
Создай port/lang-countries от этой ветки и доведи назначенную область до
проверенного результата. Не вливай WIP напрямую в main.

## Известные результаты CI снимка f0d36a3

https://github.com/B2B-AI-SaaS-Startup-LLC/GramTele/actions/runs/43383988852

fmt и clippy прошли; cargo test остановился на tg-core --lib: 48 passed,
2 failed. Это исходные незавершённые тесты WIP, не новые ошибки tg-cache:

- lang::lang_instance::tests::test_languages_replace_every_value,
  lang_instance.rs:1548: left "________", right "XXXXXXXX";
- lang::lang_values::tests::reactive_values_follow_the_language_and_the_arguments,
  lang_values.rs:399: left ["A B", "C B", "Eng", "B, C", "D, C"],
  right ["A B", "C B", "B, C", "D, C"].

Причина ещё не установлена. Начни с воспроизведения и сверки поведения
оригинала; не убирай проверки и не отфильтровывай события ради зелёного теста.
