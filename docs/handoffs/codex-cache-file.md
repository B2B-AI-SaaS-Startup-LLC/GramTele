# Codex: файловый слой кэша, 13.11.2026

Ветка port/cache-file-format от main 82200d4. Область Claude не менялась:
lang/countries, tg-ui, tg-runtime, tg-mtproto и tg-codegen сохранены.

## Реализация

crates/tg-cache содержит CtrState, EncryptionKey и File по исходным
storage_encryption.cpp/.h, storage_encrypted_file.cpp/.h. Сохранены оба
размера ключа 64/256, соль 64, SHA256 derivation и AES-256-CTR, TDEF header,
порядок checksum/format checks, Read/ReadAppend/Write, padding, мутация
буфера, восстановление после неудачной записи, seek/offset/size и Move.
Входной буфер write становится шифротекстом; ReadAppend не перемещает
позицию в конец автоматически. Файлы начинаются с 116 байт header/magic.

На Linux/GNU Move использует существующую безопасную nix-обёртку
renameat2(RENAME_NOREPLACE) и copy/remove для cross-device файла.
Особенности оригинала, включая directory/symlink и Move(from, from),
проверены C++ и сохранены. FileLock переиспользуется из tg-base.

Зависимости aes 0.9, ctr 0.10, tg-base и nix 0.31 fs уже были в workspace
и Cargo.lock. Добавлены только зависимости пакета tg-cache, без обновлений
версий или новых внешних пакетов. Оригинальный клон не менялся.

## Проверка

- Rust 1.95.0: fmt и clippy workspace/all-targets с -D warnings — PASS.
- cargo test --workspace — PASS: 643 passed, 0 failed, 3 прежних ignored.
- cargo test -p tg-cache — PASS: 13 passed, 0 failed, 0 ignored.
- Native оригинал: 30 CTR/key-derivation vectors, 22 file/header probes,
  8 Move cases и отдельный cross-device Move — PASS.
- Native Storage::File прочитал Rust файлы с ключами 64/256 и пересоздал
  их writer с исходной солью/padding; полные байты совпали — PASS.
- Реальный тест двух процессов: чтение рядом с writer и takeover — PASS.

Native — неизменённые lib_storage и base POSIX FileLock, настоящий Qt File
и OpenSSL; тестовый RAND_bytes wrapper только фиксирует соль/padding для
fixtures. Обратный Rust writer использует настоящий OS random. Сохранены
исходные SHA256 и fixtures; инструкция tg-cache/tests/reference-tools/README.md,
карта tg-cache/docs/port-map.md. База Cache и приложение не собирались.
Реальные tdata/ayudata/кэш пользователя не открывались.

Локальные логи: /tmp/ayugram-cache-workspace.log,
/tmp/ayugram-cache-file-tests-final.log, /tmp/ayugram-cache-file-final.log,
/tmp/ayugram-cache-native-read-rust-final.log. Краткий протокол также в
crates/tg-cache/docs/verification.txt.

## Реестр и оставшееся

Предлагаемые статусы:

- lib_storage/storage/storage_encryption.cpp и .h — done;
- lib_storage/storage/storage_encrypted_file.cpp и .h — partial:
  native Windows/macOS, прочие rename fallback, partial syscalls/ENOSPC и
  точный момент ошибок относительно Qt flush ещё не проверены;
- lib_storage/storage/cache/storage_encrypted_file_tests.cpp — partial:
  общие секции и POSIX перенесены, платформенный Windows/macOS process harness
  остаётся на этап 10.

Внутренняя буферизация QFile не эмулируется; здесь std::fs. Это существенная
граница, а не заявление полного паритета поведения при отказах диска.
Новый Fake Cache/Database не добавлялся. Следующий независимый срез Codex —
cache/storage_cache_types и storage_cache_binlog_reader, затем настоящие
Database/cleaner/compactor. Не начинать параллельно работу Claude по языкам.
