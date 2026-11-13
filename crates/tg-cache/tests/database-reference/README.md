# Native-эталон базы медиа

Источник — неизменённые файлы AyuGramDesktop
`0f2a31bb420de98f331990432265b131b8d836b5`, lib_storage
`ccdc72548a5065b5991b4e06e610d76bc4f6023e`.
Отдельный CMake собирает все применимые POSIX .cpp lib_storage и необходимые
реализации lib_base/lib_crl. Queue, ConcurrentTimer, rpl, QFile/QDir, XXH32
и OpenSSL настоящие. Не собирается клиент, нет подмен Main/Data или алгоритма
базы. В prefix добавлены заголовки прежнего PCH (включая QMutex).
Старые тесты fe618bd652 в native не входят.

Обвязка перехватывает только RAND_bytes и time для фиксированных данных.
Ключ: байты 00..3f. Базовое время: 1700000000; изменения времени перечислены
в main.cpp. Соль, имя значения и padding используют один последовательный
источник энтропии, как оригинал. Для wide — counter XOR counter>>8 XOR counter>>16,
для остальных — младший байт counter. Compactor работает на своей настоящей
очереди. Prune запускается нулевым таймером после updateSettings: лимиты
включаются после записи исходного набора, чтобы порядок потоков Qt не менял
число промежуточных пакетов удаления. Проверка читателя отключает новый
автоматический compactor; генерация/проверка самого сжатия его не отключает.
Иначе 257 touch могут начать ещё одно сжатие одновременно с close, и оригинал
иногда оставляет binlog-temp — это уже отдельная гонка завершения worker.

Из корня Rust-репозитория, с оригиналом только для чтения:

```bash
export AYUGRAM_ORIGINAL=/path/to/AyuGramDesktop
nix-shell -E 'let
  f = builtins.getFlake (toString ./.);
  p = f.inputs.nixpkgs.legacyPackages.x86_64-linux;
in p.mkShell {
  packages = with p; [ cmake ninja pkg-config qt6.qtbase openssl microsoft-gsl range-v3 xxHash python3 ];
  CACHE_GSL_INCLUDE = "${p.microsoft-gsl}/include";
  CACHE_RANGE_INCLUDE = "${p.range-v3}/include";
  CACHE_XXHASH_INCLUDE = "${p.xxHash}/include";
}' --run 'cmake -G Ninja -S crates/tg-cache/tests/database-reference \
  -B /tmp/ayugram-cache-database-native \
  -DTELEGRAM_ROOT="$AYUGRAM_ORIGINAL/Telegram" \
  -DGSL_INCLUDE="$CACHE_GSL_INCLUDE" -DRANGE_INCLUDE="$CACHE_RANGE_INCLUDE" \
  -DXXHASH_INCLUDE="$CACHE_XXHASH_INCLUDE" &&
  cmake --build /tmp/ayugram-cache-database-native -j3 &&
  python3 crates/tg-cache/tests/database-reference/verify.py \
    /tmp/ayugram-cache-database-native/native_cache_database'
```

Фактически выполнено на Linux x86_64: Qt 6.11.2, OpenSSL 3.6.5, GCC 15.3,
Rust 1.95.0. Build directory — /tmp, исходный клон не менялся.
CMake может потребовать OPENSSL_ROOT_DIR/Qt6_DIR из пакетов этого shell,
если на машине есть конфликтующие системные библиотеки.

verify.py каждый раз создаёт новые синтетические каталоги. Он проверяет:

1. Оригинал заново генерирует десять баз, одинаковых с fixtures/database.
2. Rust выполняет те же операции: совпадают все имена и полные байты файлов.
3. Rust читает копии баз оригинала и проверяет значения/теги/состав.
4. Оригинал читает копии баз Rust; совпадают и последующие байты binlog.
5. Оригинал воспроизводит 22 повреждения и полные конечные файлы из
   fixtures/database-probes; cargo test повторяет их портом.
6. Original QFile даёт те же short-write/ENOSPC/flush результаты; отдельная
   native-проба cleaner сравнивает root/nested symlinks, readonly и hidden.

Повторная генерация сверяет manifest.json; изменить его можно только явно
через --update-manifest после проверки новых образцов. Реальная пользовательская
tdata не открывается ни одним сценарием.

| Образец | Проверка |
|---|---|
| empty | пустой encrypted binlog; в оригинале первоначальный version может отсутствовать |
| stores | четыре Store, MultiRemove и MultiAccess; остаются три значения |
| plain | Store без времени, MultiRemove, trackEstimatedTime=false |
| removed | удалены все четыре значения |
| size | после записи включён лимит 200; из трёх значений по 120 осталось последнее |
| size-ties | те же размеры, одинаковый возраст; воспроизведён выбор unordered_map |
| time | после записи включён TTL=5; остаётся одно молодое значение |
| compact | замена значения запускает сжатие; MultiStore с текущими четырьмя значениями |
| all-records | MultiStore после сжатия, затем Store, MultiRemove, MultiAccess |
| wide | значение 0x12345 байт и MultiStore из 257 частей; старшие байты packed-полей |

22 recovery-пробы: torn-tail, unknown-tail, zero-store, zero-remove, large-count,
zero-size, large-size, multi-partial, missing-value, short-value,
short-value-header, checksum, wrong-value-key, wrong-binlog-key, short-binlog,
bad-format, track-mismatch, wrong-key-clear, bad-version, negative-version,
ready-recovery, pending-access-prune. results.csv фиксирует Error/path, count
до/после get и полные возвращённые данные. Сравниваются также все конечные
файлы: перенос на другую версию, очистка старой, позиция перезаписи хвоста,
удалённые значения, отложенные Remove/Access.

file-faults.csv снят с оригинала в отдельном процессе: RLIMIT_FSIZE=124
для маленькой буферизированной и большой прямой записи; /dev/full для ENOSPC.
Rust повторяет это отдельным helper-процессом; SIGXFSZ безопасно заблокирован
в его потоке, ограничение снимается после каждого случая. В production
добавлены буфер 16 KiB и порядок flush на read/seek/close, как у QFile.
Новые версии зависимостей для этого не добавлялись.

Сжатие с min-size ratio, без ratio, дважды и с учётом времени дополнительно
проходит как перенос сценариев 2019 года; нагрузочный тест выполняет 102 400
операций. Отдельно проверены catch-up непустого хвоста, ветки раннего возврата,
сбой назначения compactor и повтор по фактическому виртуальному deadline, удвоение задержки от 10 s до 24 h.

Windows/macOS runtime, внезапное отключение питания и произвольные внешние
гонки изменения файлов не снимались: доступен Linux-эталон. Это ограничение
области независимой проверки, а не замена native-эталона round-trip тестами.


cleaner-check.csv фиксирует именно QDir: root symlink очищает каталог цели,
вложенный symlink удаляется как ссылка, hidden остаётся, readonly-файл старого
каталога удаляется. Проверка использует защищённую и удаляемую части одного
синтетического дерева; пользовательские каталоги не затрагиваются.
SHA256 всех 24 исходных файлов и commit каждого toolkit-подмодуля входят в manifest.
