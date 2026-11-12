# Карта tdata: ключи и нижний слой

Область: перечисленные utility/domain/serialize/lock файлы; в storage_account и main_* — только пути, карта, авторизация и список аккаунтов. Остальные определения перечислены ниже отдельно. Инвентаризация universal-ctags 6.2.1, lambda не считаются отдельными функциями. Таблица проверяет роль функции в Rust; это не запуск всего QObject/Main/Cache.
В текущей таблице **133** строк; определения с телом **128**, default **5**. Windows-only строки отмечены отдельно; native overlap — utility format, key/domain и auth методы в описанной обвязке.

Связаны с исполняемыми тестами: **125** строк; **8** Windows-only строк — замена готовым tg-base, запуск недоступен. Native overlap: **31** строк (не отдельные дополнительные функции). «Да» подтверждает только исполненную ветку в fixture harness; QObject/Main/Cache и OS-fallback не исполнялись.
| Оригинал | Роль | Rust | Проверка | Эталон C++ |
|---|---|---|---|---|
| `main_account.cpp:36 Main::(anonymous)::ComposeDataString` | Удаляет # и добавляет #index+1 для дополнительных аккаунтов | `compose_data_string` | authorization_versions_instance_keys_and_account_names | — |
| `main_account.cpp:126 Main::Account::willHaveSessionUniqueId` | Добавляет test-bit 56 к ненулевому user ID | `Authorization::session_unique_id` | authorization_versions_instance_keys_and_account_names | — |
| `main_account.cpp:271 Main::Account::setMtpMainDcId` | Меняет главный DC в сохранённых Fields или Instance | `Authorization::main_dc_id / Instance::set_main_dc_id` | authorization_versions_instance_keys_and_account_names | — |
| `main_account.cpp:277 Main::Account::setLegacyMtpKey` | Добавляет legacy-ключ до запуска Instance | `Authorization::keys.push / Account::apply_read_authorization_context` | account_config_legacy_blocks_and_partial_authorization | — |
| `main_account.cpp:284 Main::Account::serializeMtpAuthorization` | Пишет wide tag/user/DC и getKeysForWrite обеих групп | `Authorization::serialize_mtp_authorization / from_instance` | original_directories_read_and_exact_write | да, описанный путь |
| `main_account.cpp:341 Main::Account::setSessionUserId` | Присваивает хранимый ID пользователя | `Authorization::user_id` | authorization_versions_instance_keys_and_account_names | — |
| `main_account.cpp:372 Main::Account::setMtpAuthorization` | Читает narrow/wide user и ключи в Fields/addKeysForDestroy | `Authorization::set_mtp_authorization / fields / apply_keys_to_destroy` | authorization_versions_instance_keys_and_account_names | да, описанный путь |
| `main_domain.cpp:98 Main::Domain::accountAddedInStorage` | Добавляет индекс без дублей, сохраняя порядок | `MainDomain::account_added_in_storage` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `main_domain.cpp:109 Main::Domain::activateFromStorage` | Запоминает активный индекс из key-файла | `MainDomain::activate_from_storage` | original_directories_read_and_exact_write | — |
| `main_domain.cpp:113 Main::Domain::activeForStorage` | Возвращает индекс для записи key-файла | `MainDomain::active_for_storage` | original_directories_read_and_exact_write | — |
| `main_domain.cpp:143 Main::Domain::accounts` | Даёт упорядоченный список пар index/authorization | `MainDomain::accounts` | original_directories_read_and_exact_write | — |
| `main_domain.cpp:507 Main::Domain::scheduleWriteAccounts` | Объединяет записи списка в одной weak-задаче главной очереди | `main_domain::schedule_write_accounts` | queued_account_list_and_map_virtual_timers | — |
| `storage_file_utilities.cpp:76 Storage::details::(anonymous)::WriteManager::WriteManager` | Создаёт очередь писателя и weak-ссылку для продолжений | `ObjectOnThread::new_with_weak / WriteManager::default` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:80 Storage::details::(anonymous)::WriteManager::write` | Объединяет записи с тем же именем и планирует фоновую запись | `WriteManager::write` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:90 Storage::details::(anonymous)::WriteManager::writeSync` | Удаляет старую отложенную запись и пишет синхронно | `WriteManager::write_sync` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:98 Storage::details::(anonymous)::WriteManager::writeNow` | Пишет временный файл, переименовывает в s; fallback 0 | `WriteManager::write_now` | asynchronous_manager_drop_flush_and_io_failures | да, описанный путь |
| `storage_file_utilities.cpp:139 Storage::details::(anonymous)::WriteManager::writeSyncAll` | Дренирует очередь перед барьером/остановкой | `WriteManager::write_sync_all` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:144 Storage::details::(anonymous)::WriteManager::writeOneScheduledNow` | Вынимает и пишет первый отложенный элемент | `WriteManager::write_one_scheduled_now` | suffix_selection_safe_priority_and_queued_write_coalescing | — |
| `storage_file_utilities.cpp:156 Storage::details::(anonymous)::WriteManager::writeHeader` | Пишет TDF$ и LE AppVersion, создаёт каталог | `encode_file / WriteManager::write_now` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_file_utilities.cpp:173 Storage::details::(anonymous)::WriteManager::path` | Добавляет суффикс к полному имени | `WriteManager::write_now (path closure)` | suffix_selection_safe_priority_and_queued_write_coalescing | да, описанный путь |
| `storage_file_utilities.cpp:178 Storage::details::(anonymous)::WriteManager::open` | Открывает файл и записывает заголовок | `WriteManager::write_now` | asynchronous_manager_drop_flush_and_io_failures | да, описанный путь |
| `storage_file_utilities.cpp:188 Storage::details::(anonymous)::WriteManager::scheduleWrite` | Ставит weak-задачу на поток писателя | `WriteManager::schedule_write` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:194 Storage::details::(anonymous)::WriteManager::writeScheduled` | Пишет один элемент и планирует следующий | `WriteManager::write_scheduled` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:200 Storage::details::(anonymous)::AsyncWriteManager::write` | Объединяет записи с тем же именем и планирует фоновую запись | `AsyncWriteManager::write` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:211 Storage::details::(anonymous)::AsyncWriteManager::writeSync` | Удаляет старую отложенную запись и пишет синхронно | `AsyncWriteManager::write_sync` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:222 Storage::details::(anonymous)::AsyncWriteManager::sync` | Ждёт дренирования очереди | `AsyncWriteManager::sync` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:230 Storage::details::(anonymous)::AsyncWriteManager::stop` | Дренирует очередь и запрещает дальнейшие записи | `AsyncWriteManager::stop` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:242 Storage::details::ToFilePart` | Пишет 16 hex-цифр, начиная с младшей тетрады | `to_file_part` | authorization_versions_instance_keys_and_account_names | да, описанный путь |
| `storage_file_utilities.cpp:253 Storage::details::KeyAlreadyUsed` | Проверяет суффиксы 0/1/s и меняет переданный путь | `key_already_used` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_file_utilities.cpp:269 Storage::details::GenerateKey` | Выбирает ненулевой свободный случайный FileKey | `generate_key` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_file_utilities.cpp:283 Storage::details::ClearKey` | Удаляет все три варианта имени ключа | `clear_key` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_file_utilities.cpp:294 Storage::details::CheckStreamStatus` | Преобразует неуспешное чтение потока в ошибку | `check_stream_status` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:302 Storage::details::CreateLocalKey` | SHA512(salt+pass+salt), затем PBKDF2-SHA512 на 256 байт | `create_local_key` | original_qt_stream_and_both_pbkdf_variants | да, описанный путь |
| `storage_file_utilities.cpp:324 Storage::details::CreateLegacyLocalKey` | PBKDF2-SHA1 на 256 байт, 4/4000 итераций | `create_legacy_local_key` | original_qt_stream_and_both_pbkdf_variants | да, описанный путь |
| `storage_file_utilities.cpp:344 Storage::details::FileReadDescriptor::~FileReadDescriptor` | Освобождает буфер, writer дополнительно ставит готовую запись в очередь | `Drop / владение Vec и Reader` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:354 Storage::details::EncryptedDescriptor::EncryptedDescriptor` | Резервирует LE length slot либо получает результат decrypt без QBuffer | `EncryptedDescriptor::new / Result<EncryptedDescriptor>` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:357 Storage::details::EncryptedDescriptor::EncryptedDescriptor` | Резервирует LE length slot либо получает результат decrypt без QBuffer | `EncryptedDescriptor::new / Result<EncryptedDescriptor>` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:370 Storage::details::EncryptedDescriptor::~EncryptedDescriptor` | Освобождает буфер, writer дополнительно ставит готовую запись в очередь | `Drop / владение Vec и Reader` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:374 Storage::details::EncryptedDescriptor::finish` | Завершает writer/digest либо снимает заимствование QBuffer средствами языка | `EncryptedDescriptor::finish (владение Vec)` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:380 Storage::details::FileWriteDescriptor::FileWriteDescriptor` | Создаёт писатель для имени или ToFilePart(FileKey) | `FileWriteDescriptor::new (для FileKey: to_file_part)` | original_directories_read_and_exact_write | — |
| `storage_file_utilities.cpp:387 Storage::details::FileWriteDescriptor::FileWriteDescriptor` | Создаёт писатель для имени или ToFilePart(FileKey) | `FileWriteDescriptor::new (для FileKey: to_file_part)` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_file_utilities.cpp:396 Storage::details::FileWriteDescriptor::~FileWriteDescriptor` | Освобождает буфер, writer дополнительно ставит готовую запись в очередь | `Drop / владение Vec и Reader` | asynchronous_manager_drop_flush_and_io_failures | да, описанный путь |
| `storage_file_utilities.cpp:400 Storage::details::FileWriteDescriptor::init` | Привязывает имя к владеющему буферу writer | `FileWriteDescriptor::new (для FileKey: to_file_part)` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_file_utilities.cpp:408 Storage::details::FileWriteDescriptor::writeData` | Пишет null/empty QByteArray и включает его в TDF digest | `FileWriteDescriptor::write_data` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_file_utilities.cpp:422 Storage::details::FileWriteDescriptor::writeEncrypted` | Шифрует descriptor и пишет его как QByteArray | `FileWriteDescriptor::write_encrypted` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_file_utilities.cpp:428 Storage::details::FileWriteDescriptor::finish` | Завершает writer/digest либо снимает заимствование QBuffer средствами языка | `FileWriteDescriptor::finish / Drop` | asynchronous_manager_drop_flush_and_io_failures | да, описанный путь |
| `storage_file_utilities.cpp:454 Storage::details::PrepareEncrypted` | Добавляет LE-длину и случайное дополнение, SHA1[0..16], AES-IGE | `prepare_encrypted` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:475 Storage::details::ReadFile` | Выбирает s либо самый новый 0/1; проверяет magic/version/MD5 | `read_file` | suffix_selection_safe_priority_and_queued_write_coalescing | да, описанный путь |
| `storage_file_utilities.cpp:586 Storage::details::DecryptLocal` | Проверяет кратность, SHA1 и границы длины расшифрованного блока | `decrypt_local` | envelopes_corruption_truncation_and_local_lengths | да, описанный путь |
| `storage_file_utilities.cpp:625 Storage::details::ReadEncryptedFile` | Читает QByteArray, расшифровывает и пропускает LE-длину | `read_encrypted_file` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_file_utilities.cpp:661 Storage::details::ReadEncryptedFile` | Читает QByteArray, расшифровывает и пропускает LE-длину | `read_encrypted_file` | original_directories_read_and_exact_write | — |
| `storage_file_utilities.cpp:669 Storage::details::Sync` | Синхронизирует глобальный async manager | `sync` | asynchronous_manager_drop_flush_and_io_failures | — |
| `storage_file_utilities.cpp:673 Storage::details::Finish` | Дренирует и останавливает глобальный manager | `finish` | asynchronous_manager_drop_flush_and_io_failures | — |
| `serialize_common.cpp:12 Serialize::ByteArrayWriter::ByteArrayWriter` | Создаёт Qt_5_1 writer, reserve заменён Vec | `ByteArrayWriter::default / Writer` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.cpp:20 Serialize::ByteArrayWriter::result` | Возвращает принадлежащий writer буфер | `Writer::0 (move)` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.cpp:25 Serialize::ByteArrayReader::ByteArrayReader` | Открывает Qt_5_1 reader над байтами | `ByteArrayReader::new` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.cpp:31 Serialize::writeColor` | Упаковывает RGBA в LE-значение, пишет BE u32 | `write_color` | original_qt_stream_and_both_pbkdf_variants | да, описанный путь |
| `serialize_common.cpp:38 Serialize::readColor` | Читает BE u32 и извлекает RGBA | `read_color` | original_qt_stream_and_both_pbkdf_variants | — |
| `serialize_common.h:21 Serialize::ByteArrayWriter::underlying` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `ByteArrayWriter / ByteArrayReader (без неявного cast)` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:24 Serialize::ByteArrayWriter::operator QDataStream&` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `явный Writer/Reader` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:36 Serialize::operator <<` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Writer::{bytes,nullable_bytes} / write_bytes` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:45 Serialize::ByteArrayReader::underlying` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `ByteArrayWriter / ByteArrayReader (без неявного cast)` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:48 Serialize::ByteArrayReader::operator QDataStream&` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `явный Writer/Reader` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:52 Serialize::ByteArrayReader::atEnd` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Reader::at_end` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:55 Serialize::ByteArrayReader::status` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Option/Result вместо числового QDataStream::Status` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:58 Serialize::ByteArrayReader::ok` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Option::is_some / Result::is_ok` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:69 Serialize::operator >>` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Reader::{bytes,raw} -> Option` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:81 Serialize::stringSize` | Считает 4 + число UTF-16 байт | `string_size` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:85 Serialize::bytearraySize` | Считает 4 + длина массива | `bytearray_size` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:89 Serialize::bytesSize` | Считает размер QByteArray-совместимой записи span | `bytes_size` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:93 Serialize::colorSize` | Возвращает 4 байта упакованного цвета | `color_size` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:104 Serialize::bytes` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `write_bytes / Reader::bytes` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:109 Serialize::operator >>` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Reader::{bytes,raw} -> Option` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:137 Serialize::bytes` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `write_bytes / Reader::bytes` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:141 Serialize::operator <<` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Writer::{bytes,nullable_bytes} / write_bytes` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:153 Serialize::operator <<` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Writer::{bytes,nullable_bytes} / write_bytes` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:157 Serialize::dateTimeSize` | Возвращает 8+4+1 байт QDateTime Qt_5_1 | `date_time_size` | qt_null_empty_utf16_bytes_color_and_bad_lengths | — |
| `serialize_common.h:162 Serialize::read` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Reader::{i32,u64,raw} -> Option` | qt_null_empty_utf16_bytes_color_and_bad_lengths | да, описанный путь |
| `serialize_common.h:169 Serialize::read` | Читает/пишет Qt_5_1 данные с null, контролем границ и явным результатом | `Reader::{i32,u64,raw} -> Option` | qt_null_empty_utf16_bytes_color_and_bad_lengths | да, описанный путь |
| `storage_account.cpp:117 Storage::(anonymous)::ComputeDataNameKey` | Берёт первые 8 байт MD5(dataName UTF-8) как LE FileKey | `compute_data_name_key` | original_directories_read_and_exact_write | — |
| `storage_account.cpp:127 Storage::(anonymous)::BaseGlobalPath` | Явный корень tdata вместо глобального working directory | `Domain::root / Account::root` | original_directories_read_and_exact_write | — |
| `storage_account.cpp:131 Storage::(anonymous)::ComputeDatabasePath` | Формирует user_dataName путь, сам Cache не открывает | `Account::support_mode_path` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:172 Storage::Account::Account` | Создаёт пути карты и auth-файла; Cache/settings часть отложена | `Account::new (пути/ключи)` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:202 Storage::Account::tempDirectory` | Формирует temp_dataName путь | `Account::temp_directory` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:206 Storage::Account::supportModePath` | Формирует user_dataName/support путь | `Account::support_mode_path` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:210 Storage::Account::legacyStart` | Читает legacy map и авторизацию | `Account::legacy_start` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:221 Storage::Account::start` | Запускает modern/legacy чтение либо передаёт local key добавленному аккаунту | `Account::start` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:230 Storage::Account::startAdded` | Запускает modern/legacy чтение либо передаёт local key добавленному аккаунту | `Account::start_added` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:307 Storage::Account::readMapWith` | Читает map envelope, legacy key и известные записи без чтения их файлов | `Account::read_map_with / AccountMap::parse` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.cpp:586 Storage::Account::writeMapDelayed` | Помечает map и ставит виртуальный таймер на 1000 мс | `AccountWrites::write_map_delayed` | queued_account_list_and_map_virtual_timers | — |
| `storage_account.cpp:591 Storage::Account::writeMapQueued` | Помечает map и ставит задачу главной очереди | `AccountWrites::write_map_queued` | queued_account_list_and_map_virtual_timers | — |
| `storage_account.cpp:598 Storage::Account::writeMap` | Пишет два null массива и encrypted map; payload сохраняется непрозрачно | `Account::write_map` | original_directories_read_and_exact_write | — |
| `storage_account.cpp:1113 Storage::Account::applyReadContext` | Предпочитает последний modern blob, иначе legacy user/DC/keys | `Account::apply_read_authorization_context (только auth)` | account_config_legacy_blocks_and_partial_authorization | — |
| `storage_account.cpp:1148 Storage::Account::writeMtpData` | Шифрует блок 0x4b с serialized authorization | `Account::write_mtp_data` | original_directories_read_and_exact_write | — |
| `storage_account.cpp:1160 Storage::Account::readMtpData` | Читает современный auth blob и старые dbiUser/dbiKey | `Account::read_mtp_data` | account_config_legacy_blocks_and_partial_authorization | — |
| `storage_account.cpp:1188 Storage::Account::writeMtpConfig` | Пишет существующий Qt-совместимый Config | `Account::write_mtp_config` | account_config_legacy_blocks_and_partial_authorization | — |
| `storage_account.cpp:1200 Storage::Account::readMtpConfig` | Читает зашифрованный QByteArray Config | `Account::read_mtp_config` | account_config_legacy_blocks_and_partial_authorization | — |
| `storage_account.h:75 Storage::Account::oldMapVersion` | Возвращает версию map-файла | `Account::old_map_version` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_account.h:82 Storage::Account::peekLegacyLocalKey` | Даёт ключ для миграции key_datas | `Account::local_key.clone` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:23 Storage::(anonymous)::BaseGlobalPath` | Явный корень tdata вместо глобального working directory | `Domain::root / Account::root` | original_directories_read_and_exact_write | — |
| `storage_domain.cpp:27 Storage::(anonymous)::ComputeKeyName` | Формирует key_ + dataName без test-суффикса | `compute_key_name` | original_directories_read_and_exact_write | — |
| `storage_domain.cpp:35 Storage::Domain::Domain` | Создаёт владельца ключа и списка аккаунтов | `Domain::new` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:42 Storage::Domain::start` | Запускает modern/legacy чтение либо передаёт local key добавленному аккаунту | `Domain::start` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:64 Storage::Domain::startAdded` | Запускает modern/legacy чтение либо передаёт local key добавленному аккаунту | `Domain::start_added` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:73 Storage::Domain::startWithSingleAccount` | Переносит legacy local key либо генерирует новый и пишет список | `Domain::start_with_single_account` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:92 Storage::Domain::generateLocalKey` | Генерирует случайные pass/salt и локальный ключ | `Domain::generate_local_key` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:106 Storage::Domain::encryptLocalKey` | Генерирует соль пароля и шифрует raw local key | `Domain::encrypt_local_key` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_domain.cpp:117 Storage::Domain::startModern` | Читает passcode/local key, фильтрует индексы и сессии, выбирает активный | `Domain::start_modern` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_domain.cpp:215 Storage::Domain::writeAccounts` | Пишет соль, зашифрованный ключ и count/indices/active | `Domain::write_accounts` | original_directories_read_and_exact_write | да, описанный путь |
| `storage_domain.cpp:240 Storage::Domain::startFromScratch` | Генерирует ключ для единственного пустого аккаунта | `Domain::start_from_scratch` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:246 Storage::Domain::checkPasscode` | Сравнивает производный ключ код-пароля | `Domain::check_passcode` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:254 Storage::Domain::setPasscode` | Перешифровывает ключ, пишет список и публикует изменение | `Domain::set_passcode` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:264 Storage::Domain::oldVersion` | Возвращает версию key-файла | `Domain::old_version` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:268 Storage::Domain::clearOldVersion` | Сбрасывает отметку старой версии | `Domain::clear_old_version` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:272 Storage::Domain::localPasscodeChanged` | Подписывает на смену код-пароля | `Domain::local_passcode_changed` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_domain.cpp:276 Storage::Domain::hasLocalPasscode` | Возвращает признак непустого код-пароля | `Domain::has_local_passcode` | legacy_local_keys_map_handles_and_passcode_changes | — |
| `storage_file_lock_posix.cpp:20 Storage::(anonymous)::KillProcess` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | account_config_legacy_blocks_and_partial_authorization; tg-base::file_lock::tests::other_process_is_killed | — |
| `storage_file_lock_posix.cpp:58 Storage::FileLock::Lock::Acquire` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | account_config_legacy_blocks_and_partial_authorization; tg-base::file_lock::tests::other_process_is_killed | — |
| `storage_file_lock_posix.cpp:79 Storage::FileLock::Lock::Lock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | account_config_legacy_blocks_and_partial_authorization; tg-base::file_lock::tests::other_process_is_killed | — |
| `storage_file_lock_posix.cpp:82 Storage::FileLock::Lock::~Lock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | account_config_legacy_blocks_and_partial_authorization; tg-base::file_lock::tests::other_process_is_killed | — |
| `storage_file_lock_posix.cpp:93 Storage::FileLock::lock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | account_config_legacy_blocks_and_partial_authorization; tg-base::file_lock::tests::other_process_is_killed | — |
| `storage_file_lock_posix.cpp:118 Storage::FileLock::unlock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | account_config_legacy_blocks_and_partial_authorization; tg-base::file_lock::tests::other_process_is_killed | — |
| `storage_file_lock_win.cpp:20 Storage::(anonymous)::CloseProcesses` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | только чтение Windows: Linux не выполняет LockFile/Restart Manager | — |
| `storage_file_lock_win.cpp:94 Storage::FileLock::Lock::Acquire` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | только чтение Windows: Linux не выполняет LockFile/Restart Manager | — |
| `storage_file_lock_win.cpp:108 Storage::FileLock::Lock::Lock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | только чтение Windows: Linux не выполняет LockFile/Restart Manager | — |
| `storage_file_lock_win.cpp:111 Storage::FileLock::Lock::~Lock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | только чтение Windows: Linux не выполняет LockFile/Restart Manager | — |
| `storage_file_lock_win.cpp:119 Storage::FileLock::lock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | только чтение Windows: Linux не выполняет LockFile/Restart Manager | — |
| `storage_file_lock_win.cpp:137 Storage::FileLock::unlock` | Тот же reopen, диапазон [0,4), owner shutdown и RAII unlock | `tg_base::file_lock::FileLock (переэкспорт storage_file_lock_posix/win)` | только чтение Windows: Linux не выполняет LockFile/Restart Manager | — |
| `storage_domain.cpp:40 Domain::~Domain = default` | Создаёт/освобождает владельца без дополнительной логики | default/move/Drop | account_config_legacy_blocks_and_partial_authorization | — |
| `storage_file_lock_posix.cpp:91 FileLock::FileLock = default` | Создаёт/освобождает владельца без дополнительной логики | default/move/Drop | account_config_legacy_blocks_and_partial_authorization | — |
| `storage_file_lock_posix.cpp:122 FileLock::~FileLock = default` | Создаёт/освобождает владельца без дополнительной логики | default/move/Drop | account_config_legacy_blocks_and_partial_authorization | — |
| `storage_file_lock_win.cpp:117 FileLock::FileLock = default` | Создаёт/освобождает владельца без дополнительной логики | default/move/Drop | только чтение Windows: нужен запуск Windows | — |
| `storage_file_lock_win.cpp:141 FileLock::~FileLock = default` | Создаёт/освобождает владельца без дополнительной логики | default/move/Drop | только чтение Windows: нужен запуск Windows | — |

