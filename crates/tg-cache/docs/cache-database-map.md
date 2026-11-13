# Сценарии lib_storage и границы переноса

Семантика и native: lib_storage **ccdc72548a5065b5991b4e06e610d76bc4f6023e**.
Полная [карта 252 функций](port-map.md) содержит реализацию и исполняемый тест
в каждой строке. PCH заменён импортами Rust. Не добавлено заглушек Main::Session
или Data; клиентский интерфейс — `tg_cache::cache::Database`.

## Исторические тесты базы

В закреплённом lib_storage тестов базы нет. По уточнению пользователя сценарии
взяты из [tdesktop fe618bd652, storage_cache_database_tests.cpp](https://github.com/telegramdesktop/tdesktop/blob/fe618bd652/Telegram/SourceFiles/storage/cache/storage_cache_database_tests.cpp)
(2019-02-19, 21 695 байт). Это набор сценариев, а не источник алгоритмов.
Старый .cpp **не включён в native**. Перенесены все 20 SECTION и отдельный
`init timers`; в Rust они объединены в шесть тестов
`tests/cache_database_legacy_scenarios.rs`.

| TEST_CASE / SECTION из fe618bd652 | Исполняемый тест Rust |
|---|---|
| init timers | legacy_init_timers |
| compacting db / simple compact with min size | legacy_compacting_min_size_without_min_double_and_time_tracking, вариант min-size |
| compacting db / simple compact without min size | тот же тест, without-min-size |
| compacting db / double compact | тот же тест, double |
| compacting db / time tracking compact | тот же тест, time |
| encrypted cache db / writing db | legacy_encrypted_write_read_tag_delete_overwrite_and_chunked_reads |
| encrypted cache db / reading and writing db | тот же тест |
| encrypted cache db / reading db | тот же тест, повторное открытие |
| encrypted cache db / deleting in db by tag | тот же тест, теги 1/2/3 |
| encrypted cache db / overwriting values | тот же тест, рост binlog и дедупликация |
| encrypted cache db / reading db in many chunks | тот же тест, 30 ключей, блоки 512, пакеты по 5 |
| cache db remove / db remove deletes value | legacy_remove_persists_and_bundled_touch_remove_lazy_or_on_close |
| cache db remove / db remove deletes value permanently | тот же тест, повторное открытие |
| cache db bundled actions / db touched written lazily | тот же тест, touch=true, on_close=false |
| cache db bundled actions / db touched written on close | тот же тест, touch=true, on_close=true |
| cache db bundled actions / db remove written lazily | тот же тест, touch=false, on_close=false |
| cache db bundled actions / db remove written on close | тот же тест, touch=false, on_close=true |
| cache db limits / db both limit | legacy_both_size_and_time_limits_with_access_updates, both |
| cache db limits / db size limit | тот же тест, size |
| cache db limits / db time limit | тот же тест, time |
| large db / time tracking large db | legacy_large_database_102400_records |

Время продвигается вручную. Интервалы старого AdvanceTime разбиты на виртуальные
шаги по 100 мс: таймеры выполняются во время интервала, а не все в его конце.
Нет QApplication, глобального семафора, случайного общего test.db или sleep.
Большой тест, выключенный флагом DisableLargeTest в 2019 году, включён:
102 400 put/get, лимит 1 MiB, сжатие, повторное открытие. Выборка get использует
воспроизводимый генератор вместо глобального rand; соли/имена в большом
нагрузочном тесте идут из обычного SystemRandom. В native-образцах энтропия
отдельно фиксирована и полностью воспроизводится.

В текущем API добавлены reconfigure/updateSettings и проверки закрытого binlog
для put/remove; код следует ccdc725, не старой реализации.

Старая проверка time limit предполагала, что touch успеет записаться перед
prune. В ccdc725 `useTime` обновляется при записи MultiAccess, а не при get.
При одновременном сроке сначала созданный prune выполняется раньше touch:
ключ 0 удаляется, ключ 1 сохраняется. Rust проверяет этот порядок явно;
native-проба `pending-access-prune` независимо подтверждает удаление ключа
с ещё не записанным touch. Алгоритм ccdc725 не изменён.
Тесты лимитов используют те же размеры классов значений (15/16 байт),
но часть диапазонов проверяется одинаковой полезной нагрузкой: формат,
теги и реальные смешанные размеры отдельно проверяются эталонами и API-тестом.

## Закреплённые тесты encrypted_file

Все девять конечных SECTION из двух TEST_CASE ccdc725 перенесены в
`tests/encrypted_file.rs`:

| Оригинал | Проверка Rust |
|---|---|
| simple encrypted file / writing file | original_sections_write_append_seek_read_and_move |
| simple / reading and writing file | тот же тест |
| simple / offset and seek | тот же тест, включая повторную запись мутированного шифротекста |
| simple / reading file | тот же тест |
| simple / moving file | тот же тест и независимые native moves |
| two process / writing file | original_two_process_read_and_writer_takeover |
| two process / start subprocess | тот же тест и original_child_process_file_holder |
| two process / read subprocess result | тот же тест, чтение при живом writer |
| two process / take subprocess result | тот же тест, реальный FileLock и завершение holder |
| ForkInit::Method / constructor / поиск executable | helper на std::process::current_exe и std::process::Command |

Готовность holder передаётся через pipe; другой pipe удерживает процесс до
захвата FileLock. Сам тест не спит. Исходный платформенный Qt/DLL bootstrap
заменён стандартным запуском Rust-процесса. Оба процессных теста доступны и
на Windows; в этой среде фактически выполнен Linux/POSIX вариант.

## Контракты и особенности ccdc725

- PlaceId — **7**, не 6 байт; EntrySize и RecordsCount — по 3. Имена файлов
  содержат сначала младший hex nibble; после первого байта идёт `/`.
- Новый open(ReadAppend) может создать `0/binlog`, не записав `version`.
  Это поведение оригинала сохранено. Version проверен отдельно через clear,
  восстановление, отрицательные/обрезанные значения и лишние байты.
- Оборванный/неизвестный хвост binlog откатывает позицию, но не усекает файл.
  MultiStore может применить корректные первые части перед отказом на следующей.
- `finish()` не выставляет finished на EOF; finished=true ставится при неверном
  типе/размере записи. Есть отдельная проверка этого поведения.
- Доступ обновляет useTime только при flush пакета. TrackEstimatedTime=false
  также отключает автоматическое вытеснение по размеру, как в оригинале.
- Равный возраст разрешается порядком unordered_map libstdc++ x86_64.
  Порт воспроизводит hash high XOR low, prime rehash и сохранение корзин после
  `_map = {}`. Native-пример size-ties подтверждает выбор жертвы; исходный
  порядок сам зависит от STL/платформы, межплатформенной гарантии у C++ нет.
- Compactor::start продолжает выполнение после fail; fail не проверяет guard.
  Сохранены backoff 10 s .. 24 h, общий guard, catch-up, ready rename и сброс
  wrap после замены. Переменная lastCatchUp в оригинале имеет тип int:
  порт сохраняет 32-битное сужение, без исправления переполнения.
- Flush binlog игнорируется там, где его игнорирует оригинал; значение обязано
  успешно flush до Store. Буферизированный write может пройти при ENOSPC,
  а flush — упасть. Native-пробы RLIMIT_FSIZE и /dev/full фиксируют это.
- QDir cleaner следует за symlink корня и удаляет содержимое цели; вложенный
  symlink удаляет как ссылку. Отдельная native-проба подтверждает это поведение,
  включая сохранение root-link на POSIX после неудачного rmdir.
- Cleaner игнорирует ошибку в DatabaseObject::cleanerDone, а waitForCleaner
  хранит один последний callback. Это не очередь независимых ожидателей.
- Databases выдаёт **один активный move-only DatabasePointer**, а не несколько
  одновременно живых ссылок. Повторное get допускается после destroy, пока
  отложенное удаление не выполнено на main. Ключ карты — буквальное написание
  пути, включая `./`; нормализуется только путь внутри самой Database.
- File::Move(from,from) удаляет файл и возвращает false — сохранённый дефект
  из предыдущего среза. POSIX lock может завершить другой writer, как исходник.

Rust выражает владение/предусловия через move, Option, Send и panic вместо
сырых указателей и Expects. Отдельных Rc<RefCell> на элементах кэша нет.
DatabaseObject, CleanerObject и Compactor живут на ObjectOnQueue; таймеры —
ConcurrentTimer. Stats — настоящий tg-rpl Producer; !Send Consumer остаётся
на main, через очередь передаются только subscription id и Send Stats.
Это локальный мост tg-cache: tg-runtime и область Claude не менялись.

## Что подтверждено и что не запускалось

На Linux x86_64 подтверждены двустороннее чтение, одинаковые байты десяти
баз, все четыре вида записей, старшие байты packed-полей, оба вида вытеснения,
сжатие и 22 ветки восстановления. Подробнее —
[нативная проверка](../tests/database-reference/README.md).

Windows/macOS runtime и их реальные файловые блокировки не запускались.
Не моделировались аварийное отключение питания, произвольные гонки с внешним
редактором файлов и Qt-специфичные Unicode/ACL варианты сортировки/удаления
нестандартных старых каталогов. std::fs — платформенная замена QFile/QDir;
гарантия побайтовой совместимости подкреплена перечисленными эталонами,
а не утверждением о проверке любой файловой системы и любого сбоя ОС.
