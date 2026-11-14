# Codex: PeerData common model

Ветка `port/data-peer`, base `959488b`, worktree
`/home/yarik/Projects/ayugram-rs-wt/data-peer`. Оригинал read-only
`0f2a31bb420de98f331990432265b131b8d836b5`. Собственный порт на Rust, без TDLib.

Перенесены common PeerData: defaults/loaded/full-info/block/TTL, names и индексы,
username delegation, about, barSettings/details, notify, translation/theme,
color/emoji IDs, photo metadata, stories, flags/предикаты/ограничения/slowmode,
stars/boost projections, миграция/forum/monoforum/group-call связи и pinned helpers
над уже существующим SharedMedia. Порядок NameUpdate/PeerUpdate, reentrant
callbacks, Unknown barSettings, full timeout >5000, timer_each и u16 nameVersion
проверены. Время инъецируется, тесты не используют sleep.

Peers — только компонент будущей Data::Session: один BTreeMap общих записей,
ensure_peer/peer_loaded и scoped доступ. Нет фиктивных UserData/ChatData/ChannelData
или enum с пустыми подклассами. Реальные borrowed PeerFields и обязательные
NameFields callbacks извлечены из настоящих полей/методов подклассов; будущие
02/03 поставляют их из своих stores. SessionScope связь и weak lifetime проверяются,
Main::Session/Account владельцы остаются 29. Forum/Monoforum keys обозначают
существующих владельцев, lookup передаёт реальный store, собственного Forum нет.
GroupCallKey — только истинный scope+CallId identity contract, модель позже в 08.

Мутации возвращают упорядоченные PeerEffect. Peers освобождает mutable borrow
до Changes/Flags fire; внешние API/UI/storage эффекты получают будущие настоящие
владельцы. Эти обработчики не представлены no-op реализациями. Полный replay
с настоящими владельцами не объявлен завершённым. Names fillNames composition
портирована с UTF-16 BTreeSet; обязательный NameTextServices не имеет production
fallback. UI text backend остаётся 5; labels даёт будущий lang 01, код Claude
не заимствован. ChatData::setName Zalgo/Ayu settings фильтр относится к 03,
общий entry принимает уже отфильтрованное значение и сохраняет empty→old name.

[Карта](../../crates/tg-core/docs/data-peer-map.md): **357 строк**, 300 done,
57 partial с адресами будущих пакетов, 0 пустых реализаций/тестовых ссылок.
В таблице отдельно тела/const overloads/inline/value contracts. Boundary тест
не означает исполнения отложенного C++ UI тела. 31 новых Rust tests,
0 новых ignored, без заглушек/todo!/unimplemented! в новом production коде.

## Пофайловый учёт

| Оригинал относительно SourceFiles | Статус | Реализация / остаток |
|---|---|---|
| data/data_peer.h | partial | Поля/контракты/предикаты; UI 5/6, wallpaper/FileOrigin 16, owners 29, subclasses 02/03 |
| data/data_peer.cpp | partial | Model phases/effects; UI 5/6, photo process 03, emoji parse/service 07, reactions 09, wallpaper 16 |
| data/data_flags.h | done | Flags/Change/essential/current/value, реентерабельность и initial snapshot |
| data/data_peer_common.h | done | StarsRating/Pending presence/equality |
| data/data_lastseen_status.h | done | Все constructors/conversions/bitfields/order/validation |
| data/data_birthday.h | partial | Birthday/operators/helpers; BirthdayText 01/5 |
| data/data_birthday.cpp | partial | Calendar helpers/timer; локализованный BirthdayText 01/5 |
| data/data_user_names.h | done | Username/UsernamesInfo |
| data/data_user_names.cpp | done | Все имена/active/editable/index rules |
| data/notify/data_peer_notify_settings.h | done | DefaultNotify/MuteValue/NotifySound/PeerNotifySettings |
| data/notify/data_peer_notify_settings.cpp | done | Unknown/default/updates/sounds/настоящий TL serialize |
| data/data_user.h | partial | Только flags + borrowed field contract; UserData 02 |
| data/data_chat.h | partial | Только flags + borrowed field contract; ChatData 03 |
| data/data_channel.h | partial | Только flags + borrowed field contract; ChannelData/sponsored/boosts 03 |
| data/data_chat_participant_status.h | partial | Добавлены ChatRestriction/ChatRestrictions, admin flags уже были; остальное 03 |
| data/data_types.h | partial | Добавлены EmojiStatusId/CallId, ранее scalar aliases; остальное 16 |
| data/data_emoji_statuses.h | partial | Только настоящие EmojiStatusCollectible data fields; service/timers 07 |
| data/data_session.h | partial | Только Peers component/common ensure/loaded contract; остальное 13/14 |
| data/data_session.cpp | partial | Только соответствующая Peers фаза; concrete constructors 02/03, остальное 13/14 |
| data/data_group_call.h | partial | Только CallId/key identity contract; модель/lifetime 08 |
| ui/image/image_location.h | partial | Headless metadata variants/valid/equality/peerPhoto; loader/serialize/cache/UI 5 |
| ui/image/image_location.cpp | partial | Только metadata semantics; остальные тела 5 |
| core/utils.h | partial | Только translitRusEng/rusKeyboardLayoutSwitch declarations; остальное 34/5 |
| core/utils.cpp | partial | Соответствующие чистые helper tables/bodies; остальные utils 34/5 |

