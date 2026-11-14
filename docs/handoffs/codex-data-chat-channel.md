# Codex: группы и каналы, пакет 03

Base main `1038c32` (data-peer `9b7013a` и tdata-settings уже приняты).
Ветка `port/data-chat-channel`, worktree
`/home/yarik/Projects/ayugram-rs-wt/data-chat-channel`. Оригинал read-only
`0f2a31bb420de98f331990432265b131b8d836b5`, 8 файлов задания/4299 LOC прочитаны полностью.

ChatData/ChannelData владеют одним PeerData в единственном Peers store.
UserBase — прежний общий срез, не фиктивный UserData: полноценный UserData ещё02.
Перенесены common/subtype fields/defaults/flags, участники/админы/боты на scoped keys,
rights/restrictions/canX, username, name с исходным Ayu Zalgo filter, slowmode/boosts,
migration/forum/monoforum/community flags, applyEditAdmin/applyEditBanned и все
ApplyChat/Channel/CommunityUpdate model phases. Есть настоящий BotCommand/ChatBotCommands,
ReactionId value/TL и AllowedReactions, StickerSetIdentifier и LocationPoint metadata.
TextWithEntities/lang/countries код не заимствован. Новых Rust dependencies/версий,
изменений Cargo.toml/Cargo.lock нет.

Мутации выполняются фазами на Peers: mutable borrow закрывается до Flags/Changes
или внешнего эффекта. После callbacks следующие шаги читают актуальные поля.
Ctor ChatData сохраняет CallNotEmpty subscription раньше клиентских подписок.
ChannelAdminChanges/MemberRankChanges explicit finish соответствует месту исходного
scoped destructor; owner отдаёт History notification после освобождения borrow.
Произвольный drop незавершённого batch/panic-unwind не эквивалентен C++ RAII;
все production owner paths здесь явно вызывают finish. Native этого пути нет.

[Карта](../../crates/tg-core/docs/data-chat-channel-map.md): **337 строк**,
287 done +50 partial с указанными будущими пакетами,0 пустых реализаций/ссылок на тест.
318 строк — тела/inline/overloads 8 исходных файлов,19 — отдельные вспомогательные
контракты. `done` обозначает model/pure фазу; boundary test не исполняет будущий handler.
19 новых tests,0 новых ignored, без sleep; время передаётся явно.

## Восемь файлов задания

| Оригинал относительно SourceFiles/data | Ledger | Реализация / остаток |
|---|---|---|
| data_chat.h | partial | Настоящий ChatData/keys/flags/коллекции; GroupCall08 и внешние owners |
| data_chat.cpp | partial | Все model phases/updates, Zalgo; call08, photo03-media/04, reaction09, History18/API/Session29, text/UI5/6 |
| data_channel.h | partial | Настоящие ChannelData/MegagroupInfo; PtsWaiter12, Forum08/Monoforum12/Community07 owners, Ui::BotVerifyDetails5 |
| data_channel.cpp | partial | Все scalar/mutation/update phases; соответствующие owner bindings07/08/09/12/13/16/18/23/29/5/6 |
| data_channel_admins.h | done | Оба настоящих scoped batch-контракта, explicit finish |
| data_channel_admins.cpp | done | add/remove/feed, creator deltas, self-only suppression, History effect |
| data_chat_participant_status.h | partial | Rights/info/options/arrays/SendError values и peer send checks; Thread16, localized/UI helpers01/5/6 |
| data_chat_participant_status.cpp | partial | Все rights TL conversions, send unions/ListOfRestrictions/CanSend peer; Thread16 и localized/prepared-file/toast paths01/5/6 |

Другие ledger строки: BotCommand и BotCommands .h/.cpp done (4 файла);
ReactionId .h/.cpp partial09 (оставшиеся query/entity helpers);
LocationPoint .h/.cpp partial09/5 (format/hash/ComputeLocation ещё не портированы);
telegram_helpers.h/.cpp partial35 (только filterZalgo). Headless pure filter живёт
в tg-core/src/ayu; полный ledger target ayu-core сохранён. Peer/Session/Types прежние
partial остаются: тут добавлены model/AllowedReactions/concrete-record/value срезы,
полные owners/типы не объявлены готовыми. Всего23 учтённые строки:6 done,17 partial.

## Native и границы

