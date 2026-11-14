# port/data-media — модели фото и документов, пакет 04

База main `0c38855`, worktree `/home/yarik/Projects/ayugram-rs-wt/data-media`.
Оригинал только чтение `0f2a31bb420de98f331990432265b131b8d836b5`.
Чужие branches/файлы, включая port/localstorage, base-linux, docs-only audit и
области Claude (lang/countries/credits/tg-ui/codegen/runtime/mtproto), не менялись.
Новых зависимостей/manifest/lock изменений нет. Пользовательская tdata и аккаунты
не открывались. Поздний main750204e уже содержит принятый global localstorage;
его нужно сохранить при обычной интеграции, не интегрировать private branch Claude.

## Реализация

Прочитаны восемь файлов задания (4338 LOC), model call sites в data_session.cpp,
используемые MIME/filename/location factory/helper тела. PhotoData/DocumentData:
remote/IDs/sizes/flags/upload, предикаты, порядок/повторное применение всех
атрибутов, sticker/custom emoji/voice/round/song/video, duration/waveform,
streaming/good-thumbnail state, thumbnail metadata, filename/MIME/glob enforcement,
save-name/filter/collision правила. FileLocation — настоящие path/size/modified
model fields и ветки проверки/remote registration/cache marker; реальные platform
inspection/bookmark/localstorage lookup — обязательный FileLocations service.
Пустого default production provider нет. Write/remove/repaint — typed effects.

TL photo/document apply идёт в исходном порядке. Documents принимает реальный
FileLocations service; model remote phase читает/переносит локальные metadata.
Photos/Documents — ownership components Data::Session, как Peers, без fake полной
Main::Session. Peers::set_userpic_photo связывает processPhoto, PhotoData::peer и
Changes. GroupEffect::ProcessPhoto host может направить в этот метод; остальные
UI/API bindings остаются владельцам соответствующих пакетов.

PhotoMedia/DocumentMedia — самостоятельные leases, без Session/record reference.
Они содержат bytes; PhotoMedia также encoded image slots/goodFor и video bytes.
Кэширование живого view слабое, lease переживает scope/model. Pixel-loaded state
не подменяется наличием bytes. Настоящие loader/cancellation/decoded pixel slots
остаются storage/file_download3/4 и этап5, явно partial.

## Identity, lookup и convert

PhotoKey/DocumentKey(scope, wire_id) сохранены для wire lookup. Добавлены отдельные
PhotoRecordKey/DocumentRecordKey(scope, serial), заменяющие pointers. Serial —
непереиспользуемая identity записи в scope, **не generation медиа** и не wire ID.
Никогда не сериализуется в TL/кэш/tdata. Model::key() стабилен; id()/wire_key()
меняются. Это уточнение первоначального эскиза stage4-plan по настоящим телам
convert, отражённое в карте и parallel-work; файл чужого docs-only audit не менялся.

Исходный C++ конфликт сохранён: map[A]=P, map[B]=Q; P→B оставляет оба объекта
с id=B, P под A и Q под B. Повторный P→C ищет map[P.id=B] и перемещает Q в C,
потом P.id=C. Под A остаётся P(id=C), под C — Q(id=B). Нет молчаливого merge/
удаления/исправления индекса. Это own tests+чтение data_session.cpp:3879/4158,
не native полного Session. Один wire key не мог бы выразить эти два объекта.

Lookup, записи и все реализованные secondary indices согласованы под одним
borrow. PhotoItems reference counts, DocumentItems sets, scheduled cache-clear
keys, video quality links, goodThumbnailPhoto и leases сохраняют record identity.
Затем borrow закрывается до fire. Проверены callbacks с mutable доступом к store,
включая registerDocumentItem music effect до вставки в index. SavedGifs09
membership передаёт реальный Stickers owner как projection, а не fake vector.
При changed ID сбрасываются upload/status, destination apply предшествует original,
затем оба moveIfEmpty и WriteSavedGifs идут в порядке C++.

`apply_cache_event` исполняет настоящие tg-cache put/putIfEmpty/moveIfEmpty,
с реальными тегами. Его тест открывает encrypted Database с виртуальными
таймерами и проверяет значения/теги, перенос и занятый destination. Unhandled
owner/file/UI commands возвращают false и обязаны идти своему host, не теряются
через пустую apply-функцию.

