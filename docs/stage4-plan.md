# Этап 4: порядок перевода ядра

Исходная точка: AyuGram Desktop 0f2a31bb420de98f331990432265b131b8d836b5.
Документ проектный. Ниже явно отделены автоматическая инвентаризация, прочитанные
места и предложения, которые ещё требуют проверки тел функций. Это не утверждение,
что все 184545 строк прочитаны или что линейная сортировка файлов разрывает циклы.

## Пересчитываемый граф

```sh
python3 tools/stage4_graph.py --original /path/to/AyuGramDesktop
```

Вход — ledger/files.csv, все literal #include оригинала. Разрешение: рядом с файлом,
SourceFiles, lib_*, Telegram. Выход: [полные рёбра](stage4/includes.tsv),
[DOT](stage4/graph.dot), [данные/классификация/пакеты](stage4/graph.json).
Macro includes, неявный stdafx/PCH, forward declarations и вызовы через указатели
не становятся рёбрами автоматически. QtCore QString/QDate не считаются GUI.
Внешние ui-группы включают lib_ui, ui, window, boxes, history/view, media/view,
chat_helpers, layout, dialogs; последнее консервативно: часть Dialogs — модель.

Поэтому «types-candidate» ниже означает только явно ограниченные type headers,
а «substantive-candidate» — очередь семантического аудита, **не доказанные вызовы UI**.
Числа полностью независимых *по смыслу* файлов пока неизвестны. Их нельзя честно
вывести из одних #include. В частности data_session.h содержит forward declarations
HistoryView::Element, а data_session.cpp вызывает ClearGlobal при очистке.

Измерено: **493 файлов / 184545 строк**, 2339 внутренних и 1865 внешних include-рёбер.

| Автоматическая категория | Файлов | Что доказано |
|---|---:|---|
| no-ui | 211 | Нет транзитивного literal include-пути к выбранным GUI-группам |
| types-candidate | 9 | Прямые GUI-зависимости ограничены перечисленными type headers |
| substantive-candidate | 273 | Требуется чтение тел: присутствует более широкая GUI-зависимость |

Нет нетривиальных SCC среди literal file includes (0); forward declarations позволяют это C++. Межкаталожные циклы при этом существуют. Рёбра каталогов сохранены в graph.json: например data/main/history/api нельзя делать взаимозависимыми крейтами. Они остаются модулями tg-core по PLAN.md.

## Типы, которые пересекают границу

Числа — лексические упоминания / файлов, включая декларации и комментарии; это не число создаваемых объектов. Полный перечень мест — type_usage в graph.json.

| Тип | Упоминаний / файлов | Решение |
|---|---:|---|
| `TextWithEntities` | 586 / 101 | Раньше этапа 5 перенести value-модель Entity/offset/length; UTF-16 offsets обязательны. Rendering не переносить. |
| `TextWithTags` | 34 / 13 | Отдельная value-модель draft/editor tags; только формат, не редактор. |
| `QImage` | 118 / 34 | Нужны encoded image bytes, размеры и thumbnail descriptor; декодирование/пиксели — ImageService, не фиктивный QImage. |
| `QPixmap` | 1 / 1 | Оконный ресурс: отложить функции до этапа 5. |
| `QColor` | 100 / 21 | Value RGBA из схем/сериализации; palette binding отделить. |
| `QFont` | 13 / 2 | Настройки/описатель шрифта отделить от метрик и shaping. |
| `style::` | 91 / 24 | Вырезать зависимости от размеров/палитры из чистых моделей; style runtime — этап 5. |
| `Ui::Text::` | 139 / 26 | Entity parser и форматные значения раньше; layout/paint — этап 5. |
| `Ui::RpWidget` | 20 / 9 | Оставить UI-функцию вне ядра; narrow effect/callback вместо widget. |
| `Window::SessionController` | 133 / 30 | Navigation/confirmation port с ID и результатом, не указатель на окно. |
| `HistoryView::` | 245 / 28 | View invalidation как effect по MessageKey; элементы/геометрия — этап 7. |
| `ClickHandler` | 423 / 34 | Семантическое действие/цель ссылки вместо callback, захватывающего QWidget. |
| `QPainter` | 24 / 12 | Только чтение/отложить rendering; не нужен headless ядру. |

## Владение и ключи

Main::Domain владеет AccountStore (локальный storage index + generation).
Main::Account владеет MTP Instance, storage Account и Option<Main::Session>.
Main::Session владеет Data::Session и сервисами API; Data::Session — хранилищами.
SessionScopeId отделяет одинаковые wire IDs разных аккаунтов/поколений входа.
Не смешивать session_id MTProto и этот локальный токен жизни.

| Хранилище Data::Session | Ключ | Содержимое/индексы |
|---|---|---|
| Peers | PeerKey(scope, PeerId) | enum User/Chat/Channel, типизированные UserKey/ChatKey/ChannelKey views; один экземпляр общих полей |
| Messages | MessageKey(scope, FullMsgId, generation) | сообщения, separate non-channel MsgId index как _nonChannelMessages |
| Histories | PeerKey | порядок сообщений/blocks, read markers; без view elements |
| Documents | DocumentKey(scope, DocumentId) | метаданные, file location, ссылки на media service |
| Photos | PhotoKey(scope, PhotoId) | sizes/locations; bytes/media lease отдельно |
| Webpages/Games/Polls | соответствующий wire ID + scope | поля и ключи зависимых сообщений |
| Stories/Forum/Topics/Saved sublists | peer + story/topic ID | membership и read state; связи ключами |
| Dependency indices | ключ владельца → set ключей | _dependentMessages, random ID mapping, группы, обратные ссылки |

