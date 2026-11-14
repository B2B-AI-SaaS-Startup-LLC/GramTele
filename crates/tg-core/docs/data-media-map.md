# Фото и документы — пакет 04

База main `0c38855`; оригинал `0f2a31bb420de98f331990432265b131b8d836b5`.
Полностью прочитаны восемь файлов задания (4338 LOC). Каждое cpp тело,
перегрузка и header-only inline — отдельная строка; cpp-строка покрывает и
декларацию из h. `done` относится к указанной model/pure фазе, не к полному
native replay класса. Deferred UI/loader bindings явно partial с будущим адресом.

Tests без префикса — [data_media.rs](../tests/data_media.rs);
[media_stores.rs](../tests/media_stores.rs) и [media_cache.rs](../tests/media_cache.rs)
проверяют owner/remap и настоящую encrypted cache queue. [Native proof scope и
воспроизведение](../tests/reference-tools/data-media/README.md).
Full data_document.cpp probe требует generated lang_auto.h; состояния/атрибуты/TL
— **own tests + чтение**, не native полного класса. Native использует реальные
чистые тела, QMimeDatabase и полный core/file_location.cpp, без подмен Session.

## Уточнённый контракт владения после чтения convert

PhotoKey/DocumentKey(scope, wire_id) сохраняются как lookup selectors и не
используются вместо указателей. PhotoRecordKey/DocumentRecordKey(scope, serial)
— отдельная стабильная identity записи; serial не переиспользуется в scope,
это не generation медиа и никогда не передаётся в TL/кэш/tdata. Model::id и
wire_key() — изменяемое поле wire ID, отдельно от стабильного key().

C++ A→занятый B сохраняет map[A]=P, map[B]=Q с P.id=B и Q.id=B. Повторный P→C
находит map[P.id=B] и перемещает Q в C: P.id=C под A, Q.id=B под C. Порт сохраняет
эту исходную особенность, не сливает объекты и не исправляет индекс по догадке.
Lookup, records и индексы меняются под одним borrow; затем он закрывается до fire.
PhotoItems counts/DocumentItems sets/scheduled photo clear, quality links, good
thumbnail photo и leases остаются привязаны к record identity. SavedGifs09
membership приходит от настоящего owner как projection; WriteSavedGifs и оба
moveIfEmpty идут в исходном порядке. Реальный tg-cache adapter реализован и
проверен на encrypted Database с виртуальными таймерами (media_cache.rs).

Photos/Documents — настоящие ownership components Data::Session, как Peers,
без фиктивной полной Main::Session. Scoped stores проверяют живость/чужие keys;
clear не переиспользует record identity. Peers::set_userpic_photo связывает
processPhoto, PhotoData::peer и Changes; peer userpic ID остаётся wire scalar,
как в оригинале, и не переназначается молча при convert.

FileLocation имеет настоящие path/size/modified model fields; platform inspection,
bookmark resolution и localstorage read — обязательный FileLocations service,
без default/пустого production provider. Запись/repaint — typed commands.
Byte leases не содержат Session/record reference; PhotoMedia хранит encoded image
slots/goodFor и video bytes, но loaded-by-pixels не имитируется наличием bytes.
Actual loaders/cancellation — storage/file_download3/4, decoded Image/clip5.
MIME registry также обязательный caller service, не список примерных расширений.
PLAN.md не предписывает его replacement dependency; platform backend —33,
новых зависимостей нет. Native registry fixture используется только в тестах.

## Восемь файлов

