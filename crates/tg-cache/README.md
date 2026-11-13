# tg-cache: шифрование и файловый формат кэша

Первый срез lib_storage: storage_encryption.cpp/.h и
storage_encrypted_file.cpp/.h. База Cache, binlog, cleaner и compactor пока
не перенесены и не подменены заглушками.

EncryptionKey принимает исходные 64 или 256 байт. Для каждой соли из 64 байт
ключ AES-256 и IV выводятся двумя SHA256 из соответствующих половин ключа
и соли; CTR использует 128-битный big-endian счётчик с переносом и offset
в байтах. Вход и offset должны быть кратны 16, как в оригинале.

File реализует Read/ReadAppend/Write, read/write, padding, flush/close,
size/offset/seek и move_file. ReadAppend начинает с позиции 0 данных;
перед добавлением вызывающий код делает seek(size()). write изменяет входной
буфер на шифротекст; write_with_padding изменяет только его полные блоки.
При ошибке записи буфер и encryption offset восстанавливаются по оригиналу.

TDEF: 4 пропускаемых байта для FileLock, 64 байта открытой соли и 48 байт
зашифрованного заголовка. Заголовок хранит format/reserved/applicationVersion
и SHA256(key + salt + metadata); текущий applicationVersion равен 0.
Данные начинаются с физического смещения 116 и CTR offset 48. При чтении
неполный последний блок не включается в логический размер. Первые 4 байта
не валидируются; checksum проверяется раньше format. Эти особенности
подтверждены исполнением оригинала и сохранены.

FileLock переиспользуется из tg-base, включая POSIX takeover второго
процесса. move_file сохраняет исходную замену назначения, перенос каталога
и переход между файловыми системами; на Linux/GNU используется безопасная
обёртка nix::renameat2(RENAME_NOREPLACE), при невозможности rename файла —
эксклюзивное создание копии и удаление источника. Все новые зависимости уже
закреплены в workspace: aes, ctr, nix и tg-base; версии не обновлялись.

Проверки: 30 native CTR/key-derivation vectors, 22 пробы файлов, 8 move
сценариев, cross-device Move, 13 Rust-тестов и двустороннее чтение/побайтовая
перезапись файлов ключей 64/256. Инструкции —
[tests/reference-tools/README.md](tests/reference-tools/README.md), карта —
[docs/port-map.md](docs/port-map.md).

Нативное исполнение проверено на Linux x86_64 little-endian. Windows/macOS
runtime и альтернативный fallback rename не выполнялись. Системный I/O здесь
идёт через std::fs: внутренняя буферизация QFile не воспроизводится, частичные
системные записи/ENOSPC и точный момент ошибки относительно flush не проверены
native. Эти ограничения не выданы за полное закрытие этапа 3.