`not_null<PeerData*>` на границе становится проверенным PeerKey, не Option<&mut Peer>:
ensure создаёт placeholder, как peer() оригинала; loaded lookup — отдельная Option.
Ссылку `&Peer` берут кратковременно внутри операции. Удаляемые сообщения получают
поколение; wire key хранится отдельно для сети/диска. Peers обычно живут до clear
Session, но scope всё равно запрещает передачу ключа соседнему аккаунту.

Changes накапливает flags по ключу, realtime шлёт сразу, postponed — в исходной
точке flush. Event содержит ключ/флаги и при необходимости snapshot старых полей.
После освобождения mutable borrow вызывается tg-rpl, чтобы callback мог читать
соседние store. Ранние realtime-точки не разрешается автоматически откладывать до
конца RPC: для них операция делится на mutation и emit в точных местах C++.
Внешние callback не удерживают &mut Session; получают scope/ID и command sink.

Ниже **эскизы интерфейса**, не уже реализованный Rust. Прочитаны полные тела этих
функций и соответствующие участки data_session.h/data_changes.cpp. Все остальные
тела 493 файлов данным документом не объявляются прочитанными.

| Функция оригинала и существенный код | Предлагаемая запись Rust |
|---|---|
| `Session::peer`: `_peers.find(id)`; иначе `make_unique<UserData/ChatData/ChannelData>(this,id)` и emplace | `fn ensure_peer(&mut self,id:PeerId)->PeerKey { self.peers.ensure(self.scope,id) }`; enum создаётся по tag wire ID; отсутствие loaded не означает отсутствие placeholder |
| `Session::processUsers`: `result=nullptr; for(user:data.v) result=processUser(user); return result;` | `fn process_users(&mut self,users:&[MtpUser])->Option<UserKey> { let mut last=None; for user in users { last=Some(self.process_user(user)); } last }`; важно вернуть последний и None на пустом векторе |
| `Session::message(peerId,itemId)`: early nullptr при !itemId, затем messagesList(peerId), find | `fn message(&self,peer:PeerKey,id:MsgId)->Option<&Message> { if id.is_zero(){return None} self.messages.get(peer,id) }`; полнота FullMsgId и non-channel index сохраняются |
| `Session::photo`: find, иначе `_photos.emplace(id, make_unique<PhotoData>(this,id))` | `fn ensure_photo(&mut self,id:PhotoId)->PhotoKey { self.photos.ensure(self.scope,id) }`; соседний peer хранит PhotoKey, не Rc<PhotoData> |
| `Changes::Manager::updated`: realtime; при dropScheduled OR старых flags + erase + fire, иначе `_updates[data] |= flags` | `updated(key,flags,drop_scheduled)` возвращает/публикует упорядоченные события; старые флаги объединяются, pending удаляется до callback. Snapshot borrow закрывается до fire |

Пример работы с соседями: `let (peers, photos)=(&mut data.peers,&mut data.photos);`
затем проверка ключей и изменение только нужных полей. Операцию с API/Changes
лучше сделать методом Data::Session, возвращающим события, чем выдавать всем
модулям SessionContext с неограниченным доступом к mutable store.

Неудобства: два изменяемых сообщения одного store (reply/dependency), обход с
удалением, callbacks в середине изменения, живущие дольше Session PhotoMedia и
DocumentMedia (см. комментарий keepAlive), замена local MsgId серверным.
Решения: проверенный get_two_mut либо remove/operate/reinsert с явной защитой
от callback; сначала snapshot ключей при обходе; двухфазные операции только там,
где это сохраняет realtime-точки; отдельный media lease с bytes/loader без ссылки
на Session; переназначение ID атомарно обновляет все вторичные индексы.
Rc допускается для независимого media lease/rpl, не для каждой записи графа.
Data::Session::clear содержит специальный порядок stories/group calls и очистку
HistoryView::Element::ClearGlobal: перенести порядок моделей, view clear — effect.

## Очередь задач и сборочные границы

0. Этот срез: sparse/shared-media/user-photos/facade/cloud-blob, lang/countries,
   config/settings/logs. Готовые MTProto/Sender/runtime/rpl используются без новых
   моделей Peer/History. UI и Main::Session binding откладывается явно.
1. Value contracts: wire IDs/flags, UTF-16 text entities и data-independent типы.
2. Store interfaces и SessionScope; настоящие records переносятся из заголовков,
   затем ensure/lookup, Changes и вторичные индексы.
3. Модели peers/media/messages/history; API-обработчики после моделей.
4. Updates и API orchestration; Account/Session lifecycle; эффекты/мультиаккаунт.
5. Ayu integration и replay полного критерия.

Автоматическая раскладка ниже покрывает все 493 файла ровно по ledger LOC.
Это **предварительные границы чтения**, не обещание, что механическое присоединение
всех перечисленных .cpp уже соберётся. Для трёх крупных пар файлов части делятся
по функциям после аудита, не произвольным обрезанием строки. APIwrap уже содержит
круговые обращения: нужны contracts этапа 1–2 этой очереди. Последовательность
должна уточняться по телам; номера пакетов не заменяют dependency gate.

Каждый пакет принимается только с экспортом завершённых модулей, без fake types,
с map функция→тест. Если зависимость находится в будущем пакете, сначала выносится
реальный value contract из её заголовка либо narrow trait по фактическим вызовам;
иначе функция получает partial и переносится в указанный будущий пакет. Изобретать
ответ сервиса или no-op trait implementation ради сборки нельзя.

Проверки по группам: lang/countries — native plural/pack и Sender memory server;
data IDs — TL/native binary и перенесённые тесты; stores/changes — lifetime,
aliasing, flush/realtime и remap tests; history — synthetic TL updates + ordered
model snapshots; API — original C++ request capture + memory server; main/core —
virtual timers, lifecycle/keys and recorded transcripts; Ayu — те же transcripts
с ghost flags. UI-функции в каждом пакете откладываются до 5/6/7 по владельцу.

Всего предварительных пакетов: **35**.