Ledger: 24 затронутые строки — 7 done, 17 partial (включая ранее partial
participant-status/types). Это не объявление всех 24 файлов полностью готовыми.
Image metadata сохранены в headless tg-core, исходный ledger target tg-app-ui
не переписан; будущий UI должен пользоваться общими значениями, не дубликатами.
Новых зависимостей, версий, изменений Cargo.toml/Cargo.lock нет. Использованы
существующие tg-base/tg-rpl/tg-runtime/tg-scheme.

## Native и ограничения

[Harness/инструкция](../../crates/tg-core/tests/reference-tools/data-peer/README.md),
[manifest](../../crates/tg-core/tests/fixtures/data-peer/manifest.json).
Отдельная CMake GCC15.3 x86_64 -O2 / Qt6.11.2 Core цель вне клона, настоящие
headers и неизменённые исходные тела pure helpers, data_user_names.cpp,
lib_base/lib_tl; QDate clock link-wrap с фиксированным настоящим QDate.
Два запуска дали **4247 одинаковых строк**, Rust сравнил их с fixture.
Подтверждены Birthday/Lastseen/Flags, 124 enum values, UsernamesInfo append,
registration, CRC/translit/keyboard, Qt normalization samples, RestrictionCheckResult,
Storage/ImageLocation equality, Qt case folding, MTP color/profile conversions.
Это сравнение вычислений/значений, не disk interoperability или live Telegram.

Полный data_peer.cpp -fsyntax-only с реальными lib_* / Qt include roots доходит
до lang/lang_keys.h и требует generated lang_auto.h; далее остаются UI/styles и
Main/Session/API. Поддельные headers/owners не вводились. Невозможность будущей
полной Qt headless сборки не доказана. PeerData state, subclass-dependent
предикаты/amRestricted, effect order/Changes/name reentrancy — **own tests + чтение**,
не запуск на настоящей C++ Session. Полная Unicode production normalization,
UI/paint/cache/click и API/storage обработчики перечисленных эффектов не проверены.

Сохранённые особенности/расхождения:

- BirthdayAge(29.02.2000,01.01.2023)=23 из-за invalid QDate anniversary.
- UsernamesInfo после удаления editable и append ставит index=0; inactive
  editable может оставить stale index вне vector. Rust сохраняет индекс, но
  getter паникует вместо C++ UB; последний сценарий не запускался native.
- Lastseen unsigned threshold и signed bitfield promotions сохранены, strict
  FromSerialized отвергает max OnlineTill. Signed overflow края фиксированы
  wrapping по измеренному GCC/x86_64 O2 результату; другие C++ платформы не доказаны.
- slowmode scalar проверяет Creator, producer — нет; combine дублирует false.
  nameVersion wrap 0/1 снова подавляет начальное Name уведомление.
- UnavailableReason Compute/Extract empty и IgnoreSensitiveMark=true, user
  allowsForwarding=true — исходные Ayu тела, не упрощение порта.
- Emoji collectible equality — shared-pointer identity (Rust Rc::ptr_eq).
  Storage/ImageLocation comparison игнорирует access hash/ref/dimensions.
- UTF-16 индекс/первый QChar сохранены, но входные Rust UTF-8 имена не представляют
  lone surrogate QString. Полная Unicode normalization production binding остаётся 5.
- Внешний mutation/effect boundary — сознательная замена C++ прямых вызовов
  сервисов без fake owners; их реальные реакции/жизненный цикл пока не доказаны.
- Никакого обхода media-ID remap/collision из прошлой задачи не добавлено;
  design gate 03/16 остаётся, peers без generation, как согласовано.

## Проверки и интеграция

WORKSPACE_CHECK_PENDING

Области Claude (lang/countries, credits_amount, tg-ui/codegen/runtime/mtproto)
и агентов port/tdata-settings, port/base-linux, docs-only stage4 audit не изменены.
Их ветки не интегрировались. Интеграция только port/data-peer в main после проверок,
пуш main и собственной ветки в gramtele без force. Оригинал clean, tdata не открывалась.
Следующий зависимый пакет — настоящие UserData 02, затем Chat/Channel 03; UI/owners
с адресами выше. Дополнительных блокирующих вопросов пользователю нет.