| Оригинал | Статус | Rust / фаза | Тест или будущий пакет | Остаток |
|---|---|---|---|---|
| `data_photo.cpp:32 ValidatePhotoImage` | partial(этап 5/6) | `—` | future image/preview package | real pixels/History preview |
| `data_photo.cpp:43 PhotoData::PhotoData` | done | `data_media_store.rs::Photos + PhotoData::new/key` | media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime | stable record handle + wire lookup; no fake Session |
| `data_photo.cpp:48 PhotoData::~PhotoData` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:55 PhotoData::setFields` | done | `data_photo.rs::set_fields` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:61 PhotoData::setExtendedMediaPreview` | partial(storage/file_download этап 3/4; pixels этап 5) | `data_photo.rs::set_extended_media_preview` | photo_fallback_preview_video_and_references | model/metadata phase implemented; actual CloudFile adapters absent |
| `data_photo.cpp:77 PhotoData::extendedMediaPreview` | done | `data_photo.rs::extended_media_preview` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:81 PhotoData::extendedMediaVideoDuration` | done | `data_photo.rs::extended_media_video_duration` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:87 PhotoData::owner` | done | `data_media_store.rs::Photos + PhotoData::new/key` | media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime | stable record handle + wire lookup; no fake Session |
| `data_photo.cpp:91 PhotoData::session` | done | `data_media_store.rs::Photos + PhotoData::new/key` | media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime | stable record handle + wire lookup; no fake Session |
| `data_photo.cpp:95 PhotoData::automaticLoadSettingsChanged` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:104 PhotoData::load` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:111 PhotoData::date` | done | `data_photo.rs::date` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:115 PhotoData::loading` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:119 PhotoData::validSizeIndex` | done | `data_photo.rs::valid_size_index` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:129 PhotoData::existingSizeIndex` | done | `data_photo.rs::existing_size_index` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:139 PhotoData::hasExact` | done | `data_photo.rs::has_exact` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:143 PhotoData::loading` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:155 PhotoData::failed` | done | `data_photo.rs::failed` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:160 PhotoData::clearFailed` | done | `data_photo.rs::clear_failed` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:164 PhotoData::location` | done | `data_photo.rs::image_location` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:168 PhotoData::SideLimit` | done | `PhotoData::SIDE_LIMIT` | native_pure_functions |  |
| `data_photo.cpp:172 PhotoData::size` | done | `data_photo.rs::size` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:185 PhotoData::imageByteSize` | done | `data_photo.rs::image_byte_size` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:193 PhotoData::displayLoading` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:202 PhotoData::cancel` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:208 PhotoData::clearLocalCache` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:223 PhotoData::progress` | partial(storage/file_download этап 3/4) | `data_photo.rs::uploading_progress` | upload_progress_and_album | upload branch only |
| `data_photo.cpp:236 PhotoData::cancelled` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:241 PhotoData::setWaitingForAlbum` | done | `data_photo.rs::set_waiting_for_album` | upload_progress_and_album |  |
| `data_photo.cpp:247 PhotoData::waitingForAlbum` | done | `data_photo.rs::waiting_for_album` | upload_progress_and_album |  |
| `data_photo.cpp:251 PhotoData::loadOffset` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:256 PhotoData::uploading` | done | `data_photo.rs::uploading` | upload_progress_and_album |  |
| `data_photo.cpp:260 PhotoData::getReplyPreview` | partial(этап 5/6) | `—` | future image/preview package | real pixels/History preview |
| `data_photo.cpp:270 PhotoData::getReplyPreview` | partial(этап 5/6) | `—` | future image/preview package | real pixels/History preview |
| `data_photo.cpp:276 PhotoData::replyPreviewLoaded` | partial(этап 5/6) | `—` | future image/preview package | real pixels/History preview |
| `data_photo.cpp:283 PhotoData::setRemoteLocation` | done | `data_photo.rs::set_remote_location` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:294 PhotoData::mtpInput` | done | `data_photo.rs::mtp_input` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:301 PhotoData::fileReference` | done | `data_photo.rs::file_reference` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:305 PhotoData::refreshFileReference` | done | `data_photo.rs::refresh_file_reference` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:312 PhotoData::collectLocalData` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:331 PhotoData::isNull` | done | `data_photo.rs::is_null` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:335 PhotoData::load` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:406 PhotoData::createMediaView` | done | `data_photo.rs::create_media_view` | byte_leases_survive_models_and_scope |  |
| `data_photo.cpp:415 PhotoData::activeMediaView` | done | `data_photo.rs::active_media_view` | byte_leases_survive_models_and_scope |  |
| `data_photo.cpp:419 PhotoData::updateImages` | partial(storage/file_download этап 3/4; pixels этап 5) | `data_photo.rs::update_images` | photo_fallback_preview_video_and_references | model/metadata phase implemented; actual CloudFile adapters absent |
| `data_photo.cpp:479 PhotoData::hasAttachedStickers` | done | `data_photo.rs::has_attached_stickers` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:483 PhotoData::setHasAttachedStickers` | done | `data_photo.rs::set_has_attached_stickers` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:487 PhotoData::width` | done | `data_photo.rs::width` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:491 PhotoData::height` | done | `data_photo.rs::height` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:495 PhotoData::mediaKey` | done | `data_photo.rs::media_key` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:499 PhotoData::location` | done | `data_photo.rs::location` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | real FileLocation model + required platform FileLocations service |
| `data_photo.cpp:510 PhotoData::setLocation` | done | `data_photo.rs::set_location` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | real FileLocation model + required platform FileLocations service |
| `data_photo.cpp:517 PhotoData::videoFile` | done | `data_photo.rs::video_file` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:525 PhotoData::videoFile` | done | `data_photo.rs::video_file` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:534 PhotoData::hasVideo` | done | `data_photo.rs::has_video` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:538 PhotoData::hasVideoSmall` | done | `data_photo.rs::has_video_small` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:542 PhotoData::videoLoading` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:546 PhotoData::videoFailed` | done | `data_photo.rs::video_failed` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:551 PhotoData::loadVideo` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.cpp:578 PhotoData::videoLocation` | done | `data_photo.rs::video_location` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:583 PhotoData::videoByteSize` | done | `data_photo.rs::video_byte_size` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.cpp:587 PhotoData::videoStartPosition` | done | `data_photo.rs::video_start_position` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:591 PhotoData::setVideoPlaybackFailed` | done | `data_photo.rs::set_video_playback_failed` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:597 PhotoData::videoPlaybackFailed` | done | `data_photo.rs::video_playback_failed` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:601 PhotoData::videoCanBePlayed` | done | `data_photo.rs::video_can_be_played` | photo_fallback_preview_video_and_references |  |
| `data_photo.cpp:605 PhotoData::createStreamingLoader` | partial(storage/file_download этап 3/4; FileLocation33) | `—` | future loader/cache/file-location package | actual service/loader lifetime required |
| `data_photo.h:38 PhotoSizeIndex` | done | `PhotoSize::index` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.h:114 inlineThumbnailBytes` | done | `data_photo.rs::inline_thumbnail_bytes` | photo_fallback_preview_video_and_references |  |
| `data_photo.h:117 clearInlineThumbnailBytes` | done | `data_photo.rs::clear_inline_thumbnail_bytes` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo.h:176 getDC` | done | `data_photo.rs::dc_id` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo_media.cpp:23 PhotoMedia::PhotoMedia` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:30 PhotoMedia::~PhotoMedia` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:32 PhotoMedia::owner` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:36 PhotoMedia::thumbnailInline` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:51 PhotoMedia::image` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:58 PhotoMedia::imageBytes` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:65 PhotoMedia::resolveLoadedImage` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:82 PhotoMedia::wanted` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:89 PhotoMedia::size` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:98 PhotoMedia::set` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:120 PhotoMedia::videoContent` | done | `data_photo_media.rs::video_content` | byte_leases_survive_models_and_scope |  |
| `data_photo_media.cpp:125 PhotoMedia::videoSize` | done | `data_photo_media.rs::video_size` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_photo_media.cpp:130 PhotoMedia::videoWanted` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:136 PhotoMedia::setVideo` | done | `data_photo_media.rs::set_video` | byte_leases_survive_models_and_scope |  |
| `data_photo_media.cpp:141 PhotoMedia::loaded` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:147 PhotoMedia::progress` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:153 PhotoMedia::autoLoadThumbnailAllowed` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:165 PhotoMedia::automaticLoad` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:173 PhotoMedia::automaticLoad` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:189 PhotoMedia::collectLocalData` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:203 PhotoMedia::saveToFile` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_photo_media.cpp:222 PhotoMedia::setToClipboard` | partial(этап 5 pixels; storage/file_download 3/4) | `—` | future actual pixel/loader package | byte lease exists; image ownership/loaded state not represented by fake pixels |
| `data_document.cpp:60 JoinStringList` | done | `data_document_filename.rs::promote_save_filter (join)` | save_name_rules_filters_collisions_and_voice_override; native_pure_functions |  |
| `data_document.cpp:79 UpdateStickerSetIdentifier` | done | `data_document.rs::sticker_set_identifier` | sticker_defaults_premium_and_text_color |  |
| `data_document.cpp:92 MillisecondsFromSeconds` | done | `data_media_types.rs::milliseconds_from_seconds` | native_pure_functions |  |
| `data_document.cpp:100 ResolveAttributeVsTranscodeQuality` | done | `data_media_types.rs::resolve_attribute_vs_transcode_quality` | native_pure_functions |  |
| `data_document.cpp:112 FileNameUnsafe` | partial(UI5/6; lang01; loader/storage 3/4; MIME registry33) | `data_document_filename.rs::promote_save_filter/available_file_name` | save_name_rules_filters_collisions_and_voice_override; native_pure_functions | pure filename/filter phases implemented; real dialog/settings/path/loader short circuit not implemented |
| `data_document.cpp:203 FileNameForSave` | partial(UI5/6; lang01; loader/storage 3/4; MIME registry33) | `data_document_filename.rs::file_name_for_save_platform` | save_name_rules_filters_collisions_and_voice_override; native_pure_functions | pure filename/filter phases implemented; real dialog/settings/path/loader short circuit not implemented |
| `data_document.cpp:232 DocumentFileNameForSave` | partial(UI5/6; lang01; loader/storage 3/4; MIME registry33) | `DocumentData::file_name_for_save` | save_name_rules_filters_collisions_and_voice_override; native_pure_functions | pure filename/filter phases implemented; real dialog/settings/path/loader short circuit not implemented |
| `data_document.cpp:291 StickerData::setOrigin` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:298 StickerData::isStatic` | done | `StickerType::is_static` | attribute_order_webm_lottie_and_reapplication |  |
| `data_document.cpp:302 StickerData::isLottie` | done | `StickerType::is_lottie` | attribute_order_webm_lottie_and_reapplication |  |
| `data_document.cpp:306 StickerData::isAnimated` | done | `StickerType::is_animated` | attribute_order_webm_lottie_and_reapplication |  |
| `data_document.cpp:310 StickerData::isWebm` | done | `StickerType::is_webm` | attribute_order_webm_lottie_and_reapplication |  |
| `data_document.cpp:314 VoiceData::~VoiceData` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:324 DocumentData::DocumentData` | done | `data_media_store.rs::Documents + DocumentData::new/key` | media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime | stable record handle + wire lookup; no fake Session |
| `data_document.cpp:329 DocumentData::~DocumentData` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:335 DocumentData::owner` | done | `data_media_store.rs::Documents + DocumentData::new/key` | media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime | stable record handle + wire lookup; no fake Session |
| `data_document.cpp:339 DocumentData::session` | done | `data_media_store.rs::Documents + DocumentData::new/key` | media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime | stable record handle + wire lookup; no fake Session |
| `data_document.cpp:343 DocumentData::setattributes` | partial(storage/file_download 3/4; MIME registry33) | `data_document.rs::set_attributes` | attribute_order_webm_lottie_and_reapplication; video_data_survives_attributes_and_prefix_resets; audio_voice_cover_and_round_waveform; sticker_defaults_premium_and_text_color; filename_enforcement_and_thumbnail_erasure | all model attribute branches; original VoiceData async-task destructor binding absent |
| `data_document.cpp:549 DocumentData::setVideoQualities` | done | `data_media_store.rs::Documents::set_video_qualities` | media_stores.rs::document_tl_qualities_keep_record_links_across_remap | actual stored records, immutable projections, no model stand-in |
| `data_document.cpp:558 DocumentData::setVideoQualities` | done | `data_document.rs::set_video_qualities` | video_variants_dedup_quality_and_closest_choice | cross-record immutable projections; equal-key unstable-sort identity not native-proven |
| `data_document.cpp:623 DocumentData::resolveVideoQuality` | done | `data_document.rs::resolve_video_quality` | video_variants_dedup_quality_and_closest_choice | cross-record immutable projections; equal-key unstable-sort identity not native-proven |
| `data_document.cpp:656 DocumentData::resolveOriginalVideoQuality` | done | `data_document.rs::resolve_original_video_quality` | video_variants_dedup_quality_and_closest_choice | cross-record immutable projections; equal-key unstable-sort identity not native-proven |
| `data_document.cpp:669 DocumentData::initialPlaybackVideoQuality` | done | `data_document.rs::initial_playback_video_quality` | video_variants_dedup_quality_and_closest_choice | cross-record immutable projections; equal-key unstable-sort identity not native-proven |
| `data_document.cpp:680 DocumentData::resolveQualities` | partial(History/Media18) | `—` | future History/Media18 | actual context/list eligibility |
| `data_document.cpp:691 DocumentData::chooseQuality` | partial(History/Media18) | `data_document.rs::choose_quality` | video_variants_dedup_quality_and_closest_choice | closest-variant pure phase implemented, real resolveQualities eligibility still future18 |
| `data_document.cpp:723 DocumentData::validateLottieSticker` | done | `data_document.rs::set_attributes (initial Lottie branch)` | attribute_order_webm_lottie_and_reapplication |  |
| `data_document.cpp:733 DocumentData::setDataAndCache` | partial(storage/file_download 3/4) | `data_document.rs::set_data_and_cache` | data_and_cache_keeps_inclusive_payload_limit_and_live_lease_only | bytes mutation + typed CachePut effect; actual cache dispatch absent |
| `data_document.cpp:746 DocumentData::checkWallPaperProperties` | done | `data_document.rs::check_wallpaper_properties` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:762 DocumentData::updateThumbnails` | partial(storage/file_download 3/4; pixels5) | `data_document.rs::update_thumbnails` | audio_voice_cover_and_round_waveform; tl_document_mutation_order_thumbnail_inline_and_mime | metadata mutation + ordered effects; actual loader/pixels/cache dispatch absent |
| `data_document.cpp:806 DocumentData::isWallPaper` | done | `data_document.rs::is_wallpaper` | document_cache_keys_send_rights_wallpaper_and_playback; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:810 DocumentData::isPatternWallPaper` | done | `data_document.rs::is_pattern_wallpaper` | document_cache_keys_send_rights_wallpaper_and_playback; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:815 DocumentData::isPatternWallPaperPNG` | done | `data_document.rs::is_pattern_wallpaper_png` | document_cache_keys_send_rights_wallpaper_and_playback; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:819 DocumentData::isPatternWallPaperSVG` | done | `data_document.rs::is_pattern_wallpaper_svg` | document_cache_keys_send_rights_wallpaper_and_playback; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:823 DocumentData::isSvgImage` | done | `data_document.rs::is_svg_image` | document_cache_keys_send_rights_wallpaper_and_playback; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:828 DocumentData::isPremiumSticker` | done | `data_document.rs::is_premium_sticker` | sticker_defaults_premium_and_text_color |  |
| `data_document.cpp:836 DocumentData::isPremiumEmoji` | done | `data_document.rs::is_premium_emoji` | sticker_defaults_premium_and_text_color |  |
| `data_document.cpp:844 DocumentData::emojiUsesTextColor` | done | `data_document.rs::uses_text_color` | sticker_defaults_premium_and_text_color |  |
| `data_document.cpp:848 DocumentData::overrideEmojiUsesTextColor` | done | `data_document.rs::override_emoji_uses_text_color` | sticker_defaults_premium_and_text_color |  |
| `data_document.cpp:856 DocumentData::hasThumbnail` | done | `data_document.rs::has_thumbnail` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:862 DocumentData::thumbnailLoading` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:866 DocumentData::thumbnailFailed` | done | `data_document.rs::thumbnail_failed` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:870 DocumentData::loadThumbnail` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:895 DocumentData::thumbnailLocation` | done | `data_document.rs::thumbnail_location` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:899 DocumentData::thumbnailByteSize` | done | `data_document.rs::thumbnail_byte_size` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:903 DocumentData::hasVideoThumbnail` | done | `data_document.rs::has_video_thumbnail` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:907 DocumentData::videoThumbnailLoading` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:911 DocumentData::videoThumbnailFailed` | done | `data_document.rs::video_thumbnail_failed` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:915 DocumentData::loadVideoThumbnail` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:939 DocumentData::videoThumbnailLocation` | done | `data_document.rs::video_thumbnail_location` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:943 DocumentData::videoThumbnailByteSize` | done | `data_document.rs::video_thumbnail_byte_size` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:947 DocumentData::goodThumbnailCacheKey` | done | `data_document.rs::good_thumbnail_cache_key` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:951 DocumentData::goodThumbnailChecked` | done | `data_document.rs::good_thumbnail_checked` | thumbnail_state_machine |  |
| `data_document.cpp:956 DocumentData::goodThumbnailGenerating` | done | `data_document.rs::good_thumbnail_generating` | thumbnail_state_machine |  |
| `data_document.cpp:961 DocumentData::goodThumbnailNoData` | done | `data_document.rs::good_thumbnail_no_data` | thumbnail_state_machine |  |
| `data_document.cpp:966 DocumentData::setGoodThumbnailGenerating` | done | `data_document.rs::set_good_thumbnail_generating` | thumbnail_state_machine |  |
| `data_document.cpp:971 DocumentData::setGoodThumbnailDataReady` | done | `data_document.rs::set_good_thumbnail_data_ready` | thumbnail_state_machine |  |
| `data_document.cpp:978 DocumentData::setGoodThumbnailChecked` | done | `data_document.rs::set_good_thumbnail_checked` | thumbnail_state_machine |  |
| `data_document.cpp:991 DocumentData::createMediaView` | done | `data_document.rs::create_media_view` | byte_leases_survive_models_and_scope |  |
| `data_document.cpp:1000 DocumentData::activeMediaView` | done | `data_document.rs::active_media_view` | byte_leases_survive_models_and_scope |  |
| `data_document.cpp:1004 DocumentData::setGoodThumbnailPhoto` | done | `data_document.rs::set_good_thumbnail_photo` | thumbnail_state_machine |  |
| `data_document.cpp:1008 DocumentData::goodThumbnailPhoto` | done | `data_document.rs::good_thumbnail_photo` | thumbnail_state_machine |  |
| `data_document.cpp:1012 DocumentData::bigFileBaseCacheKey` | done | `data_document.rs::big_file_base_cache_key` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1025 DocumentData::forceToCache` | done | `data_document.rs::force_to_cache` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1033 DocumentData::saveToCache` | done | `data_document.rs::save_to_cache` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1045 DocumentData::automaticLoadSettingsChanged` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1053 DocumentData::finishLoad` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1070 DocumentData::destroyLoader` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1080 DocumentData::loading` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1084 DocumentData::permitLoadFromCloud` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1090 DocumentData::setForbidsFileSave` | done | `data_document.rs::set_forbids_file_save` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1094 DocumentData::forbidsFileSave` | done | `data_document.rs::forbids_file_save` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1098 DocumentData::loadingFilePath` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1102 DocumentData::displayLoading` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1108 DocumentData::progress` | partial(storage/file_download 3/4) | `data_document.rs::uploading_progress` | upload_progress_and_album | upload branch only |
| `data_document.cpp:1123 DocumentData::loadOffset` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1127 DocumentData::uploading` | done | `data_document.rs::uploading` | upload_progress_and_album |  |
| `data_document.cpp:1131 DocumentData::loadedInMediaCache` | done | `data_document.rs::loaded_in_media_cache` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1135 DocumentData::setLoadedInMediaCache` | done | `data_document.rs::set_loaded_in_media_cache` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | model phase + required real platform lookup; persistence/repaint commands routed by host33/5 |
| `data_document.cpp:1155 DocumentData::requiredSendRight` | done | `data_document.rs::required_send_right` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1171 DocumentData::setFileName` | done | `data_document.rs::set_file_name` | filename_enforcement_and_thumbnail_erasure; filename_platform_precedence_and_security |  |
| `data_document.cpp:1193 DocumentData::enforceNameType` | partial(MIME registry33) | `data_document.rs::enforce_name_type` | filename_enforcement_and_thumbnail_erasure; filename_platform_precedence_and_security | pure ordered-glob algorithm complete; platform registry is required caller service |
| `data_document.cpp:1211 DocumentData::setLoadedInMediaCacheLocation` | done | `data_document.rs::set_loaded_in_media_cache_location` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | model phase + required real platform lookup; persistence/repaint commands routed by host33/5 |
| `data_document.cpp:1216 DocumentData::setWaitingForAlbum` | done | `data_document.rs::set_waiting_for_album` | upload_progress_and_album |  |
| `data_document.cpp:1222 DocumentData::waitingForAlbum` | done | `data_document.rs::waiting_for_album` | upload_progress_and_album |  |
| `data_document.cpp:1226 DocumentData::save` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1335 DocumentData::handleLoaderUpdates` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1375 DocumentData::cancel` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1385 DocumentData::cancelled` | done | `data_document.rs::cancelled` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1389 DocumentData::resetCancelled` | done | `data_document.rs::reset_cancelled` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1393 documentWaveformDecode` | done | `data_media_types.rs::document_waveform_decode` | native_pure_functions |  |
| `data_document.cpp:1426 documentWaveformEncode5bit` | done | `data_media_types.rs::document_waveform_encode_5bit` | native_pure_functions |  |
| `data_document.cpp:1445 DocumentData::location` | done | `data_document.rs::location` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | model phase + required real platform lookup; persistence/repaint commands routed by host33/5 |
| `data_document.cpp:1458 DocumentData::setLocation` | done | `data_document.rs::set_location` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | model phase + required real platform lookup; persistence/repaint commands routed by host33/5 |
| `data_document.cpp:1466 DocumentData::filepath` | done | `data_document.rs::filepath` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | model phase + required real platform lookup; persistence/repaint commands routed by host33/5 |
| `data_document.cpp:1472 DocumentData::saveFromData` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1476 DocumentData::saveFromDataSilent` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1482 DocumentData::saveFromDataChecked` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1509 DocumentData::refreshPossibleCoverThumbnail` | partial(storage/file_download 3/4; pixels5) | `data_document.rs::refresh_possible_cover_thumbnail` | audio_voice_cover_and_round_waveform; tl_document_mutation_order_thumbnail_inline_and_mime | metadata mutation + ordered effects; actual loader/pixels/cache dispatch absent |
| `data_document.cpp:1534 DocumentData::isStickerSetInstalled` | partial(Stickers09; Session16) | `—` | future Stickers09/Session16 |  |
| `data_document.cpp:1550 DocumentData::getReplyPreview` | partial(этап 5/6; History18) | `—` | future pixels/History preview |  |
| `data_document.cpp:1574 DocumentData::getReplyPreview` | partial(этап 5/6; History18) | `—` | future pixels/History preview |  |
| `data_document.cpp:1580 DocumentData::replyPreviewLoaded` | partial(этап 5/6; History18) | `—` | future pixels/History preview |  |
| `data_document.cpp:1589 DocumentData::sticker` | done | `data_document.rs::sticker` | sticker_defaults_premium_and_text_color |  |
| `data_document.cpp:1595 DocumentData::stickerSetOrigin` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1606 DocumentData::stickerOrGifOrigin` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1614 DocumentData::song` | done | `data_document.rs::song` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1620 DocumentData::song` | done | `data_document.rs::song` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1624 DocumentData::voice` | done | `data_document.rs::voice` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1630 DocumentData::voice` | done | `data_document.rs::voice` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1634 DocumentData::round` | done | `data_document.rs::round` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1640 DocumentData::round` | done | `data_document.rs::round` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1644 DocumentData::video` | done | `data_document.rs::video` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1650 DocumentData::video` | done | `data_document.rs::video` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1654 DocumentData::hasRemoteLocation` | done | `data_document.rs::has_remote_location` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1658 DocumentData::useStreamingLoader` | done | `data_document.rs::use_streaming_loader` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1670 DocumentData::canBeStreamed` | done | `data_document.rs::can_be_streamed` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1674 DocumentData::setInappPlaybackFailed` | done | `data_document.rs::set_inapp_playback_failed` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1678 DocumentData::inappPlaybackFailed` | done | `data_document.rs::inapp_playback_failed` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1682 DocumentData::videoPreloadPrefix` | done | `data_document.rs::video_preload_prefix` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1686 DocumentData::videoPreloadLocation` | done | `data_document.rs::video_preload_location` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1699 DocumentData::createStreamingLoader` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:1733 DocumentData::hasWebLocation` | done | `data_document.rs::has_web_location` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1737 DocumentData::isNull` | done | `data_document.rs::is_null` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | actual model location field |
| `data_document.cpp:1745 DocumentData::mtpInput` | done | `data_document.rs::mtp_input` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1755 DocumentData::fileReference` | done | `data_document.rs::file_reference` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1759 DocumentData::refreshFileReference` | done | `data_document.rs::refresh_file_reference` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1765 DocumentData::filename` | done | `data_document.rs::filename` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1769 DocumentData::nameType` | done | `data_document.rs::name_type` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1773 DocumentData::mimeString` | done | `data_document.rs::mime_string` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1777 DocumentData::hasMimeType` | done | `data_document.rs::has_mime_type` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1781 DocumentData::setMimeString` | done | `data_document.rs::set_mime_string` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.cpp:1786 DocumentData::mediaKey` | done | `data_document.rs::media_key` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1790 DocumentData::cacheKey` | done | `data_document.rs::cache_key` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1800 DocumentData::cacheTag` | done | `data_document.rs::cache_tag` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1815 DocumentData::locationType` | done | `data_document.rs::location_type` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1823 DocumentData::forceIsStreamedAnimation` | done | `data_document.rs::force_is_streamed_animation` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1829 DocumentData::isMusicForProfile` | done | `data_document.rs::is_music_for_profile` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1833 DocumentData::isVoiceMessage` | done | `data_document.rs::is_voice_message` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1837 DocumentData::isVideoMessage` | done | `data_document.rs::is_video_message` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1841 DocumentData::isAnimation` | done | `data_document.rs::is_animation` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1851 DocumentData::isGifv` | done | `data_document.rs::is_gifv` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1856 DocumentData::isTheme` | done | `data_document.rs::is_theme` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1865 DocumentData::isSong` | done | `data_document.rs::is_song` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1869 DocumentData::isSongWithCover` | done | `data_document.rs::is_song_with_cover` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1873 DocumentData::isAudioFile` | done | `data_document.rs::is_audio_file` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1895 DocumentData::isSharedMediaMusic` | done | `data_document.rs::is_shared_media_music` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1899 DocumentData::isVideoFile` | done | `data_document.rs::is_video_file` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1903 DocumentData::isSilentVideo` | done | `data_document.rs::is_silent_video` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1907 DocumentData::duration` | done | `data_document.rs::duration` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1911 DocumentData::hasDuration` | done | `data_document.rs::has_duration` | audio_voice_cover_and_round_waveform |  |
| `data_document.cpp:1915 DocumentData::isImage` | done | `data_document.rs::is_image` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1919 DocumentData::hasAttachedStickers` | done | `data_document.rs::has_attached_stickers` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1923 DocumentData::supportsStreaming` | done | `data_document.rs::supports_streaming` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1927 DocumentData::setNotSupportsStreaming` | done | `data_document.rs::set_not_supports_streaming` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1932 DocumentData::setMaybeSupportsStreaming` | done | `data_document.rs::set_maybe_supports_streaming` | video_data_survives_attributes_and_prefix_resets |  |
| `data_document.cpp:1942 DocumentData::recountIsImage` | done | `data_document.rs::recount_is_image` | cache_thresholds_and_predicates |  |
| `data_document.cpp:1953 DocumentData::setRemoteLocation` | done | `data_document.rs::set_remote_location` | media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration | model phase + required real platform lookup; persistence/repaint commands routed by host33/5 |
| `data_document.cpp:1978 DocumentData::setStoryMedia` | done | `data_document.rs::set_story_media` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1987 DocumentData::storyMedia` | done | `data_document.rs::story_media` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1991 DocumentData::setContentUrl` | done | `data_document.rs::set_content_url` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1995 DocumentData::setWebLocation` | done | `data_document.rs::set_web_location` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.cpp:1999 DocumentData::collectLocalData` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.cpp:2016 LookupVideoCover` | partial(History/Media18; Webpage09) | `—` | future History/Media18/Webpage09 |  |
| `data_document.h:54 mediaMix32To64` | done | `data_document.rs::media_key` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.h:60 mediaKey` | done | `data_document.rs::media_key` | document_cache_keys_send_rights_wallpaper_and_playback |  |
| `data_document.h:65 ~DocumentAdditionalData` | partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable) | `—` | future loader/file-origin/bookmark package | no dummy loader/FileLocation/Session body |
| `data_document.h:246 inlineThumbnailBytes` | done | `data_document.rs::inline_thumbnail_bytes` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.h:249 inlineThumbnailIsPath` | done | `data_document.rs::inline_thumbnail_is_path` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.h:252 clearInlineThumbnailBytes` | done | `data_document.rs::clear_inline_thumbnail_bytes` | tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.h:327 getDC` | done | `data_document.rs::dc_id` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_document.h:350 is_flag_type` | done | `data_document.rs::u16/u8 private bit operations` | thumbnail_state_machine; cache_thresholds_and_predicates |  |
| `data_document.h:361 is_flag_type` | done | `data_document.rs::u16/u8 private bit operations` | thumbnail_state_machine; cache_thresholds_and_predicates |  |
| `data_document_media.cpp:52 MayHaveGoodThumbnail` | done | `data_document_media.rs::may_have_good_thumbnail` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_document_media.cpp:61 PrepareGoodThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:126 VideoPreviewState::VideoPreviewState` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:131 VideoPreviewState::automaticLoad` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:141 VideoPreviewState::makeAnimation` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:156 VideoPreviewState::usingThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:160 VideoPreviewState::loading` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:168 VideoPreviewState::loaded` | partial(FileLocation33/storage 3/4) | `data_document_media.rs::loaded` | document_cache_keys_send_rights_wallpaper_and_playback | pure decision uses supplied actual filepath; filesystem checking adapter absent |
| `data_document_media.cpp:176 DocumentMedia::DocumentMedia` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:183 DocumentMedia::~DocumentMedia` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:185 DocumentMedia::owner` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:189 DocumentMedia::goodThumbnailWanted` | done | `data_document_media.rs::good_thumbnail_wanted` | byte_leases_survive_models_and_scope |  |
| `data_document_media.cpp:193 DocumentMedia::goodThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:202 DocumentMedia::setGoodThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:210 DocumentMedia::thumbnailInline` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:225 DocumentMedia::thumbnailPath` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:238 DocumentMedia::thumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:242 DocumentMedia::thumbnailWanted` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:248 DocumentMedia::thumbnailSize` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:256 DocumentMedia::setThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:261 DocumentMedia::videoThumbnailContent` | done | `data_document_media.rs::video_thumbnail_content` | byte_leases_survive_models_and_scope |  |
| `data_document_media.cpp:265 DocumentMedia::videoThumbnailSize` | done | `data_document_media.rs::video_thumbnail_size` | metadata_accessors_and_thumbnail_eligibility |  |
| `data_document_media.cpp:270 DocumentMedia::videoThumbnailWanted` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:276 DocumentMedia::setVideoThumbnail` | done | `data_document_media.rs::set_video_thumbnail` | byte_leases_survive_models_and_scope |  |
| `data_document_media.cpp:280 DocumentMedia::checkStickerLarge` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:303 DocumentMedia::automaticLoad` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:352 DocumentMedia::collectLocalData` | partial(этап 5 pixels) | `data_document_media.rs::collect_local_bytes` | byte_leases_survive_models_and_scope | byte/flags copy implemented; actual pixels still absent |
| `data_document_media.cpp:370 DocumentMedia::setBytes` | done | `data_document_media.rs::set_bytes` | byte_leases_survive_models_and_scope |  |
| `data_document_media.cpp:376 DocumentMedia::bytes` | done | `data_document_media.rs::bytes` | byte_leases_survive_models_and_scope |  |
| `data_document_media.cpp:380 DocumentMedia::loaded` | partial(FileLocation33/storage 3/4) | `data_document_media.rs::loaded` | document_cache_keys_send_rights_wallpaper_and_playback | pure decision uses supplied actual filepath; filesystem checking adapter absent |
| `data_document_media.cpp:384 DocumentMedia::progress` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:390 DocumentMedia::canBePlayed` | partial(FileLocation33/storage 3/4) | `data_document_media.rs::can_be_played` | document_cache_keys_send_rights_wallpaper_and_playback | pure decision uses supplied actual filepath; filesystem checking adapter absent |
| `data_document_media.cpp:396 DocumentMedia::thumbnailEnoughForSticker` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:406 DocumentMedia::checkStickerSmall` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:418 DocumentMedia::getStickerLarge` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:423 DocumentMedia::getStickerSmall` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:431 DocumentMedia::checkStickerLarge` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:440 DocumentMedia::GenerateGoodThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:502 DocumentMedia::CheckGoodThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:508 DocumentMedia::ReadOrGenerateThumbnail` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:544 DocumentIconFrameGenerator` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.cpp:582 DocumentIconFrameGenerator` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |
| `data_document_media.h:94 is_flag_type` | partial(этап 5/6; storage/file_download этап 3/4) | `—` | future actual pixel/loader package | no fake Image, Clip, FrameGenerator or Session |

Всего строк восьми файлов: **307**; **188 done**, **119 partial**.


## Смежные функции и интерфейсы (не входят в число 307 выше)

| Источник / функция | Реализация | Тест / статус |
|---|---|---|
| data_types.h UploadState | data_media_types.rs UploadState | upload_progress_and_album |
| data_types.h DocumentType/FileStatus/LocationType | data_media_types.rs enums; data_document.rs::location_type | attribute_order_webm_lottie_and_reapplication; document_cache_keys_send_rights_wallpaper_and_playback |
| data_types.cpp GoodStickerDimensions / media_common.h ValidFrameSize | data_media_types.rs::good_sticker_dimensions | native_pure_functions |
| data_types.cpp DocumentCacheKey | data_media_location.rs::document_cache_key | location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices |
| data_types.cpp DocumentThumbCacheKey | data_media_location.rs::document_thumb_cache_key | document_cache_keys_send_rights_wallpaper_and_playback |
| data_types.cpp WebDocumentCacheKey | data_media_location.rs::web_document_cache_key | location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices (independent hashlib known answer + чтение; не native original function) |
| data_types.cpp UrlCacheKey | data_media_location.rs::url_cache_key | location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices (same proof scope) |
| data_types.cpp GeoPointCacheKey | DownloadLocation::cache_key Geo branch | cached_photo_document_video_thumbnail_and_geo_projection; finite domain only, malformed float cast edges differ from C++ UB |
| data_types.cpp AudioAlbumThumbCacheKey | DownloadLocation::cache_key AudioAlbumThumb branch | location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices |
| image_location.cpp StorageFileLocation::cacheKey | data_media_location.rs::cache_key | location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices |
| image_location.cpp StorageFileLocation::bigFileBaseCacheKey | data_media_location.rs::big_file_base_cache_key | location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices; document_cache_keys_send_rights_wallpaper_and_playback |
| image_location.cpp DownloadLocation::cacheKey | data_media_location.rs::cache_key | location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices (Geo branch: cached_photo_document_video_thumbnail_and_geo_projection) |
| image_location.cpp refreshFileReference | DownloadLocation::refresh_file_reference | photo_fallback_preview_video_and_references; tl_document_mutation_order_thumbnail_inline_and_mime |
| data_cloud_file.cpp CloudFile::clear | CloudFileMetadata::clear | cloud_metadata_grows_and_preserves_progressive_hints; partial actual loader destruction storage/file_download3/4 |
| data_cloud_file.cpp UpdateCloudFile | CloudFileMetadata::update | cloud_metadata_grows_and_preserves_progressive_hints; partial(loader restart/preloaded pixels/cache dispatch storage/file_download3/4, pixels5). This is only the no-loader metadata branch; no actual loader is imitated. |
| data_session.cpp FindInlineThumbnail / FindPhotoInlineThumbnail / FindDocumentInlineThumbnail | data_media_from_tl.rs::find_inline_thumbnail | tl_photo_selection_progressive_and_empty_updates; tl_document_mutation_order_thumbnail_inline_and_mime |
| data_session.cpp FindDocumentThumbnail | data_media_from_tl.rs::find_document_thumbnail | tl_document_mutation_order_thumbnail_inline_and_mime; tl_photo_selection_progressive_and_empty_updates (first tie) |
| data_session.cpp FindDocumentVideoThumbnail | data_media_from_tl.rs::find_document_video_thumbnail | cached_photo_document_video_thumbnail_and_geo_projection |
| data_session.cpp VideoStartTime | data_media_from_tl.rs::video_start_time | tl_photo_selection_progressive_and_empty_updates |
| image_location_factory.cpp FromPhotoSize(photo) | data_media_from_tl.rs Remote::image | tl_photo_selection_progressive_and_empty_updates; partial(cache/loader effects3/4; pixels5). Cached-size branch: cached_photo_document_video_thumbnail_and_geo_projection. |
| image_location_factory.cpp FromProgressiveSize | photo_apply_fields progressive thumbnail branch | tl_photo_selection_progressive_and_empty_updates |
| image_location_factory.cpp FromPhotoSize(document) | Remote::image | tl_document_mutation_order_thumbnail_inline_and_mime; partial(cache/loader effects3/4; pixels5) |
| image_location_factory.cpp FromVideoSize(photo) | Remote::video | tl_photo_selection_progressive_and_empty_updates |
| image_location_factory.cpp FromVideoSize(document) | Remote::video | cached_photo_document_video_thumbnail_and_geo_projection |
| data_session.cpp photoApplyFields TL overloads | photo_apply_fields | tl_photo_selection_progressive_and_empty_updates; partial storage/file_download3/4,pixels5 |
| data_session.cpp documentApplyFields TL overloads | document_apply_fields_with_files + Documents required FileLocations | tl_document_mutation_order_thumbnail_inline_and_mime; media_stores.rs local_file_metadata_paths_cache_marker_and_remote_registration; partial loaders3/4,pixels5 |
| data_session.cpp photo/processPhoto/photoConvert | data_media_store.rs Photos | media_stores.rs photo_noncollision_rekeys_lookup_not_identity_or_indices; photo_collision_and_repeated_conversion_preserve_original_lookup_quirk; convert_effects_run_after_atomic_model_and_lookup_mutation |
| data_session.cpp document/processDocument/documentConvert | data_media_store.rs Documents | media_stores.rs document_convert_cache_moves_order_uploads_and_stable_photo_reference; document_collision_repeated_empty_convert_and_reentrant_cache_sink; document_tl_qualities_keep_record_links_across_remap |
| core/mime_type.cpp SplitExtensions | core/mime_type.rs constant sets, split_whitespace | native_pure_functions; filename_platform_precedence_and_security |
| core/mime_type.cpp MimeType::MimeType(Known) | mime_type_for_name Known match | native_pure_functions |
| core/mime_type.cpp MimeType::MimeType(QMimeType) | required MimeDatabase::for_name | partial33 actual platform registry; native_pure_functions uses captured real registry inputs |
| core/mime_type.cpp MimeType::globPatterns | MimeType::glob_patterns | native_pure_functions |
| core/mime_type.cpp MimeType::filterString | MimeType::filter_string | native_pure_functions |
| core/mime_type.cpp MimeType::name | MimeType::name | native_pure_functions |
| core/mime_type.cpp MimeTypeForName | core/mime_type.rs::mime_type_for_name | native_pure_functions; partial33 actual registry backend |
| core/mime_type.cpp FileExtension | core/mime_type.rs::file_extension | native_pure_functions |
| core/mime_type.cpp DetectNameType | core/mime_type.rs::detect_name_type/for | native_pure_functions (Linux); filename_platform_precedence_and_security (Windows/macOS own tests+чтение) |
| core/mime_type.cpp NameTypeAllowsThumbnail | core/mime_type.rs::name_type_allows_thumbnail | native_pure_functions |
| core/mime_type.cpp FileIsImage | core/mime_type.rs::file_is_image | filename_platform_precedence_and_security (own tests+чтение) |
| core/mime_type.cpp IsMimeStickerLottie | core/mime_type.rs::is_mime_sticker_lottie | native_pure_functions |
| core/mime_type.cpp IsMimeStickerWebm | core/mime_type.rs::is_mime_sticker_webm | native_pure_functions |
| core/mime_type.cpp IsMimeStickerAnimated | core/mime_type.rs::is_mime_sticker_animated | native_pure_functions |
| core/mime_type.cpp IsMimeSticker | core/mime_type.rs::is_mime_sticker | native_pure_functions |
| core/mime_type.cpp IsMimeSentAsVideo | core/mime_type.rs::is_mime_sent_as_video | native_pure_functions |
| core/mime_type.cpp IsMimeAcceptedForPhotoVideoAlbum | core/mime_type.rs::is_mime_accepted_for_photo_video_album | native_pure_functions |
| base_file_utilities.cpp FileNameFromUserString + Linux platform body | core/mime_type.rs::filename_from_user_string | native_pure_functions; Windows/macOS own tests+чтение |

Остальной core/mime_type (file/data sniffing, QMimeData, URLs, clipboard,
IsIpRevealingPath) — partial33/этап5/6, без production подстановок.
Дополнения: FileLocation model/verified_by/check → native_pure_functions и media_stores.rs local_file_metadata_paths_cache_marker_and_remote_registration; platform bookmarks/I/O adapter остаются33.
Qt ASCII CI prefix/suffix → native_pure_functions (весь Unicode scalar domain, кроме surrogates; два non-ASCII folds ſ/K), filename_platform_precedence_and_security.
PhotoMedia encoded image slots → byte_leases_survive_models_and_scope; set/imageBytes/loaded остаются partial5 по настоящим pixels.
Photos/Documents graph indices и liveness → media_stores.rs, actual cache apply → media_cache.rs.
Чужие storage/localstorage файлы и docs-only audit plan не менялись; это уточнение owner contract служит дополнением к раннему эскизу stage4-plan.md.