## Следующая задача

Ниже только определения вне текущей области. Settings/cache/chat/self/UI lifetime не представлены готовыми заглушками. В readMapWith/writeMap сейчас готов только envelope и сохранение непрозрачных известных records; применение и каноническая перезапись settings/chat payload — следующая задача. Аналогично applyReadContext переводит только auth-ветку. main_account::startMtp и lifecycle Main::Session целиком остаются этапу 4; текущий формат связан с настоящим Instance через Authorization::{fields,from_instance,apply_keys_to_destroy}.

| Файл:строка → функция | Что нужно |
|---|---|
| `main_account.cpp:47 Account` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:54 ~Account` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:62 domainLocal` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:66 legacyStart` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:73 prepareToStart` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:78 start` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:89 prepareToStartAdded` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:94 watchProxyChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:117 watchSessionChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:135 createSession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:145 createSession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:186 createSession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:204 destroySession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:225 destroyingSession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:229 sessionExists` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:233 session` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:239 maybeSession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:243 sessionValue` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:247 sessionChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:251 mtpValue` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:257 mtpMainSessionValue` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:263 mtpUpdates` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:267 mtpNewSessionCreated` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:347 setSessionFromStorage` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:361 getSessionSettings` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:426 startMtp` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:510 checkForUpdates` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:520 checkForNewSession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:530 logOut` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:543 loggingOut` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:547 forcedLogOut` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:554 loggedOut` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:562 destroyMtpKeys` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:597 suggestMainDcId` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:606 destroyStaleAuthorizationKeys` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:621 setHandleLoginCode` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:625 handleLoginCode` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.cpp:631 resetAuthorizationKeys` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.h:37 domain` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.h:65 appConfig` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.h:71 local` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.h:82 mtp` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_account.h:117 lifetime` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:31 Domain` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:62 started` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:66 start` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:79 finish` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:85 suggestExportIfNeeded` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:117 resetWithForgottenPasscode` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:128 activateAfterStarting` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:147 orderedAccounts` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:168 accountsChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:172 maybeLastOrSomeAuthedAccount` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:186 accountsAuthedCount` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:196 activeValue` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:200 active` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:207 activeChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:213 activeSessionChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:217 activeSessionValue` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:224 unreadBadge` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:228 unreadBadgeMuted` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:232 unreadBadgeChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:236 notifyUnreadBadgeChanged` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:244 updateUnreadBadge` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:259 scheduleUpdateUnreadBadge` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:270 add` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:318 addActivated` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:342 watchSession` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:373 closeAccountWindows` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:394 removePasscodeIfEmpty` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:413 removeRedundantAccounts` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:446 checkForLastProductionConfig` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:461 maybeActivate` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:471 activate` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:518 maxAccounts` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.cpp:528 maxAccountsChanges` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `main_domain.h:48 local` | Main::Session/Core, аккаунтный lifecycle и UI (этап 4; storage hooks — продолжение этапа 3) |
| `storage_account.cpp:109 EmptyMessageDraftSources` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:139 LegacyTempDirectory` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:143 SerializeSuggest` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:154 DeserializeSuggest` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:189 ~Account` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:237 clearLegacyFiles` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:247 collectGoodNames` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:766 reset` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:848 writeLocations` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:927 writeLocationsQueued` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:934 writeLocationsDelayed` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:939 readLocations` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1003 updateDownloads` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1009 downloadsSerialized` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1013 writeSessionSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1017 writeSessionSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1066 prepareReadSettingsContext` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1072 readSessionSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1218 EnumerateDrafts` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1264 registerDraftSource` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1274 unregisterDraftSource` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1286 writeDrafts` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1385 writeDraftCursors` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1442 clearDraftCursors` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1451 readDraftCursors` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1501 readDraftCursorsLegacy` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1535 readDraftsWithCursors` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1675 readDraftsWithCursorsLegacy` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1749 hasDraftCursors` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1753 hasDraft` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1757 writeFileLocation` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1799 removeFileLocation` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1810 readFileLocation` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1828 cacheKey` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1834 cacheBigFileKey` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1838 cachePath` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1844 cacheSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1853 updateCacheSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1874 cacheBigFilePath` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1880 cacheBigFileSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1889 writeStickerSet` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:1944 writeStickerSets` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2046 readStickerSets` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2318 writeInstalledStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2346 writeFeaturedStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2369 writeFeaturedCustomEmoji` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2385 writeRecentStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2395 writeFavedStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2404 writeArchivedStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2417 writeArchivedMasks` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2430 writeInstalledMasks` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2444 writeRecentMasks` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2454 writeInstalledCustomEmoji` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2472 importOldRecentStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2592 readInstalledStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2606 readFeaturedStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2627 readFeaturedCustomEmoji` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2636 readRecentStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2642 readRecentMasks` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2648 readFavedStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2654 readArchivedStickers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2667 readArchivedMasks` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2680 readInstalledMasks` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2689 readInstalledCustomEmoji` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2698 writeSavedGifs` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2726 readSavedGifs` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2771 writeRecentHashtagsAndBots` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2824 readRecentHashtagsAndBots` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2889 saveRecentHashtags` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2924 saveRecentSentHashtags` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2934 saveRecentSearchHashtags` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:2944 writeExportSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3005 readExportSettings` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3101 setMediaLastPlaybackPosition` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3126 mediaLastPlaybackPosition` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3135 writeMediaLastPlaybackPositions` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3160 readMediaLastPlaybackPositions` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3191 writeSearchSuggestionsDelayed` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3199 writeSearchSuggestionsIfNeeded` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3206 writeSearchSuggestions` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3241 readSearchSuggestions` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3282 writeSelf` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3286 readSelf` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3310 writeTrustedPeers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3345 readTrustedPeers` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3397 markPeerTrustedOpenGame` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3410 isPeerTrustedOpenGame` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3417 markPeerTrustedPayment` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3432 isPeerTrustedPayment` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3439 markPeerTrustedOpenWebView` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3454 isPeerTrustedOpenWebView` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3461 markPeerTrustedPayForMessage` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3476 isPeerTrustedPayForMessage` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3488 peerTrustedPayForMessageRead` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3492 hasPeerTrustedPayForMessageEntry` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3496 clearPeerTrustedPayForMessage` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3504 enforceModernStorageIdBots` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3512 resolveStorageIdBots` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3540 resolveStorageIdOther` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3552 readRoundPlaceholder` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3577 writeRoundPlaceholder` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3598 readInlineBotsDownloads` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3624 writeInlineBotsDownloads` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3636 writeBotStorage` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3665 readBotStorage` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3693 encrypt` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3705 decrypt` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3717 TonSiteStorageId` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3731 clearPref` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3740 writePrefGeneric` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3755 readPrefGeneric` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3760 writePrefsDelayed` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3765 writePrefs` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3801 readPrefs` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3834 readPrefImpl` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.cpp:3842 writePrefImpl` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.h:186 writePref` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.h:192 readPref` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
| `storage_account.h:235 is_flag_type` | settings/cache/chat модели и безопасное сохранение неизвестного содержимого (этап 3) |
