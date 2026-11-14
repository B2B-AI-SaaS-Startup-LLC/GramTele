# PeerData native: границы доказательства

Оригинал `0f2a31bb420de98f331990432265b131b8d836b5`, GCC15.3/x86_64 `-O2`,
Qt6.11.2 Core. Сборка `/tmp/ayugram-data-peer-native` вне read-only клона.
Ни настоящая tdata, ни аккаунт, ни сеть Telegram не используются.

CMake компилирует настоящий `data_user_names.cpp`, lib_base CRC/assert/integration,
lib_tl и неизменённые тела чистых функций, извлечённые `extract.py` из оригинала.
Используются настоящие Birthday/Lastseen/Flags/PeerId/ImageLocation headers,
исходные enum User/Chat/Channel/Restrictions, исходные TL generated definitions
peerColor/profileTab и их TypeCreator методы. Алгоритмы не переписываются в harness.
`#define private public` позволяет только задать поля реального StorageFileLocation.
QImage нужен при разборе реального заголовка: подключены настоящие QtGui headers,
объекты QImage не создаются, QtGui не линкуется. Сетевых/MTP owners нет.

`QDate::currentDate` перехвачен GNU ld `--wrap`, возвращает настоящий фиксированный
QDate. BirthdayAge исполняет оригинальное тело; часы не заменены своим алгоритмом.
Чистые Qt RemoveAccents/PrepareSearchWords/IsDiacritic извлечены неизменёнными для
reference-only: production Rust backend **не** объявлен переведённым. Проверка
UTF-16 индекса использует строгий тестовый adapter шести записанных пар, который
падает на незаписанном входе. Другой тестовый ограниченный ASCII/Cyrillic adapter
проверяет композицию fillNames, а не полноту Unicode нормализации.

`pure.csv` — 4247 детерминированных строк (два запуска побайтно совпали):
3960 дат, 20 BirthdayAge/Today, 82 Lastseen packed/OnlineTill/legacy,
124 значения enum flags, 7 событий essential flags, 7 регистраций,
3 PeerId/Service классификации, 7 translit/keyboard + 7 fake-name CRC IDs,
6 Qt normalization, Usernames append/index, 3 состояния RestrictionCheckResult,
2 image equality сценария, 4 case-fold spambot, 8 MTP profile tabs и 6 color inputs.
Rust `native_pure_functions_reference` + `native_qt_normalization_samples_preserve_utf16_index_values`
сверяют эти результаты. Полные SHA256 оригинала/generated/fixture — в
[manifest.json](../../fixtures/data-peer/manifest.json).

Полный `data_peer.cpp` отдельно проверялся `-fsyntax-only` с реальными include
roots lib_* и Qt headers. Разбор доходит до `lang/lang_keys.h`, затем требует
отсутствующий generated `lang_auto.h`. Эта цель также включает styles/UI и
Main::Session/API владельцев. Никакие фиктивные заголовки/тела вместо них не
добавлялись; текущая pure-цель не доказывает невозможность будущей честной Qt
headless сборки. Состояние PeerData, наследуемые предикаты/ограничения через
реальные поля projections, Changes reentrancy и последовательности эффектов —
**own tests + чтение**. Только RestrictionCheckResult value helpers — native,
не полная PeerData::amRestricted на настоящих User/Chat/Channel объектах.

Измерены исходные особенности BirthdayAge(29 февраля), смешанные integer
promotions Lastseen, editable index при append, equality ImageLocation и Qt
case folding. Отдельный out-of-range editable index случай C++ имеет UB:
native его не исполняет, Rust сохраняет индекс и явно паникует при обращении.
Signed-overflow Lastseen края — фактический GCC15.3/x86_64 O2 результат, не
обещание переносимости C++ UB. Новые Rust зависимости/версии не добавлены.

## Повторение

Generated `scheme.h/.cpp` получить оригинальным генератором по
[tg-scheme reference](../../../../tg-scheme/tests/reference-tools/README.md).
Проверенная директория `/tmp/ayugram-tl-reference/gen`. Нужны C++23, Qt6 Core
(с настоящими QtGui headers), Microsoft GSL и range-v3. В Nix доступны Qt Core
из текущего build environment и CMake/Ninja; внешние include paths задаются явно.

```bash
cmake -G Ninja -S crates/tg-core/tests/reference-tools/data-peer \
  -B /tmp/ayugram-data-peer-native \
  -DTELEGRAM_ROOT=/path/to/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/path/to/generated/scheme \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayugram-data-peer-native -j2
python3 crates/tg-core/tests/reference-tools/data-peer/capture.py \
  /tmp/ayugram-data-peer-native/native_data_peer \
  /path/to/AyuGramDesktop/Telegram /path/to/generated/scheme \
  crates/tg-core/tests/fixtures/data-peer
python3 crates/tg-core/tests/reference-tools/data-peer/validate.py
nix develop --command cargo test -p tg-core --test data_peer
```

`capture.py` проверяет закреплённый git HEAD и детерминизм двух запусков;
генератор исходных тел не пишет в оригинал. Пробная full-цель не входит в
native CMake PASS; её неподтверждённость указана явно.
