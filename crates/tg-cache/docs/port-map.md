# lib_storage: шифрование и файлы

Оригинальные .cpp/.h и вызывающие места Cache::DatabaseObject прочитаны.
Ниже соответствие каждой функции текущего среза. Исходная точка — 0f2a31b.

| Оригинал | Rust | Проверка |
|---|---|---|
| CtrState::CtrState | CtrState::new | original_openssl_ctr_vectors_offsets_wrap_and_key_derivation; encryption_preconditions_match_key_salt_and_block_contracts |
| CtrState::process | CtrState::process | original_openssl_ctr_vectors_offsets_wrap_and_key_derivation |
| CtrState::incrementedIv | CtrState::incremented_iv | native vectors: ff counter/carry, high offsets |
| CtrState::encrypt/decrypt | CtrState::encrypt/decrypt | 30 OpenSSL vectors, split-offset calls, native file read/rewrite |
| EncryptionKey::EncryptionKey | EncryptionKey::new / Default | native derived vectors for 64/256 bytes; precondition tests |
| EncryptionKey::empty/operator bool | empty / !empty() | encryption_preconditions_match_key_salt_and_block_contracts |
| EncryptionKey::data | data | native derived vectors; default empty tests |
| EncryptionKey::prepareCtrState | prepare_ctr_state | native derived vectors, key64/key256.tdef, Rust writer → native exact rewrite |
| BasicHeader constructor/format get/set | explicit 112-byte header, low format byte | native_file_headers_damage_truncation_and_read_results |
| File::open | File::open | original sections; native probes; append/truncate/bad opens |
| File::attemptOpen | attempt_open | all three Mode branches, original sections |
| File::attemptOpenForRead | attempt_open Read branch | native probes and read-only failed writes |
| File::attemptOpenForReadAppend | attempt_open ReadAppend branch | append creation, original sections and two-process takeover |
| File::attemptOpenForWrite | attempt_open Write branch | padding matrix, original sections, Rust → native rewrite |
| File::writeHeader | write_header | Rust → native read and exact rewrite; both keys |
| File::readHeader | read_header | 22 C++ probes, format/checksum/reserved/truncation |
| File::readPlain/writePlain | read_plain/write_plain | native file probes and exact rewrite; read-only error |
| File::decrypt/encrypt | decrypt/encrypt | original sections, padding, native file fixtures/rewrite |
| File::read | read | original sections, native probes incl. unaligned physical tails |
| File::write | write | original sections, cipher buffer mutation and failed-write rollback |
| File::decryptBack | decrypt_back | read_only_failed_writes_restore_buffer_and_encryption_offset |
| File::readWithPadding | read_with_padding | padding_all_remainders_mutates_full_blocks_and_preserves_partial_input |
| File::writeWithPadding | write_with_padding | padding matrix, source buffer semantics, native exact rewrite |
| File::flush | flush | original sections, exporter and child process |
| File::close/isOpen | close/is_open, Drop | closed/invalid seek; bad-open resets; original sections |
| File::size/offset/seek | size/offset/seek | original sections; native probes; invalid/unaligned seek contract |
| File::Move | move_file; rename_no_replace; copy_and_remove | 8 C++ file/directory/symlink cases, cross-device C++ sample |
| test: simple encrypted file sections | original_sections_write_append_seek_read_and_move | write/append/seek/read/Move, including ciphertext input mutation |
| test: ForkInit::Method / two process sections | original_child_process_file_holder / original_two_process_read_and_writer_takeover | real child, concurrent read and original FileLock writer takeover |
| test: ForkInit constructor/QProcess application setup | std::process::Command and per-test helper | Linux test harness; no application singleton or DLL setup |

Header fields use little-endian as in the original Linux x86_64 execution.
No generic fake Cache or Main::Session was added. Header metadata remains zero
on write, as in the original BasicHeader; the app version is not substituted.
Payload is processed as the original stream; no new checksum is added.

Remaining validation: native short-write/ENOSPC/flush timing (QFile buffering
is not recreated), Windows/macOS lock/move runtime, other-platform rename
fallbacks and filesystem races. The file bytes/CTR/ordinary Linux operations
are verified independently of those unverified cases. The cache binlog,
database object, cleaner/compactor and storage_databases are still todo and
are outside this slice.

Proposed ledger: storage_encryption.cpp/.h done; storage_encrypted_file.cpp/.h
partial pending platform/error-path parity; storage_encrypted_file_tests.cpp
partial pending platform-specific process cases. No status claims for the
rest of lib_storage.