| № | LOC | Файлы / часть | Зависимости, проверка, отложенное |
|---|---:|---|---|
| 01 | 5740 | `SourceFiles/lang/lang_cloud_manager.cpp`<br>`SourceFiles/lang/lang_cloud_manager.h`<br>`SourceFiles/lang/lang_file_parser.cpp`<br>`SourceFiles/lang/lang_file_parser.h`<br>`SourceFiles/lang/lang_hardcoded.h`<br>`SourceFiles/lang/lang_instance.cpp`<br>`SourceFiles/lang/lang_instance.h`<br>`SourceFiles/lang/lang_keys.cpp`<br>`SourceFiles/lang/lang_keys.h`<br>`SourceFiles/lang/lang_numbers_animation.cpp`<br>`SourceFiles/lang/lang_numbers_animation.h`<br>`SourceFiles/lang/lang_pch.h`<br>`SourceFiles/lang/lang_tag.cpp`<br>`SourceFiles/lang/lang_tag.h`<br>`SourceFiles/lang/lang_text_entity.cpp`<br>`SourceFiles/lang/lang_text_entity.h`<br>`SourceFiles/lang/lang_translator.cpp`<br>`SourceFiles/lang/lang_translator.h`<br>`SourceFiles/lang/lang_values.h`<br>`SourceFiles/lang/translate_mtproto_provider.cpp`<br>`SourceFiles/lang/translate_mtproto_provider.h`<br>`SourceFiles/lang/translate_provider.cpp`<br>`SourceFiles/lang/translate_provider.h`<br>`SourceFiles/lang/translate_url_provider.cpp`<br>`SourceFiles/lang/translate_url_provider.h`<br>`SourceFiles/countries/countries_instance.cpp`<br>`SourceFiles/countries/countries_instance.h`<br>`SourceFiles/countries/countries_manager.cpp`<br>`SourceFiles/countries/countries_manager.h` | MTProto/Sender, text contracts; native plural/pack; UI→5 |
| 02 | 5820 | `SourceFiles/data/data_peer_id.cpp`<br>`SourceFiles/data/data_peer_id.h`<br>`SourceFiles/data/data_msg_id.h`<br>`SourceFiles/data/data_changes.cpp`<br>`SourceFiles/data/data_changes.h`<br>`SourceFiles/data/data_user.cpp`<br>`SourceFiles/data/data_user.h`<br>`SourceFiles/data/data_user_names.cpp`<br>`SourceFiles/data/data_user_names.h`<br>`SourceFiles/data/data_user_photos.cpp`<br>`SourceFiles/data/data_user_photos.h`<br>`SourceFiles/data/data_chat.cpp`<br>`SourceFiles/data/data_chat.h`<br>`SourceFiles/data/data_chat_filters.cpp`<br>`SourceFiles/data/data_chat_filters.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 03 | 4604 | `SourceFiles/data/data_chat_participant_status.cpp`<br>`SourceFiles/data/data_chat_participant_status.h`<br>`SourceFiles/data/data_channel.cpp`<br>`SourceFiles/data/data_channel.h`<br>`SourceFiles/data/data_channel_admins.cpp`<br>`SourceFiles/data/data_channel_admins.h`<br>`SourceFiles/data/data_channel_earn.h`<br>`SourceFiles/data/data_photo.cpp`<br>`SourceFiles/data/data_photo.h`<br>`SourceFiles/data/data_photo_media.cpp`<br>`SourceFiles/data/data_photo_media.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 04 | 5919 | `SourceFiles/data/data_document.cpp`<br>`SourceFiles/data/data_document.h`<br>`SourceFiles/data/data_document_media.cpp`<br>`SourceFiles/data/data_document_media.h`<br>`SourceFiles/data/data_document_resolver.cpp`<br>`SourceFiles/data/data_document_resolver.h`<br>`SourceFiles/data/business/data_business_chatbots.cpp`<br>`SourceFiles/data/business/data_business_chatbots.h`<br>`SourceFiles/data/business/data_business_common.cpp`<br>`SourceFiles/data/business/data_business_common.h`<br>`SourceFiles/data/business/data_business_info.cpp`<br>`SourceFiles/data/business/data_business_info.h`<br>`SourceFiles/data/business/data_shortcut_messages.cpp`<br>`SourceFiles/data/business/data_shortcut_messages.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 05 | 5008 | `SourceFiles/data/components/credits.cpp`<br>`SourceFiles/data/components/credits.h`<br>`SourceFiles/data/components/ephemeral_messages.cpp`<br>`SourceFiles/data/components/ephemeral_messages.h`<br>`SourceFiles/data/components/factchecks.cpp`<br>`SourceFiles/data/components/factchecks.h`<br>`SourceFiles/data/components/gift_auctions.cpp`<br>`SourceFiles/data/components/gift_auctions.h`<br>`SourceFiles/data/components/location_pickers.cpp`<br>`SourceFiles/data/components/location_pickers.h`<br>`SourceFiles/data/components/passkeys.cpp`<br>`SourceFiles/data/components/passkeys.h`<br>`SourceFiles/data/components/promo_suggestions.cpp`<br>`SourceFiles/data/components/promo_suggestions.h`<br>`SourceFiles/data/components/recent_inline_bots.cpp`<br>`SourceFiles/data/components/recent_inline_bots.h`<br>`SourceFiles/data/components/recent_peers.cpp`<br>`SourceFiles/data/components/recent_peers.h`<br>`SourceFiles/data/components/recent_shared_media_gifts.cpp`<br>`SourceFiles/data/components/recent_shared_media_gifts.h`<br>`SourceFiles/data/components/scheduled_messages.cpp`<br>`SourceFiles/data/components/scheduled_messages.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 06 | 5851 | `SourceFiles/data/components/sponsored_messages.cpp`<br>`SourceFiles/data/components/sponsored_messages.h`<br>`SourceFiles/data/components/top_peers.cpp`<br>`SourceFiles/data/components/top_peers.h`<br>`SourceFiles/data/components/welcome_messages.cpp`<br>`SourceFiles/data/components/welcome_messages.h`<br>`SourceFiles/data/data_abstract_sparse_ids.h`<br>`SourceFiles/data/data_abstract_structure.cpp`<br>`SourceFiles/data/data_abstract_structure.h`<br>`SourceFiles/data/data_ai_compose_tones.cpp`<br>`SourceFiles/data/data_ai_compose_tones.h`<br>`SourceFiles/data/data_audio_msg_id.cpp`<br>`SourceFiles/data/data_audio_msg_id.h`<br>`SourceFiles/data/data_authorization.h`<br>`SourceFiles/data/data_auto_download.cpp`<br>`SourceFiles/data/data_auto_download.h`<br>`SourceFiles/data/data_birthday.cpp`<br>`SourceFiles/data/data_birthday.h`<br>`SourceFiles/data/data_boosts.h`<br>`SourceFiles/data/data_bot_app.cpp`<br>`SourceFiles/data/data_bot_app.h`<br>`SourceFiles/data/data_cloud_file.cpp`<br>`SourceFiles/data/data_cloud_file.h`<br>`SourceFiles/data/data_cloud_themes.cpp`<br>`SourceFiles/data/data_cloud_themes.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 07 | 5335 | `SourceFiles/data/data_community.cpp`<br>`SourceFiles/data/data_community.h`<br>`SourceFiles/data/data_credits.h`<br>`SourceFiles/data/data_credits_earn.h`<br>`SourceFiles/data/data_download_manager.cpp`<br>`SourceFiles/data/data_download_manager.h`<br>`SourceFiles/data/data_drafts.cpp`<br>`SourceFiles/data/data_drafts.h`<br>`SourceFiles/data/data_emoji_statuses.cpp`<br>`SourceFiles/data/data_emoji_statuses.h`<br>`SourceFiles/data/data_file_click_handler.cpp`<br>`SourceFiles/data/data_file_click_handler.h`<br>`SourceFiles/data/data_file_origin.cpp`<br>`SourceFiles/data/data_file_origin.h`<br>`SourceFiles/data/data_flags.h`<br>`SourceFiles/data/data_folder.cpp`<br>`SourceFiles/data/data_folder.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 08 | 5573 | `SourceFiles/data/data_forum.cpp`<br>`SourceFiles/data/data_forum.h`<br>`SourceFiles/data/data_forum_icons.cpp`<br>`SourceFiles/data/data_forum_icons.h`<br>`SourceFiles/data/data_forum_topic.cpp`<br>`SourceFiles/data/data_forum_topic.h`<br>`SourceFiles/data/data_game.cpp`<br>`SourceFiles/data/data_game.h`<br>`SourceFiles/data/data_group_call.cpp`<br>`SourceFiles/data/data_group_call.h`<br>`SourceFiles/data/data_groups.cpp`<br>`SourceFiles/data/data_groups.h`<br>`SourceFiles/data/data_histories.cpp`<br>`SourceFiles/data/data_histories.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 09 | 5600 | `SourceFiles/data/data_history_messages.cpp`<br>`SourceFiles/data/data_history_messages.h`<br>`SourceFiles/data/data_lastseen_status.h`<br>`SourceFiles/data/data_location.cpp`<br>`SourceFiles/data/data_location.h`<br>`SourceFiles/data/data_media_preload.cpp`<br>`SourceFiles/data/data_media_preload.h`<br>`SourceFiles/data/data_media_rotation.cpp`<br>`SourceFiles/data/data_media_rotation.h`<br>`SourceFiles/data/data_media_types.cpp`<br>`SourceFiles/data/data_media_types.h`<br>`SourceFiles/data/data_message_reaction_id.cpp`<br>`SourceFiles/data/data_message_reaction_id.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 10 | 3939 | `SourceFiles/data/data_message_reactions.cpp`<br>`SourceFiles/data/data_message_reactions.h`<br>`SourceFiles/data/data_messages.cpp`<br>`SourceFiles/data/data_messages.h`<br>`SourceFiles/data/data_passkey_deserialize.cpp`<br>`SourceFiles/data/data_passkey_deserialize.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 11 | 5937 | `SourceFiles/data/data_peer.cpp`<br>`SourceFiles/data/data_peer.h`<br>`SourceFiles/data/data_peer_bot_command.cpp`<br>`SourceFiles/data/data_peer_bot_command.h`<br>`SourceFiles/data/data_peer_bot_commands.cpp`<br>`SourceFiles/data/data_peer_bot_commands.h`<br>`SourceFiles/data/data_peer_colors.h`<br>`SourceFiles/data/data_peer_common.h`<br>`SourceFiles/data/data_peer_values.cpp`<br>`SourceFiles/data/data_peer_values.h`<br>`SourceFiles/data/data_poll.cpp`<br>`SourceFiles/data/data_poll.h`<br>`SourceFiles/data/data_poll_messages.cpp`<br>`SourceFiles/data/data_poll_messages.h`<br>`SourceFiles/data/data_premium_limits.cpp`<br>`SourceFiles/data/data_premium_limits.h`<br>`SourceFiles/data/data_premium_subscription_option.h`<br>`SourceFiles/data/data_pts_waiter.cpp`<br>`SourceFiles/data/data_pts_waiter.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 12 | 5816 | `SourceFiles/data/data_replies_list.cpp`<br>`SourceFiles/data/data_replies_list.h`<br>`SourceFiles/data/data_reply_preview.cpp`<br>`SourceFiles/data/data_reply_preview.h`<br>`SourceFiles/data/data_report.h`<br>`SourceFiles/data/data_saved_messages.cpp`<br>`SourceFiles/data/data_saved_messages.h`<br>`SourceFiles/data/data_saved_music.cpp`<br>`SourceFiles/data/data_saved_music.h`<br>`SourceFiles/data/data_saved_sublist.cpp`<br>`SourceFiles/data/data_saved_sublist.h`<br>`SourceFiles/data/data_search_calendar.cpp`<br>`SourceFiles/data/data_search_calendar.h`<br>`SourceFiles/data/data_search_controller.cpp`<br>`SourceFiles/data/data_search_controller.h`<br>`SourceFiles/data/data_send_action.cpp`<br>`SourceFiles/data/data_send_action.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 13 | 3894 | `SourceFiles/data/data_session.cpp`<br>`SourceFiles/data/data_session.h`; часть 1/2 | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 14 | 3893 | `SourceFiles/data/data_session.cpp`<br>`SourceFiles/data/data_session.h`; часть 2/2 | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 15 | 5924 | `SourceFiles/data/data_shared_media.cpp`<br>`SourceFiles/data/data_shared_media.h`<br>`SourceFiles/data/data_sparse_ids.cpp`<br>`SourceFiles/data/data_sparse_ids.h`<br>`SourceFiles/data/data_star_gift.cpp`<br>`SourceFiles/data/data_star_gift.h`<br>`SourceFiles/data/data_statistics.h`<br>`SourceFiles/data/data_statistics_chart.cpp`<br>`SourceFiles/data/data_statistics_chart.h`<br>`SourceFiles/data/data_statistics_lists.h`<br>`SourceFiles/data/data_stories.cpp`<br>`SourceFiles/data/data_stories.h`<br>`SourceFiles/data/data_stories_ids.cpp`<br>`SourceFiles/data/data_stories_ids.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 16 | 5650 | `SourceFiles/data/data_story.cpp`<br>`SourceFiles/data/data_story.h`<br>`SourceFiles/data/data_streaming.cpp`<br>`SourceFiles/data/data_streaming.h`<br>`SourceFiles/data/data_subscriptions.h`<br>`SourceFiles/data/data_thread.cpp`<br>`SourceFiles/data/data_thread.h`<br>`SourceFiles/data/data_todo_list.cpp`<br>`SourceFiles/data/data_todo_list.h`<br>`SourceFiles/data/data_types.cpp`<br>`SourceFiles/data/data_types.h`<br>`SourceFiles/data/data_unread_value.cpp`<br>`SourceFiles/data/data_unread_value.h`<br>`SourceFiles/data/data_wall_paper.cpp`<br>`SourceFiles/data/data_wall_paper.h`<br>`SourceFiles/data/data_web_page.cpp`<br>`SourceFiles/data/data_web_page.h`<br>`SourceFiles/data/notify/data_notify_settings.cpp`<br>`SourceFiles/data/notify/data_notify_settings.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 17 | 4672 | `SourceFiles/data/notify/data_peer_notify_settings.cpp`<br>`SourceFiles/data/notify/data_peer_notify_settings.h`<br>`SourceFiles/data/notify/data_peer_notify_volume.cpp`<br>`SourceFiles/data/notify/data_peer_notify_volume.h`<br>`SourceFiles/data/raw/raw_countries_bounds.cpp`<br>`SourceFiles/data/raw/raw_countries_bounds.h`<br>`SourceFiles/data/stickers/data_custom_emoji.cpp`<br>`SourceFiles/data/stickers/data_custom_emoji.h`<br>`SourceFiles/data/stickers/data_stickers.cpp`<br>`SourceFiles/data/stickers/data_stickers.h`<br>`SourceFiles/data/stickers/data_stickers_set.cpp`<br>`SourceFiles/data/stickers/data_stickers_set.h` | IDs/store contracts, Changes; model snapshots; rendering→5/7 |
| 18 | 5473 | `SourceFiles/history/history.cpp`<br>`SourceFiles/history/history.h` | peers/messages/stores; update replay; history view→7 |
| 19 | 4918 | `SourceFiles/history/history_item.cpp`<br>`SourceFiles/history/history_item.h`; часть 1/2 | peers/messages/stores; update replay; history view→7 |
| 20 | 4917 | `SourceFiles/history/history_item.cpp`<br>`SourceFiles/history/history_item.h`; часть 2/2 | peers/messages/stores; update replay; history view→7 |
| 21 | 5382 | `SourceFiles/history/history_item_components.cpp`<br>`SourceFiles/history/history_item_components.h`<br>`SourceFiles/history/history_item_edition.cpp`<br>`SourceFiles/history/history_item_edition.h`<br>`SourceFiles/history/history_item_helpers.cpp`<br>`SourceFiles/history/history_item_helpers.h`<br>`SourceFiles/history/history_item_reply_markup.cpp`<br>`SourceFiles/history/history_item_reply_markup.h` | peers/messages/stores; update replay; history view→7 |
| 22 | 5793 | `SourceFiles/history/history_item_text.cpp`<br>`SourceFiles/history/history_item_text.h`<br>`SourceFiles/api/api_attached_stickers.cpp`<br>`SourceFiles/api/api_attached_stickers.h`<br>`SourceFiles/api/api_authorizations.cpp`<br>`SourceFiles/api/api_authorizations.h`<br>`SourceFiles/api/api_blocked_peers.cpp`<br>`SourceFiles/api/api_blocked_peers.h`<br>`SourceFiles/api/api_bot.cpp`<br>`SourceFiles/api/api_bot.h`<br>`SourceFiles/api/api_chat_filters.cpp`<br>`SourceFiles/api/api_chat_filters.h`<br>`SourceFiles/api/api_chat_filters_remove_manager.cpp`<br>`SourceFiles/api/api_chat_filters_remove_manager.h`<br>`SourceFiles/api/api_chat_invite.cpp`<br>`SourceFiles/api/api_chat_invite.h`<br>`SourceFiles/api/api_chat_links.cpp`<br>`SourceFiles/api/api_chat_links.h`<br>`SourceFiles/api/api_chat_participants.cpp`<br>`SourceFiles/api/api_chat_participants.h` | peers/messages/stores; update replay; history view→7 |
| 23 | 5750 | `SourceFiles/api/api_cloud_password.cpp`<br>`SourceFiles/api/api_cloud_password.h`<br>`SourceFiles/api/api_common.cpp`<br>`SourceFiles/api/api_common.h`<br>`SourceFiles/api/api_communities.cpp`<br>`SourceFiles/api/api_communities.h`<br>`SourceFiles/api/api_compose_with_ai.cpp`<br>`SourceFiles/api/api_compose_with_ai.h`<br>`SourceFiles/api/api_confirm_phone.cpp`<br>`SourceFiles/api/api_confirm_phone.h`<br>`SourceFiles/api/api_credits.cpp`<br>`SourceFiles/api/api_credits.h`<br>`SourceFiles/api/api_credits_history_entry.cpp`<br>`SourceFiles/api/api_credits_history_entry.h`<br>`SourceFiles/api/api_earn.cpp`<br>`SourceFiles/api/api_earn.h`<br>`SourceFiles/api/api_editing.cpp`<br>`SourceFiles/api/api_editing.h`<br>`SourceFiles/api/api_filter_updates.h`<br>`SourceFiles/api/api_global_privacy.cpp`<br>`SourceFiles/api/api_global_privacy.h`<br>`SourceFiles/api/api_hash.cpp`<br>`SourceFiles/api/api_hash.h`<br>`SourceFiles/api/api_invite_links.cpp`<br>`SourceFiles/api/api_invite_links.h`<br>`SourceFiles/api/api_media.cpp`<br>`SourceFiles/api/api_media.h` | models/Sender; captured requests + memory server; confirmation/navigation→6 |
| 24 | 5958 | `SourceFiles/api/api_messages_search.cpp`<br>`SourceFiles/api/api_messages_search.h`<br>`SourceFiles/api/api_messages_search_merged.cpp`<br>`SourceFiles/api/api_messages_search_merged.h`<br>`SourceFiles/api/api_peer_colors.cpp`<br>`SourceFiles/api/api_peer_colors.h`<br>`SourceFiles/api/api_peer_photo.cpp`<br>`SourceFiles/api/api_peer_photo.h`<br>`SourceFiles/api/api_peer_search.cpp`<br>`SourceFiles/api/api_peer_search.h`<br>`SourceFiles/api/api_polls.cpp`<br>`SourceFiles/api/api_polls.h`<br>`SourceFiles/api/api_premium.cpp`<br>`SourceFiles/api/api_premium.h`<br>`SourceFiles/api/api_premium_option.cpp`<br>`SourceFiles/api/api_premium_option.h`<br>`SourceFiles/api/api_reactions_notify_settings.cpp`<br>`SourceFiles/api/api_reactions_notify_settings.h`<br>`SourceFiles/api/api_read_metrics.cpp`<br>`SourceFiles/api/api_read_metrics.h`<br>`SourceFiles/api/api_report.cpp`<br>`SourceFiles/api/api_report.h`<br>`SourceFiles/api/api_rich_tasks.cpp`<br>`SourceFiles/api/api_rich_tasks.h`<br>`SourceFiles/api/api_ringtones.cpp`<br>`SourceFiles/api/api_ringtones.h`<br>`SourceFiles/api/api_self_destruct.cpp`<br>`SourceFiles/api/api_self_destruct.h`<br>`SourceFiles/api/api_send_progress.cpp`<br>`SourceFiles/api/api_send_progress.h` | models/Sender; captured requests + memory server; confirmation/navigation→6 |
| 25 | 5958 | `SourceFiles/api/api_sending.cpp`<br>`SourceFiles/api/api_sending.h`<br>`SourceFiles/api/api_sensitive_content.cpp`<br>`SourceFiles/api/api_sensitive_content.h`<br>`SourceFiles/api/api_single_message_search.cpp`<br>`SourceFiles/api/api_single_message_search.h`<br>`SourceFiles/api/api_statistics.cpp`<br>`SourceFiles/api/api_statistics.h`<br>`SourceFiles/api/api_statistics_data_deserialize.cpp`<br>`SourceFiles/api/api_statistics_data_deserialize.h`<br>`SourceFiles/api/api_statistics_sender.cpp`<br>`SourceFiles/api/api_statistics_sender.h`<br>`SourceFiles/api/api_stickers_creator.cpp`<br>`SourceFiles/api/api_stickers_creator.h`<br>`SourceFiles/api/api_suggest_post.cpp`<br>`SourceFiles/api/api_suggest_post.h`<br>`SourceFiles/api/api_text_entities.cpp`<br>`SourceFiles/api/api_text_entities.h`<br>`SourceFiles/api/api_todo_lists.cpp`<br>`SourceFiles/api/api_todo_lists.h`<br>`SourceFiles/api/api_toggling_media.cpp`<br>`SourceFiles/api/api_toggling_media.h`<br>`SourceFiles/api/api_transcribes.cpp`<br>`SourceFiles/api/api_transcribes.h` | models/Sender; captured requests + memory server; confirmation/navigation→6 |
| 26 | 5677 | `SourceFiles/api/api_unread_things.cpp`<br>`SourceFiles/api/api_unread_things.h`<br>`SourceFiles/api/api_updates.cpp`<br>`SourceFiles/api/api_updates.h`<br>`SourceFiles/api/api_user_names.cpp`<br>`SourceFiles/api/api_user_names.h`<br>`SourceFiles/api/api_user_privacy.cpp`<br>`SourceFiles/api/api_user_privacy.h`<br>`SourceFiles/api/api_views.cpp`<br>`SourceFiles/api/api_views.h`<br>`SourceFiles/api/api_websites.cpp`<br>`SourceFiles/api/api_websites.h`<br>`SourceFiles/api/api_who_reacted.cpp`<br>`SourceFiles/api/api_who_reacted.h` | models/Sender; captured requests + memory server; confirmation/navigation→6 |
| 27 | 3416 | `SourceFiles/apiwrap.cpp`<br>`SourceFiles/apiwrap.h`; часть 1/2 | contracts предыдущих ступеней; native/scalar tests; GUI отдельно |
| 28 | 3415 | `SourceFiles/apiwrap.cpp`<br>`SourceFiles/apiwrap.h`; часть 2/2 | contracts предыдущих ступеней; native/scalar tests; GUI отдельно |
| 29 | 5213 | `SourceFiles/main/main_account.cpp`<br>`SourceFiles/main/main_account.h`<br>`SourceFiles/main/main_app_config.cpp`<br>`SourceFiles/main/main_app_config.h`<br>`SourceFiles/main/main_app_config_values.cpp`<br>`SourceFiles/main/main_app_config_values.h`<br>`SourceFiles/main/main_domain.cpp`<br>`SourceFiles/main/main_domain.h`<br>`SourceFiles/main/main_session.cpp`<br>`SourceFiles/main/main_session.h`<br>`SourceFiles/main/main_session_settings.cpp`<br>`SourceFiles/main/main_session_settings.h`<br>`SourceFiles/main/session/send_as_peers.cpp`<br>`SourceFiles/main/session/send_as_peers.h`<br>`SourceFiles/main/session/session_show.cpp`<br>`SourceFiles/main/session/session_show.h` | stores/API/storage; lifecycle and keys; windows→6 |
| 30 | 4886 | `SourceFiles/core/application.cpp`<br>`SourceFiles/core/application.h`<br>`SourceFiles/core/bank_card_click_handler.cpp`<br>`SourceFiles/core/bank_card_click_handler.h`<br>`SourceFiles/core/base_integration.cpp`<br>`SourceFiles/core/base_integration.h`<br>`SourceFiles/core/cached_webview_availability.h`<br>`SourceFiles/core/changelogs.cpp`<br>`SourceFiles/core/changelogs.h`<br>`SourceFiles/core/click_handler_types.cpp`<br>`SourceFiles/core/click_handler_types.h`<br>`SourceFiles/core/core_cloud_password.cpp`<br>`SourceFiles/core/core_cloud_password.h`<br>`SourceFiles/core/core_screenshot_protection.cpp`<br>`SourceFiles/core/core_screenshot_protection.h` | Account/Session services; virtual clocks; application/platform UI→6/10 |
| 31 | 5875 | `SourceFiles/core/core_settings.cpp`<br>`SourceFiles/core/core_settings.h`<br>`SourceFiles/core/core_settings_proxy.cpp`<br>`SourceFiles/core/core_settings_proxy.h`<br>`SourceFiles/core/crash_report_window.cpp`<br>`SourceFiles/core/crash_report_window.h`<br>`SourceFiles/core/crash_reports.cpp`<br>`SourceFiles/core/crash_reports.h` | Account/Session services; virtual clocks; application/platform UI→6/10 |
| 32 | 5604 | `SourceFiles/core/credits_amount.h`<br>`SourceFiles/core/current_geo_location.cpp`<br>`SourceFiles/core/current_geo_location.h`<br>`SourceFiles/core/deadlock_detector.h`<br>`SourceFiles/core/deep_links/deep_links_chats.cpp`<br>`SourceFiles/core/deep_links/deep_links_chats.h`<br>`SourceFiles/core/deep_links/deep_links_contacts.cpp`<br>`SourceFiles/core/deep_links/deep_links_contacts.h`<br>`SourceFiles/core/deep_links/deep_links_new.cpp`<br>`SourceFiles/core/deep_links/deep_links_new.h`<br>`SourceFiles/core/deep_links/deep_links_router.cpp`<br>`SourceFiles/core/deep_links/deep_links_router.h`<br>`SourceFiles/core/deep_links/deep_links_settings.cpp`<br>`SourceFiles/core/deep_links/deep_links_settings.h`<br>`SourceFiles/core/deep_links/deep_links_types.h`<br>`SourceFiles/core/external_control.cpp`<br>`SourceFiles/core/external_control.h`<br>`SourceFiles/core/file_location.cpp`<br>`SourceFiles/core/file_location.h`<br>`SourceFiles/core/file_utilities.cpp`<br>`SourceFiles/core/file_utilities.h`<br>`SourceFiles/core/launcher.cpp`<br>`SourceFiles/core/launcher.h` | Account/Session services; virtual clocks; application/platform UI→6/10 |
| 33 | 5940 | `SourceFiles/core/local_url_handlers.cpp`<br>`SourceFiles/core/local_url_handlers.h`<br>`SourceFiles/core/mime_type.cpp`<br>`SourceFiles/core/mime_type.h`<br>`SourceFiles/core/phone_click_handler.cpp`<br>`SourceFiles/core/phone_click_handler.h`<br>`SourceFiles/core/proxy_rotation_manager.cpp`<br>`SourceFiles/core/proxy_rotation_manager.h`<br>`SourceFiles/core/sandbox.cpp`<br>`SourceFiles/core/sandbox.h`<br>`SourceFiles/core/shortcuts.cpp`<br>`SourceFiles/core/shortcuts.h` | Account/Session services; virtual clocks; application/platform UI→6/10 |
| 34 | 5966 | `SourceFiles/core/ui_integration.cpp`<br>`SourceFiles/core/ui_integration.h`<br>`SourceFiles/core/update_channel.h`<br>`SourceFiles/core/update_checker.cpp`<br>`SourceFiles/core/update_checker.h`<br>`SourceFiles/core/update_keys.cpp`<br>`SourceFiles/core/update_keys.h`<br>`SourceFiles/core/update_verify.cpp`<br>`SourceFiles/core/update_verify.h`<br>`SourceFiles/core/utils.cpp`<br>`SourceFiles/core/utils.h`<br>`SourceFiles/core/version.h`<br>`SourceFiles/ayu/ayu_infra.cpp`<br>`SourceFiles/ayu/ayu_infra.h`<br>`SourceFiles/ayu/ayu_lang.cpp`<br>`SourceFiles/ayu/ayu_lang.h`<br>`SourceFiles/ayu/ayu_url_handlers.cpp`<br>`SourceFiles/ayu/ayu_url_handlers.h`<br>`SourceFiles/ayu/ayu_worker.cpp`<br>`SourceFiles/ayu/ayu_worker.h` | Account/Session services; virtual clocks; application/platform UI→6/10 |
| 35 | 5229 | `SourceFiles/ayu/features/filters/filters_cache_controller.cpp`<br>`SourceFiles/ayu/features/filters/filters_cache_controller.h`<br>`SourceFiles/ayu/features/filters/filters_controller.cpp`<br>`SourceFiles/ayu/features/filters/filters_controller.h`<br>`SourceFiles/ayu/features/filters/filters_utils.cpp`<br>`SourceFiles/ayu/features/filters/filters_utils.h`<br>`SourceFiles/ayu/utils/ayu_mapper.cpp`<br>`SourceFiles/ayu/utils/ayu_mapper.h`<br>`SourceFiles/ayu/utils/qt_key_modifiers_extended.h`<br>`SourceFiles/ayu/utils/rc_manager.cpp`<br>`SourceFiles/ayu/utils/rc_manager.h`<br>`SourceFiles/ayu/utils/telegram_helpers.cpp`<br>`SourceFiles/ayu/utils/telegram_helpers.h`<br>`SourceFiles/ayu/utils/windows_utils.cpp`<br>`SourceFiles/ayu/utils/windows_utils.h`<br>`SourceFiles/config.h`<br>`SourceFiles/logs.cpp`<br>`SourceFiles/logs.h`<br>`SourceFiles/settings.cpp`<br>`SourceFiles/settings.h`<br>`SourceFiles/stdafx.h` | models/API/lang; ghost/revision transcripts; Ayu UI→8 |

## Записанные сценарии без настоящего аккаунта

Не использовать реальные tdata/ключи/сообщения. Источник сценария — вручную
составленный synthetic TL: self, users/chats, history, updates, server errors.
Сценарий задаёт фиксированные часы, random, DC, msg IDs и ожидаемые команды UI.
Общий **языконезависимый** файл описывает входы, не парсится tg-tl-gen.
Сохраняются запросы до шифрования (constructor + bytes), response/update order,
модельные snapshots и события. Сравнивать сырые auth envelopes нельзя без
фиксированных key/salt/session/entropy; нормализация IDs задаётся заранее, не
выбрасывает invokeAfter/flags/содержание запросов.

Оригинал собирать отдельными CMake целями вне клона, как reference tools прошлых
этапов. Начать с pure request-building и update-processing функций, используя
настоящую схему, настоящие классы моделей и записанные TL ответы. Sender boundary
перехватывает запросы вместо сокета; ответ подаётся обратно по сценарию. Для
завершённого ядра порт подключается к настоящему memory MTProto server с synthetic
ключом/авторизованным состоянием; вход через Telegram не нужен.

Риск: Main::Session/APIwrap требуют Core::App, generated styles и UI сервисы;
сборка всего пути без UI может оказаться невозможной. Link-time gc не убирает
зависимости при разборе заголовков. Сначала пробовать маленькие оригинальные цели
и чистые функции. Если приходится менять тела или выдумывать Core/History классы,
это уже не независимый эталон. Такой сценарий маркируется «own tests + чтение»,
критерий сравнения запросов остаётся открытым. Варианты решения пользователя:
разрешить минимальную Qt/offscreen headless сборку реальных Core owners либо
перенести полное сравнение до каркаса приложения. Новых живых входов не делать:
Telethon 1.45 независимо подтвердил внешнюю блокировку.

## Что прочитано и что ещё нет

Для владения прочитаны data_session.h (хранилища/связи), перечисленные полные тела
peer/user/processUsers/message/photo, участок clear/keepAlive, Changes::Manager::updated/
sendRealtimeNotifications и Main::Domain::add. Заголовки lang и storage рассматриваются
в следующем срезе. Автоматический проход остальных файлов — чтение текста для
include/identifier inventory, **не семантический аудит тел**. Условные include не
вычислены препроцессором; в этом графе одновременно могут быть ветви разных OS.
Полные семантические числа no-UI/type-only/substantive и окончательные границы
всех пакетов пока не подтверждены. Открытая часть плана перечислена явно.

## Вопросы пользователю

1. Где закрепить ранние value-типы text entities: в существующем tg-ui без renderer,
   или разрешить отдельный общий крейт? В tg-core их дублировать не предлагается.
2. **Решено пользователем:** generation только у удаляемых сообщений; peers/media
   используют scope+wire ID без generation. Реализованы контракты в port/data-changes.
   Найденные media ID collisions/remap и отличие pointer order от typed-key order
   перечислены как design gates будущих моделей в
   [карте Changes](../crates/tg-core/docs/data-changes-map.md); обходных alias-моделей нет.
3. Если реальный C++ Core не удастся связать по частям, допустима ли минимальная
   Qt/offscreen цель с реальными owners для replay, либо отложить полный oracle до 6?
4. Предварительные пакеты ниже требуют чтения тел. Нужен отдельный срез семантического
   аудита всех 493 файлов до перевода store/API частей, чтобы подтвердить три числа
   зависимости от UI; текущие автоматические candidate counts не заменяют этот аудит.
