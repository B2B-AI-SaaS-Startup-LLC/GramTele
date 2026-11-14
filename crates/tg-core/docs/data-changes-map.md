# Data::Changes и контракты хранилищ

Оригинал `0f2a31bb420de98f331990432265b131b8d836b5`: полностью прочитаны
`data_changes.h` (483 LOC), `.cpp` (394 LOC), `data_chat_participant_status.h`
и `main_session.h`; для ключей — реальные store declarations и тела
`peer/message/unregisterMessage/photoConvert/documentConvert`, forum remap/remove,
saved sublist remove. Вызовы Changes сверены в peer/session/forum/saved models.
Новых Peer/History/Session record-моделей, UI substitutes или CRUD-заглушек нет.

Реализация: [data_changes.rs](../src/data/data_changes.rs),
[data_store_keys.rs](../src/data/data_store_keys.rs),
[data_types.rs](../src/data/data_types.rs),
[data_chat_participant_status.rs](../src/data/data_chat_participant_status.rs).
Все названия тестов ниже — функции в
[data_changes_tests.rs](../src/data/data_changes_tests.rs).
[Native harness, scope доказательства и повторение](../tests/reference-tools/data-changes/README.md).

Две исходные Changes единицы `done`; два вспомогательных заголовка `partial`:
у participant-status переведён только enum/flags, остаток — пакет **03**;
у data_types — FolderId/PhotoId/DocumentId, остаток — пакет **16**.
`data_session.h` не объявляется переведённым: контракт ключей извлечён из
его реальных сигнатур, ensure/loaded/records/secondary indices — пакеты **13/14**.
Ни один такой будущий метод не представлен готовой фиктивной реализацией.

## Семантика и границы

- SessionScope — уникальный локальный owner token на каждый login/clear, не
  сетевой session_id. Ключи не сериализуются. Peer/Document/Photo без generation;
  удаляемые сообщения имеют generation. MessageKeys — только identity index,
  не хранилище фиктивных HistoryItem. `insert` выдаёт один ключ для живого wire ID,
  после `remove` — новое поколение. `Destroyed` испускается **до** invalidation,
  как в Session::unregisterMessage (data_session.cpp:3478).
- Имя: BTreeSet<u16> сохраняет QChar (включая lone surrogate); rank: Vec<u16>
  сохраняет QString code units без потери. Peer/User keys проверяют scope/type;
  ID сам по себе не означает loaded. Настоящий ensure создаст placeholder из
  оригинального заголовка, loaded останется отдельным Option.
- Все флаги и raw masks сохранены. Rust overloading: `updates(flags)` и
  `updates_for(key,flags)`; enum LastUsedBit — associated const LAST_USED_BIT.
  None в realtime/CountBit нарушает исходное precondition и даёт assert.
- Realtime доставляет исходную маску **до** изменения pending; repeated realtime
  не сливается. dropScheduled объединяет pending и удаляет его **до** fire;
  Destroyed не планирует flush. manager snapshot берётся перед fire, отдельно
  при достижении каждой категории. Mutable borrow не проходит через callbacks.
- Общий flush: peer → history → message → entry → topic → sublist → story.
  notify сбрасывается до fire, вложенный flush разрешён. Старое main-queue
  задание после ручного flush **не отменяется версией** и может доставить новую
  порцию раньше позднее поставленного задания — как в исходнике.
- Измеренная ошибка C++ `1U << i` сохранена через wrapping_shl32: high-only Peer
  bits 32..57 не посылают realtime; низкие 0..25 также посылают streams 32..57.
  Payload остаётся 64-битным. Native O0/O2 и mixed/raw masks зафиксированы;
  переносимость исходного UB на другие компиляторы не заявляется.
- **Явное отличие порядка внутри одной категории:** C++ flat_map сортирует по
  адресу объекта, Rust BTreeMap — по Ord typed key (scope/wire ID/generation;
  у EntryKey сначала вариант enum).
  С порядком произвольного C++ allocator они не эквивалентны. Сохранены sorted
  snapshot, порядок категорий, flags и точки callbacks. Native сравнение
  нормализует возрастающие адреса в возрастающие ключи; оно не скрывает отличие.
  Если будущий клиент требует конкретный allocator order, это требует отдельного
  согласованного контракта модели; скрытый адрес/слот в заданные tuple keys не добавлен.
