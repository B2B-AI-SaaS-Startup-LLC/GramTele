# ChatData / ChannelData / rights — пакет 03

Оригинал `0f2a31bb420de98f331990432265b131b8d836b5`: полностью прочитаны
8 файлов задания (4299 LOC): data_chat.h/.cpp, data_channel.h/.cpp,
data_channel_admins.h/.cpp, data_chat_participant_status.h/.cpp. Их функции,
inline-методы и перегрузки ниже идут отдельными строками; `.cpp` строка покрывает
также declaration в `.h`. Добавленные value dependencies выделены отдельными
контрактами; это не утверждение, что весь их большой исходный файл переведён.

Rust реализации — `src/data/`; мутации владельца — `data_group_store.rs`,
TL model phases — `data_chat_updates.rs` / `data_channel_updates.rs`. Имена тестов
ниже — функции [data_chat_channel.rs](../tests/data_chat_channel.rs).
[Native proof / воспроизведение](../tests/reference-tools/data-chat-channel/README.md).

## Владение и привязки

Peers хранит один PeerRecord на scope+PeerId. ChatData/ChannelData владеют одним
настоящим PeerData. UserBase — ранее принятый общий срез, не фиктивный UserData;
модель UserData02 ещё отсутствует. Участники/боты/админы/миграции — scoped keys.
UserParticipantFields — обязательные настоящие поля UserData/бота, без Default
или production provider-заглушки. GroupContext принимает часы, self key и реальные
User/theme facts callbacks, Ayu filter setting приходит явно. Никакого Thread,
History, Session, GroupCall, Forum, SavedMessages, Community или UI stand-in нет.

Flags service handle клонируется за пределы borrow. Каждый fire выполняется
после освобождения Peers map borrow, а следующие фазы читают актуальное состояние,
включая вложенные мутации callbacks. Ctor ChatData устанавливает CallNotEmpty
подписку раньше клиентских подписок. Счётчики/получатели и order не исправляются.
ChannelAdminChanges/MemberRankChanges используют явный finish на том же месте
исходной scoped destruction; owner испускает итог после освобождения borrow.
Произвольный drop незавершённого batch / panic-unwinding не эквивалентны C++ RAII;
production owner paths явно вызывают finish, тесты проверяют эти границы.

GroupEffect — упорядоченные вызовы будущих настоящих сервисов, не no-op apply.
Обработчики обязаны учитывать наличие History, owner lifetime и очередь UI как
в исходнике; они ещё не реализованы. `done` обозначает перенесённую model/pure
фазу, не подтверждение внешней реакции сервиса. Full-update функции помечены
partial по оставшимся bindings. Boundary test не исполняет отложенное тело.

GroupCall08 хранится только ключом и регистрационными/миграционными командами;
никаких фиктивных данных звонка. Forum08/SavedMessages12/Community07 — owner keys
и команды, реальные record stores позже. PtsWaiter12 не подменён числом pts.
Photo processing03-media/04, notify/wallpaper16, History18, API23/29, UI5/6, lang01
и настоящие effects apply ещё не проверены полным Session replay.

## Эталон и особенности исходника

Native481 строк: реальные rights conversions и send unions; неизменённая чистая
начальная фаза ChannelData::setFlags; исходный IsRestrictedForever; исходный
filterZalgo и Qt/PCRE Mn classification для всех Unicode scalar values.
Два native запуска совпадают, Rust сверяет CSV. Полные модели, inherited canX,
state/TL effect ordering — **own tests + чтение**, без fake C++ classes.
Unicode Mn таблица фиксирует Qt6.11.2/PCRE данного native, не произвольные версии.
Входные UTF-8 имена не выражают lone surrogate QString.

Сохранены: двойное изменение adminsCount в applyEditAdmin; emplace сохраняет
старые self admin/restriction entries; remove admin оставляет rank; self-only
History batch подавляется; null monoforum link сохраняет прежний key и меняет
disabled; Skipped version не присваивается (full participants всё равно продолжается);
invalidateParticipants сохраняет ranks/default bans/lastAuthors/markupSenders;
boosts события идут только при смене unrestricted, перед Rights|Slowmode;
setStoriesState напрямую меняет Flags, минуя setFlags. ChannelFull оставляет
HasStarsPerMessage после перехода stars→0 из-за условной mask.
ReactionId Custom(0) не пуст при создании, но TL conversion теряет этот variant,
как исходный ReactionToMTP. Старшая realtime маска Changes не исправлялась.

Явные замены: C++ pointers → typed keys, pointer order → key Ord (уже зафиксированный
design gate), state mutations → released-borrow effect phases. Передача GroupCall
key сохраняет CallId/scope; настоящий record/lifetime08 не доказан. Media ID remap
gate03/04/16 не обходился, generation остаётся только messages.

Signed-overflow края счётчиков/version/slowmode: Rust wrapping, C++ UB вне
проверенного native/state домена не подтверждено. Forum detach/retain/recreate
lifetime с вложенным callback требует настоящего owner08; один owner key
не доказывает жизнь прежнего объекта.

## Карта

**337 строк: 287 done + 50 partial**, 0 пустых реализаций/тестовых ссылок; все partial с будущим пакетом.

