# PeerData: модель, владельцы и границы

Оригинал `0f2a31bb420de98f331990432265b131b8d836b5`, `data_peer.h/.cpp` прочитаны
полностью, как и flags/common/lastseen/birthday/UsernamesInfo/PeerNotifySettings.
Реализация находится в `src/data/`, текстовые helper-таблицы — `src/core/utils.rs`.
Имена тестов ниже — функции [data_peer.rs](../tests/data_peer.rs).
[Native proof и воспроизведение](../tests/reference-tools/data-peer/README.md).

Каждая строка — отдельное исходное тело/inline/overload либо явно названный
вспомогательный контракт. `.cpp` строка покрывает также его declaration в `.h`;
const/non-const overloads показаны отдельно. Default/destructors/equality выражены
Rust defaults/ownership/derives. Запрещённые copy/assignment не реализованы.
`done` в строке означает перенос функции/чистой фазы; статус файла в ledger
остаётся partial, когда отсутствует UI/owner binding. Boundary-тест не объявляет
отложенное тело протестированным. Числа ниже — строки карты, не процент LOC.

## Владение и эффекты

Peers — компонент будущего Data::Session, один BTreeMap общих PeerData, ключи
PeerKey(scope,PeerId). Нет фиктивной Session, History или owned enum с пустыми
User/Chat/Channel: concrete variants отложены до **02/03**. Borrowed PeerFields
содержат реальные поля/результаты методов заголовков, без Default; будущие модели
предоставляют их сами. Forum/Monoforum представлены ключом владельца, их методы
принимают обязательный lookup настоящего store. GroupCallKey — scope+CallId,
только извлечённый ID-контракт (модель **08**). Main::Session/Account связи **29**.

Mutable borrow завершается перед Changes/Flags fire. Методы возвращают
PeerMutation с упорядоченными эффектами, Peers dispatch испускает Changes сразу,
остальные команды отдаёт реальному будущему владельцу; API/UI/storage обработчики
не подменены. NameFields вызывает настоящие subtype setters вне общего borrow;
reentrant callback может изменить имя до fillNames, индекс строится по актуальному
имени. Обязательный NameTextServices не имеет fallback backend: production
RemoveAccents/PrepareSearchWords остаются **5**, lang labels передаёт **01**.
ChatData::setName Ayu Zalgo-фильтр/настройки — **03**, общий set_name получает уже
отфильтрованное значение и сохраняет исходное правило empty→old name.

## Измеренные особенности и непроверенное

- Native pure.csv: реальные C++ Birthday/Lastseen/Flags/UsernamesInfo, Qt text
  helpers, CRC/translit/keyboard, регистрация, ограничения, MTP color/profile,
  enum bits и ImageLocation equality. Session-состояние и порядок эффектов:
  **own tests + чтение**, без подмены тел владельцев.
- BirthdayAge для 29.02.2000 на 01.01.2023 даёт 23: invalid QDate anniversary
  сортируется раньше valid date. Сохранено.
- UsernamesInfo append после удаления editable ставит index 0; inactive editable
  может оставить index вне вектора. Индекс сохранён, Rust getter в последнем
  случае panic вместо неопределённого чтения C++; native UB не запускается.
- Lastseen signed/unsigned promotions и strict serialized validation сохранены;
  max OnlineTill не проходит обратную strict validation. Signed-overflow края
  C++ проверены только на GCC15.3 x86_64 -O2, перенос фиксирует wrapping там, где
  исходник выходит за defined domain; иные компиляторы не доказаны.
- slowmodeAppliedValue не проверяет Creator (в отличие от scalar); повторные
  false из combine остаются. NameVersion u16 оборачивается, вновь подавляя flags.
- Ayu allowsForwarding(user)=true и UnavailableReason Compute/Extract empty,
  IgnoreSensitiveMark=true — настоящие тела pinned fork, не заглушки порта.
- EmojiStatus collectible equality — pointer identity; ImageLocation equality
  игнорирует размеры/access hash/file reference. Qt case folding spambot проверен
  также для long-s ſ. Изменения ID media из прошлой задачи остаются gate **03/16**.
- UTF-16 words/first QChar сохраняются; входные имена Rust UTF-8 не выражают lone
  surrogate QString. Полная Unicode text production binding и UI не проверены.
- Sponsored/boost fields принадлежат ChannelData, не common PeerData; здесь
  используются только действительные ограничения/boost projections. Сами поля
  и обработчики **03**, полная пользовательская модель не объявлена готовой.

## Функции и контракты

Всего **357** строк: **300 done**, **57 partial** с указанными пакетами; 0 пустых реализаций/тестовых ссылок.