## Native и проверки

Отдельная CMake цель вне клона `/tmp/ayugram-data-media-native`: неизменённые
standalone тела оригинала, настоящий MimeType header/QMimeDatabase, QSize,
полный оригинальный core/file_location.cpp и lib_base algorithm/debug/integration/
assertion. Два запуска побайтно совпали: **1706 строк =1688 сравнений +18 registry
inputs**. Фикстура/SHA256 и воспроизведение/proof scope находятся в
crates/tg-core/tests/{fixtures,reference-tools}/data-media.

Native покрывает waveform encode/decode/padding, invalid/extreme dimensions,
finite/nonfinite duration, quality helper, расширения QMimeDatabase, Known MIME/
alias/glob/filter, 81 filter case, file-location constructor/check и весь Unicode
scalar domain для ASCII CI matching. Кроме ASCII ровно два fold: ſ→s, K→k;
обычный lowercase недостаточен для .opus/.svg/theme/security suffixes. Это
перенесено, без новой Unicode dependency. Native выявил исправленные ошибки
переноса: zero-side QSize возвращает рамку; regex \s без UCP использует ASCII.

Full data_document.cpp probe с реальными includes/generated scheme требует
lang_auto.h (`/tmp/data-media-full-probe.log`). Fake header/model body не добавлялся.
Полные состояния, классификация по атрибутам, TL, Session/remap, qualities и
reentrant callbacks — **own tests + чтение**, не native классов. setattributes
не чистая fresh-classification функция: сохраняет старый type/additional и
обращается к owner/cloud paths. Windows/macOS file/bookmark поведение не доказано.

33 новых теста:22 data_media,10 media_stores,1 media_cache; последние targeted
clippy/test PASS. Три обязательные финальные workspace проверки и интеграция
ещё выполняются; результат будет дописан после фактического завершения.

Карта data-media-map.md: **307 строк восьми файлов,188 done/119 partial,
0 pending**. Смежные функции/owner contracts перечислены отдельно. done —
указанная model/pure фаза с тестом, partial имеет адрес будущего пакета.
Все 8 файлов ledger partial из-за loader/UI remainder, не объявлены fully done.

## Остаток и отличия

Actual network/cache loaders и task cancellation/FileOrigin — storage/file_download
этап3/4. Images/QImage/good thumbnails/clip/reply preview/clipboard — этап5/6;
History/Media eligibility/video cover18, Stickers/Webpage09, save captions01;
platform MIME registry/inspection/bookmark/persistence adapters33. PLAN.md не
задаёт замену QMimeDatabase, поэтому backend — обязательный MimeDatabase service,
без guessed extension list/пустого default provider. Native registry — только
входные данные теста. Source core/mime_type остальное — partial33.

Сохранены source quirks: type/dimensions/filename не сбрасываются всеми attrs;
waveform padding; streaming No до нового setattributes; premium/text-color reset;
Webm default dimensions; strict owner size<10MiB и inclusive payload<=10MiB;
Photo refresh не трогает video refs, пустая ref не стирает StorageFileLocation;
big-file dc shift <<16 &FF00 обнуляет dc contribution. Независимый SHA256 known
answer фиксирует LE slices cache keys. Никакие форматы сети/кэша не менялись.

Адаптации: explicit LE вместо host unaligned uint16; record keys вместо pointers;
transient projections и typed commands вместо удержания Session; QString lone
surrogates не представимы Rust String. Equal-quality/equal-size unstable sort
identity order Rust/libstdc++ не native-proven. C++ signed overflow/невалидные
Geo float casts имеют UB; Rust wrapping/saturating края не объявляются доказанной
эквивалентностью. Полный Session teardown из callback/late loader delivery требует
настоящего owner29/loader integration, здесь не доказан. Известную гонку runtime
ObjectAsync::drop оставили Claude; его файлы не менялись.

Предварительная полная проверка на базе0c38855: fmt --check PASS, clippy --workspace --all-targets -D warnings PASS, test --workspace PASS (802 успешных результата,0 failed,3 прежних ignored). После совмещения с уже принятым main750204e проверки повторяются.