- Native исполняет настоящие Manager тела всех 7 типов и Changes ctor/flush.
  Session accessor, main guard, callbacks с Main lifecycle — **own tests + чтение**,
  до пакета **29**. Никаких fake Main/Account тел для обхода этой границы нет.

## Контракт будущих моделей — найденные изменения идентичности

1. **Фото/документы, пакеты 03/04 + Session 13/14**: photoConvert (data_session.cpp:3879)
   и documentConvert (:4158) меняют id существующего объекта. Если destination ID
   свободен, unique_ptr переезжает в новый map key. Если destination уже существует,
   два разных объекта могут иметь одинаковый id; исходный объект остаётся под старым
   map key, а поля применяются также к destination. `(scope,id)` не различает эти
   два pointer identities. Нужен честный design gate пакета моделей, включая все
   references, saved GIFs/cache moves/media leases; ни alias-map, ни generation у
   media в этом срезе не выдуманы. Простого «переименовать map key» недостаточно.
2. **Сообщения, пакеты 13/14, 18..20**: local→server FullMsgId remap переносит
   ту же запись; нужно атомарно обновить все references/non-channel/dependency
   indices/Changes pending и subscriptions в точных исходных emit-точках.
   API ремапа здесь не имитируется без настоящего HistoryItem. MessageKeys::remove
   вызывается только после Destroyed, и старый generation не удаляет новый ключ.
3. **Topics, пакет 08**: Forum::created меняет rootId того же объекта
   (data_forum.cpp:578); topic key/Entry references должны измениться согласованно.
   Topic/SavedSublist могут быть erased/recreated (forum.cpp:233/555,
   saved_messages.cpp:527). Дополнительная generation им не добавлена по решению
   пользователя; подписки старого объекта обязаны завершаться его lifetime, чтобы
   одинаковый tuple не захватил новую запись. Соответствующие модели пока не портированы.

## Карта

Строки — каждая функция Changes/Manager (включая обе overload updates),
перенесённые value contracts и добавленные операции идентичности. `done` для
Changes означает завершённый порт с перечисленными адаптациями/ограничениями,
а не побайтный wire/disk формат (такого формата у Changes нет).

