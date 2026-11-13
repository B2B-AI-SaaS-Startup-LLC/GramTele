# Нативный эталон файлов кэша

Оригинал: AyuGramDesktop 0f2a31bb420de98f331990432265b131b8d836b5.
Исходный клон только читается; отдельный CMake собирает неизменённые
storage_encryption.cpp/storage_encrypted_file.cpp, base::FileLock POSIX,
bytes/random, assertion/integration. SHA/AES/CTR — настоящий OpenSSL,
файлы — настоящий QFile. RAND_bytes перехвачен byte counter только в harness.
Не собираются приложение, окно, Cache::Database и платформенный bootstrap.

Из корня репозитория, с отдельным клоном оригинала:

```bash
export AYUGRAM_ORIGINAL=/path/to/AyuGramDesktop
nix-shell -E 'let
  f = builtins.getFlake (toString ./.);
  p = f.inputs.nixpkgs.legacyPackages.x86_64-linux;
in p.mkShell {
  packages = with p; [ cmake ninja pkg-config qt6.qtbase openssl microsoft-gsl range-v3 python3 ];
  CACHE_GSL_INCLUDE = "${p.microsoft-gsl}/include";
  CACHE_RANGE_INCLUDE = "${p.range-v3}/include";
}' --run 'cmake -G Ninja -S crates/tg-cache/tests/reference-tools \
  -B /tmp/ayu-cache-native \
  -DTELEGRAM_ROOT="$AYUGRAM_ORIGINAL/Telegram" \
  -DGSL_INCLUDE="$CACHE_GSL_INCLUDE" -DRANGE_INCLUDE="$CACHE_RANGE_INCLUDE" &&
  cmake --build /tmp/ayu-cache-native -j2 &&
  python3 crates/tg-cache/tests/reference-tools/generate.py \
  /tmp/ayu-cache-native/native_cache_file crates/tg-cache/tests/fixtures \
  --telegram-root "$AYUGRAM_ORIGINAL/Telegram"'
```

Нужны Linux/POSIX и /dev/shm на другом устройстве для cross-case. Генератор
создаёт только собственные временные каталоги и перезаписывает конкретные
fixtures в указанном выходе. SHA256 исходных файлов и образцов — в manifest.json.

- ctr.bin: 30 записей с исходными key/IV или key/salt, offset, plaintext и
  ciphertext. Покрыты нулевой IV, переносы, IV=ff..ff, offsets 0/16/256/4096/
  2^40/наибольший кратный 16 i64, оба размера ключа.
- key64/key256.tdef: write, padding, seek overwrite и ReadAppend, 80 байт данных.
- probes.txt и остальные .tdef: wrong key, truncated header/tail, magic/salt/
  metadata/payload bit flips, checksum-valid unknown format и reserved fields.
- moves.txt: обычный перенос, замена назначения, отсутствующие источник/parent,
  одинаковый путь, каталог, symlink-источник/назначение. В том числе сохранено
  исходное удаление файла при Move(from, from) и усечение цели symlink.
- cross.txt: успешный исходный Move из /tmp на tmpfs /dev/shm.

Rust читает эти образцы обычными cargo test, без Qt/OpenSSL во время теста.
Для обратного направления используется пример с новыми синтетическими файлами:

```bash
nix develop --command cargo run -p tg-cache --example cache_reference -- /tmp/new-rust-cache
/tmp/ayu-cache-native/native_cache_file verify /tmp/new-rust-cache
```

Пример требует отсутствующего выходного каталога. verify читает 64/256 файлы
оригинальным Storage::File, проверяет plaintext и размер, затем пересоздаёт
файлы оригинальным writer с исходной солью и уже прочитанными padding-байтами.
Полные байты сравниваются; случайность Rust writer не подменяется. verify
пишет только новые .native в заданном синтетическом каталоге.

Rust-тесты дополнительно переносят обычные секции оригинального
storage_encrypted_file_tests.cpp и двухпроцессное чтение/захват writer,
проверяют все остатки padding 0..65, отмену мутации буфера после неудачной
записи, closed/seek и precondition panic. OS failure injection, races,
буферизация QFile и Windows/macOS runtime этим эталоном не проверены.
