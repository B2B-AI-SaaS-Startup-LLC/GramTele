"""Function inventory: each out-of-line body plus header-only inline body.
Declarations covered by an out-of-line body are not counted twice.
Run from repository root with the read-only SourceFiles directory argument.
"""
from pathlib import Path
import re, sys
root = Path(sys.argv[1])
dest = Path('crates/tg-core/docs/data-media-map.md')
files = ['data_photo', 'data_photo_media', 'data_document', 'data_document_media']
rows = []
for stem in files:
    for ext in ['cpp', 'h']:
        for line, text in enumerate((root/'data'/f'{stem}.{ext}').read_text().splitlines(), 1):
            if ext == 'cpp':
                if not text or text[0].isspace() or text[0] in '/#,}' or text.startswith(': ') or '(' not in text:
                    continue
                head = text.split('(')[0]
                if '=' in head:
                    continue
                match = re.search(r'([\w:~]+)$', head)
            else:
                match = re.search(r'([\w:~]+)\s*\(', text) if '(' in text and ('{' in text or '= default;' in text) else None
            if match:
                rows.append((stem, ext, line, match[1]))
def snake(name):
    return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',name)).lower().replace('wall_paper', 'wallpaper')
photo_test = 'photo_fallback_preview_video_and_references'
meta_test = 'metadata_accessors_and_thumbnail_eligibility'
doc_test = 'cache_thresholds_and_predicates'
def classify(stem, ext, line, name):
    member = name.split('::')[-1]
    rust = snake(member)
    module = stem+'.rs'
    test = doc_test if stem == 'data_document' else photo_test
    status = 'done'
    note = ''
    if stem == 'data_photo':
        if member == 'PhotoSizeIndex': rust='PhotoSize::index'; test=meta_test
        elif member == 'SideLimit': rust='PhotoData::SIDE_LIMIT'; test='native_pure_functions'
        elif member == 'getDC': rust='dc_id'; test=meta_test
        elif member == 'location' and line < 400: rust='image_location'
        elif member in ['createMediaView','activeMediaView']: test='byte_leases_survive_models_and_scope'
        elif member in ['setWaitingForAlbum','waitingForAlbum','uploading']: test='upload_progress_and_album'
        elif member in ['key','mtpInput','setRemoteLocation','mediaKey','failed','clearFailed','fileReference','clearInlineThumbnailBytes','videoFailed','videoLoaded','videoByteSize']: test=meta_test
        elif member in ['setFields','setHasAttachedStickers','width','height','setVideoPlaybackFailed','videoPlaybackFailed','videoCanBePlayed','hasVideo','hasVideoSmall','videoFile','videoLocation','videoStartPosition','existingSizeIndex','validSizeIndex','hasExact','isNull','size','imageByteSize','date','extendedMediaPreview','extendedMediaVideoDuration','refreshFileReference','hasAttachedStickers','inlineThumbnailBytes']: pass
        elif member in ['setExtendedMediaPreview','updateImages']:
            status='partial(storage/file_download этап 3/4; pixels этап 5)'; note='model/metadata phase implemented; actual CloudFile adapters absent'
        elif member == 'progress': rust='uploading_progress'; status='partial(storage/file_download этап 3/4)'; test='upload_progress_and_album'; note='upload branch only'
        elif member in ['PhotoData','owner','session']:
            status='done'; rust='data_media_store.rs::Photos + PhotoData::new/key'; test='media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime'; note='stable record handle + wire lookup; no fake Session'
        elif member in ['location','setLocation']:
            status='done'; rust=snake(member); test='media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration'; note='real FileLocation model + required platform FileLocations service'
        elif member in ['ValidatePhotoImage','getReplyPreview','replyPreviewLoaded']:
            status='partial(этап 5/6)'; rust='—'; test='future image/preview package'; note='real pixels/History preview'
        else:
            status='partial(storage/file_download этап 3/4; FileLocation33)'; rust='—'; test='future loader/cache/file-location package'; note='actual service/loader lifetime required'
    elif stem == 'data_document':
        if member in ['documentWaveformDecode','documentWaveformEncode5bit','MillisecondsFromSeconds','ResolveAttributeVsTranscodeQuality']:
            module='data_media_types.rs'; rust={'documentWaveformDecode':'document_waveform_decode','documentWaveformEncode5bit':'document_waveform_encode_5bit'}.get(member,snake(member)); test='native_pure_functions'
        elif member == 'UpdateStickerSetIdentifier': rust='sticker_set_identifier'; test='sticker_defaults_premium_and_text_color'
        elif member in ['JoinStringList','FileNameUnsafe','FileNameForSave','DocumentFileNameForSave']:
            module='data_document_filename.rs'; rust={'JoinStringList':'promote_save_filter (join)','FileNameUnsafe':'promote_save_filter/available_file_name','FileNameForSave':'file_name_for_save_platform','DocumentFileNameForSave':'DocumentData::file_name_for_save'}[member]
            test='save_name_rules_filters_collisions_and_voice_override; native_pure_functions'
            if member != 'JoinStringList': status='partial(UI5/6; lang01; loader/storage 3/4; MIME registry33)'; note='pure filename/filter phases implemented; real dialog/settings/path/loader short circuit not implemented'
        elif name.startswith('StickerData::is'):
            rust='StickerType::'+snake(member); test='attribute_order_webm_lottie_and_reapplication'
        elif member == 'setattributes':
            rust='set_attributes'; test='attribute_order_webm_lottie_and_reapplication; video_data_survives_attributes_and_prefix_resets; audio_voice_cover_and_round_waveform; sticker_defaults_premium_and_text_color; filename_enforcement_and_thumbnail_erasure'
            status='partial(storage/file_download 3/4; MIME registry33)'; note='all model attribute branches; original VoiceData async-task destructor binding absent'
        elif member in ['DocumentData','owner','session']:
            status='done'; rust='data_media_store.rs::Documents + DocumentData::new/key'; test='media_stores.rs::owner_liveness_foreign_scopes_clear_and_lease_lifetime'; note='stable record handle + wire lookup; no fake Session'
        elif member == 'setVideoQualities' and line == 549:
            status='done'; rust='data_media_store.rs::Documents::set_video_qualities'; test='media_stores.rs::document_tl_qualities_keep_record_links_across_remap'; note='actual stored records, immutable projections, no model stand-in'
        elif member in ['setVideoQualities','resolveVideoQuality','resolveOriginalVideoQuality','initialPlaybackVideoQuality']:
            test='video_variants_dedup_quality_and_closest_choice'; note='cross-record immutable projections; equal-key unstable-sort identity not native-proven'
        elif member == 'resolveQualities':
            rust='—'; status='partial(History/Media18)'; test='future History/Media18'; note='actual context/list eligibility'
        elif member == 'chooseQuality':
            test='video_variants_dedup_quality_and_closest_choice'; status='partial(History/Media18)'; note='closest-variant pure phase implemented, real resolveQualities eligibility still future18'
        elif member == 'validateLottieSticker': rust='set_attributes (initial Lottie branch)'; test='attribute_order_webm_lottie_and_reapplication'
        elif member == 'setDataAndCache':
            test='data_and_cache_keeps_inclusive_payload_limit_and_live_lease_only'; status='partial(storage/file_download 3/4)'; note='bytes mutation + typed CachePut effect; actual cache dispatch absent'
        elif member in ['updateThumbnails','refreshPossibleCoverThumbnail']:
            status='partial(storage/file_download 3/4; pixels5)'; test='audio_voice_cover_and_round_waveform; tl_document_mutation_order_thumbnail_inline_and_mime'; note='metadata mutation + ordered effects; actual loader/pixels/cache dispatch absent'
        elif member in ['setFileName','enforceNameType']:
            test='filename_enforcement_and_thumbnail_erasure; filename_platform_precedence_and_security'
            if member == 'enforceNameType': status='partial(MIME registry33)'; note='pure ordered-glob algorithm complete; platform registry is required caller service'
        elif member.startswith('goodThumbnail') or member.startswith('setGoodThumbnail'):
            test='thumbnail_state_machine'
            if member in ['goodThumbnailCacheKey','bigFileBaseCacheKey']: test='document_cache_keys_send_rights_wallpaper_and_playback'
        elif member in ['createMediaView','activeMediaView']: test='byte_leases_survive_models_and_scope'
        elif member in ['setWaitingForAlbum','waitingForAlbum','uploading','progress']:
            test='upload_progress_and_album'
            if member == 'progress': rust='uploading_progress'; status='partial(storage/file_download 3/4)'; note='upload branch only'
        elif member in ['song','voice','round']: test='audio_voice_cover_and_round_waveform'
        elif member == 'sticker': test='sticker_defaults_premium_and_text_color'
        elif member == 'video': test='video_data_survives_attributes_and_prefix_resets'
        elif member in ['isPremiumEmoji','isPremiumSticker','emojiUsesTextColor','overrideEmojiUsesTextColor']:
            rust={'emojiUsesTextColor':'uses_text_color','overrideEmojiUsesTextColor':'override_emoji_uses_text_color'}.get(member,rust); test='sticker_defaults_premium_and_text_color'
        elif member in ['mediaMix32To64','mediaKey','requiredSendRight','cacheKey','cacheTag','locationType','checkWallPaperProperties','bigFileBaseCacheKey','setContentUrl','setWebLocation','forceIsStreamedAnimation','setStoryMedia','storyMedia']:
            rust='media_key' if member=='mediaMix32To64' else rust; test='document_cache_keys_send_rights_wallpaper_and_playback'
        elif member in ['duration','hasDuration','isVoiceMessage','isVideoMessage','isSong','isSongWithCover','isMusicForProfile','isSharedMediaMusic','song']:
            test='audio_voice_cover_and_round_waveform'
        elif member in ['setMaybeSupportsStreaming','setNotSupportsStreaming','supportsStreaming','videoPreloadPrefix','isSilentVideo','isVideoFile']:
            test='video_data_survives_attributes_and_prefix_resets'
        elif member in ['hasRemoteLocation','hasWebLocation','mtpInput','fileReference','refreshFileReference','filename','nameType','mimeString','setMimeString','hasMimeType','hasThumbnail','hasVideoThumbnail','inlineThumbnailBytes','inlineThumbnailIsPath','clearInlineThumbnailBytes','thumbnailLocation','thumbnailFailed','thumbnailByteSize','videoThumbnailLocation','videoThumbnailFailed','videoThumbnailByteSize','videoPreloadLocation']:
            test='tl_document_mutation_order_thumbnail_inline_and_mime; metadata_accessors_and_thumbnail_eligibility'
        elif member == 'isNull':
            rust='is_null'; status='done'; test='media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration'; note='actual model location field'
        elif member == 'getDC': rust='dc_id'; test=meta_test
        elif member in ['cancelled','resetCancelled','loadedInMediaCache']: test=meta_test
        elif member in ['isWallPaper','isPatternWallPaper','isPatternWallPaperPNG','isPatternWallPaperSVG','isSvgImage']: test='document_cache_keys_send_rights_wallpaper_and_playback; metadata_accessors_and_thumbnail_eligibility'
        elif member == 'LookupVideoCover': rust='—'; status='partial(History/Media18; Webpage09)'; test='future History/Media18/Webpage09'
        elif member in ['isStickerSetInstalled']: rust='—'; status='partial(Stickers09; Session16)'; test='future Stickers09/Session16'
        elif member in ['getReplyPreview','replyPreviewLoaded']: rust='—'; status='partial(этап 5/6; History18)'; test='future pixels/History preview'
        elif member in ['location','setLocation','filepath','setLoadedInMediaCache','setLoadedInMediaCacheLocation','setRemoteLocation']:
            status='done'; test='media_stores.rs::local_file_metadata_paths_cache_marker_and_remote_registration'; note='model phase + required real platform lookup; persistence/repaint commands routed by host33/5'
        elif member in ['~DocumentAdditionalData','~VoiceData','~DocumentData','setOrigin','stickerSetOrigin','stickerOrGifOrigin','setLoadedInMediaCache','setLoadedInMediaCacheLocation','setRemoteLocation','location','setLocation','filepath','isNull','videoPreloadLocation','createStreamingLoader','thumbnailLoading','thumbnailFailed','loadThumbnail','thumbnailByteSize','videoThumbnailLoading','videoThumbnailFailed','loadVideoThumbnail','videoThumbnailLocation','videoThumbnailByteSize','automaticLoadSettingsChanged','finishLoad','destroyLoader','loading','permitLoadFromCloud','loadingFilePath','displayLoading','loadOffset','save','handleLoaderUpdates','cancel','saveFromData','saveFromDataSilent','saveFromDataChecked','collectLocalData']:
            rust='—'; status='partial(storage/file_download этап 3/4; FileLocation33; pixels5 where applicable)'; test='future loader/file-origin/bookmark package'; note='no dummy loader/FileLocation/Session body'
        elif member == 'is_flag_type': rust='u16/u8 private bit operations'; test='thumbnail_state_machine; cache_thresholds_and_predicates'
    elif stem == 'data_photo_media':
        if member in ['videoContent','setVideo']: test='byte_leases_survive_models_and_scope'
        elif member == 'videoSize': test=meta_test
        else:
            status='partial(этап 5 pixels; storage/file_download 3/4)'; test='future actual pixel/loader package'; rust='—'; note='byte lease exists; image ownership/loaded state not represented by fake pixels'
    elif stem == 'data_document_media':
        if member in ['setBytes','bytes','setVideoThumbnail','videoThumbnailContent','goodThumbnailWanted']:
            test='byte_leases_survive_models_and_scope'
            if member=='videoThumbnailContent': rust='video_thumbnail_content'
        elif member in ['MayHaveGoodThumbnail','videoThumbnailSize']:
            rust=snake(member); test=meta_test
        elif member in ['loaded','canBePlayed']:
            status='partial(FileLocation33/storage 3/4)'; test='document_cache_keys_send_rights_wallpaper_and_playback'; note='pure decision uses supplied actual filepath; filesystem checking adapter absent'
        elif member == 'collectLocalData':
            rust='collect_local_bytes'; status='partial(этап 5 pixels)'; test='byte_leases_survive_models_and_scope'; note='byte/flags copy implemented; actual pixels still absent'
        else:
            rust='—'; status='partial(этап 5/6; storage/file_download этап 3/4)'; test='future actual pixel/loader package'; note='no fake Image, Clip, FrameGenerator or Session'
    if rust != '—' and '::' not in rust:
        rust = f'{module}::{rust}'
    return status, rust, test, note