| Оригинал / контракт | Rust | Тест | Доказательство | Статус |
|---|---|---|---|---|
| `details::CountBit` | `count_bit` | `count_bit_boundaries; count_bit_zero; count_bit_multiple` | native declarations + own preconditions | done |
| `NameUpdate::NameUpdate` | `NameUpdate::new` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `Changes::Manager::updated` | `Manager::updated` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Manager::sendRealtimeNotifications` | `Manager::send_realtime_notifications` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Manager::updates(flags)` | `Manager::updates` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Manager::updates(data,flags)` | `Manager::updates_for` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Manager::realtimeUpdates` | `Manager::realtime_updates` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Manager::flagsValue` | `Manager::flags_value` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Manager::drop` | `Manager::drop_scheduled` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Manager::sendNotifications` | `Manager::send_notifications` | `native_all_manager_sequences_and_changes_flush_order; native_peer_shift_all_58_bits` | native O0/O2, all 7 managers | done |
| `Changes::Changes` | `Changes::new / with_queue` | `name_admin_rank_are_immediate_utf16_and_scoped; queued_flush_cancels_with_scope_or_service` | native ctor + own scope guard | done |
| `Changes::session` | `Changes::session` | `name_admin_rank_are_immediate_utf16_and_scoped; changes_reject_foreign_scope` | scope binding: own tests + чтение (Main lifecycle package 29) | done |
| `Changes::nameUpdated` | `Changes::name_updated` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `Changes::realtimeNameUpdates()` | `Changes::realtime_name_updates` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `Changes::realtimeNameUpdates(peer)` | `Changes::realtime_name_updates_for` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `PeerUpdate::Flag/Flags/LastUsedBit` | `PeerFlag / PeerFlags / LAST_USED_BIT` | `native_all_flag_values_and_last_bits` | native O0/O2, exact enum bits | done |
| `PeerUpdate aggregate fields` | `impl Update for PeerUpdate::key` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `PeerUpdate aggregate fields` | `impl Update for PeerUpdate::bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `PeerUpdate aggregate fields` | `impl Update for PeerUpdate::from_bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `Changes::peerUpdated` | `Changes::peer_updated` | `peer_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::peerUpdates(flags)` | `Changes::peer_updates` | `peer_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::peerUpdates(data,flags)` | `Changes::peer_updates_for` | `peer_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::peerFlagsValue` | `Changes::peer_flags_value` | `peer_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::realtimePeerUpdates` | `Changes::realtime_peer_updates` | `peer_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `HistoryUpdate::Flag/Flags/LastUsedBit` | `HistoryFlag / HistoryFlags / LAST_USED_BIT` | `native_all_flag_values_and_last_bits` | native O0/O2, exact enum bits | done |
| `HistoryUpdate aggregate fields` | `impl Update for HistoryUpdate::key` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `HistoryUpdate aggregate fields` | `impl Update for HistoryUpdate::bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `HistoryUpdate aggregate fields` | `impl Update for HistoryUpdate::from_bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `Changes::historyUpdated` | `Changes::history_updated` | `history_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::historyUpdates(flags)` | `Changes::history_updates` | `history_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::historyUpdates(data,flags)` | `Changes::history_updates_for` | `history_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::historyFlagsValue` | `Changes::history_flags_value` | `history_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::realtimeHistoryUpdates` | `Changes::realtime_history_updates` | `history_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `TopicUpdate::Flag/Flags/LastUsedBit` | `TopicFlag / TopicFlags / LAST_USED_BIT` | `native_all_flag_values_and_last_bits` | native O0/O2, exact enum bits | done |
| `TopicUpdate aggregate fields` | `impl Update for TopicUpdate::key` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `TopicUpdate aggregate fields` | `impl Update for TopicUpdate::bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `TopicUpdate aggregate fields` | `impl Update for TopicUpdate::from_bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `Changes::topicUpdated` | `Changes::topic_updated` | `topic_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::topicUpdates(flags)` | `Changes::topic_updates` | `topic_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::topicUpdates(data,flags)` | `Changes::topic_updates_for` | `topic_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::topicFlagsValue` | `Changes::topic_flags_value` | `topic_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::realtimeTopicUpdates` | `Changes::realtime_topic_updates` | `topic_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::topicRemoved` | `Changes::topic_removed` | `topic_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `SublistUpdate::Flag/Flags/LastUsedBit` | `SublistFlag / SublistFlags / LAST_USED_BIT` | `native_all_flag_values_and_last_bits` | native O0/O2, exact enum bits | done |
| `SublistUpdate aggregate fields` | `impl Update for SublistUpdate::key` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `SublistUpdate aggregate fields` | `impl Update for SublistUpdate::bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `SublistUpdate aggregate fields` | `impl Update for SublistUpdate::from_bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `Changes::sublistUpdated` | `Changes::sublist_updated` | `sublist_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::sublistUpdates(flags)` | `Changes::sublist_updates` | `sublist_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::sublistUpdates(data,flags)` | `Changes::sublist_updates_for` | `sublist_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::sublistFlagsValue` | `Changes::sublist_flags_value` | `sublist_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::realtimeSublistUpdates` | `Changes::realtime_sublist_updates` | `sublist_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::sublistRemoved` | `Changes::sublist_removed` | `sublist_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `MessageUpdate::Flag/Flags/LastUsedBit` | `MessageFlag / MessageFlags / LAST_USED_BIT` | `native_all_flag_values_and_last_bits` | native O0/O2, exact enum bits | done |
| `MessageUpdate aggregate fields` | `impl Update for MessageUpdate::key` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `MessageUpdate aggregate fields` | `impl Update for MessageUpdate::bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `MessageUpdate aggregate fields` | `impl Update for MessageUpdate::from_bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `Changes::messageUpdated` | `Changes::message_updated` | `message_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::messageUpdates(flags)` | `Changes::message_updates` | `message_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::messageUpdates(data,flags)` | `Changes::message_updates_for` | `message_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::messageFlagsValue` | `Changes::message_flags_value` | `message_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::realtimeMessageUpdates` | `Changes::realtime_message_updates` | `message_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `EntryUpdate::Flag/Flags/LastUsedBit` | `EntryFlag / EntryFlags / LAST_USED_BIT` | `native_all_flag_values_and_last_bits` | native O0/O2, exact enum bits | done |
| `EntryUpdate aggregate fields` | `impl Update for EntryUpdate::key` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `EntryUpdate aggregate fields` | `impl Update for EntryUpdate::bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `EntryUpdate aggregate fields` | `impl Update for EntryUpdate::from_bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `Changes::entryUpdated` | `Changes::entry_updated` | `entry_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::entryUpdates(flags)` | `Changes::entry_updates` | `entry_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::entryUpdates(data,flags)` | `Changes::entry_updates_for` | `entry_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::entryFlagsValue` | `Changes::entry_flags_value` | `entry_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::realtimeEntryUpdates` | `Changes::realtime_entry_updates` | `entry_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::entryRemoved` | `Changes::entry_removed` | `entry_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `StoryUpdate::Flag/Flags/LastUsedBit` | `StoryFlag / StoryFlags / LAST_USED_BIT` | `native_all_flag_values_and_last_bits` | native O0/O2, exact enum bits | done |
| `StoryUpdate aggregate fields` | `impl Update for StoryUpdate::key` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `StoryUpdate aggregate fields` | `impl Update for StoryUpdate::bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `StoryUpdate aggregate fields` | `impl Update for StoryUpdate::from_bits` | `native_all_manager_sequences_and_changes_flush_order` | native O0/O2 | done |
| `Changes::storyUpdated` | `Changes::story_updated` | `story_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::storyUpdates(flags)` | `Changes::story_updates` | `story_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::storyUpdates(data,flags)` | `Changes::story_updates_for` | `story_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::storyFlagsValue` | `Changes::story_flags_value` | `story_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::realtimeStoryUpdates` | `Changes::realtime_story_updates` | `story_public_api_filters_initial_realtime_and_flush` | own tests + чтение | done |
| `Changes::chatAdminChanged` | `Changes::chat_admin_changed` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `Changes::chatAdminChanges` | `Changes::chat_admin_changes` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `Changes::chatMemberRankChanged` | `Changes::chat_member_rank_changed` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `Changes::chatMemberRankChanges` | `Changes::chat_member_rank_changes` | `name_admin_rank_are_immediate_utf16_and_scoped` | own tests + чтение | done |
| `Changes::scheduleNotifications` | `Changes::schedule_notifications` | `manual_flush_does_not_cancel_old_queued_job; queued_flush_cancels_with_scope_or_service; cross_category_updates_join_current_flush_but_earlier_category_waits` | own tests + чтение (native main guard: package 29) | done |
| `Changes::sendNotifications` | `Changes::send_notifications` | `native_all_manager_sequences_and_changes_flush_order; pending_borrows_are_released_cross_category_and_recursive_flush` | native category order; own queue/reentrancy | done |
| `ChatAdminRight / ChatAdminRights (data_chat_participant_status.h)` | `ChatAdminRight / Flags<ChatAdminRight>` | `native_all_flag_values_and_last_bits; name_admin_rank_are_immediate_utf16_and_scoped` | native declarations; header partial → package 03 | contract done |
| `data_types.h::FolderId` | `data_types::FolderId` | `scope_keys_message_reuse_and_typed_views` | own tests + header widths; header partial → package 16 | contract done |
| `data_types.h::PhotoId` | `data_types::PhotoId` | `scope_keys_message_reuse_and_typed_views` | own tests + header widths; header partial → package 16 | contract done |
| `data_types.h::DocumentId` | `data_types::DocumentId` | `scope_keys_message_reuse_and_typed_views` | own tests + header widths; header partial → package 16 | contract done |
| `Session lifetime (stage4 ownership adaptation)` | `SessionScope::new` | `scope_keys_message_reuse_and_typed_views; queued_flush_cancels_with_scope_or_service` | own tests + чтение | contract done |
| `Session lifetime (stage4 ownership adaptation)` | `SessionScope::default` | `scope_keys_message_reuse_and_typed_views; queued_flush_cancels_with_scope_or_service` | own tests + чтение | contract done |
| `Session lifetime (stage4 ownership adaptation)` | `SessionScope::id` | `scope_keys_message_reuse_and_typed_views; queued_flush_cancels_with_scope_or_service` | own tests + чтение | contract done |
| `Session lifetime (stage4 ownership adaptation)` | `SessionScope::downgrade` | `scope_keys_message_reuse_and_typed_views; queued_flush_cancels_with_scope_or_service` | own tests + чтение | contract done |
| `Local SessionScope token, not MTProto session_id` | `SessionScopeId::get` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Data::Session store identity` | `SessionScope::peer_key` | `scope_keys_message_reuse_and_typed_views` | own tests + source headers; records → packages 02..20 | contract done |
| `Data::Session store identity` | `SessionScope::document_key` | `scope_keys_message_reuse_and_typed_views` | own tests + source headers; records → packages 02..20 | contract done |
| `Data::Session store identity` | `SessionScope::photo_key` | `scope_keys_message_reuse_and_typed_views` | own tests + source headers; records → packages 02..20 | contract done |
| `Data::Session store identity` | `SessionScope::story_key` | `scope_keys_message_reuse_and_typed_views` | own tests + source headers; records → packages 02..20 | contract done |
| `Data::Session store identity` | `SessionScope::folder_key` | `scope_keys_message_reuse_and_typed_views` | own tests + source headers; records → packages 02..20 | contract done |
| `Scoped wire identity` | `PeerKey::id` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `PeerKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `DocumentKey::id` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `DocumentKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `PhotoKey::id` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `PhotoKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `StoryKey::id` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `StoryKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `FolderKey::id` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Scoped wire identity` | `FolderKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `UserKey::try_from` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `UserKey::peer` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `UserKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `ChatKey::try_from` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `ChatKey::peer` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `ChatKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `ChannelKey::try_from` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `ChannelKey::peer` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `PeerData typed views` | `ChannelKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `HistoryItem allocation identity` | `MessageKey::id` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `HistoryItem allocation identity` | `MessageKey::generation` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `HistoryItem allocation identity` | `MessageKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + чтение | contract done |
| `Data::Session::_messages identity index` | `MessageKeys::new` | `scope_keys_message_reuse_and_typed_views; zero_message_not_registered; destroyed_message_is_still_live_during_notifications` | own tests + unregisterMessage ordering; records/ID remap → packages 13/14, 18..20 | contract done |
| `Data::Session::_messages identity index` | `MessageKeys::get` | `scope_keys_message_reuse_and_typed_views; zero_message_not_registered; destroyed_message_is_still_live_during_notifications` | own tests + unregisterMessage ordering; records/ID remap → packages 13/14, 18..20 | contract done |
| `Data::Session::_messages identity index` | `MessageKeys::insert` | `scope_keys_message_reuse_and_typed_views; zero_message_not_registered; destroyed_message_is_still_live_during_notifications` | own tests + unregisterMessage ordering; records/ID remap → packages 13/14, 18..20 | contract done |
| `Data::Session::_messages identity index` | `MessageKeys::remove` | `scope_keys_message_reuse_and_typed_views; zero_message_not_registered; destroyed_message_is_still_live_during_notifications` | own tests + unregisterMessage ordering; records/ID remap → packages 13/14, 18..20 | contract done |
| `Data::Session::_messages identity index` | `MessageKeys::contains` | `scope_keys_message_reuse_and_typed_views; zero_message_not_registered; destroyed_message_is_still_live_during_notifications` | own tests + unregisterMessage ordering; records/ID remap → packages 13/14, 18..20 | contract done |
| `ForumTopic::owningHistory/rootId` | `TopicKey / ScopedKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + header; mutable root ID → package 08 | contract done |
| `SavedSublist::owningHistory/sublistPeer` | `SublistKey::new` | `scope_keys_message_reuse_and_typed_views; sublist_cannot_join_accounts` | own tests + header; records → package 12 | contract done |
| `SavedSublist::owningHistory/sublistPeer` | `SublistKey::history` | `scope_keys_message_reuse_and_typed_views; sublist_cannot_join_accounts` | own tests + header; records → package 12 | contract done |
| `SavedSublist::owningHistory/sublistPeer` | `SublistKey::peer` | `scope_keys_message_reuse_and_typed_views; sublist_cannot_join_accounts` | own tests + header; records → package 12 | contract done |
| `SavedSublist::owningHistory/sublistPeer` | `SublistKey::scope` | `scope_keys_message_reuse_and_typed_views; sublist_cannot_join_accounts` | own tests + header; records → package 12 | contract done |
| `Dialogs::Entry::Type / owner` | `EntryKey / ScopedKey::scope` | `scope_keys_message_reuse_and_typed_views` | own tests + four actual variants | contract done |
| `Session ownership precondition` | `Changes::assert_alive / check` | `changes_reject_foreign_scope; queued_flush_cancels_with_scope_or_service` | own tests + чтение | contract done |
| `Rust Manager initialization` | `Manager::new` | `native_all_manager_sequences_and_changes_flush_order` | native sequences | done |

Учёт: **136 строк**, **0 без теста**, **0 partial строк**. Полный список функций обеих Changes единиц: **59** (49 Changes + 8 Manager + CountBit + NameUpdate ctor). Вне этих единиц отложенное названо выше по пакетам 03/08/12/13/14/16/18..20/29. Проверки: **25 новых tests**, 3 native CSV / 600 строк.
