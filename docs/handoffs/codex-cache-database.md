# Codex: база кэша медиа

Результат ветки `port/cache-database`, основание `main` 20695cb.
Проверено для интеграции fast-forward в main и пуша обеих ссылок в
[B2B-AI-SaaS-Startup-LLC/GramTele](https://github.com/B2B-AI-SaaS-Startup-LLC/GramTele).
Без force push. Область Claude не изменялась.

Завершены открытие/закрытие, клиентские операции, явный LE codec всех записей,
восстановление, delayed bundles, учёт доступа, size/time eviction, pruning,
compactor с catch-up/ready rename/backoff, cleaner, legacy cleanup и Databases.
Объекты базы и workers используют ObjectOnQueue, таймеры — ConcurrentTimer.
Stats приходит настоящим tg-rpl Producer на main. Ядро Main/Data не имитируется.

## Учёт каждого файла

Исходник: Telegram/lib_storage/storage. Всего **24 файла, 4 598 строк**:
**23 done, 1 replaced; partial/todo = 0**. Предыдущие три partial закрыты:
File.h/.cpp — добавлены QFile write buffering и native fault-пробы;
encrypted_file_tests.cpp — все секции и двухпроцессный IPC без sleep.
Ограничение Windows/macOS runtime явно сохранено ниже.

| Файл | Статус |
|---|---|
| cache/storage_cache_binlog_reader.cpp | done |
| cache/storage_cache_binlog_reader.h | done |
| cache/storage_cache_cleaner.cpp | done |
| cache/storage_cache_cleaner.h | done |
| cache/storage_cache_compactor.cpp | done |
| cache/storage_cache_compactor.h | done |
| cache/storage_cache_database.cpp | done |
| cache/storage_cache_database.h | done |
| cache/storage_cache_database_object.cpp | done |
| cache/storage_cache_database_object.h | done |
| cache/storage_cache_types.cpp | done |
| cache/storage_cache_types.h | done |
| cache/storage_encrypted_file_tests.cpp | done |
| storage_clear_legacy.cpp | done |
| storage_clear_legacy.h | done |
| storage_clear_legacy_posix.cpp | done |
| storage_clear_legacy_win.cpp | done |
| storage_databases.cpp | done |
| storage_databases.h | done |
| storage_encrypted_file.cpp | done |
| storage_encrypted_file.h | done |
| storage_encryption.cpp | done |
| storage_encryption.h | done |
| storage_pch.h | replaced |

PCH заменён импортами Rust; runtime-методов в нём нет.
[port-map.md](../../crates/tg-cache/docs/port-map.md): **252 строки функций,
252 реализации, 252 с исполняемым тестом, 0 отложенных**; ссылки на 31 имя
теста. [Карта сценариев](../../crates/tg-cache/docs/cache-database-map.md)
разделяет ccdc725 и исторические тесты fe618bd652: все 20 SECTION плюс init timers,
включая ранее выключенный large db на 102 400 записей.

## Независимая проверка

Настоящие POSIX lib_storage + нужные lib_base/lib_crl собраны отдельным CMake
в /tmp/ayugram-cache-database-native, без изменения клона оригинала.
Qt 6.11.2, OpenSSL 3.6.5, XXH32, GCC 15.3; нет подмен алгоритмов базы.
Fixed key/salt/clock, исходные SHA256 и commits — в
[manifest.json](../../crates/tg-cache/tests/database-reference/manifest.json).

- Десять баз: empty, stores, plain, removed, size, size-ties, time,
  compact, all-records, wide. **284 файла, 130 848 байт** совпали полностью.
- Rust читает C++ и возвращает те же значения/теги. C++ читает Rust и
  возвращает те же значения/теги. После чтения совпадают **284 файла,
  135 360 байт**, включая дописанные Access.
- 22 recovery-пробы: **72 файла, 14 797 байт** и results.csv совпали по
  Error/path, count, возвращённым данным и конечным файлам.
- Original QFile отдельно подтвердил short write, ENOSPC и flush failures
  (RLIMIT_FSIZE, /dev/full); native cleaner подтвердил root/nested symlinks.
- Wide проверяет значение 0x12345 и пакет из 257 записей. PlaceId в исходнике
  занимает **7 байт**, sizes/counts — по 3, а не указанные в задании 6.

Воспроизведение: [database-reference/README.md](../../crates/tg-cache/tests/database-reference/README.md).
Старая версия storage_cache_database_tests.cpp не включена в native;
она использована только как сценарии, по уточнению пользователя.
Реальная tdata не открывалась.

## Проверки и зависимости

Одна команда PORTING.md выполнена успешно:

```text
cargo fmt --check                                  PASS, exit 0
cargo clippy --workspace --all-targets -- -D warnings
  Finished dev profile in 5.08s                    PASS, exit 0
cargo test --workspace
  678 passed; 0 failed; 3 existing ignored         PASS, exit 0
```

Все команды через `nix develop --command`, Rust 1.95.0.
В tg-cache **40 тестов**, было 13: +27, без новых ignored.
Большой сценарий в общем прогоне — 90.40 s вместе с остальными сценариями
того бинарника. Native verify.py завершился `bidirectional original-code reference: PASS`.

Новых Rust-пакетов или версий нет. Добавлены существующие workspace-зависимости
`tg-runtime`, `tg-rpl`; у существующего nix включён feature resource для теста
RLIMIT_FSIZE. XXH32 реализован внутри codec и проверен по настоящему xxhash.
Qt/OpenSSL/GSL/range-v3/xxhash нужны только для отдельного native-harness.

## Особенности оригинала и оставшаяся проверка

Подробности — в карте сценариев. Сохранены:
первоначальный open без version; откат позиции без truncate; частичное
применение MultiStore; finish() без finished на EOF; отложенное обновление
useTime; отключение size eviction при trackEstimatedTime=false; равные возраста
по Linux/libstdc++; продолжение Compactor::start после fail и незащищённые
повторные уведомления; 32-битный lastCatchUp; игнорирование flush binlog и
ошибки cleaner; один последний waitForCleaner callback; Move(from,from),
удаляющий файл. Root-directory symlink в QDir cleaner следует в целевой каталог,
а вложенный symlink удаляется как ссылка — это тоже сохранено и проверено
только на синтетических папках.

Databases имеет один активный move-only указатель, а не несколько одновременно
живых ссылок. Re-get отменяет отложенный erase после close/cleaner/main;
ключ — буквальный путь, даже если ./ ведёт к тому же каталогу.
Rust заменяет сырые указатели на владение/Option/Send, Expects — на panic,
а локальный bridge Stats не меняет зарезервированный Claude tg-runtime.

Неподтверждённое: Windows/macOS runtime, их native locks, ACL/нестандартные
Unicode-имена старых каталогов, power loss и произвольные внешние гонки ФС.
При равном возрасте порядок на другой C++ STL может отличаться: порт фиксирует
проверенный Linux/libstdc++ порядок. В проверенных байтах расхождений нет.
Это границы проверки, а не скрытые TODO или заглушки. Вопросов пользователю нет.