| Источник | Функция / контракт | Статус | Rust реализация / граница | Тест |
|---|---|---|---|---|
| data/data_peer.cpp:69 | `[[nodiscard]] const std::vector<QString> &IgnoredReasons( not_null<Main::Session*> session)` | partial(29) | unused by fork Compute; actual AppConfig getter binding deferred | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:74 | `[[nodiscard]] int ParseRegistrationDate(const QString &text)` | done | `src/data/data_peer.rs:707` | native_pure_functions_reference |
| data/data_peer.cpp:86 | `[[nodiscard]] int RegistrationYear(int date)` | done | `src/data/data_peer.rs:571`; `src/data/data_peer.rs:726` | native_pure_functions_reference |
| data/data_peer.cpp:91 | `[[nodiscard]] int RegistrationMonth(int date)` | done | `src/data/data_peer.rs:566`; `src/data/data_peer.rs:734` | native_pure_functions_reference |
| data/data_peer.cpp:100 | `uint8 DecideColorIndex(PeerId peerId)` | done | `src/data/data_peer.rs:695` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:104 | `PeerId FakePeerIdForJustName(const QString &name)` | done | `src/data/data_peer.rs:698` | native_pure_functions_reference |
| data/data_peer.cpp:112 | `bool UnavailableReason::sensitive() const` | done | `src/data/data_peer.rs:114` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:116 | `UnavailableReason UnavailableReason::Sensitive()` | done | `src/data/data_peer.rs:117` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:120 | `QString UnavailableReason::Compute( not_null<Main::Session*> session, const std::vector<UnavailableReason> &list)` | done | `src/data/data_peer.rs:124` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:126 | `bool UnavailableReason::IgnoreSensitiveMark( not_null<Main::Session*> session)` | done | `src/data/data_peer.rs:127` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:133 | `std::vector<UnavailableReason> UnavailableReason::Extract( const MTPvector<MTPRestrictionReason> *list)` | done | `src/data/data_peer.rs:130` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:138 | `bool ApplyBotMenuButton( not_null<BotInfo*> info, const MTPBotMenuButton *button)` | done | `src/data/data_peer.rs:1649` | color_profile_tabs_and_bot_button_value_helpers |
| data/data_peer.cpp:159 | `AllowedReactions Parse( const MTPChatReactions &value, int maxCount, bool paidEnabled)` | partial(09) | `src/data/notify/data_peer_notify_settings.rs:81` — AllowedReactions/ReactionId parsing, no ReactionId model yet | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:190 | `PeerData *PeerFromInputMTP( not_null<Session*> owner, const MTPInputPeer &input)` | partial(02/03/13) | constructors/access-hash state of actual subclasses | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:210 | `UserData *UserFromInputMTP( not_null<Session*> owner, const MTPInputUser &input)` | partial(02/13) | actual UserData input constructors | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:224 | `Ui::ColorCollectible ParseColorCollectible( const MTPDpeerColorCollectible &data)` | partial(5) | Ui::ColorCollectible/QColor/render data | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:250 | `PeerClickHandler::PeerClickHandler(not_null<PeerData*> peer) : _peer(peer)` | partial(6) | real ClickHandler/navigation | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:255 | `void PeerClickHandler::onClick(ClickContext context) const` | partial(6) | real click/navigation | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:278 | `PeerData::PeerData(not_null<Data::Session*> owner, PeerId id) : id(id) , _owner(owner) , _colorIndex(Data::DecideColorIndex(id))` | done | `src/data/data_peer.rs:205`; `src/data/data_session.rs:56` | peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.cpp:284 | `Data::Session &PeerData::owner() const` | partial(13/14/29) | `src/data/data_peer.rs:259` — SessionScope identity implemented; real owner/lifecycle binding deferred | Не выполнена отложенная часть; boundary: peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.cpp:288 | `Main::Session &PeerData::session() const` | partial(13/14/29) | `src/data/data_peer.rs:255` — SessionScope identity implemented; real owner/lifecycle binding deferred | Не выполнена отложенная часть; boundary: peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.cpp:292 | `Main::Account &PeerData::account() const` | partial(29/6) | real Main::Account lifecycle | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:296 | `void PeerData::updateNameDelayed( const QString &newName, const QString &newNameOrPhone, const QString &newUsername)` | done | `src/data/data_session.rs:139` | chat_name_old_letters_first_update_suppression_and_wrap16 |
| data/data_peer.cpp:350 | `not_null<Ui::EmptyUserpic*> PeerData::ensureEmptyUserpic() const` | partial(5) | Ui::EmptyUserpic/painting/cache owner | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:362 | `void PeerData::invalidateEmptyUserpic()` | done | `src/data/data_peer.rs:1119` | chat_name_old_letters_first_update_suppression_and_wrap16 |
| data/data_peer.cpp:366 | `void PeerData::checkTrustedPayForMessage()` | done | `src/data/data_peer.rs:1103` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:377 | `ClickHandlerPtr PeerData::createOpenLink()` | partial(6) | ClickHandler | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:381 | `void PeerData::setUserpic( PhotoId photoId, const ImageLocation &location, bool hasVideo)` | done | `src/data/data_peer.rs:633` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:390 | `void PeerData::setUserpicPhoto(const MTPPhoto &data)` | partial(03) | `src/data/data_peer.rs:1511` — processed PhotoKey/id stage implemented; owner.processPhoto awaits actual media model | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:404 | `QImage *PeerData::userpicCloudImage(Ui::PeerUserpicView &view) const` | partial(5) | CloudImage/QImage cache | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:426 | `void PeerData::paintUserpic( QPainter &p, Ui::PeerUserpicView &view, PaintUserpicContext context) const` | partial(5) | painting | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:452 | `void PeerData::loadUserpic()` | partial(5) | image loader/cache | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:456 | `bool PeerData::hasUserpic() const` | done | `src/data/data_peer.rs:617` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:460 | `Ui::PeerUserpicView PeerData::activeUserpicView()` | partial(5) | Ui::PeerUserpicView | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:464 | `Ui::PeerUserpicView PeerData::createUserpicView()` | partial(5) | Ui::PeerUserpicView | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:473 | `bool PeerData::useEmptyUserpic(Ui::PeerUserpicView &view) const` | partial(5) | UI image view | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:477 | `InMemoryKey PeerData::userpicUniqueKey(Ui::PeerUserpicView &view) const` | partial(5) | UI view cache key | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:486 | `QImage PeerData::GenerateUserpicImage( not_null<PeerData*> peer, Ui::PeerUserpicView &view, int size, std::optional<int> radius)` | partial(5) | QImage rendering | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:546 | `ImageLocation PeerData::userpicLocation() const` | done | `src/data/data_peer.rs:614` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:550 | `bool PeerData::userpicPhotoUnknown() const` | done | `src/data/data_peer.rs:620` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:554 | `PhotoId PeerData::userpicPhotoId() const` | done | `src/data/data_peer.rs:623` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:558 | `bool PeerData::userpicHasVideo() const` | done | `src/data/data_peer.rs:630` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:562 | `Data::FileOrigin PeerData::userpicOrigin() const` | partial(16) | real FileOrigin variants | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:566 | `Data::FileOrigin PeerData::userpicPhotoOrigin() const` | partial(16) | real FileOrigin variants | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:572 | `void PeerData::updateUserpic( PhotoId photoId, MTP::DcId dcId, bool hasVideo)` | done | `src/data/data_peer.rs:1485` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:591 | `void PeerData::clearUserpic()` | done | `src/data/data_peer.rs:671` | userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:595 | `void PeerData::setUserpicChecked( PhotoId photoId, const ImageLocation &location, bool hasVideo)` | done | `src/data/data_peer.rs:647`; `src/data/data_session.rs:263` | premium_photo_check_reads_state_after_realtime_callback |
| data/data_peer.cpp:611 | `auto PeerData::unavailableReasons() const -> const std::vector<Data::UnavailableReason> &` | done | `src/data/data_peer.rs:587` — empty base list; actual subtype getter callback | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:617 | `QString PeerData::computeUnavailableReason() const` | done | `src/data/data_peer.rs:124` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:623 | `bool PeerData::hasSensitiveContent() const` | done | `src/data/data_peer.rs:582` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:627 | `void PeerData::setUnavailableReasonsList( std::vector<Data::UnavailableReason> &&reasons)` | done | `src/data/data_peer.rs:590` — base Unexpected; actual subtype overrides via required callback | base_unavailable_list_setter_keeps_original_unexpected |
| data/data_peer.cpp:632 | `void PeerData::setUnavailableReasons( std::vector<Data::UnavailableReason> &&reasons)` | done | `src/data/data_session.rs:242` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:657 | `void PeerData::setHasSensitiveContent(bool has)` | done | `src/data/data_peer.rs:595` | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.cpp:665 | `bool PeerData::canPinMessages() const` | done | `src/data/data_peer.rs:1059` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:680 | `bool PeerData::canCreatePolls(bool forbidInForums) const` | done | `src/data/data_peer.rs:1319` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:693 | `bool PeerData::canCreateTodoLists(bool forbidInForums) const` | done | `src/data/data_peer.rs:1339` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:702 | `bool PeerData::canCreateTopics() const` | done | `src/data/data_peer.rs:1352` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:712 | `bool PeerData::canManageTopics() const` | done | `src/data/data_peer.rs:1363` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:723 | `bool PeerData::canPostStories() const` | done | `src/data/data_peer.rs:1372` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:730 | `bool PeerData::canEditStories() const` | done | `src/data/data_peer.rs:1379` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:737 | `bool PeerData::canDeleteStories() const` | done | `src/data/data_peer.rs:1386` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:744 | `bool PeerData::canManageGifts() const` | done | `src/data/data_peer.rs:1393` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:751 | `bool PeerData::canTransferGifts() const` | done | `src/data/data_peer.rs:1400` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:758 | `bool PeerData::canEditMessagesIndefinitely() const` | done | `src/data/data_peer.rs:1407` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:771 | `bool PeerData::canExportChatHistory() const` | done | `src/data/data_peer.rs:1587` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:792 | `bool PeerData::autoTranslation() const` | done | `src/data/data_peer.rs:816` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:799 | `bool PeerData::setAbout(const QString &newAbout)` | done | `src/data/data_peer.rs:311` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:808 | `void PeerData::checkFolder(FolderId folderId)` | done | `src/data/data_peer.rs:1834` | folder_notify_and_bar_detail_clear_keep_original_drop_conditions |
| data/data_peer.cpp:819 | `void PeerData::clearBusinessBot()` | done | `src/data/data_session.rs:125` | folder_notify_and_bar_detail_clear_keep_original_drop_conditions |
| data/data_peer.cpp:838 | `void PeerData::setTranslationDisabled(bool disabled)` | done | `src/data/data_peer.rs:403` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:850 | `PeerData::TranslationFlag PeerData::translationFlag() const` | done | `src/data/data_peer.rs:400` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:854 | `void PeerData::saveTranslationDisabled(bool disabled)` | done | `src/data/data_peer.rs:415` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:864 | `void PeerData::setBarSettings(const MTPPeerSettings &data)` | done | `src/data/data_session.rs:226` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:929 | `void PeerData::setBarSettings(PeerBarSettings which)` | done | `src/data/data_session.rs:117` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:948 | `int PeerData::paysPerMessage() const` | done | `src/data/data_peer.rs:527` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:952 | `void PeerData::clearPaysPerMessage()` | done | `src/data/data_peer.rs:530` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:969 | `bool PeerData::hideLinks() const` | done | `src/data/data_peer.rs:523` | migrated_hide_links_and_name_resolution_projections |
| data/data_peer.cpp:980 | `QString PeerData::requestChatTitle() const` | done | `src/data/data_peer.rs:548` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:984 | `TimeId PeerData::requestChatDate() const` | done | `src/data/data_peer.rs:553` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:988 | `UserData *PeerData::businessBot() const` | done | `src/data/data_peer.rs:545` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:992 | `QString PeerData::businessBotManageUrl() const` | done | `src/data/data_peer.rs:561` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:996 | `QString PeerData::phoneCountryCode() const` | done | `src/data/data_peer.rs:556` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:1000 | `int PeerData::registrationMonth() const` | done | `src/data/data_peer.rs:566`; `src/data/data_peer.rs:734` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:1006 | `int PeerData::registrationYear() const` | done | `src/data/data_peer.rs:571`; `src/data/data_peer.rs:726` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:1010 | `TimeId PeerData::nameChangeDate() const` | done | `src/data/data_peer.rs:576` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:1014 | `TimeId PeerData::photoChangeDate() const` | done | `src/data/data_peer.rs:579` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.cpp:1018 | `bool PeerData::changeBackgroundEmojiId( const tl::conditional<MTPlong> &cloudBackgroundEmoji)` | done | `src/data/data_peer.rs:460` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1025 | `bool PeerData::changeColorCollectible( const tl::conditional<MTPPeerColor> &cloudColor)` | partial(5) | pure scalar color path + synchronous actual collectible callback in Peers::change_color; UI storage deferred | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1039 | `bool PeerData::changeColor( const tl::conditional<MTPPeerColor> &cloudColor)` | done | `src/data/data_session.rs:330` | color_profile_tabs_and_bot_button_value_helpers |
| data/data_peer.cpp:1051 | `bool PeerData::changeColorProfile( const tl::conditional<MTPPeerColor> &cloudColor)` | done | `src/data/data_session.rs:349` | color_profile_tabs_and_bot_button_value_helpers |
| data/data_peer.cpp:1063 | `void PeerData::fillNames()` | done | `src/data/data_peer_search.rs:14` | fill_names_requires_real_text_boundary_and_uses_only_active_first_username |
| data/data_peer.cpp:1119 | `PeerData::~PeerData() = default;` | done | Rust Drop of owned fields | peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.cpp:1121 | `void PeerData::updateFull()` | done | `src/data/data_peer.rs:353` | full_update_timeout_and_creator_slowmode_producer_difference |
| data/data_peer.cpp:1128 | `void PeerData::updateFullForced()` | done | `src/data/data_peer.rs:361` | full_update_timeout_and_creator_slowmode_producer_difference |
| data/data_peer.cpp:1137 | `void PeerData::fullUpdated()` | done | `src/data/data_peer.rs:346` | full_update_timeout_and_creator_slowmode_producer_difference |
| data/data_peer.cpp:1142 | `UserData *PeerData::asBot()` | done | `src/data/data_peer.rs:1287` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1146 | `const UserData *PeerData::asBot() const` | done | `src/data/data_peer.rs:1287` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1152 | `UserData *PeerData::asUser()` | done | `src/data/data_peer.rs:1278` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1156 | `const UserData *PeerData::asUser() const` | done | `src/data/data_peer.rs:1278` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1160 | `ChatData *PeerData::asChat()` | done | `src/data/data_peer.rs:1281` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1164 | `const ChatData *PeerData::asChat() const` | done | `src/data/data_peer.rs:1281` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1168 | `ChannelData *PeerData::asChannel()` | done | `src/data/data_peer.rs:1284` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1172 | `const ChannelData *PeerData::asChannel() const` | done | `src/data/data_peer.rs:1284` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1178 | `ChannelData *PeerData::asMegagroup()` | done | `src/data/data_peer.rs:1290` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1182 | `const ChannelData *PeerData::asMegagroup() const` | done | `src/data/data_peer.rs:1290` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1188 | `ChannelData *PeerData::asBroadcast()` | done | `src/data/data_peer.rs:1293` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1192 | `const ChannelData *PeerData::asBroadcast() const` | done | `src/data/data_peer.rs:1293` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1198 | `ChatData *PeerData::asChatNotMigrated()` | done | `src/data/data_peer.rs:1713` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1205 | `const ChatData *PeerData::asChatNotMigrated() const` | done | `src/data/data_peer.rs:1713` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1212 | `ChannelData *PeerData::asChannelOrMigrated()` | done | `src/data/data_peer.rs:1722` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1219 | `const ChannelData *PeerData::asChannelOrMigrated() const` | done | `src/data/data_peer.rs:1722` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1226 | `ChannelData *PeerData::asMonoforum()` | done | `src/data/data_peer.rs:1296` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1231 | `const ChannelData *PeerData::asMonoforum() const` | done | `src/data/data_peer.rs:1296` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1236 | `ChatData *PeerData::migrateFrom() const` | done | `src/data/data_peer.rs:1683` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1245 | `ChannelData *PeerData::migrateTo() const` | done | `src/data/data_peer.rs:1697` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1254 | `not_null<PeerData*> PeerData::migrateToOrMe()` | done | `src/data/data_peer.rs:1731` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1261 | `not_null<const PeerData*> PeerData::migrateToOrMe() const` | done | `src/data/data_peer.rs:1731` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1268 | `not_null<PeerData*> PeerData::userpicPaintingPeer()` | done | `src/data/data_peer.rs:1763` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1275 | `not_null<const PeerData*> PeerData::userpicPaintingPeer() const` | done | `src/data/data_peer.rs:1763` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1279 | `Ui::PeerUserpicShape PeerData::userpicShape() const` | partial(5) | UI shape | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:1290 | `ChannelData *PeerData::monoforumBroadcast() const` | done | `src/data/data_peer.rs:1739` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1295 | `ChannelData *PeerData::broadcastMonoforum() const` | done | `src/data/data_peer.rs:1751` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1300 | `const QString &PeerData::topBarNameText() const` | done | `src/data/data_peer.rs:1312` | migrated_hide_links_and_name_resolution_projections |
| data/data_peer.cpp:1311 | `int PeerData::nameVersion() const` | done | `src/data/data_peer.rs:299` | chat_name_old_letters_first_update_suppression_and_wrap16 |
| data/data_peer.cpp:1315 | `const QString &PeerData::name() const` | done | `src/data/data_peer.rs:1613` | migrated_hide_links_and_name_resolution_projections |
| data/data_peer.cpp:1324 | `const QString &PeerData::shortName() const` | done | `src/data/data_peer.rs:1299` | migrated_hide_links_and_name_resolution_projections |
| data/data_peer.cpp:1335 | `QString PeerData::username() const` | done | `src/data/data_peer.rs:678` | user_name_phone_username_and_source_index_edge_rules |
| data/data_peer.cpp:1344 | `QString PeerData::editableUsername() const` | done | `src/data/data_peer.rs:682` | user_name_phone_username_and_source_index_edge_rules |
| data/data_peer.cpp:1353 | `const std::vector<QString> &PeerData::usernames() const` | done | `src/data/data_peer.rs:686` | user_name_phone_username_and_source_index_edge_rules |
| data/data_peer.cpp:1363 | `bool PeerData::isUsernameEditable(QString username) const` | done | `src/data/data_peer.rs:690` | user_name_phone_username_and_source_index_edge_rules |
| data/data_peer.cpp:1372 | `bool PeerData::changeColorCollectible(Ui::ColorCollectible data)` | partial(5) | pure scalar color path + synchronous actual collectible callback in Peers::change_color; UI storage deferred | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1383 | `bool PeerData::clearColorCollectible()` | partial(5) | Ui::ColorCollectible owner | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1391 | `bool PeerData::changeColorIndex(uint8 index)` | done | `src/data/data_peer.rs:426` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1401 | `bool PeerData::clearColorIndex()` | done | `src/data/data_peer.rs:435` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1410 | `DocumentId PeerData::backgroundEmojiId() const` | done | `src/data/data_peer.rs:457` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1414 | `bool PeerData::changeBackgroundEmojiId(DocumentId id)` | done | `src/data/data_peer.rs:460` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1422 | `bool PeerData::changeColorProfileCollectible(Ui::ColorCollectible data)` | partial(5) | pure profile path + synchronous actual collectible callback; UI owner deferred | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1431 | `bool PeerData::changeColorProfileCollectible( const tl::conditional<MTPPeerColor> &cloudColor)` | partial(5) | pure profile path + synchronous actual collectible callback; UI owner deferred | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1445 | `bool PeerData::clearColorProfileCollectible()` | partial(5) | Ui::ColorCollectible owner | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1453 | `bool PeerData::changeColorProfileIndex(uint8 index)` | done | `src/data/data_peer.rs:446` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1462 | `bool PeerData::clearColorProfileIndex()` | done | `src/data/data_peer.rs:454` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1470 | `DocumentId PeerData::profileBackgroundEmojiId() const` | done | `src/data/data_peer.rs:467` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1474 | `bool PeerData::changeProfileBackgroundEmojiId(DocumentId id)` | done | `src/data/data_peer.rs:470` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1482 | `void PeerData::setEmojiStatus(const MTPEmojiStatus &status)` | partial(07) | `src/data/data_peer.rs:480` — parsed identity mutation implemented; real EmojiStatuses::Parse awaits service | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1487 | `void PeerData::setEmojiStatus(EmojiStatusId emojiStatusId, TimeId until)` | done | `src/data/data_peer.rs:480` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1495 | `EmojiStatusId PeerData::emojiStatusId() const` | done | `src/data/data_peer.rs:477` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1499 | `bool PeerData::isBot() const` | done | `src/data/data_peer.rs:833` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1506 | `bool PeerData::isSelf() const` | done | `src/data/data_peer.rs:829` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1513 | `bool PeerData::isVerified() const` | done | `src/data/data_peer.rs:744` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1522 | `bool PeerData::isPremium() const` | done | `src/data/data_peer.rs:837` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1529 | `bool PeerData::isScam() const` | done | `src/data/data_peer.rs:752` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1538 | `bool PeerData::isFake() const` | done | `src/data/data_peer.rs:760` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1547 | `bool PeerData::isMegagroup() const` | done | `src/data/data_peer.rs:800` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1554 | `bool PeerData::isBroadcast() const` | done | `src/data/data_peer.rs:804` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1561 | `bool PeerData::isForum() const` | done | `src/data/data_peer.rs:841` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1570 | `bool PeerData::isMonoforum() const` | done | `src/data/data_peer.rs:808` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1577 | `bool PeerData::isGigagroup() const` | done | `src/data/data_peer.rs:812` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1584 | `bool PeerData::isRepliesChat() const` | done | `src/data/data_peer.rs:289` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.cpp:1595 | `bool PeerData::isVerifyCodes() const` | done | `src/data/data_peer.rs:286` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.cpp:1600 | `bool PeerData::isFreezeAppealChat() const` | done | `src/data/data_peer.rs:292` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1604 | `bool PeerData::sharedMediaInfo() const` | done | `src/data/data_peer.rs:849` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1608 | `bool PeerData::savedSublistsInfo() const` | done | `src/data/data_peer.rs:853` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1612 | `bool PeerData::hasStoriesHidden() const` | done | `src/data/data_peer.rs:768` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1621 | `void PeerData::setStoriesHidden(bool hidden)` | done | `src/data/data_peer.rs:1082` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1635 | `Ui::BotVerifyDetails *PeerData::botVerifyDetails() const` | partial(5) | Ui::BotVerifyDetails ownership | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:1644 | `Data::Forum *PeerData::forum() const` | done | `src/data/data_peer.rs:1767` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1653 | `Data::ForumTopic *PeerData::forumTopicFor(MsgId rootId) const` | done | `src/data/data_peer.rs:1779` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1662 | `Data::SavedMessages *PeerData::monoforum() const` | done | `src/data/data_peer.rs:1790` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1669 | `Data::SavedSublist *PeerData::monoforumSublistFor( PeerId sublistPeerId) const` | done | `src/data/data_session.rs:371` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1679 | `bool PeerData::useSubsectionTabs() const` | done | `src/data/data_peer.rs:1565` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1688 | `bool PeerData::displayAsForum() const` | done | `src/data/data_peer.rs:1454` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1698 | `bool PeerData::displaySubsectionTabs() const` | done | `src/data/data_peer.rs:1576` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1705 | `bool PeerData::viewForumAsMessages() const` | done | `src/data/data_peer.rs:824` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1712 | `void PeerData::processTopics(const MTPVector<MTPForumTopic> &topics)` | done | `src/data/data_peer.rs:1801` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1718 | `bool PeerData::isAyuNoForwards() const` | done | `src/data/data_peer.rs:890` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1729 | `bool PeerData::allowsForwarding() const` | done | `src/data/data_peer.rs:900` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1740 | `Data::RestrictionCheckResult PeerData::amRestricted( ChatRestriction right) const` | done | `src/data/data_peer.rs:908` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1797 | `bool PeerData::amAnonymous() const` | done | `src/data/data_peer.rs:988` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1806 | `bool PeerData::canRevokeFullHistory() const` | done | `src/data/data_peer.rs:1421` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1824 | `bool PeerData::slowmodeApplied() const` | done | `src/data/data_peer.rs:1000` | full_update_timeout_and_creator_slowmode_producer_difference |
| data/data_peer.cpp:1833 | `rpl::producer<bool> PeerData::slowmodeAppliedValue() const` | done | `src/data/data_peer.rs:1667` | full_update_timeout_and_creator_slowmode_producer_difference |
| data/data_peer.cpp:1858 | `int PeerData::slowmodeSecondsLeft() const` | done | `src/data/data_peer.rs:1004` | full_update_timeout_and_creator_slowmode_producer_difference |
| data/data_peer.cpp:1870 | `bool PeerData::canManageGroupCall() const` | done | `src/data/data_peer.rs:1028` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1886 | `bool PeerData::canManageRanks() const` | done | `src/data/data_peer.rs:1041` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1900 | `bool PeerData::canManageWelcomeMessages() const` | done | `src/data/data_peer.rs:1054` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1912 | `bool PeerData::amMonoforumAdmin() const` | done | `src/data/data_peer.rs:820` | chat_and_channel_restrictions_admin_public_boost_priority |
| data/data_peer.cpp:1919 | `int PeerData::starsPerMessage() const` | done | `src/data/data_peer.rs:857` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1928 | `int PeerData::starsPerMessageChecked() const` | done | `src/data/data_peer.rs:865` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1939 | `Data::StarsRating PeerData::starsRating() const` | done | `src/data/data_peer.rs:875` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:1946 | `Data::GroupCall *PeerData::groupCall() const` | done | `src/data/data_peer.rs:1811` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1955 | `PeerId PeerData::groupCallDefaultJoinAs() const` | done | `src/data/data_peer.rs:1823` | migration_forum_monoforum_and_group_call_keys_are_real_projections |
| data/data_peer.cpp:1964 | `void PeerData::setThemeToken(const QString &token)` | done | `src/data/data_peer.rs:1250` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1979 | `const QString &PeerData::themeToken() const` | done | `src/data/data_peer.rs:1275` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:1983 | `void PeerData::setWallPaper( std::optional<Data::WallPaper> paper, bool overriden)` | partial(16/5) | real WallPaper/document/UI ownership | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:2005 | `bool PeerData::wallPaperOverriden() const` | partial(16/5) | wallpaper model binding | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:2009 | `const Data::WallPaper *PeerData::wallPaper() const` | partial(16/5) | wallpaper model binding | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:2013 | `bool PeerData::hasActiveStories() const` | done | `src/data/data_peer.rs:776` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:2022 | `bool PeerData::hasUnreadStories() const` | done | `src/data/data_peer.rs:784` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:2031 | `bool PeerData::hasActiveVideoStream() const` | done | `src/data/data_peer.rs:792` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:2040 | `void PeerData::setStoriesState(StoriesState state)` | done | `src/data/data_peer.rs:1092` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:2050 | `int PeerData::peerGiftsCount() const` | done | `src/data/data_peer.rs:882` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:2059 | `void PeerData::setMainProfileTab(Data::ProfileTab tab)` | done | `src/data/data_peer.rs:496` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:2066 | `Data::ProfileTab PeerData::mainProfileTab() const` | done | `src/data/data_peer.rs:493` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:2070 | `MTPInputPeer PeerData::input() const` | partial(02/03) | actual subclass input/access-hash/from-message state | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.cpp:2105 | `void PeerData::setIsBlocked(bool is)` | done | `src/data/data_peer.rs:377` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:2124 | `void PeerData::setLoadedStatus(LoadedStatus status)` | done | `src/data/data_peer.rs:321` | peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.cpp:2128 | `TimeId PeerData::messagesTTL() const` | done | `src/data/data_peer.rs:336` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:2132 | `void PeerData::setMessagesTTL(TimeId period)` | done | `src/data/data_peer.rs:339` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.cpp:2143 | `void SetTopPinnedMessageId( not_null<PeerData*> peer, MsgId messageId)` | done | `src/data/data_peer_pinned.rs:16` | pinned_existing_storage_migrated_priority_and_effect_order |
| data/data_peer.cpp:2171 | `FullMsgId ResolveTopPinnedId( not_null<PeerData*> peer, MsgId topicRootId, PeerId monoforumPeerId, PeerData *migrated)` | done | `src/data/data_peer_pinned.rs:57` | pinned_existing_storage_migrated_priority_and_effect_order |
| data/data_peer.cpp:2211 | `FullMsgId ResolveMinPinnedId( not_null<PeerData*> peer, MsgId topicRootId, PeerId monoforumPeerId, PeerData *migrated)` | done | `src/data/data_peer_pinned.rs:90` | pinned_existing_storage_migrated_priority_and_effect_order |
| data/data_peer.cpp:2251 | `uint64 BackgroundEmojiIdFromColor(const MTPPeerColor *color)` | done | `src/data/data_peer.rs:1524` | color_profile_tabs_and_bot_button_value_helpers |
| data/data_peer.cpp:2264 | `std::optional<uint8> ColorIndexFromColor(const MTPPeerColor *color)` | done | `src/data/data_peer.rs:1531` | color_profile_tabs_and_bot_button_value_helpers |
| data/data_peer.cpp:2275 | `ProfileTab ParseProfileTab(const MTPProfileTab *tab)` | done | `src/data/data_peer.rs:1537` | color_profile_tabs_and_bot_button_value_helpers |
| data/data_peer.cpp:2298 | `MTPProfileTab ProfileTabToMTP(ProfileTab tab)` | done | `src/data/data_peer.rs:1551` | color_profile_tabs_and_bot_button_value_helpers |
| data/data_peer.cpp:2313 | `bool IsBotUserCreatesTopics(not_null<PeerData*> peer)` | done | `src/data/data_peer.rs:1446` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.cpp:2320 | `bool IsBotCreatesTopics(not_null<const PeerData*> peer)` | done | `src/data/data_peer.rs:1450` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_birthday.cpp:18 | `[[nodiscard]] bool Validate(int day, int month, int year)` | done | `src/data/data_birthday.rs:19` | native_pure_functions_reference |
| data/data_birthday.cpp:38 | `[[nodiscard]] int Serialize(int day, int month, int year)` | done | `src/data/data_birthday.rs:31` | native_pure_functions_reference |
| data/data_birthday.cpp:44 | `Birthday::Birthday(int day, int month, int year) : _value(Validate(day, month, year) ? Serialize(day, month, year) : 0)` | done | `src/data/data_birthday.rs:31` | native_pure_functions_reference |
| data/data_birthday.cpp:48 | `Birthday Birthday::FromSerialized(int value)` | done | `src/data/data_birthday.rs:42` | native_pure_functions_reference |
| data/data_birthday.cpp:52 | `int Birthday::serialize() const` | done | `src/data/data_birthday.rs:45` | native_pure_functions_reference |
| data/data_birthday.cpp:56 | `bool Birthday::valid() const` | done | `src/data/data_birthday.rs:48` | native_pure_functions_reference |
| data/data_birthday.cpp:60 | `int Birthday::day() const` | done | `src/data/data_birthday.rs:51` | native_pure_functions_reference |
| data/data_birthday.cpp:64 | `int Birthday::month() const` | done | `src/data/data_birthday.rs:54` | native_pure_functions_reference |
| data/data_birthday.cpp:68 | `int Birthday::year() const` | done | `src/data/data_birthday.rs:57` | native_pure_functions_reference |
| data/data_birthday.cpp:72 | `QString BirthdayText(Birthday date, bool fullMonth)` | partial(01/5) | real lang MonthDay/MonthSmall; Claude area untouched | Не выполнена отложенная часть; boundary: birthday_minute_producer_virtual_calendar_and_leap_bug |
| data/data_birthday.cpp:96 | `QString BirthdayCake()` | done | `src/data/data_birthday.rs:61` | birthday_minute_producer_virtual_calendar_and_leap_bug |
| data/data_birthday.cpp:100 | `int BirthdayAge(Birthday date)` | done | `src/data/data_birthday.rs:64` | native_pure_functions_reference |
| data/data_birthday.cpp:116 | `bool IsBirthdayToday(Birthday date)` | done | `src/data/data_birthday.rs:88` | native_pure_functions_reference |
| data/data_birthday.cpp:124 | `rpl::producer<bool> IsBirthdayTodayValue(Birthday date)` | done | `src/data/data_birthday.rs:91` | birthday_minute_producer_virtual_calendar_and_leap_bug |
| data/data_user_names.cpp:12 | `UsernamesInfo::UsernamesInfo() = default;` | done | `src/data/data_user_names.rs:15` | user_name_phone_username_and_source_index_edge_rules; native_pure_functions_reference |
| data/data_user_names.cpp:14 | `void UsernamesInfo::setUsername(const QString &username)` | done | `src/data/data_user_names.rs:18` | user_name_phone_username_and_source_index_edge_rules; native_pure_functions_reference |
| data/data_user_names.cpp:40 | `void UsernamesInfo::setUsernames(const Usernames &usernames)` | done | `src/data/data_user_names.rs:36` | user_name_phone_username_and_source_index_edge_rules; native_pure_functions_reference |
| data/data_user_names.cpp:69 | `QString UsernamesInfo::username() const` | done | `src/data/data_user_names.rs:54` | user_name_phone_username_and_source_index_edge_rules; native_pure_functions_reference |
| data/data_user_names.cpp:73 | `QString UsernamesInfo::editableUsername() const` | done | `src/data/data_user_names.rs:57` | user_name_phone_username_and_source_index_edge_rules; native_pure_functions_reference |
| data/data_user_names.cpp:79 | `const std::vector<QString> &UsernamesInfo::usernames() const` | done | `src/data/data_user_names.rs:60` | user_name_phone_username_and_source_index_edge_rules; native_pure_functions_reference |
| data/data_user_names.cpp:83 | `bool UsernamesInfo::isEditable(const QString &username) const` | done | `src/data/data_user_names.rs:63` | user_name_phone_username_and_source_index_edge_rules; native_pure_functions_reference |
| data/notify/data_peer_notify_settings.cpp:15 | `[[nodiscard]] MTPinputPeerNotifySettings DefaultSettings()` | done | `src/data/notify/data_peer_notify_settings.rs:98`; `src/data/notify/data_peer_notify_settings.rs:221` — serialize default when unknown/empty | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:27 | `[[nodiscard]] NotifySound ParseSound(const MTPNotificationSound &sound)` | done | `src/data/notify/data_peer_notify_settings.rs:39` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:42 | `[[nodiscard]] MTPNotificationSound SerializeSound( const std::optional<NotifySound> &sound)` | done | `src/data/notify/data_peer_notify_settings.rs:58` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:59 | `int MuteValue::until() const` | done | `src/data/notify/data_peer_notify_settings.rs:27` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:103 | `NotifyPeerSettingsValue::NotifyPeerSettingsValue( const MTPDpeerNotifySettings &data)` | done | `src/data/notify/data_peer_notify_settings.rs:81` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:108 | `bool NotifyPeerSettingsValue::change(const MTPDpeerNotifySettings &data)` | done | `src/data/notify/data_peer_notify_settings.rs` — NotifyPeerSettingsValue::parse/change | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:126 | `bool NotifyPeerSettingsValue::change( MuteValue muteForSeconds, std::optional<bool> silentPosts, std::optional<NotifySound> sound, std::optional<bool> storiesMuted)` | done | `src/data/notify/data_peer_notify_settings.rs` — change_local (time injected) | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:151 | `bool NotifyPeerSettingsValue::change( std::optional<int> mute, std::optional<NotifySound> sound, std::optional<bool> showPreviews, std::optional<bool> silentPosts, std::optional<bool> storiesMuted)` | done | `src/data/notify/data_peer_notify_settings.rs` — NotifyPeerSettingsValue::parse/change | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:172 | `std::optional<TimeId> NotifyPeerSettingsValue::muteUntil() const` | done | `src/data/notify/data_peer_notify_settings.rs:212` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:176 | `std::optional<bool> NotifyPeerSettingsValue::silentPosts() const` | done | `src/data/notify/data_peer_notify_settings.rs:215` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:180 | `std::optional<NotifySound> NotifyPeerSettingsValue::sound() const` | done | `src/data/notify/data_peer_notify_settings.rs:218` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:184 | `MTPinputPeerNotifySettings NotifyPeerSettingsValue::serialize() const` | done | `src/data/notify/data_peer_notify_settings.rs:98`; `src/data/notify/data_peer_notify_settings.rs:221` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:204 | `PeerNotifySettings::PeerNotifySettings() = default;` | done | `src/data/data_store_keys.rs:33`; `src/data/data_flags.rs:27`; `src/data/data_image_location.rs:155` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:206 | `bool PeerNotifySettings::change(const MTPPeerNotifySettings &settings)` | done | `src/data/notify/data_peer_notify_settings.rs` — PeerNotifySettings::change | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:225 | `bool PeerNotifySettings::change( MuteValue muteForSeconds, std::optional<bool> silentPosts, std::optional<NotifySound> sound, std::optional<bool> storiesMuted)` | done | `src/data/notify/data_peer_notify_settings.rs` — change_local (time injected) | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:259 | `bool PeerNotifySettings::resetToDefault()` | done | `src/data/notify/data_peer_notify_settings.rs:201` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:268 | `std::optional<TimeId> PeerNotifySettings::muteUntil() const` | done | `src/data/notify/data_peer_notify_settings.rs:212` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:274 | `bool PeerNotifySettings::settingsUnknown() const` | done | `src/data/notify/data_peer_notify_settings.rs:209` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:278 | `std::optional<bool> PeerNotifySettings::silentPosts() const` | done | `src/data/notify/data_peer_notify_settings.rs:215` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:284 | `std::optional<NotifySound> PeerNotifySettings::sound() const` | done | `src/data/notify/data_peer_notify_settings.rs:218` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:290 | `MTPinputPeerNotifySettings PeerNotifySettings::serialize() const` | done | `src/data/notify/data_peer_notify_settings.rs:98`; `src/data/notify/data_peer_notify_settings.rs:221` | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/notify/data_peer_notify_settings.cpp:296 | `PeerNotifySettings::~PeerNotifySettings() = default;` | done | Rust Drop of owned fields | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/data_peer.h:54 | `static RestrictionCheckResult Allowed()` | done | RestrictionCheckResult enum variants | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:57 | `static RestrictionCheckResult WithEveryone()` | done | RestrictionCheckResult enum variants | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:60 | `static RestrictionCheckResult Explicit()` | done | RestrictionCheckResult enum variants | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:64 | `explicit operator bool() const` | done | `src/data/data_peer.rs:104` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:68 | `bool operator==(const RestrictionCheckResult &other) const` | done | derived value equality/order | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:71 | `bool operator!=(const RestrictionCheckResult &other) const` | done | derived value equality/order | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:75 | `bool isAllowed() const` | done | `src/data/data_peer.rs:95` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:78 | `bool isWithEveryone() const` | done | `src/data/data_peer.rs:98` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:81 | `bool isExplicit() const` | done | `src/data/data_peer.rs:101` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:86 | `RestrictionCheckResult(int value) : _value(value)` | done | three-state enum constructors | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:97 | `friend inline bool operator==( const UnavailableReason &, const UnavailableReason &) = default;` | done | derived value equality/order | unavailable_reason_preload_list_assignment_and_notification_order |
| data/data_peer.h:142 | `friend inline bool operator==( const AllowedReactions &, const AllowedReactions &) = default;` | partial(09) | AllowedReactions equality, same future ReactionId package as Parse | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.h:168 | `not_null<PeerData*> peer() const` | partial(6) | ClickHandler peer accessor — deferred stage6 | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.h:230 | `uint8 colorIndex() const` | done | `src/data/data_peer.rs:423` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.h:233 | `auto colorCollectible() const -> const std::shared_ptr<Ui::ColorCollectible> &` | partial(5) | Ui::ColorCollectible owner | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.h:244 | `std::optional<uint8> colorProfileIndex() const` | done | `src/data/data_peer.rs:443` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.h:247 | `auto colorProfileCollectible() const -> const std::shared_ptr<Ui::ColorCollectible> &` | partial(5) | Ui::ColorCollectible owner | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.h:264 | `bool isUser() const` | done | `src/data/data_peer.rs:268` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.h:267 | `bool isChat() const` | done | `src/data/data_peer.rs:271` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.h:270 | `bool isChannel() const` | done | `src/data/data_peer.rs:274` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.h:294 | `bool isNotificationsUser() const` | done | `src/data/data_peer.rs:277` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:298 | `bool isServiceUser() const` | done | `src/data/data_peer.rs:280` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:301 | `bool isSavedHiddenAuthor() const` | done | `src/data/data_peer.rs:283` | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:318 | `Data::PeerNotifySettings &notify()` | done | `src/data/data_peer.rs:1460` | folder_notify_and_bar_detail_clear_keep_original_drop_conditions |
| data/data_peer.h:321 | `const Data::PeerNotifySettings &notify() const` | done | `src/data/data_peer.rs:1460` | folder_notify_and_bar_detail_clear_keep_original_drop_conditions |
| data/data_peer.h:379 | `bool wasFullUpdated() const` | done | `src/data/data_peer.rs:350` | full_update_timeout_and_creator_slowmode_producer_difference |
| data/data_peer.h:393 | `const base::flat_set<QString> &nameWords() const` | done | `src/data/data_peer.rs:302` | chat_name_old_letters_first_update_suppression_and_wrap16 |
| data/data_peer.h:396 | `const base::flat_set<QChar> &nameFirstLetters() const` | done | `src/data/data_peer.rs:305` | chat_name_old_letters_first_update_suppression_and_wrap16 |
| data/data_peer.h:410 | `void paintUserpic( QPainter &p, Ui::PeerUserpicView &view, int x, int y, int size, bool forceCircle = false) const` | partial(5) | painting | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.h:425 | `void paintUserpicLeft( QPainter &p, Ui::PeerUserpicView &view, int x, int y, int w, int size, bool forceCircle = false) const` | partial(5) | painting/RTL | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.h:469 | `const ClickHandlerPtr &openLink()` | partial(6) | ClickHandler | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| data/data_peer.h:494 | `const QString &about() const` | done | `src/data/data_peer.rs:308` | user_predicates_read_real_flag_projections_and_ayu_policy |
| data/data_peer.h:501 | `auto barSettings() const` | done | `src/data/data_peer.rs:503` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.h:506 | `auto barSettingsValue() const` | done | `src/data/data_peer.rs:511` | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.h:547 | `BlockStatus blockStatus() const` | done | `src/data/data_peer.rs:371` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.h:550 | `bool isBlocked() const` | done | `src/data/data_peer.rs:374` | common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_peer.h:561 | `LoadedStatus loadedStatus() const` | done | `src/data/data_peer.rs:318` | peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.h:564 | `bool isMinimalLoaded() const` | done | `src/data/data_peer.rs:324` | peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.h:567 | `bool isLoaded() const` | done | `src/data/data_peer.rs:327` | peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.h:570 | `bool isFullLoaded() const` | done | `src/data/data_peer.rs:333` | peer_store_scope_load_states_and_reentrant_changes |
| data/data_flags.h:37 | `Flags() = default;` | done | `src/data/data_flags.rs:32` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:38 | `Flags(Type value) : _value(value)` | done | `src/data/data_flags.rs:32` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:41 | `void set(Type which)` | done | `src/data/data_flags.rs:44` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:47 | `void add(Type which)` | done | `src/data/data_flags.rs:51` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:53 | `void remove(Type which)` | done | `src/data/data_flags.rs:58` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:59 | `auto current() const` | done | `src/data/data_flags.rs:65` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:62 | `auto changes() const` | done | `src/data/data_flags.rs:68` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:65 | `auto value() const` | done | `src/data/data_flags.rs:71` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_flags.h:72 | `void updated(Type diff)` | done | `src/data/data_flags.rs:77` | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_peer_common.h:18 | `explicit operator bool() const` | done | `src/data/data_peer_common.rs` — is_present | peer_constants_restriction_values_and_rating_presence |
| data/data_peer_common.h:22 | `friend inline bool operator==(StarsRating, StarsRating) = default;` | done | derived value equality/order | peer_constants_restriction_values_and_rating_presence |
| data/data_peer_common.h:29 | `explicit operator bool() const` | done | `src/data/data_peer_common.rs` — is_present | peer_constants_restriction_values_and_rating_presence |
| data/data_lastseen_status.h:16 | `LastseenStatus() = default;` | done | `src/data/data_lastseen_status.rs:10` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:18 | `static LastseenStatus Recently(bool byMe = false)` | done | `src/data/data_lastseen_status.rs:17` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:21 | `static LastseenStatus WithinWeek(bool byMe = false)` | done | `src/data/data_lastseen_status.rs:20` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:24 | `static LastseenStatus WithinMonth(bool byMe = false)` | done | `src/data/data_lastseen_status.rs:23` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:27 | `static LastseenStatus LongAgo(bool byMe = false)` | done | `src/data/data_lastseen_status.rs:26` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:30 | `static LastseenStatus OnlineTill( TimeId till, bool local = false, bool hiddenByMe = false)` | done | `src/data/data_lastseen_status.rs:29` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:39 | `bool isHidden() const` | done | `src/data/data_lastseen_status.rs:41` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:42 | `bool isRecently() const` | done | `src/data/data_lastseen_status.rs:44` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:45 | `bool isWithinWeek() const` | done | `src/data/data_lastseen_status.rs:47` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:48 | `bool isWithinMonth() const` | done | `src/data/data_lastseen_status.rs:50` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:51 | `bool isLongAgo() const` | done | `src/data/data_lastseen_status.rs:53` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:54 | `bool isHiddenByMe() const` | done | `src/data/data_lastseen_status.rs:56` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:58 | `bool isOnline(TimeId now) const` | done | `src/data/data_lastseen_status.rs:59` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:62 | `bool isLocalOnlineValue() const` | done | `src/data/data_lastseen_status.rs:62` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:65 | `TimeId onlineTill() const` | done | `src/data/data_lastseen_status.rs:65` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:71 | `uint32 serialize() const` | done | `src/data/data_lastseen_status.rs:72` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:76 | `static LastseenStatus FromSerialized(uint32 value)` | done | `src/data/data_lastseen_status.rs:75` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:84 | `static LastseenStatus FromLegacy(int32 value)` | done | `src/data/data_lastseen_status.rs:87` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:99 | `friend inline constexpr auto operator<=>( LastseenStatus, LastseenStatus) = default;` | done | derived value equality/order | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:102 | `friend inline constexpr bool operator==( LastseenStatus a, LastseenStatus b) = default;` | done | derived value equality/order | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:114 | `bool valid() const` | done | `src/data/data_lastseen_status.rs:97` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_lastseen_status.h:120 | `LastseenStatus(uint32 value, bool available, bool hiddenByMe) : _value(value) , _available(available ? 1 : 0) , _hiddenByMe(hiddenByMe ? 1 : 0)` | done | `src/data/data_lastseen_status.rs:10` | lastseen_order_hidden_bits_and_serialized_validation; native_pure_functions_reference |
| data/data_birthday.h:14 | `constexpr Birthday() = default;` | done | `src/data/data_birthday.rs:31` | native_pure_functions_reference |
| data/data_birthday.h:26 | `explicit operator bool() const` | done | `src/data/data_birthday.rs` — valid | native_pure_functions_reference |
| data/data_birthday.h:30 | `friend inline constexpr auto operator<=>(Birthday, Birthday) = default;` | done | derived value equality/order | native_pure_functions_reference |
| data/data_birthday.h:31 | `friend inline constexpr bool operator==(Birthday, Birthday) = default;` | done | derived value equality/order | native_pure_functions_reference |
| data/notify/data_peer_notify_settings.h:32 | `explicit operator bool() const` | done | `src/data/notify/data_peer_notify_settings.rs` — MuteValue::is_present | notify_unknown_default_partial_changes_sound_and_tl_flags |
| data/data_flags.h:30 | `Flags::Change ctor / FlagsUnderlying / essential mask` | done | data_flags.rs — Change/FlagsUnderlying/with_essential | flags_reentrant_essential_initial_snapshot_and_lifetime |
| data/data_peer.h:186 | `PeerBarSetting/is_flag_type/PeerBarDetails defaults` | done | data_peer.rs — PeerBarSetting/PeerBarDetails | bar_unknown_initial_suppression_reentrant_hide_links_and_details |
| data/data_peer.h:320 | `notify() const and mutable access` | done | data_peer.rs — notify/notify_mut | folder_notify_and_bar_detail_clear_keep_original_drop_conditions |
| data/data_peer.h:215 | `kServiceNotificationsId/kSavedHiddenAuthorId/kUnknownPhotoId` | done | data_peer.rs — constants | peer_constants_restriction_values_and_rating_presence |
| data/data_peer.h:576 | `All common data fields / default values / owner scope` | partial(16/5/6/29) | data_peer.rs — real common fields; wallpaper, collectible, images/UI and lifecycle deferred | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_session.cpp:594 | `Session::peer/peerLoaded common-field owner component` | partial(02/03/13/14) | data_session.rs — Peers::ensure_peer/peer_loaded/read/update/dispatch; concrete enum deferred | Не выполнена отложенная часть; boundary: peer_store_scope_load_states_and_reentrant_changes |
| data/data_peer.h:415 | `updateNameDelayed phased real subclass callbacks` | done | data_session.rs — NameFields callbacks, no mutable borrow through fire | channel_name_flags_callback_can_reenter_and_fill_current_name |
| data/data_peer.cpp:1063 | `fillNames composition and UTF-16 sorted index` | partial(5) | data_peer_search.rs — fill_names/NameTextServices; real RemoveAccents/PrepareSearchWords backend awaits UI text package | Не выполнена отложенная часть; boundary: fill_names_requires_real_text_boundary_and_uses_only_active_first_username |
| data/data_user.h:119 | `UserDataFlag values only; remaining UserData header` | partial(02) | data_user.rs — exact enum, Self renamed SelfUser (Rust keyword) | Не выполнена отложенная часть; boundary: native_pure_functions_reference |
| data/data_chat.h:16 | `ChatDataFlag values only; remaining ChatData header` | partial(03) | data_chat.rs — exact enum; concrete ChatData/Zalgo setting owner deferred | Не выполнена отложенная часть; boundary: native_pure_functions_reference |
| data/data_channel.h:43 | `ChannelDataFlag values only; remaining ChannelData header` | partial(03) | data_channel.rs — exact enum; real capability projection | Не выполнена отложенная часть; boundary: native_pure_functions_reference |
| data/data_chat_participant_status.h:51 | `ChatRestriction bit values; remaining rights model/conversions` | partial(03) | data_chat_participant_status.rs — ChatRestriction/ChatRestrictions (admin enum already ported) | Не выполнена отложенная часть; boundary: native_pure_functions_reference |
| data/data_types.h:150 | `EmojiStatusId equality/collectible pointer identity; CallId` | partial(16) | data_types.rs — Rc::ptr_eq, scalar aliases; remaining types deferred | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_emoji_statuses.h:26 | `EmojiStatusCollectible real data fields only` | partial(07) | data_emoji_statuses.rs — data fields; full status service/timers later | Не выполнена отложенная часть; boundary: common_colors_emoji_identity_ttl_block_and_theme_effects |
| data/data_group_call.h:74 | `GroupCall::id identity contract only` | partial(08) | data_store_keys.rs — GroupCallKey; real model ownership/lifetime later | Не выполнена отложенная часть; boundary: migration_forum_monoforum_and_group_call_keys_are_real_projections |
| ui/image/image_location.h:52 | `StorageFile/Download/ImageLocation value constructors, valid/equality and peerPhoto` | partial(5) | data_image_location.rs — metadata variants; serialization/cache/loader/UI methods later | Не выполнена отложенная часть; boundary: userpic_location_identity_excludes_dimensions_hash_and_reference |
| ui/image/image_location.cpp:40 | `StorageFile valid/equality, ImageLocation constructor; remaining bodies` | partial(5) | data_image_location.rs — exact metadata semantics, no loader or fake QImage | Не выполнена отложенная часть; boundary: native_pure_functions_reference |
| core/utils.cpp:439 | `translitRusEng/initializeTranslit/rusKeyboardLayoutSwitch/initializeKeyboardSwitch` | partial(34/5) | core/utils.rs — exact translit and keyboard tables only; remaining utils not part of PeerData | Не выполнена отложенная часть; boundary: native_pure_functions_reference |
