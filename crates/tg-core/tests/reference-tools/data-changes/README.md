# Native Data::Changes

Проверен оригинал `0f2a31bb420de98f331990432265b131b8d836b5`, GCC 15.3.0,
Linux x86_64, Qt 6.11.2. Настоящий `data_changes.cpp` включён неизменённым в TU,
с настоящими `data_changes.h`, `main_session.h`, lib_base/lib_rpl/lib_crl.
`private public` открывает менеджеры только в тестовом TU. Нет подмены тел
Session/PeerData/History или алгоритмов Changes. Build лежит **вне клона**.
Полные hashes оригинала, generated scheme и результатов — в
[manifest.json](../../fixtures/data-changes/manifest.json).

Адреса синтетических aligned byte buffers — только непрозрачные identity tokens.
Они никогда не разыменовываются как модели. Для проверки `Changes::sendNotifications`
конструктор сохраняет непрозрачный Main::Session token; `_notify` выставляется
тестом, события ставятся настоящими private Manager. `session()` и
`scheduleNotifications()` не вызываются: первый разыменовывает Session, второй
создаёт weak guard, которому нужен настоящий сконструированный Main::Session.
Его конструктор требует Account/MTPUser/settings и запускает Data/API/storage
сервисы. Их здесь нет, тела не подставлялись. Эти две привязки и main-queue
реентерабельность имеют статус **own tests + чтение**, до пакета lifecycle 29.
Компиляция полного .cpp не объявляется выполнением всех его методов.

`-ffunction-sections` / `--gc-sections` удаляют неиспользованные lifecycle
bindings при линковке. Qt Core нужен для QString и настоящих исходных headers;
Qt GUI объекты, пользовательские данные и сеть не используются.

Результаты `-O0` и `-O2` побайтно совпали во всех трёх CSV:

- `flags.csv`: 148 значений, включая None, LastUsedBit, дыры Topic/Sublist,
  все 20 ChatAdminRight и все 58 Peer flags;
- `peer-shifts.csv`: каждый отдельный бит 0..57 и три mixed/raw маски, 242 строки;
- `managers.csv`: 210 строк — realtime с полными входными flags, repeated OR,
  postponed/filter/flagsValue, dropScheduled с объединением и erase до callback,
  drop, нулевая маска, flush внутри realtime и postponed callback,
  snapshot pending, 7 категорий и порядок общего flush.

C++ сортирует flat_map по **адресам**. Native tokens a/b расположены по возрастанию
адресов и нормализованы в 1/2; Rust тест использует возрастающие typed keys 1/2.
Это доказывает одинаковый алгоритм sorted snapshot, но не равенство порядка
произвольных C++ адресов порядку wire ID. Изменение идентичности/порядка явно
описано в [карте](../../../docs/data-changes-map.md).

## Ошибка realtime PeerUpdate

[Оригинал, строка 37](https://github.com/AyuGram/AyuGramDesktop/blob/0f2a31bb420de98f331990432265b131b8d836b5/Telegram/SourceFiles/data/data_changes.cpp#L37)
использует `1U << i` при `kCount=58`. Для i>=32 это undefined behavior в C++.
На проверенном GCC/x86_64 в обоих режимах сдвиг оборачивается по модулю 32:
бит 0 вызывает streams 0 и 32; бит 25 — 25 и 57; биты 26..31 — только себя;
изолированные биты 32..57 **не вызывают ни одного realtime stream**.
Postponed получает все исходные 64 бита; realtime payload тоже содержит полную
маску, в том числе старшие биты, когда доставку запустил младший бит.
Rust явно использует `1u32.wrapping_shl`, фиксируя это измеренное поведение
без UB. Поведение других C++ компиляторов/архитектур не доказано.

## Воспроизведение

Нужен оригинальный generated `scheme.h`; его получение описано в
[tg-scheme reference](../../../../tg-scheme/tests/reference-tools/README.md).
Проверенный каталог — `/tmp/ayugram-tl-reference/gen`. Нужны C++23, Qt6 Core,
Microsoft GSL, range-v3; пути задаются для своей машины:

```bash
cmake -G Ninja -S crates/tg-core/tests/reference-tools/data-changes \
  -B /tmp/ayugram-data-changes-native \
  -DTELEGRAM_ROOT=/path/to/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/path/to/generated/scheme \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayugram-data-changes-native -j2
python3 crates/tg-core/tests/reference-tools/data-changes/generate.py \
  /tmp/ayugram-data-changes-native crates/tg-core/tests/fixtures/data-changes \
  --telegram-root /path/to/AyuGramDesktop/Telegram \
  --scheme-include /path/to/generated/scheme
nix develop --command cargo test -p tg-core data_changes
```

Генератор требует закреплённый HEAD, запускает оба executable и отказывается
писать fixtures при любом различии O0/O2. Rust тесты читают эти CSV, не запускают
C++ на каждой сборке. Исходных Changes tests в закреплённом data-каталоге нет.
