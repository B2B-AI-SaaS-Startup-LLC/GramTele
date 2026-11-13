# Нативный эталон ID ядра

Оригинал AyuGramDesktop 0f2a31bb420de98f331990432265b131b8d836b5 только
читается. Отдельный executable компилирует настоящие data_peer_id.cpp/.h и
data_msg_id.h. Подстановок PeerId/MsgId/TextWithEntities/CreditsAmount нет.
Из generated scheme.cpp берётся неизменённый Peer-блок с тремя creator methods;
полный исходный scheme.h используется без правок. Qt Core нужен для TL и
QDataStream. QtGui headers приходят из оригинального text_entity.h, однако
GUI-объекты и весь клиент не создаются; исполняемый файл линкуется с Qt Core.

Нужны generated scheme.h/.cpp от исходного TL generator. Их получение уже
описано в [tg-scheme reference](../../../../tg-scheme/tests/reference-tools/README.md).
В проверенном запуске это /tmp/ayugram-tl-reference/gen. Полные SHA256 исходных
трёх файлов и generated scheme записываются в manifest.json.

Конфигурация отдельного CMake (пути задаются для своей машины):

```bash
cmake -G Ninja -S crates/tg-core/tests/reference-tools/data-ids \
  -B /tmp/ayu-data-ids-native \
  -DTELEGRAM_ROOT=/path/to/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/path/to/generated/scheme \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayu-data-ids-native -j2
python3 crates/tg-core/tests/reference-tools/data-ids/generate.py \
  /tmp/ayu-data-ids-native/native_data_ids crates/tg-core/tests/fixtures/data-ids \
  --telegram-root /path/to/AyuGramDesktop/Telegram \
  --scheme-include /path/to/generated/scheme
nix develop --command cargo test -p tg-core --test data_ids
nix develop --command cargo test -p tg-core --doc
```

CMake находит Qt6 Core и Python3; нужны C++23, headers QtGui/GSL/range-v3.
На Nix используйте тот же pinned nixpkgs и native shell, что описан в соседнем
tdata/reference harness. Build находится вне клона оригинала.

Генератор пишет только cases.txt/native.txt/manifest.json в выбранный выход.
IDs синтетические. Seed 0xA71D2026, 1792 команды:

- P: конструкторы и классификация, извлечение bare, все legacy tags и 256 raw
  type bytes, marker bit 55 и верхние биты, отрицательные MTPint, boxed TL
  input/output, serializable PeerId и его Qt_5_1 big-endian bytes;
- M: границы диапазонов, conversions, defined-domain arithmetic, prefix/postfix,
  truthiness FullMsgId/FullStoryId/GlobalMsgId и сравнения;
- C/I: все исходные константы, client/story index conversion, включая индекс
  больше диапазона StoryId (разрешён исходной функцией).

Native reference не выполняет signed overflow/нарушения Expects (C++ может
завершить процесс); Rust проверки этих условий отдельные. Четыре compile-fail
примера проверяют типовую границу. Сетевых запросов, аккаунтов, tdata или GUI
в этих проверках нет. deferred MessageHighlight/FullReplyTo/SuggestOptions не
объявлены проверенными за счёт одного наличия их исходного заголовка.
