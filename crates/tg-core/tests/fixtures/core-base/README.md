Нативный оригинал: AyuGramDesktop `0f2a31bb420de98f331990432265b131b8d836b5`.
`tests/reference-tools/core-base` собирается отдельным CMake вне клона.
`TELEGRAM_ROOT`, `GSL_INCLUDE`, `RANGE_INCLUDE` задаются при конфигурации;
Qt6 Core и Python нужны для сборки. Исходный клон не изменяется.

* `native_lists` → `sparse-native.txt`: неизменённый
  storage_sparse_ids_list.cpp; адаптер сильного MsgId содержит только его
  64-битное значение, сравнение MsgRange и исходную границу 1<<56.
* `native_lists photos` → `photos-native.txt`: неизменённый
  storage_user_photos.cpp, события после каждой операции, включая
  промежуточное событие detachBack и неизвестный count.
* `native_plurals | gzip -n` → `plurals-native.txt.gz`: функции
  ChoosePlural*, PluralsKey и GeneratePluralRulesMap извлечены из исходного
  lang_tag.cpp без изменения тела; каждая запись действующей таблицы,
  i=0..220, v=0..2. Проверяет условия правил, не округление float/QLocale.

Это эталоны исполнения C++; собственные тесты cloud blob не являются
эталоном исполнения C++.

`storage-trace.txt` — 7031 команда с фиксированным seed 0xA720261113;
`storage-trace-native.txt` — результаты их исполнения target
`native_storage_trace`. Он компилирует неизменённые sparse/shared-media/
user-photos/facade .cpp, включая настоящие rpl и base::flat_set. Адаптеры
идентификаторов те же, что у native_lists. Ненужные платформенные функции
отсекаются linker GC; Core/Session/UI не создаются.

Тест `native_storage_operations_match_each_event_and_query` воспроизводит
команды через Rust Facade и сравнивает каждое событие, завершение producer,
snapshot/query, empty, countAfter и число вызовов предиката. Есть дубликаты,
пустые/соприкасающиеся/раздельные диапазоны, нулевые счётчики, Ayu reversed
range, IDs >32 бит, все 13 типов медиа, темы и монофорумы. Искусственные
значения не взяты из аккаунта. SHA256 входа/ответов — в manifest JSON.

Пересоздание после конфигурации того же отдельного CMake:

```bash
cmake --build /tmp/ayugram-storage-trace-reference --target native_storage_trace -j2
python3 crates/tg-core/tests/reference-tools/core-base/generate_trace.py \
  /tmp/ayugram-storage-trace-reference/native_storage_trace \
  crates/tg-core/tests/fixtures/core-base
nix develop --command cargo test -p tg-core --test storage_trace
```

Генератор перезаписывает три storage-trace файла в заданном выходном каталоге.
Отложенное чтение при подписке и отписка проверяются отдельными Rust-тестами;
native trace не заявляет сравнения реентерабельных мутаций из callbacks,
переполнений signed C++ или поведения producer после уничтожения владельца.