[Harness и воспроизведение](../../crates/tg-core/tests/reference-tools/data-chat-channel/README.md),
[manifest](../../crates/tg-core/tests/fixtures/data-chat-channel/manifest.json).
CMake вне клона: GCC15.3 x86_64 O2 Qt6.11.2 Core, настоящие headers,
неизменённые rights CPP bodies/send unions, настоящие generated scheme classes
и TypeCreator/lib_base/lib_tl. Из ChannelData::setFlags исполняется только
неизменённая чистая normalization фаза before diff; IsRestrictedForever — исходный
inline. Это не запуск полного ChannelData с фиктивной Session.

**481 строка**, два native запуска побайтно совпали, Rust сравнение PASS:
37 rights masks,1 send unions,67 flags normalization masks,5 forever values,
365 Unicode Mn ranges и6 Zalgo inputs. FilterZalgo body/pattern исполняется настоящий:
Qt/PCRE классифицировал каждый Unicode scalar, production Rust использует зафиксированные
365 диапазонов. 3 scalar Mn → U+2060 по каждому captured UTF-16 unit; supplementary Mn
и bidi controls проверены. Иные Qt/Unicode версии и lone surrogate QString не доказаны.

Полные classes/Session, inherited predicates/amRestricted, state/TL ordering и
reentrant callbacks — **own tests + чтение**. Full data_channel.cpp syntax probe
требует отсутствующий tl/expected.hpp; предыдущий data-peer probe с реальными
headers также требует generated lang_auto.h/UI/styles. Fake-заголовков/тел нет;
невозможность будущей честной Qt headless сборки не утверждается.

GroupEffect — упорядоченный service call boundary, не no-op apply:
реальные photo/notify/stories/reactions, History loaded checks, windows/UI queue,
API revenue timeout, Main::Session lifecycle ещё не подключены. GroupCall08 только
keys/registration/migration commands; Forum08/SavedMessages12/Community07 только
owner keys/команды, не модели-заменители. PtsWaiter12 не подменён искусственным pts.
Theme callbacks обязаны дать настоящие emoji/theme facts; name indexing требует
реального backend5 и labels01. Живых аккаунтов/сети Telegram/tdata не использовалось.

## Исходные особенности и отличия

Сохранены и протестированы двойные изменения adminsCount в applyEditAdmin,
emplace с сохранением старых self entries, rank после удаления админа и self-only
History suppression; monoforum null оставляет old link и меняет disabled;
Skipped не присваивает версию, full participants всё равно обрабатывается;
invalidate не очищает ranks/default bans/lastAuthors/markupSenders; boosts fire
только при смене unrestricted и раньше Rights|Slowmode; stories обходят setFlags;
ChannelFull сохраняет HasStarsPerMessage при stars→0. ReactionId Custom(0) не пуст,
но To/FromMTP теряет его variant — исходное поведение. ProcessJoinRequests не
переносится в TL rights, ManageLinkedPeers/ManageWelcomeMessages имеют другие TL
bit positions, проверено native. Известный x86 realtime shift Changes не исправлялся:
новые tests для Members/Rights используют postponed delivery/flush.

Замены владения: pointers→scoped keys, pointer-order→key Ord, мутация→effect phases,
scoped destructor→explicit finish. Полные внешние реакции и panic-unwind не доказаны. Signed overflow счётчиков/
version/slowmode выражен Rust wrapping; C++ UB края вне проверенного домена
native/state сценариев не подтверждены. Forum detach/retain/recreate lifetime
требует реального owner08: один owner key сам по себе не доказывает время жизни
старого объекта во вложенном callback.
Media ID remap/collision gate03/04/16 не обходился, generation только messages;
GroupCall actual record/lifetime остаётся08.

## Проверки и интеграция

Все три команды PORTING.md через nix develop PASS:

```text
cargo fmt --check                                      PASS
cargo clippy --workspace --all-targets -- -D warnings   PASS
cargo test --workspace                                PASS
769 successful results, 0 failed, 3 existing ignored
19 new data_chat_channel tests                        PASS
native/map/manifest/Mn table + git diff --check         PASS
```

769 включает существующий дочерний процессный результат (768 основных +1 child).
Root source и собственный worktree проверены перед коммитом. Интеграция только
port/data-chat-channel в main; push main и ветки в gramtele без force.
Области Claude (lang/countries/credits/tg-ui/codegen/runtime/mtproto), storage settings,
base-linux и docs-only stage4 audit не менялись/не интегрировались.
Сообщённую пользователем гонку ObjectAsync::drop оставили владельцу tg-runtime;
в этой локальной полной проверке она не воспроизвелась. Вопросов, блокирующих этот
срез, нет. Следующие зависимости перечислены выше, их готовность не подразумевается.