header = '# Фото и документы — пакет 04\n\nБаза main `0c38855`; оригинал `0f2a31bb420de98f331990432265b131b8d836b5`.\nПолностью прочитаны восемь файлов задания (4338 LOC). Каждое cpp тело,\nперегрузка и header-only inline — отдельная строка; cpp-строка покрывает и\nдекларацию из h. `done` относится к указанной model/pure фазе, не к полному\nnative replay класса. Deferred UI/loader bindings явно partial с будущим адресом.\n\nTests без префикса — [data_media.rs](../tests/data_media.rs);\n[media_stores.rs](../tests/media_stores.rs) и [media_cache.rs](../tests/media_cache.rs)\nпроверяют owner/remap и настоящую encrypted cache queue. [Native proof scope и\nвоспроизведение](../tests/reference-tools/data-media/README.md).\nFull data_document.cpp probe требует generated lang_auto.h; состояния/атрибуты/TL\n— **own tests + чтение**, не native полного класса. Native использует реальные\nчистые тела, QMimeDatabase и полный core/file_location.cpp, без подмен Session.\n\n## Уточнённый контракт владения после чтения convert\n\nPhotoKey/DocumentKey(scope, wire_id) сохраняются как lookup selectors и не\nиспользуются вместо указателей. PhotoRecordKey/DocumentRecordKey(scope, serial)\n— отдельная стабильная identity записи; serial не переиспользуется в scope,\nэто не generation медиа и никогда не передаётся в TL/кэш/tdata. Model::id и\nwire_key() — изменяемое поле wire ID, отдельно от стабильного key().\n\nC++ A→занятый B сохраняет map[A]=P, map[B]=Q с P.id=B и Q.id=B. Повторный P→C\nнаходит map[P.id=B] и перемещает Q в C: P.id=C под A, Q.id=B под C. Порт сохраняет\nэту исходную особенность, не сливает объекты и не исправляет индекс по догадке.\nLookup, records и индексы меняются под одним borrow; затем он закрывается до fire.\nPhotoItems counts/DocumentItems sets/scheduled photo clear, quality links, good\nthumbnail photo и leases остаются привязаны к record identity. SavedGifs09\nmembership приходит от настоящего owner как projection; WriteSavedGifs и оба\nmoveIfEmpty идут в исходном порядке. Реальный tg-cache adapter реализован и\nпроверен на encrypted Database с виртуальными таймерами (media_cache.rs).\n\nPhotos/Documents — настоящие ownership components Data::Session, как Peers,\nбез фиктивной полной Main::Session. Scoped stores проверяют живость/чужие keys;\nclear не переиспользует record identity. Peers::set_userpic_photo связывает\nprocessPhoto, PhotoData::peer и Changes; peer userpic ID остаётся wire scalar,\nкак в оригинале, и не переназначается молча при convert.\n\nFileLocation имеет настоящие path/size/modified model fields; platform inspection,\nbookmark resolution и localstorage read — обязательный FileLocations service,\nбез default/пустого production provider. Запись/repaint — typed commands.\nByte leases не содержат Session/record reference; PhotoMedia хранит encoded image\nslots/goodFor и video bytes, но loaded-by-pixels не имитируется наличием bytes.\nActual loaders/cancellation — storage/file_download3/4, decoded Image/clip5.\nMIME registry также обязательный caller service, не список примерных расширений.\nPLAN.md не предписывает его replacement dependency; platform backend —33,\nновых зависимостей нет. Native registry fixture используется только в тестах.\n\n## Восемь файлов\n\n| Оригинал | Статус | Rust / фаза | Тест или будущий пакет | Остаток |\n|---|---|---|---|---|\n'

lines=[]
counts={}
for stem,ext,line,name in rows:
    status,rust,test,note=classify(stem,ext,line,name)
    bucket=status.split('(')[0]
    counts[bucket]=counts.get(bucket,0)+1
    lines.append(f'| `{stem}.{ext}:{line} {name}` | {status} | `{rust}` | {test} | {note} |')
support = '''

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
'''
dest.write_text(header+'\n'.join(lines)+'\n\nВсего строк восьми файлов: **'+str(len(rows))+'**; '+', '.join(f'**{v} {k}**' for k,v in sorted(counts.items()))+'.\n'+support)
print(len(rows),counts)
