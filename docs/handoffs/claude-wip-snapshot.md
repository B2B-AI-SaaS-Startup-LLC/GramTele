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
