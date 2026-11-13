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
