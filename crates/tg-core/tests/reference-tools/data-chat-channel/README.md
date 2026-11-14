# Native rights / chat-channel pure phases

Оригинал `0f2a31bb420de98f331990432265b131b8d836b5`, build вне клона:
`/tmp/ayugram-data-chat-channel-native`. GCC15.3 Linux x86_64 `-O2`, Qt6.11.2
Core, C++23. Original clone read-only; ни аккаунты, ни tdata не открываются.

Настоящий `data_chat_participant_status.h` включён целиком. `extract.py` берёт
неизменённые тела rights conversions и send-restriction unions из `.cpp`;
используются настоящие generated scheme.h + исходные class/TypeCreator blocks
chatAdminRights/chatBannedRights и lib_base/lib_tl. UI include-зависимости других
тел не подменяются фиктивными моделями.

Из `ChannelData::setFlags` исполняется **только неизменённая чистая начальная
фаза** нормализации which, до `const auto diff`: MonoforumAdmin→Monoforum→Megagroup,
Forum→Megagroup, Monoforum выключает Forum. В harness добавлена свободная функция
с этим исходным префиксом и return which; это не запуск полного ChannelData.
`IsRestrictedForever` — исходное inline тело и его исходная INT_MAX константа.
Исходные enum declarations взяты без перестановок. Полные classes/Session,
наследуемые canX/amRestricted и последствия изменения флагов — **own tests + чтение**.
Полный data_channel.cpp syntax probe сначала требует отсутствующий tl/expected.hpp;
предыдущий data-peer probe с реальными class headers также требует generated lang_auto.h,
UI/styles и Main/API владельцев; их fake-замен не вводилось. Возможность будущей
честной Qt headless сборки полного графа не объявляется опровергнутой.

`filterZalgo` и kZalgoPattern извлечены неизменёнными из ayu/utils/telegram_helpers.cpp.
Все Unicode scalar values проверены настоящим QRegularExpression на `\p{Mn}`:
365 диапазонов составляют зафиксированную production таблицу Rust. Не используется
приближение по combining blocks. Алгоритм учитывает 3 scalar characters для regex
quantifier, затем число UTF-16 units для замен на U+2060, включая supplementary Mn.
Bidi controls заменяются по одному, как исходный regex. Это фактическая Qt/PCRE
таблица данной сборки; другие версии Unicode/Qt не сверялись. Rust UTF-8 строки
не выражают lone surrogate QString.

`pure.csv`: **481** строка: 37 rights масок (каждый бит0..31 и 5 смешанных),
1 строка send unions, 365 Mn ranges, 6 Zalgo строк, 67 normalization масок
(каждый бит0..63 и 3 mixed), 5 forever значений. Проверяются raw internal flags
и обратные MTP flags: в частности ProcessJoinRequests не переводится, а
ManageLinkedPeers/ManageWelcomeMessages имеют иные TL bit positions. Это исходник,
не усовершенствование порта. Подтверждение native не ограничивается roundtrip Rust.
Два запуска побайтно совпадают; SHA256 источников/generated/CSV — в manifest.json.

## Повторение

Original generated scheme.h/.cpp: инструкция
[tg-scheme reference](../../../../tg-scheme/tests/reference-tools/README.md).
Проверенный каталог `/tmp/ayugram-tl-reference/gen`. Нужны Qt6 Core (с настоящими
QtGui headers для исходного prefix), Microsoft GSL, range-v3, CMake/Ninja и Python3.

```bash
cmake -G Ninja -S crates/tg-core/tests/reference-tools/data-chat-channel \
  -B /tmp/ayugram-data-chat-channel-native \
  -DTELEGRAM_ROOT=/path/to/AyuGramDesktop/Telegram \
  -DSCHEME_INCLUDE=/path/to/generated/scheme \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayugram-data-chat-channel-native -j2
python3 crates/tg-core/tests/reference-tools/data-chat-channel/capture.py \
  /tmp/ayugram-data-chat-channel-native/native_groups \
  /path/to/AyuGramDesktop/Telegram /path/to/generated/scheme \
  crates/tg-core/tests/fixtures/data-chat-channel
python3 crates/tg-core/tests/reference-tools/data-chat-channel/validate.py
nix develop --command cargo test -p tg-core --test data_chat_channel
```

Capture требует закреплённый HEAD и совпадение двух запусков. Порт production Mn
таблицы проверяется validate.py против native fixture; для её пересоздания пары
mn,start,end буквально переносятся в константу MN telegram_helpers.rs, без изменения
порядка. Остальные функции большого telegram_helpers.cpp здесь не портируются.