| Источник | Функция / контракт | Статус | Реализация / граница | Тест |
|---|---|---|---|---|
| data/data_chat.cpp:34 | ChatData::ChatData(not_null<Data::Session*> owner, PeerId id) : PeerData(owner, id) | done | `src/data/data_chat.rs::new` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:46 | void ChatData::setPhoto(const MTPChatPhoto &photo) | done | `src/data/data_group_store.rs::set_group_photo` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_chat.cpp:57 | ChatAdminRightsInfo ChatData::defaultAdminRights(not_null<UserData*> user) | done | `src/data/data_chat.rs::default_admin_rights` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:72 | bool ChatData::isAyuNoForwards() const | done | `src/data/data_chat.rs::is_ayu_no_forwards` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:76 | bool ChatData::allowsForwarding() const | done | `src/data/data_chat.rs::allows_forwarding` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:80 | bool ChatData::canEditInformation() const | done | `src/data/data_chat.rs::can_edit_information` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:84 | bool ChatData::canEditPermissions() const | done | `src/data/data_chat.rs::can_edit_permissions` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:89 | bool ChatData::canEditUsername() const | done | `src/data/data_chat.rs::can_edit_username` | names_zalgo_usernames_source_update_order |
| data/data_chat.cpp:94 | bool ChatData::canEditPreHistoryHidden() const | done | `src/data/data_chat.rs::can_edit_pre_history_hidden` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:98 | bool ChatData::canDeleteMessages() const | done | `src/data/data_chat.rs::can_delete_messages` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:103 | bool ChatData::canAddMembers() const | done | `src/data/data_chat.rs::can_add_members` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:107 | bool ChatData::canAddAdmins() const | done | `src/data/data_chat.rs::can_add_admins` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:111 | bool ChatData::canBanMembers() const | done | `src/data/data_chat.rs::can_ban_members` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:116 | bool ChatData::anyoneCanAddMembers() const | done | `src/data/data_chat.rs::anyone_can_add_members` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:120 | void ChatData::setName(const QString &newName) | done | `src/data/data_group_store.rs::set_chat_name` | names_zalgo_usernames_source_update_order |
| data/data_chat.cpp:129 | void ChatData::applyEditAdmin(not_null<UserData*> user, bool isAdmin) | done | `src/data/data_group_store.rs::apply_chat_edit_admin` | edited_admin_original_double_counts_bans_and_stale_emplace |
| data/data_chat.cpp:138 | void ChatData::invalidateParticipants() | done | `src/data/data_group_store.rs::invalidate_chat_participants` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:150 | void ChatData::setFlags(ChatDataFlags which) | done | `src/data/data_group_store.rs::set_chat_flags` | flags_callbacks_run_after_borrow_and_before_followup |
| data/data_chat.cpp:162 | void ChatData::setInviteLink(const QString &newInviteLink) | done | `src/data/data_group_store.rs::set_chat_invite_link` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:166 | bool ChatData::canHaveInviteLink() const | done | `src/data/data_chat.rs::can_have_invite_link` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:171 | void ChatData::setAdminRights(ChatAdminRights rights) | done | `src/data/data_group_store.rs::set_chat_admin_rights` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:184 | void ChatData::setDefaultRestrictions(ChatRestrictions rights) | done | `src/data/data_group_store.rs::set_chat_default_restrictions` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:192 | void ChatData::refreshBotStatus() | done | `src/data/data_group_store.rs::refresh_chat_bot_status` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:205 | auto ChatData::applyUpdateVersion(int version) -> UpdateStatus | done | `src/data/data_group_store.rs::apply_chat_update_version` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:217 | ChannelData *ChatData::getMigrateToChannel() const | done | `src/data/data_chat.rs::get_migrate_to_channel` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_chat.cpp:221 | void ChatData::setMigrateToChannel(ChannelData *channel) | done | `src/data/data_group_store.rs::set_chat_migrate_to_channel` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_chat.cpp:230 | void ChatData::setGroupCall( const MTPInputGroupCall &call, TimeId scheduleDate, bool rtmp) | partial(08) | `src/data/data_group_store.rs::set_group_call` — keys/registration commands only | Отложенная часть не исполнялась; boundary: group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_chat.cpp:265 | void ChatData::clearGroupCall() | partial(08) | `src/data/data_group_store.rs::clear_group_call` — keys/unregistration/migration commands only | Отложенная часть не исполнялась; boundary: group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_chat.cpp:278 | void ChatData::setGroupCallDefaultJoinAs(PeerId peerId) | done | `src/data/data_group_store.rs::set_group_call_join_as` | group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_chat.cpp:282 | PeerId ChatData::groupCallDefaultJoinAs() const | done | `src/data/data_chat.rs::group_call_default_join_as` | group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_chat.cpp:286 | void ChatData::setBotCommands(const std::vector<Data::BotCommands> &list) | done | `src/data/data_group_store.rs::set_chat_bot_commands` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:292 | void ChatData::setPendingRequestsCount( int count, const QVector<MTPlong> &recentRequesters) | done | `src/data/data_group_store.rs::set_chat_pending_requests` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:302 | void ChatData::setPendingRequestsCount( int count, std::vector<UserId> recentRequesters) | done | `src/data/data_group_store.rs::set_chat_pending_requests` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:313 | void ChatData::setAllowedReactions(Data::AllowedReactions value) | done | `src/data/data_group_store.rs::set_chat_allowed_reactions` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:330 | const Data::AllowedReactions &ChatData::allowedReactions() const | done | `src/data/data_chat.rs::allowed_reactions` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:334 | MTPlong ChatData::inputChat() const | done | `src/data/data_chat.rs::input_chat` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.cpp:340 | void ApplyChatUpdate( not_null<ChatData*> chat, const MTPDupdateChatParticipants &update) | done | `src/data/data_chat_updates.rs::apply_chat_participants_update` — real loaded-user projection/History command boundary | chat_tl_versions_missing_users_forbidden_and_bot_participants |
| data/data_chat.cpp:346 | void ApplyChatUpdate( not_null<ChatData*> chat, const MTPDupdateChatParticipantAdd &update) | done | `src/data/data_chat_updates.rs::apply_chat_participant_add` — real loaded-user projection/History command boundary | chat_tl_versions_missing_users_forbidden_and_bot_participants |
| data/data_chat.cpp:387 | void ApplyChatUpdate( not_null<ChatData*> chat, const MTPDupdateChatParticipantDelete &update) | done | `src/data/data_chat_updates.rs::apply_chat_participant_delete` — real loaded-user projection/History command boundary | chat_tl_versions_missing_users_forbidden_and_bot_participants |
| data/data_chat.cpp:429 | void ApplyChatUpdate( not_null<ChatData*> chat, const MTPDupdateChatParticipantAdmin &update) | done | `src/data/data_chat_updates.rs::apply_chat_participant_admin` — real loaded-user projection/History command boundary | chat_tl_versions_missing_users_forbidden_and_bot_participants |
| data/data_chat.cpp:459 | void ApplyChatUpdate( not_null<ChatData*> chat, const MTPDupdateChatParticipantRank &update) | done | `src/data/data_chat_updates.rs::apply_chat_participant_rank` — real loaded-user projection/History command boundary | chat_tl_versions_missing_users_forbidden_and_bot_participants |
| data/data_chat.cpp:485 | void ApplyChatUpdate( not_null<ChatData*> chat, const MTPDupdateChatDefaultBannedRights &update) | done | `src/data/data_chat_updates.rs::apply_chat_default_banned_rights` — real loaded-user projection/History command boundary | chat_tl_versions_missing_users_forbidden_and_bot_participants |
| data/data_chat.cpp:496 | void ApplyChatUpdate(not_null<ChatData*> chat, const MTPDchatFull &update) | partial(03/08/09/13/16/18/29/5) | `src/data/data_chat_updates.rs::apply_chat_full` — full owner effect bindings where applicable | Отложенная часть не исполнялась; boundary: tl_full_updates_apply_scalar_state_and_preserve_stale_star_flag |
| data/data_chat.cpp:559 | void ApplyChatUpdate( not_null<ChatData*> chat, const MTPChatParticipants &participants) | done | `src/data/data_chat_updates.rs::apply_chat_participants` — real loaded-user callback, no UserData model substitution | chat_tl_versions_missing_users_forbidden_and_bot_participants |
| data/data_chat.h:44 | bool noParticipantInfo() const | done | `src/data/data_chat.rs::no_participant_info` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:49 | void addFlags(ChatDataFlags which) | done | `src/data/data_group_store.rs::add_chat_flags` | flags_callbacks_run_after_borrow_and_before_followup |
| data/data_chat.h:52 | void removeFlags(ChatDataFlags which) | done | `src/data/data_group_store.rs::remove_chat_flags` | flags_callbacks_run_after_borrow_and_before_followup |
| data/data_chat.h:55 | auto flags() const | done | `src/data/data_chat.rs::flags` | flags_callbacks_run_after_borrow_and_before_followup |
| data/data_chat.h:58 | auto flagsValue() const | done | `src/data/data_chat.rs::flags_value` | flags_callbacks_run_after_borrow_and_before_followup |
| data/data_chat.h:62 | auto adminRights() const | done | `src/data/data_chat.rs::admin_rights` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:65 | auto adminRightsValue() const | done | `src/data/data_chat.rs::admin_rights_value` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:69 | bool hasAdminRights() const | done | `src/data/data_chat.rs::has_admin_rights` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:73 | auto defaultRestrictions() const | done | `src/data/data_chat.rs::default_restrictions` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:76 | auto defaultRestrictionsValue() const | done | `src/data/data_chat.rs::default_restrictions_value` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:81 | bool isForbidden() const | done | `src/data/data_chat.rs::is_forbidden` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:84 | bool amIn() const | done | `src/data/data_chat.rs::am_in` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:87 | bool haveLeft() const | done | `src/data/data_chat.rs::have_left` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:90 | bool amCreator() const | done | `src/data/data_chat.rs::am_creator` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:93 | bool isDeactivated() const | done | `src/data/data_chat.rs::is_deactivated` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:96 | bool isMigrated() const | done | `src/data/data_chat.rs::is_migrated` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_chat.h:100 | bool hasWelcomeMessages() const | done | `src/data/data_chat.rs::has_welcome_messages` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:123 | QString inviteLink() const | done | `src/data/data_chat.rs::invite_link` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:134 | int version() const | done | `src/data/data_chat.rs::version` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:137 | void setVersion(int version) | done | `src/data/data_group_store.rs::set_chat_version` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:145 | Data::GroupCall *groupCall() const | done | `src/data/data_chat.rs::group_call` | group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_chat.h:157 | const Data::ChatBotCommands &botCommands() const | done | `src/data/data_chat.rs::bot_commands` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:161 | int pendingRequestsCount() const | done | `src/data/data_chat.rs::pending_requests_count` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_chat.h:164 | const std::vector<UserId> &recentRequesters() const | done | `src/data/data_chat.rs::recent_requesters` | chat_permissions_versions_and_partial_participant_invalidation |
| data/data_channel.cpp:54 | MegagroupInfo::MegagroupInfo() = default; | done | `src/data/data_channel.rs::default` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:56 | MegagroupInfo::~MegagroupInfo() = default; | partial(08/12/29) | Rust owned drop; external model lifetime remains its package | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:58 | ChatData *MegagroupInfo::getMigrateFromChat() const | done | `src/data/data_channel.rs::get_migrate_from_chat` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:62 | void MegagroupInfo::setMigrateFromChat(ChatData *chat) | done | `src/data/data_channel.rs::set_migrate_from_chat` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:66 | const ChannelLocation *MegagroupInfo::getLocation() const | done | `src/data/data_channel.rs::get_location` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:70 | void MegagroupInfo::setLocation(const ChannelLocation &location) | done | `src/data/data_channel.rs::set_location` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:74 | Data::ChatBotCommands::Changed MegagroupInfo::setBotCommands( const std::vector<Data::BotCommands> &list) | done | `src/data/data_channel.rs::set_bot_commands` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:79 | void MegagroupInfo::ensureForum(not_null<ChannelData*> that) | partial(08) | actual Forum/History owner creation command | Отложенная часть не исполнялась; boundary: flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:87 | Data::Forum *MegagroupInfo::forum() const | done | `src/data/data_channel.rs::forum` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:91 | std::unique_ptr<Data::Forum> MegagroupInfo::takeForumData() | partial(08/18) | actual Forum detach/lifetime command | Отложенная часть не исполнялась; boundary: flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:99 | void MegagroupInfo::ensureMonoforum(not_null<ChannelData*> that) | partial(12/18) | actual SavedMessages/History owner | Отложенная часть не исполнялась; boundary: flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:109 | Data::SavedMessages *MegagroupInfo::monoforum() const | done | `src/data/data_channel.rs::monoforum` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:113 | std::unique_ptr<Data::SavedMessages> MegagroupInfo::takeMonoforumData() | partial(12/18) | actual SavedMessages detach | Отложенная часть не исполнялась; boundary: flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:122 | ChannelData::ChannelData(not_null<Data::Session*> owner, PeerId id) : PeerData(owner, id) , _ptsWaiter(&owner->session().updates()) | done | `src/data/data_channel.rs::new` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:127 | void ChannelData::setPhoto(const MTPChatPhoto &photo) | done | `src/data/data_group_store.rs::set_group_photo` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:138 | void ChannelData::setName( const QString &newName, const QString &newUsername) | done | `src/data/data_group_store.rs::set_channel_name` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:149 | void ChannelData::setUsername(const QString &username) | done | `src/data/data_user_names.rs::set_username` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:153 | void ChannelData::setUsernames(const Data::Usernames &newUsernames) | done | `src/data/data_user_names.rs::set_usernames` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:170 | QString ChannelData::username() const | done | `src/data/data_channel.rs::username` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:174 | QString ChannelData::editableUsername() const | done | `src/data/data_channel.rs::editable_username` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:178 | const std::vector<QString> &ChannelData::usernames() const | done | `src/data/data_channel.rs::usernames` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:182 | bool ChannelData::isUsernameEditable(QString username) const | done | `src/data/data_channel.rs::is_username_editable` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:186 | void ChannelData::setAccessHash(uint64 accessHash) | done | `src/data/data_group_store.rs::set_channel_access_hash` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:190 | void ChannelData::setFlags(ChannelDataFlags which) | done | `src/data/data_group_store.rs::set_channel_flags` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_channel.cpp:309 | void ChannelData::addFlags(ChannelDataFlags which) | done | `src/data/data_group_store.rs::add_channel_flags` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_channel.cpp:313 | void ChannelData::removeFlags(ChannelDataFlags which) | done | `src/data/data_group_store.rs::remove_channel_flags` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_channel.cpp:317 | void ChannelData::setInviteLink(const QString &newInviteLink) | done | `src/data/data_group_store.rs::set_channel_invite_link` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:321 | bool ChannelData::canHaveInviteLink() const | done | `src/data/data_channel.rs::can_have_invite_link` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:326 | void ChannelData::setLocation(const MTPChannelLocation &data) | done | `src/data/data_group_store.rs::set_channel_location` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:353 | const ChannelLocation *ChannelData::getLocation() const | done | `src/data/data_channel.rs::get_location` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:357 | void ChannelData::setDiscussionLink(ChannelData *linked) | done | `src/data/data_group_store.rs::set_channel_discussion_link` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:368 | ChannelData *ChannelData::discussionLink() const | done | `src/data/data_channel.rs::discussion_link` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:372 | bool ChannelData::discussionLinkKnown() const | done | `src/data/data_channel.rs::discussion_link_known` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:376 | void ChannelData::setMonoforumLink(ChannelData *link) | done | `src/data/data_group_store.rs::set_channel_monoforum_link` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:399 | ChannelData *ChannelData::monoforumLink() const | done | `src/data/data_channel.rs::monoforum_link` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:403 | bool ChannelData::monoforumDisabled() const | done | `src/data/data_channel.rs::monoforum_disabled` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:407 | void ChannelData::setLinkedCommunityId(ChannelId id) | done | `src/data/data_group_store.rs::set_channel_linked_community_id` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:419 | not_null<Data::CommunityInfo*> ChannelData::ensuredCommunityInfo() | partial(07) | `src/data/data_group_store.rs::ensure_channel_community_info` — actual CommunityInfo owner command/key | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:426 | ChannelId ChannelData::linkedCommunityId() const | done | `src/data/data_channel.rs::linked_community_id` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:430 | bool ChannelData::canManageLinkedPeers() const | done | `src/data/data_channel.rs::can_manage_linked_peers` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:436 | bool ChannelData::communityAnyoneCanAddPeers() const | done | `src/data/data_channel.rs::community_anyone_can_add_peers` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:441 | void ChannelData::setMembersCount(int newMembersCount) | done | `src/data/data_group_store.rs::set_channel_members_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:455 | void ChannelData::setAdminsCount(int newAdminsCount) | done | `src/data/data_group_store.rs::set_channel_admins_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:462 | void ChannelData::setRestrictedCount(int newRestrictedCount) | done | `src/data/data_group_store.rs::set_channel_restricted_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:469 | void ChannelData::setKickedCount(int newKickedCount) | done | `src/data/data_group_store.rs::set_channel_kicked_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:476 | void ChannelData::setPendingRequestsCount( int count, const QVector<MTPlong> &recentRequesters) | done | `src/data/data_group_store.rs::set_channel_pending_requests` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:486 | void ChannelData::setPendingRequestsCount( int count, std::vector<UserId> recentRequesters) | done | `src/data/data_group_store.rs::set_channel_pending_requests` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:497 | bool ChannelData::useSubsectionTabs() const | done | `src/data/data_channel.rs::use_subsection_tabs` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:502 | ChatRestrictionsInfo ChannelData::KickedRestrictedRights( not_null<PeerData*> participant) | done | `src/data/data_channel.rs::kicked_restricted_rights` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:523 | void ChannelData::applyEditAdmin( not_null<UserData*> user, ChatAdminRightsInfo oldRights, ChatAdminRightsInfo newRights, const QString &rank) | done | `src/data/data_group_store.rs::apply_channel_edit_admin` | edited_admin_original_double_counts_bans_and_stale_emplace |
| data/data_channel.cpp:587 | void ChannelData::applyEditMemberRank( not_null<UserData*> user, const QString &rank) | done | `src/data/data_group_store.rs::apply_channel_edit_member_rank` | edited_admin_original_double_counts_bans_and_stale_emplace |
| data/data_channel.cpp:598 | void ChannelData::applyEditBanned( not_null<PeerData*> participant, ChatRestrictionsInfo oldRights, ChatRestrictionsInfo newRights) | done | `src/data/data_group_store.rs::apply_channel_edit_banned` | edited_admin_original_double_counts_bans_and_stale_emplace |
| data/data_channel.cpp:668 | void ChannelData::setViewAsMessagesFlag(bool enabled) | done | `src/data/data_group_store.rs::set_channel_view_as_messages` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:676 | void ChannelData::markForbidden() | done | `src/data/data_group_store.rs::mark_channel_forbidden` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:687 | bool ChannelData::isGroupAdmin(not_null<UserData*> user) const | done | `src/data/data_channel.rs::is_group_admin` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:694 | bool ChannelData::lastParticipantsRequestNeeded() const | done | `src/data/data_channel.rs::last_participants_request_needed` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:708 | auto ChannelData::unavailableReasons() const -> const std::vector<Data::UnavailableReason> & | done | `src/data/data_channel.rs::unavailable_reasons` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:713 | void ChannelData::setUnavailableReasonsList( std::vector<Data::UnavailableReason> &&reasons) | done | `src/data/data_group_store.rs::set_channel_unavailable_reasons_list` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:718 | void ChannelData::setAvailableMinId(MsgId availableMinId) | done | `src/data/data_group_store.rs::set_channel_available_min_id` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:724 | bool ChannelData::canBanMembers() const | done | `src/data/data_channel.rs::can_ban_members` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:728 | bool ChannelData::canPostMessages() const | done | `src/data/data_channel.rs::can_post_messages` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:732 | bool ChannelData::canEditMessages() const | done | `src/data/data_channel.rs::can_edit_messages` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:736 | bool ChannelData::canDeleteMessages() const | done | `src/data/data_channel.rs::can_delete_messages` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:740 | bool ChannelData::canPostStories() const | done | `src/data/data_channel.rs::can_post_stories` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:744 | bool ChannelData::canEditStories() const | done | `src/data/data_channel.rs::can_edit_stories` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:751 | bool ChannelData::canDeleteStories() const | done | `src/data/data_channel.rs::can_delete_stories` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:755 | bool ChannelData::canAccessMonoforum() const | done | `src/data/data_channel.rs::can_access_monoforum` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:759 | bool ChannelData::canPostPaidMedia() const | done | `src/data/data_channel.rs::can_post_paid_media` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:763 | bool ChannelData::anyoneCanAddMembers() const | done | `src/data/data_channel.rs::anyone_can_add_members` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:767 | bool ChannelData::hiddenPreHistory() const | done | `src/data/data_channel.rs::hidden_pre_history` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:771 | bool ChannelData::canAddMembers() const | done | `src/data/data_channel.rs::can_add_members` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:779 | bool ChannelData::canAddAdmins() const | done | `src/data/data_channel.rs::can_add_admins` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:783 | bool ChannelData::isAyuNoForwards() const | done | `src/data/data_channel.rs::is_ayu_no_forwards` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:787 | bool ChannelData::allowsForwarding() const | done | `src/data/data_channel.rs::allows_forwarding` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:791 | bool ChannelData::canViewMembers() const | done | `src/data/data_channel.rs::can_view_members` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:798 | bool ChannelData::canViewAdmins() const | done | `src/data/data_channel.rs::can_view_admins` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:802 | bool ChannelData::canViewBanned() const | done | `src/data/data_channel.rs::can_view_banned` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:806 | bool ChannelData::canEditInformation() const | done | `src/data/data_channel.rs::can_edit_information` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:812 | bool ChannelData::canEditPermissions() const | done | `src/data/data_channel.rs::can_edit_permissions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:818 | bool ChannelData::canEditSignatures() const | done | `src/data/data_channel.rs::can_edit_signatures` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:822 | bool ChannelData::canEditAutoTranslate() const | done | `src/data/data_channel.rs::can_edit_auto_translate` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:826 | bool ChannelData::canEditPreHistoryHidden() const | done | `src/data/data_channel.rs::can_edit_pre_history_hidden` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:832 | bool ChannelData::canEditUsername() const | done | `src/data/data_channel.rs::can_edit_username` | names_zalgo_usernames_source_update_order |
| data/data_channel.cpp:837 | bool ChannelData::canEditStickers() const | done | `src/data/data_channel.rs::can_edit_stickers` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:841 | bool ChannelData::canEditEmoji() const | done | `src/data/data_channel.rs::can_edit_emoji` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:845 | bool ChannelData::canDelete() const | done | `src/data/data_channel.rs::can_delete` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:849 | bool ChannelData::canEditLastAdmin(not_null<UserData*> user) const | done | `src/data/data_channel.rs::can_edit_last_admin` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:861 | bool ChannelData::canEditAdmin(not_null<UserData*> user) const | done | `src/data/data_channel.rs::can_edit_admin` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:873 | bool ChannelData::canRestrictParticipant( not_null<PeerData*> participant) const | done | `src/data/data_channel.rs::can_restrict_participant` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:888 | void ChannelData::setBotVerifyDetails(Ui::BotVerifyDetails details) | partial(5) | Ui::BotVerifyDetails | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:903 | void ChannelData::setBotVerifyDetailsIcon(DocumentId iconId) | partial(5) | Ui::BotVerifyDetails | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:915 | void ChannelData::setAdminRights(ChatAdminRights rights) | done | `src/data/data_group_store.rs::set_channel_admin_rights` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:947 | void ChannelData::setRestrictions(ChatRestrictionsInfo rights) | done | `src/data/data_group_store.rs::set_channel_restrictions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:971 | void ChannelData::setDefaultRestrictions(ChatRestrictions rights) | done | `src/data/data_group_store.rs::set_channel_default_restrictions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:979 | ChatData *ChannelData::getMigrateFromChat() const | done | `src/data/data_channel.rs::get_migrate_from_chat` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:986 | void ChannelData::setMigrateFromChat(ChatData *chat) | done | `src/data/data_group_store.rs::set_channel_migrate_from_chat` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:998 | int ChannelData::slowmodeSeconds() const | done | `src/data/data_channel.rs::slowmode_seconds` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1005 | void ChannelData::setSlowmodeSeconds(int seconds) | done | `src/data/data_group_store.rs::set_channel_slowmode_seconds` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1013 | TimeId ChannelData::slowmodeLastMessage() const | done | `src/data/data_channel.rs::slowmode_last_message` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1022 | void ChannelData::growSlowmodeLastMessage(TimeId when) | done | `src/data/data_group_store.rs::grow_channel_slowmode_last_message` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1038 | int ChannelData::starsPerMessage() const | done | `src/data/data_channel.rs::stars_per_message` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1042 | int ChannelData::commonStarsPerMessage() const | partial(13/23) | `src/data/data_channel.rs::common_stars_per_message` — required actual common-star provider | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1046 | void ChannelData::setStarsPerMessage(int stars) | done | `src/data/data_group_store.rs::set_channel_stars_per_message` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1054 | int ChannelData::peerGiftsCount() const | done | `src/data/data_channel.rs::peer_gifts_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1058 | void ChannelData::setPeerGiftsCount(int count) | done | `src/data/data_group_store.rs::set_channel_peer_gifts_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1065 | int ChannelData::boostsApplied() const | done | `src/data/data_channel.rs::boosts_applied` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1072 | int ChannelData::boostsUnrestrict() const | done | `src/data/data_channel.rs::boosts_unrestrict` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1079 | bool ChannelData::unrestrictedByBoosts() const | done | `src/data/data_channel.rs::unrestricted_by_boosts` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1087 | rpl::producer<bool> ChannelData::unrestrictedByBoostsValue() const | done | `src/data/data_channel.rs::unrestricted_by_boosts_value` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1094 | void ChannelData::setBoostsUnrestrict(int applied, int unrestrict) | done | `src/data/data_group_store.rs::set_channel_boosts_unrestrict` | slowmode_clock_correction_and_boosts_reentrant_delivery |
| data/data_channel.cpp:1113 | void ChannelData::setInvitePeek(const QString &hash, TimeId expires) | done | `src/data/data_group_store.rs::set_channel_invite_peek` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1121 | void ChannelData::clearInvitePeek() | done | `src/data/data_group_store.rs::clear_channel_invite_peek` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1125 | TimeId ChannelData::invitePeekExpires() const | done | `src/data/data_channel.rs::invite_peek_expires` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1129 | QString ChannelData::invitePeekHash() const | done | `src/data/data_channel.rs::invite_peek_hash` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1133 | void ChannelData::privateErrorReceived() | done | `src/data/data_group_store.rs::channel_private_error_received` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1147 | void ChannelData::migrateCall(std::unique_ptr<Data::GroupCall> call) | partial(08) | move key + actual GroupCall setPeer command | Отложенная часть не исполнялась; boundary: group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_channel.cpp:1157 | void ChannelData::setGroupCall( const MTPInputGroupCall &call, TimeId scheduleDate, bool rtmp) | partial(08) | `src/data/data_group_store.rs::set_group_call` — keys/registration commands only | Отложенная часть не исполнялась; boundary: group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_channel.cpp:1189 | void ChannelData::clearGroupCall() | partial(08) | `src/data/data_group_store.rs::clear_group_call` — keys/unregistration/migration commands only | Отложенная часть не исполнялась; boundary: group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_channel.cpp:1199 | void ChannelData::setGroupCallDefaultJoinAs(PeerId peerId) | done | `src/data/data_group_store.rs::set_group_call_join_as` | group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_channel.cpp:1203 | PeerId ChannelData::groupCallDefaultJoinAs() const | done | `src/data/data_channel.rs::group_call_default_join_as` | group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_channel.cpp:1207 | void ChannelData::setAllowedReactions(Data::AllowedReactions value) | done | `src/data/data_group_store.rs::set_channel_allowed_reactions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1227 | const Data::AllowedReactions &ChannelData::allowedReactions() const | done | `src/data/data_channel.rs::allowed_reactions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1231 | bool ChannelData::hasActiveStories() const | done | `src/data/data_channel.rs::has_active_stories` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:1235 | bool ChannelData::hasUnreadStories() const | done | `src/data/data_channel.rs::has_unread_stories` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:1239 | bool ChannelData::hasActiveVideoStream() const | done | `src/data/data_channel.rs::has_active_video_stream` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:1243 | void ChannelData::setStoriesState(StoriesState state) | done | `src/data/data_group_store.rs::set_channel_stories_state` | photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_channel.cpp:1277 | int ChannelData::levelHint() const | done | `src/data/data_channel.rs::level_hint` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1281 | void ChannelData::updateLevelHint(int levelHint) | done | `src/data/data_group_store.rs::update_channel_level_hint` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1285 | TimeId ChannelData::subscriptionUntilDate() const | done | `src/data/data_channel.rs::subscription_until_date` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1289 | UserData *ChannelData::guardBot() const | done | `src/data/data_channel.rs::guard_bot` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1295 | void ChannelData::setGuardBotId(UserId userId) | done | `src/data/data_group_store.rs::set_channel_guard_bot_id` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1299 | void ChannelData::updateSubscriptionUntilDate(TimeId subscriptionUntilDate) | done | `src/data/data_group_store.rs::update_channel_subscription_until` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1303 | MTPInputChannel ChannelData::inputChannel() const | done | `src/data/data_channel.rs::input_channel` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.cpp:1318 | void ApplyMigration( not_null<ChatData*> chat, not_null<ChannelData*> channel) | done | `src/data/data_group_store.rs::apply_migration` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.cpp:1327 | void ApplyChannelUpdate( not_null<ChannelData*> channel, const MTPDupdateChatDefaultBannedRights &update) | done | `src/data/data_channel_updates.rs::apply_channel_default_banned_rights` — source does not perform chat-style version check | tl_full_updates_apply_scalar_state_and_preserve_stale_star_flag |
| data/data_channel.cpp:1334 | void ApplyChannelUpdate( not_null<ChannelData*> channel, const MTPDchannelFull &update) | partial(03/07/08/09/12/13/16/18/23/29/5) | `src/data/data_channel_updates.rs::apply_channel_full` — full owner effect bindings where applicable | Отложенная часть не исполнялась; boundary: tl_full_updates_apply_scalar_state_and_preserve_stale_star_flag |
| data/data_channel.cpp:1615 | void ApplyCommunityUpdate( not_null<ChannelData*> channel, const MTPDcommunityFull &update) | partial(03/07) | `src/data/data_channel_updates.rs::apply_community_full` — photo/community owner effect bindings | Отложенная часть не исполнялась; boundary: tl_full_updates_apply_scalar_state_and_preserve_stale_star_flag |
| data/data_channel.h:29 | friend inline bool operator==( const ChannelLocation &a, const ChannelLocation &b) | done | derived / explicit value equality | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:36 | friend inline bool operator!=( const ChannelLocation &a, const ChannelLocation &b) | done | derived / explicit value equality | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:131 | const Data::ChatBotCommands &botCommands() const | done | `src/data/data_channel.rs::bot_commands` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:203 | uint64 accessHash() const | done | `src/data/data_channel.rs::access_hash` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:211 | auto flags() const | done | `src/data/data_channel.rs::flags` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_channel.h:214 | auto flagsValue() const | done | `src/data/data_channel.rs::flags_value` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_channel.h:223 | int membersCount() const | done | `src/data/data_channel.rs::members_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:227 | bool membersCountKnown() const | done | `src/data/data_channel.rs::members_count_known` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:231 | int adminsCount() const | done | `src/data/data_channel.rs::admins_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:236 | int restrictedCount() const | done | `src/data/data_channel.rs::restricted_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:241 | int kickedCount() const | done | `src/data/data_channel.rs::kicked_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:246 | int pendingRequestsCount() const | done | `src/data/data_channel.rs::pending_requests_count` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:249 | const std::vector<UserId> &recentRequesters() const | done | `src/data/data_channel.rs::recent_requesters` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:259 | bool haveLeft() const | done | `src/data/data_channel.rs::have_left` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:262 | bool amIn() const | done | `src/data/data_channel.rs::am_in` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:265 | bool addsSignature() const | done | `src/data/data_channel.rs::adds_signature` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:268 | bool signatureProfiles() const | done | `src/data/data_channel.rs::signature_profiles` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:271 | bool isForbidden() const | done | `src/data/data_channel.rs::is_forbidden` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:274 | bool isVerified() const | done | `src/data/data_channel.rs::is_verified` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:277 | bool isScam() const | done | `src/data/data_channel.rs::is_scam` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:280 | bool isFake() const | done | `src/data/data_channel.rs::is_fake` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:283 | bool hasStoriesHidden() const | done | `src/data/data_channel.rs::has_stories_hidden` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:286 | bool viewForumAsMessages() const | done | `src/data/data_channel.rs::view_forum_as_messages` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.h:289 | bool stargiftsAvailable() const | done | `src/data/data_channel.rs::stargifts_available` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:292 | bool paidMessagesAvailable() const | done | `src/data/data_channel.rs::paid_messages_available` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:295 | bool hasStarsPerMessage() const | done | `src/data/data_channel.rs::has_stars_per_message` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:298 | bool starsPerMessageKnown() const | done | `src/data/data_channel.rs::stars_per_message_known` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:301 | bool hasWelcomeMessages() const | done | `src/data/data_channel.rs::has_welcome_messages` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:309 | static bool IsRestrictedForever(TimeId until) | done | `src/data/data_channel.rs::is_restricted_forever` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:330 | bool isMegagroup() const | done | `src/data/data_channel.rs::is_megagroup` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:333 | bool isBroadcast() const | done | `src/data/data_channel.rs::is_broadcast` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:336 | bool isGigagroup() const | done | `src/data/data_channel.rs::is_gigagroup` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:339 | bool isForum() const | done | `src/data/data_channel.rs::is_forum` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.h:342 | bool isMonoforum() const | done | `src/data/data_channel.rs::is_monoforum` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel.h:345 | bool isCommunity() const | done | `src/data/data_channel.rs::is_community` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:348 | bool collapsedInDialogs() const | done | `src/data/data_channel.rs::collapsed_in_dialogs` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:351 | bool hasUsername() const | done | `src/data/data_channel.rs::has_username` | names_zalgo_usernames_source_update_order |
| data/data_channel.h:354 | bool hasLocation() const | done | `src/data/data_channel.rs::has_location` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:357 | bool isPublic() const | done | `src/data/data_channel.rs::is_public` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:360 | bool amCreator() const | done | `src/data/data_channel.rs::am_creator` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:363 | bool joinToWrite() const | done | `src/data/data_channel.rs::join_to_write` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:366 | bool requestToJoin() const | done | `src/data/data_channel.rs::request_to_join` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:370 | UserId guardBotId() const | done | `src/data/data_channel.rs::guard_bot_id` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:374 | bool antiSpamMode() const | done | `src/data/data_channel.rs::anti_spam_mode` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:377 | bool autoTranslation() const | done | `src/data/data_channel.rs::auto_translation` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:380 | auto adminRights() const | done | `src/data/data_channel.rs::admin_rights` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:383 | auto adminRightsValue() const | done | `src/data/data_channel.rs::admin_rights_value` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:387 | bool hasAdminRights() const | done | `src/data/data_channel.rs::has_admin_rights` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:391 | auto restrictions() const | done | `src/data/data_channel.rs::restrictions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:394 | auto restrictionsValue() const | done | `src/data/data_channel.rs::restrictions_value` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:397 | TimeId restrictedUntil() const | done | `src/data/data_channel.rs::restricted_until` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:401 | bool hasRestrictions() const | done | `src/data/data_channel.rs::has_restrictions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:404 | bool hasRestrictions(TimeId now) const | done | `src/data/data_channel.rs::has_restrictions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:409 | auto defaultRestrictions() const | done | `src/data/data_channel.rs::default_restrictions` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:412 | auto defaultRestrictionsValue() const | done | `src/data/data_channel.rs::default_restrictions_value` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:452 | Ui::BotVerifyDetails *botVerifyDetails() const | partial(5) | Ui::BotVerifyDetails | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:457 | QString inviteLink() const | done | `src/data/data_channel.rs::invite_link` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:476 | Data::CommunityInfo *communityInfo() const | partial(07) | `src/data/data_channel.rs::community_info` — CommunityInfo owner key only | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:483 | void ptsInit(int32 pts) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:486 | void ptsReceived(int32 pts) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:489 | bool ptsUpdateAndApply(int32 pts, int32 count) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:492 | bool ptsUpdateAndApply( int32 pts, int32 count, const MTPUpdate &update) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:498 | bool ptsUpdateAndApply( int32 pts, int32 count, const MTPUpdates &updates) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:504 | int32 pts() const | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:507 | bool ptsInited() const | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:510 | bool ptsRequesting() const | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:513 | void ptsSetRequesting(bool isRequesting) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:516 | // < 0 - not waiting void ptsSetWaitingForShortPoll(int32 ms) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:517 | void ptsSetWaitingForShortPoll(int32 ms) | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:520 | bool ptsWaitingForSkipped() const | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:523 | bool ptsWaitingForShortPoll() const | partial(12) | actual PtsWaiter, no fabricated PTS state | Отложенная часть не исполнялась; boundary: channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:527 | MsgId availableMinId() const | done | `src/data/data_channel.rs::available_min_id` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:559 | Data::GroupCall *groupCall() const | done | `src/data/data_channel.rs::group_call` | group_call_keys_migrate_and_clear_without_groupcall_standin |
| data/data_channel.h:579 | Data::Forum *forum() const | done | `src/data/data_channel.rs::forum` | channel_rights_public_boosts_hidden_members_and_subtype_projections |
| data/data_channel.h:582 | Data::SavedMessages *monoforum() const | done | `src/data/data_channel.rs::monoforum` | flags_forum_monoforum_membership_migration_and_retained_links |
| data/data_channel_admins.cpp:18 | ChannelAdminChanges::ChannelAdminChanges(not_null<ChannelData*> channel) : _channel(channel) , _admins(_channel->mgInfo->admins) , _oldCreator(_channel->mgInfo->creator) | done | `src/data/data_channel_admins.rs::new` | admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self |
| data/data_channel_admins.cpp:24 | void ChannelAdminChanges::add(UserId userId, const QString &rank) | done | `src/data/data_channel_admins.rs::add` | admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self |
| data/data_channel_admins.cpp:42 | void ChannelAdminChanges::remove(UserId userId) | done | `src/data/data_channel_admins.rs::remove` | admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self |
| data/data_channel_admins.cpp:48 | ChannelAdminChanges::~ChannelAdminChanges() | done | `src/data/data_channel_admins.rs::finish` | admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self |
| data/data_channel_admins.cpp:67 | ChannelMemberRankChanges::ChannelMemberRankChanges( not_null<ChannelData*> channel) : _channel(channel) , _memberRanks(_channel->mgInfo->memberRanks) | done | `src/data/data_channel_admins.rs::new` | admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self |
| data/data_channel_admins.cpp:73 | void ChannelMemberRankChanges::feed( UserId userId, const QString &rank) | done | `src/data/data_channel_admins.rs::feed` | admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self |
| data/data_channel_admins.cpp:89 | ChannelMemberRankChanges::~ChannelMemberRankChanges() | done | `src/data/data_channel_admins.rs::finish` | admin_and_rank_batches_keep_ranks_on_remove_and_suppress_self |
| data/data_chat_participant_status.cpp:35 | ChatAdminRights ChatAdminRightsFlags( const MTPChatAdminRights &rights) | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:69 | ChatRestrictions ChatBannedRightsFlags( const MTPChatBannedRights &rights) | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:99 | TimeId ChatBannedRightsUntilDate( const MTPChatBannedRights &rights) | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:108 | ChatAdminRightsInfo::ChatAdminRightsInfo(const MTPChatAdminRights &rights) : flags(ChatAdminRightsFlags(rights)) | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:112 | MTPChatAdminRights AdminRightsToMTP(ChatAdminRightsInfo info) | done | `src/data/data_chat_participant_status.rs::admin_rights_to_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:146 | ChatRestrictionsInfo::ChatRestrictionsInfo(const MTPChatBannedRights &rights) : flags(ChatBannedRightsFlags(rights)) , until(ChatBannedRightsUntilDate(rights)) | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:151 | MTPChatBannedRights RestrictionsToMTP(ChatRestrictionsInfo info) | done | `src/data/data_chat_participant_status.rs::restrictions_to_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:185 | std::vector<ChatRestrictions> ListOfRestrictions( RestrictionsSetOptions options) | done | `src/data/data_chat_participant_status.rs::list_of_restrictions` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:193 | ChatRestrictions AllSendRestrictions() | done | `src/data/data_chat_participant_status.rs::all_send_restrictions` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:204 | ChatRestrictions FilesSendRestrictions() | done | `src/data/data_chat_participant_status.rs::files_send_restrictions` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:215 | ChatRestrictions TabbedPanelSendRestrictions() | done | `src/data/data_chat_participant_status.rs::tabbed_panel_send_restrictions` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.cpp:227 | bool CanSendAnyOf( not_null<const Thread*> thread, ChatRestrictions rights, bool forbidInForums) | partial(16) | `src/data/data_chat_participant_status.rs::can_send_any_of` — actual Thread/topic closed-state owner | Отложенная часть не исполнялась; boundary: can_send_any_threadless_groups_freeze_and_zero_mask |
| data/data_chat_participant_status.cpp:238 | bool CanSendAnyOf( not_null<const PeerData*> peer, ChatRestrictions rights, bool forbidInForums) | done | `src/data/data_chat_participant_status.rs::can_send_any_of` | can_send_any_threadless_groups_freeze_and_zero_mask |
| data/data_chat_participant_status.cpp:303 | SendError RestrictionError( not_null<PeerData*> peer, ChatRestriction restriction) | partial(01/5) | localized text/premium/boost/date labels | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.cpp:468 | SendError AnyFileRestrictionError(not_null<PeerData*> peer) | partial(01/5) | localized errors | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.cpp:478 | SendError FileRestrictionError( not_null<PeerData*> peer, const Ui::PreparedList &list, std::optional<bool> compress) | partial(01/5) | real Ui::PreparedFile/List | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.cpp:502 | SendError FileRestrictionError( not_null<PeerData*> peer, const Ui::PreparedFile &file, std::optional<bool> compress) | partial(01/5) | real Ui::PreparedFile/List | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.cpp:546 | void ShowSendErrorToast( not_null<Window::SessionNavigation*> navigation, not_null<PeerData*> peer, Data::SendError error) | partial(6) | real Window/ChatHelpers toast/navigation | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.cpp:553 | void ShowSendErrorToast( std::shared_ptr<ChatHelpers::Show> show, not_null<PeerData*> peer, Data::SendError error) | partial(6) | real Window/ChatHelpers toast/navigation | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.cpp:571 | bool ShowSendError( std::shared_ptr<ChatHelpers::Show> show, not_null<PeerData*> peer, const Ui::PreparedList &list, std::optional<bool> compress, bool ignoreSlowmodeLeft, bool ignoreRestrictions) | partial(01/5/6) | prepared bundles/limits/localized UI | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.cpp:623 | bool ShowSendError( std::shared_ptr<ChatHelpers::Show> show, not_null<PeerData*> peer, const Ui::PreparedBundle &bundle, bool ignoreSlowmodeLeft, bool ignoreRestrictions) | partial(01/5/6) | prepared bundles/limits/localized UI | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.h:81 | ChatAdminRightsInfo() = default; | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.h:82 | explicit ChatAdminRightsInfo(ChatAdminRights flags) : flags(flags) | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.h:92 | ChatRestrictionsInfo() = default; | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.h:93 | ChatRestrictionsInfo(ChatRestrictions flags, TimeId until) : flags(flags) , until(until) | done | `src/data/data_chat_participant_status.rs::from_mtp` | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.h:207 | SendError(QString text = QString()) : text(std::move(text)) | done | `src/data/data_chat_participant_status.rs::new` | scalar_wrappers_predicate_matrix_locations_and_pending_fields |
| data/data_chat_participant_status.h:217 | SendError(Args &&args) : text(std::move(args.text)) , boostsToLift(args.boostsToLift) , monoforumAdmin(args.monoforumAdmin) , premiumToLift(args.premiumToLift) , frozen(args.frozen) | done | `src/data/data_chat_participant_status.rs::from_args` | scalar_wrappers_predicate_matrix_locations_and_pending_fields |
| data/data_chat_participant_status.h:231 | SendError value_or(SendError other) const | done | `src/data/data_chat_participant_status.rs::value_or` | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.h:235 | explicit operator bool() const | done | `src/data/data_chat_participant_status.rs::is_present` | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.h:238 | bool has_value() const | done | `src/data/data_chat_participant_status.rs::has_value` | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_chat_participant_status.h:241 | const QString &operator*() const | done | `src/data/data_chat_participant_status.rs::deref` | scalar_wrappers_predicate_matrix_locations_and_pending_fields |
| data/data_channel.h:115 | MegagroupInfo::Admin/Restricted constructors and defaults | done | data_channel.rs::Admin/Restricted/MegagroupInfo::default | edited_admin_original_double_counts_bans_and_stale_emplace |
| data/data_channel.h:25 | ChannelLocation equality/inequality, empty address ignores point | done | data_channel.rs::ChannelLocation::eq | scalar_wrappers_predicate_matrix_locations_and_pending_fields |
| data/data_chat_participant_status.h:26 | Admin/Restriction enum, flags operators, options and defaults | done | data_chat_participant_status.rs | native_rights_flags_normalization_zalgo_and_forever |
| data/data_chat_participant_status.h:248 | SendErrorWithThread actual Thread identity binding | partial(16) | Thread model not fabricated | Отложенная часть не исполнялась; boundary: can_send_any_threadless_groups_freeze_and_zero_mask |
| data/data_session.cpp:594 | Peers::ensure/read/edit concrete Chat/Channel records + existing UserBase common slice | partial(02/13/14) | data_session.rs::PeerRecord / data_group_store.rs::ensure_chat/ensure_channel; full UserData and Session remain | Отложенная часть не исполнялась; boundary: real_records_defaults_scope_and_common_base |
| data/data_peer.cpp:1964 | Theme mutation with required real emoji/theme facts; no default text service | partial(5/29) | data_group_store.rs::apply_group_theme / GroupContext::theme_facts | Отложенная часть не исполнялась; boundary: tl_full_updates_apply_scalar_state_and_preserve_stale_star_flag |
| ayu/utils/telegram_helpers.cpp:1269 | filterZalgo whole helper/pattern; other file functions excluded | partial(35) | ayu/utils/telegram_helpers.rs::filter_zalgo/is_mn; rest of file35 | Отложенная часть не исполнялась; boundary: native_rights_flags_normalization_zalgo_and_forever |
| data/data_peer_bot_command.h:14 | BotCommand fields/equality | done | data_peer_bot_command.rs::BotCommand | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_peer_bot_command.cpp:14 | BotCommandFromTL | done | data_peer_bot_command.rs::bot_command_from_tl | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_peer_bot_commands.h:14 | BotStatus/BotCommands/ChatBotCommands real types | done | data_peer_bot_commands.rs | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_peer_bot_commands.cpp:12 | ChatBotCommands::update | done | data_peer_bot_commands.rs::update | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_peer_bot_commands.cpp:37 | BotCommandsFromTL | done | data_peer_bot_commands.rs::bot_commands_from_tl | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_message_reaction_id.h:26 | ReactionId PaidTag/Paid/empty/paid/emoji/custom/bool/default/UTF16 ordering | partial(09) | data_message_reaction_id.rs real value contract; remaining query/entity helpers09 | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_message_reaction_id.cpp:79 | ReactionFromMTP/ReactionToMTP; remaining bodies | partial(09) | data_message_reaction_id.rs::reaction_from_mtp/reaction_to_mtp; query/entity helpers09 | Отложенная часть не исполнялась; boundary: reaction_and_bot_command_values_and_source_partial_updates |
| data/data_peer.cpp:159 | AllowedReactions fields/equality/Parse, no reaction service substitute | done | data_peer.rs::AllowedReactions/parse_allowed_reactions | reaction_and_bot_command_values_and_source_partial_updates |
| data/data_types.h:244 | StickerSetIdentifier fields/empty/bool; remaining data_types | partial(16) | data_types.rs::StickerSetIdentifier; remaining types16 | Отложенная часть не исполнялась; boundary: scalar_wrappers_predicate_matrix_locations_and_pending_fields |
| data/data_location.h:15 | LocationPoint data/from GeoPoint and equality; remaining methods | partial(09/5) | data_location.rs::LocationPoint; formatting/hash/ComputeLocation09/5 | Отложенная часть не исполнялась; boundary: scalar_wrappers_predicate_matrix_locations_and_pending_fields |
| data/data_location.cpp:28 | LocationPoint::LocationPoint(MTPDgeoPoint) fields; remaining bodies | partial(09/5) | data_location.rs::from_mtp; formatting/hash/ComputeLocation09/5 | Отложенная часть не исполнялась; boundary: photo_stories_location_and_metadata_commands_use_real_record_state |
| data/data_changes.cpp:37 | Inherited x86 realtime-shift quirk: Members/Rights high bits need postponed stream | done | existing data_changes.rs unchanged; new model tests use postponed flush | slowmode_clock_correction_and_boosts_reentrant_delivery |
