# Нативный эталон нижнего tdata

Оригинал только читается. Сборка и generated .cpp/.h находятся вне клона.
Нужна оригинальная generated scheme.h; её создаёт предыдущий
[tg-scheme reference harness](../../../tg-scheme/tests/reference-tools/README.md).
В текущем запуске: /tmp/ayugram-tl-reference/gen. Объект scheme.cpp не нужен:
этот executable не создаёт API-конструкторов схемы.

```bash
python3 crates/tg-core/tests/reference-tools/extract.py \
  /home/yarik/Projects/AyuGramDesktop/Telegram /tmp/ayugram-tdata-capture
nix-shell -E 'let f=builtins.getFlake "/home/yarik/Projects/ayugram-rs";
  p=f.inputs.nixpkgs.legacyPackages.x86_64-linux;
  in p.mkShell { packages=[p.cmake p.ninja p.pkg-config p.qt6.qtbase p.openssl p.range-v3 p.microsoft-gsl]; }' \
  --run 'cmake -G Ninja -S crates/tg-core/tests/reference-tools \
    -B /tmp/ayugram-tdata-reference-clean \
    -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
    -DCAPTURE=/tmp/ayugram-tdata-capture \
    -DSCHEME_INCLUDE=/tmp/ayugram-tl-reference/gen && \
    cmake --build /tmp/ayugram-tdata-reference-clean -j2'
/tmp/ayugram-tdata-reference-clean/native_tdata create /tmp/new-original-fixtures
/tmp/ayugram-tdata-reference-clean/native_tdata samples /tmp/qt-pbkdf.bin
```

Команды create/samples **пишут** только заданный выход. Используйте новый
синтетический каталог/файл; реальную tdata передавать нельзя. Фиксированы local
key 0..255, auth keys по account/slot, RAND_bytes counter с reset на каталог,
пароль test-passcode либо пустой, crl::now=0. У каждого аккаунта DC2/DC4 + DC1
на уничтожение, user ID >32 bit. Сохранённые fixtures и manifest — ../fixtures.

`extract.py` извлекает тела исходных функций без переписывания криптографии или
кодека. MD5 также оригинальный (core/utils.cpp). CMake компилирует оригинальные
AuthKey, base SHA/AES wrappers и lib_tl. Исключены background queue и глобальное
окружение приложения. Manager.write/writeSync немедленно вызывают оригинальный
writeNow; writeHeader/open/path и QSaveFile safe path выполняются исходным кодом.
OS fallback functions аварийно завершают capture: их native паритет не заявлен.
Main owner/account-start фасад хранит записи; это не оригинальный Main::Session.
Пустой map payload строится обвязкой из двух null + encrypted empty records через
оригинальный FileWriteDescriptor. Полный Storage::Account с Cache не компилируется.

Для обратного направления используется **только синтетический exporter**:

```bash
AYUGRAM_SYNTHETIC_REFERENCE_OUTPUT=/tmp/new-rust-fixtures \
  nix develop --command cargo test -p tg-core --test tdata \
  export_directories_for_native_reader -- --ignored --nocapture
/tmp/ayugram-tdata-reference-clean/native_tdata verify /tmp/new-rust-fixtures
```

Exporter требует отсутствующего выходного каталога. verify запускает оригинальные
ReadFile/DecryptLocal/Domain::startModern/setMtpAuthorization и проверяет число,
порядок аккаунтов, активный индекс и полные записи всех ключей. Он может удалять
legacy альтернативы, как оригинальный ReadFile; реальную tdata не передавайте.
Постоянный cargo test original_directories_read_and_exact_write проверяет чтение
original fixtures и полную побайтовую идентичность Rust writer.

samples создаёт Qt_5_1 null/empty QByteArray/QString, unpaired UTF16, RGBA,
256-байтовые результаты modern/legacy PBKDF2 и narrow authorization + повторную
wide-запись оригиналом. Ключи/соли fixtures публичные синтетические, не из аккаунта.

Подтверждённый запуск: Qt 6.11.2/OpenSSL 3.6.5/GCC 15.3, обе стороны exit 0;
логи в ../../docs/verification. Полный Qt-клиент, Cache, процессный bootstrap,
Windows LockFile и аварийные ветви оригинального FS этим harness не проверены.
