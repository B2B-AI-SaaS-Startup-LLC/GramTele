# Независимые storage: карта функций

Оригинальные .cpp/.h прочитаны целиком. Тесты перечислены по именам;
`native_reference` означает исполнение отдельного C++ target, а не чтение.
Отдельных тестов этих списков в исходном клоне не найдено.

| Оригинал | Назначение / Rust | Проверка |
|---|---|---|
| `storage_sparse_ids_list.cpp`: `SparseIdsList::Slice::Slice(base::flat_set<MsgId> &&messages, MsgRange range)` | SparseIdsList::add_range (слияние закрытых диапазонов) | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::Slice::merge(const Range &moreMessages, MsgRange moreNoSkipRange)` | SparseIdsList::add_range (слияние закрытых диапазонов) | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::uniteAndAdd(SparseIdsSliceUpdate &update, base::flat_set<Slice>::iterator uniteFrom, base::flat_set<Slice>::iterator uniteTill, const Range &messages, MsgRange noSkipRange)` | SparseIdsList::add_range (слияние закрытых диапазонов) | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::addRangeItemsAndCountNew(SparseIdsSliceUpdate &update, const Range &messages, MsgRange noSkipRange)` | SparseIdsList::add_range (слияние закрытых диапазонов) | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::addRange(const Range &messages, MsgRange noSkipRange, std::optional<int> count, bool incrementCount)` | SparseIdsList::add_range (слияние закрытых диапазонов) | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::addNew(MsgId messageId)` | SparseIdsList::add_new | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::addExisting(MsgId messageId, MsgRange noSkipRange)` | SparseIdsList::add_existing | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::addSlice(std::vector<MsgId> &&messageIds, MsgRange noSkipRange, std::optional<int> count)` | SparseIdsList::add_slice | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::removeOne(MsgId messageId)` | SparseIdsList::remove_one | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::removeAll()` | SparseIdsList::remove_all | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::countAfter(MsgId tillId, int limit, Fn<bool(MsgId)` | SparseIdsList::count_after | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::invalidateBottom()` | SparseIdsList::invalidate_bottom | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::query(SparseIdsListQuery &&query)` | SparseIdsList::query | sparse_query_limits_and_lazy_snapshot |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::snapshot(const SparseIdsListQuery &query)` | SparseIdsList::snapshot | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::empty()` | SparseIdsList::empty | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::sliceUpdated()` | SparseIdsList::slice_updated | sparse_ayu_reversed_range_and_update_precedes_expansion |
| `storage_sparse_ids_list.cpp`: `SparseIdsList::queryFromSlice(const SparseIdsListQuery &query, const Slice &slice)` | snapshot (ограничения и skipped) | sparse_native_reference; sparse_merge_counts_and_known_ends |
| `storage_user_photos.cpp`: `UserPhotos::List::setBack(PhotoId photoId)` | UserPhotos / List::set_back | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::detachBack()` | UserPhotos / List::detach_back | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::attachBack()` | UserPhotos / List::attach_back | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::addNew(PhotoId photoId)` | UserPhotos / List::add_new | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::addSlice(std::vector<PhotoId> &&photoIds, int count)` | UserPhotos / List::add_slice | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::removeOne(PhotoId photoId)` | UserPhotos / List::remove_one | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::removeAfter(PhotoId photoId)` | UserPhotos / List::remove_after | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::replace(PhotoId oldPhotoId, PhotoId newPhotoId)` | UserPhotos / List::replace | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::sendUpdate()` | UserPhotos / List::send_update | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::query(UserPhotosQuery &&query)` | UserPhotos / List::query | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::List::sliceUpdated()` | UserPhotos / List::slice_updated | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::sliceUpdated()` | UserPhotos / List::slice_updated | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::enforceLists(UserId user)` | UserPhotos / List::enforce_lists | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::add(UserPhotosSetBack &&query)` | UserPhotos / List::add → set_back | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::add(UserPhotosAddNew &&query)` | UserPhotos / List::add → add_new | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::add(UserPhotosAddSlice &&query)` | UserPhotos / List::add → add_slice | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::remove(UserPhotosRemoveOne &&query)` | UserPhotos / List::remove → remove_one | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::remove(UserPhotosRemoveAfter &&query)` | UserPhotos / List::remove → remove_after | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::replace(UserPhotosReplace &&query)` | UserPhotos / List::replace | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_user_photos.cpp`: `UserPhotos::query(UserPhotosQuery &&query)` | UserPhotos / List::query | photos_native_reference; photo_order_back_intermediate_notifications_replace_remove |
| `storage_shared_media.cpp`: `SharedMedia::enforceLists(Key key)` | SharedMedia::enforce_lists | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::add(SharedMediaAddNew &&query)` | SharedMedia::add | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::add(SharedMediaAddExisting &&query)` | SharedMedia::add | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::add(SharedMediaAddSlice &&query)` | SharedMedia::add | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::remove(SharedMediaRemoveOne &&query)` | SharedMedia::remove | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::remove(SharedMediaRemoveAll &&query)` | SharedMedia::remove | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::invalidate(SharedMediaInvalidateBottom &&query)` | SharedMedia::invalidate | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::unload(SharedMediaUnloadThread &&query)` | SharedMedia::unload | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::query(SharedMediaQuery &&query)` | SharedMedia::query | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::snapshot(const SharedMediaQuery &query)` | SharedMedia::snapshot | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::empty(const SharedMediaKey &key)` | SharedMedia::empty | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::sliceUpdated()` | SharedMedia::slice_updated | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::oneRemoved()` | SharedMedia::one_removed | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::allRemoved()` | SharedMedia::all_removed | media_scopes_masks_facade_and_events |
| `storage_shared_media.cpp`: `SharedMedia::bottomInvalidated()` | SharedMedia::bottom_invalidated | media_scopes_masks_facade_and_events |
| `storage_facade.cpp`: `Facade::Impl::add(SharedMediaAddNew &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::add(SharedMediaAddExisting &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::add(SharedMediaAddSlice &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::remove(SharedMediaRemoveOne &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::remove(SharedMediaRemoveAll &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::invalidate(SharedMediaInvalidateBottom &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::unload(SharedMediaUnloadThread &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::query(SharedMediaQuery &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::snapshot(const SharedMediaQuery &query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::empty(const SharedMediaKey &key)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::sharedMediaSliceUpdated()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::sharedMediaOneRemoved()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::sharedMediaAllRemoved()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::sharedMediaBottomInvalidated()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::add(UserPhotosSetBack &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::add(UserPhotosAddNew &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::add(UserPhotosAddSlice &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::remove(UserPhotosRemoveOne &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::remove(UserPhotosRemoveAfter &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::replace(UserPhotosReplace &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::query(UserPhotosQuery &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Impl::userPhotosSliceUpdated()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::Facade()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::add(SharedMediaAddNew &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::add(SharedMediaAddExisting &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::add(SharedMediaAddSlice &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::remove(SharedMediaRemoveOne &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::remove(SharedMediaRemoveAll &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::invalidate(SharedMediaInvalidateBottom &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::unload(SharedMediaUnloadThread &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::query(SharedMediaQuery &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::snapshot(const SharedMediaQuery &query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::empty(const SharedMediaKey &key)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::sharedMediaSliceUpdated()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::sharedMediaOneRemoved()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::sharedMediaAllRemoved()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::sharedMediaBottomInvalidated()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::add(UserPhotosSetBack &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::add(UserPhotosAddNew &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::add(UserPhotosAddSlice &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::remove(UserPhotosRemoveOne &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::remove(UserPhotosRemoveAfter &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::replace(UserPhotosReplace &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::query(UserPhotosQuery &&query)` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::userPhotosSliceUpdated()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_facade.cpp`: `Facade::~Facade()` | Facade.shared_media / Facade.user_photos: прямая делегация, Default/Drop вместо PImpl | media_scopes_masks_facade_and_events; photos_native_reference |
| `storage_cloud_blob.cpp`: `ReadFinalFile(const QString &path)` | read_final_file | extracts_selected_files_and_checks_traversal; rejects_empty_files_missing_parents_and_corruption; rejects_oversized_archives_and_entries |
| `storage_cloud_blob.cpp`: `ExtractZipFile(zlib::FileToRead &zip, const QString path)` | extract_zip_file | extracts_selected_files_and_checks_traversal; rejects_empty_files_missing_parents_and_corruption; rejects_oversized_archives_and_entries |
| `storage_cloud_blob.cpp`: `UnpackBlob(const QString &path, const QString &folder, Fn<bool(const QString &)` | unpack_blob | extracts_selected_files_and_checks_traversal; rejects_empty_files_missing_parents_and_corruption; rejects_oversized_archives_and_entries |
| `storage_cloud_blob.cpp`: `ReadFinalFile(path)` | read_final_file | extracts_selected_files_and_checks_traversal; rejects_empty_files_missing_parents_and_corruption; rejects_oversized_archives_and_entries |
| `storage_cloud_blob.cpp`: `ExtractZipFile(zip, path)` | extract_zip_file | extracts_selected_files_and_checks_traversal; rejects_empty_files_missing_parents_and_corruption; rejects_oversized_archives_and_entries |
| `storage_cloud_blob.cpp`: `StateDescription(const BlobState &state, tr::phrase<> activeText)` | отложено: этап 4 Main::Session / этап 5 Ui::FormatSizeText, tr::phrase | только чтение: отсутствуют реальные владельцы и UI-форматирование; заглушки не добавлены |
| `storage_cloud_blob.cpp`: `BlobLoader::BlobLoader(QObject *parent, not_null<Main::Session*> session, int id, MTP::DedicatedLoader::Location location, const QString &folder, int64 size)` | отложено: этап 4 Main::Session / этап 5 Ui::FormatSizeText, tr::phrase | только чтение: отсутствуют реальные владельцы и UI-форматирование; заглушки не добавлены |
| `storage_cloud_blob.cpp`: `BlobLoader::id()` | отложено: этап 4 Main::Session / этап 5 Ui::FormatSizeText, tr::phrase | только чтение: отсутствуют реальные владельцы и UI-форматирование; заглушки не добавлены |
| `storage_cloud_blob.cpp`: `BlobLoader::state()` | отложено: этап 4 Main::Session / этап 5 Ui::FormatSizeText, tr::phrase | только чтение: отсутствуют реальные владельцы и UI-форматирование; заглушки не добавлены |
| `storage_cloud_blob.cpp`: `BlobLoader::setImplementation(std::unique_ptr<MTP::DedicatedLoader> loader)` | отложено: этап 4 Main::Session / этап 5 Ui::FormatSizeText, tr::phrase | только чтение: отсутствуют реальные владельцы и UI-форматирование; заглушки не добавлены |
| `storage_cloud_blob.cpp`: `BlobLoader::fail()` | отложено: этап 4 Main::Session / этап 5 Ui::FormatSizeText, tr::phrase | только чтение: отсутствуют реальные владельцы и UI-форматирование; заглушки не добавлены |

Заголовки: конструкторы запросов заменены struct literal; сравнения ключей —
derive Eq; enum с kCount заменён закрытым SharedMediaType (13 вариантов).
UserPhotos-параметры передаются непосредственно, без одноразовых C++ query-обёрток.
Facade::Impl не содержит логики, поэтому удалён в пользу прямого владения.
Обновления содержат снимок множества/очереди вместо указателя на изменяемый контейнер.
MsgId и topicRootId — i64, ServerMaxMsgId = 1 << 56.

Непокрытое пока: события shared media и отдельные отрицательные ветки будут
проверены дополнительными тестами до окончательного обновления ledger.
