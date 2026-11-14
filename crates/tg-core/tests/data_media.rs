use std::{collections::BTreeMap, rc::Rc};
use tg_core::data::data_document_filename::*;
use tg_core::{
    core::mime_type::*,
    data::{
        data_document::*, data_document_media::*, data_image_location::*, data_media_from_tl::*,
        data_media_location::*, data_media_types::*, data_photo::*, data_store_keys::*,
    },
};
use tg_scheme as tl;
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn string(s: &str) -> String {
    String::from_utf8(unhex(s)).unwrap()
}
struct NativeRegistry(BTreeMap<String, MimeType>);
impl NativeRegistry {
    fn new() -> Self {
        Self(
            include_str!("fixtures/data-media/pure.csv")
                .lines()
                .filter_map(|l| {
                    let a: Vec<_> = l.split(',').collect();
                    (a[0] == "registry").then(|| {
                        (
                            string(a[1]),
                            MimeType {
                                name: string(a[2]),
                                glob_patterns: if a[3].is_empty() {
                                    vec![]
                                } else {
                                    string(a[3]).split('|').map(str::to_owned).collect()
                                },
                                filter_string: string(a[4]),
                            },
                        )
                    })
                })
                .collect(),
        )
    }
}
impl MimeDatabase for NativeRegistry {
    fn for_name(&self, name: &str) -> MimeType {
        self.0
            .get(name)
            .unwrap_or_else(|| panic!("uncaptured registry query: {name}"))
            .clone()
    }
}
fn image(letter: u8, w: i32, h: i32) -> ImageWithLocation {
    ImageWithLocation {
        location: ImageLocation::new(
            DownloadLocation::Storage(StorageFileLocation {
                kind: StorageFileType::Photo,
                dc_id: 4,
                id: 10,
                size_letter: letter,
                ..Default::default()
            }),
            w,
            h,
        ),
        bytes_count: w * h,
        ..Default::default()
    }
}
fn attr_video(flags: u64, w: i32, h: i32) -> tl::DocumentAttribute {
    tl::DocumentAttribute::DocumentAttributeVideo(
        tl::DocumentAttributeVideoData {
            flags: tl::tl::Flags::from_raw(flags),
            duration: 1.0005,
            w,
            h,
            preload_prefix_size: Some(1234),
            video_start_ts: Some(0.0005),
            video_codec: Some(b"vp9".to_vec()),
        }
        .into(),
    )
}
fn attr_sticker(emoji: bool, flags: u64) -> tl::DocumentAttribute {
    let set = tl::mtp_input_sticker_set_id(12, 34);
    if emoji {
        tl::mtp_document_attribute_custom_emoji(
            tl::tl::Flags::from_raw(flags),
            b"alt".to_vec(),
            set,
        )
    } else {
        tl::mtp_document_attribute_sticker(
            tl::tl::Flags::from_raw(flags),
            b"alt".to_vec(),
            set,
            None,
        )
    }
}
#[test]
fn native_pure_functions() {
    let database = NativeRegistry::new();
    let mut rows = 0;
    for l in include_str!("fixtures/data-media/pure.csv").lines() {
        let a: Vec<_> = l.split(',').collect();
        match a[0] {
            "asciifold" => {
                let c = char::from_u32(a[1].parse().unwrap()).unwrap().to_string();
                assert_eq!(qt_ascii_ci_prefix(&c, a[2]), Some(c.len()));
                assert!(qt_ascii_ci_suffix(&c, a[2]));
            }
            "fileloc" => {
                use tg_core::core::file_location::*;
                let info = FileInfo {
                    path: "file.bin".into(),
                    exists: true,
                    readable: true,
                    size: 3,
                    modified_ms: Some(1600000000123),
                };
                let location = match a[1] {
                    "empty" => FileLocation::default(),
                    "cache" => FileLocation::in_media_cache_location(),
                    _ => FileLocation::from_info(&info),
                };
                let current = FileInfo {
                    size: if a[1] == "changed" { 4 } else { 3 },
                    ..info
                };
                assert_eq!(location.is_empty(), a[2] == "1");
                assert_eq!(location.in_media_cache(), a[3] == "1");
                assert_eq!(location.size.to_string(), a[4]);
                assert_eq!(location.modified_ms.unwrap_or(0).to_string(), a[5]);
                assert_eq!(location.check(&current), a[6] == "1");
            }
            "registry" => continue,
            "filter" => assert_eq!(
                promote_save_filter(&string(a[1]), &string(a[2])),
                string(a[3]),
                "{l}"
            ),
            "wave" => {
                let wave: Vec<_> = unhex(a[1]).iter().map(|&n| n as i8).collect();
                let encoded = document_waveform_encode_5bit(&wave);
                assert_eq!(encoded, unhex(a[2]), "{l}");
                assert_eq!(
                    document_waveform_decode(&encoded),
                    unhex(a[3]).iter().map(|&n| n as i8).collect::<Vec<_>>(),
                    "{l}"
                );
            }
            "decode" => assert_eq!(
                document_waveform_decode(&unhex(a[1])),
                unhex(a[2]).iter().map(|&n| n as i8).collect::<Vec<_>>(),
                "{l}"
            ),
            "size" => {
                let size = MediaSize::new(a[1].parse().unwrap(), a[2].parse().unwrap());
                assert_eq!(
                    size.fit(MediaSize::new(2560, 2560)),
                    MediaSize::new(a[3].parse().unwrap(), a[4].parse().unwrap()),
                    "{l}"
                );
                assert_eq!(
                    good_sticker_dimensions(size.width, size.height),
                    a[5] == "1",
                    "{l}"
                );
            }
            "ms" => assert_eq!(
                milliseconds_from_seconds(f64::from_bits(a[1].parse().unwrap())).to_string(),
                a[2],
                "{l}"
            ),
            "quality" => assert_eq!(
                resolve_attribute_vs_transcode_quality(
                    a[1].parse().unwrap(),
                    a[2].parse().unwrap()
                )
                .to_string(),
                a[3],
                "{l}"
            ),
            "name" => {
                let name = string(a[1]);
                assert_eq!(file_extension(&name), string(a[2]), "{l}");
                let kind = detect_name_type_for(&name, FilenamePlatform::Linux);
                assert_eq!((kind as u8).to_string(), a[3], "{l}");
                assert_eq!(name_type_allows_thumbnail(kind), a[4] == "1", "{l}");
                assert_eq!(
                    filename_from_user_string(&name, FilenamePlatform::Linux),
                    string(a[5]),
                    "{l}"
                );
            }
            "mime" => {
                let m = string(a[1]);
                let result = mime_type_for_name(&m, &database);
                assert_eq!(result.name, string(a[2]), "{l}");
                assert_eq!(result.glob_patterns.join("|"), string(a[3]), "{l}");
                assert_eq!(result.filter_string, string(a[4]), "{l}");
                let flags = [
                    is_mime_sticker_lottie(&m),
                    is_mime_sticker_webm(&m),
                    is_mime_sticker_animated(&m),
                    is_mime_sticker(&m),
                    is_mime_sent_as_video(&m),
                    is_mime_accepted_for_photo_video_album(&m),
                ]
                .map(|b| if b { '1' } else { '0' })
                .iter()
                .collect::<String>();
                assert_eq!(flags, a[5], "{l}");
            }
            _ => panic!("unknown row: {l}"),
        }
        rows += 1;
    }
    assert_eq!(rows, 1688);
}
#[test]
fn filename_platform_precedence_and_security() {
    assert_eq!(
        filename_from_user_string("x.\u{17f}cf", FilenamePlatform::Windows),
        "x.\u{17f}cf.download"
    );
    assert_eq!(
        filename_from_user_string("x.ln\u{212a}", FilenamePlatform::Windows),
        "x.ln\u{212a}.download"
    );
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 42);
    d.set_file_name("x.opu\u{17f}");
    assert!(d.is_audio_file());
    d.set_file_name("x.\u{17f}vg");
    assert!(d.is_svg_image());
    d.set_file_name("x.tde\u{17f}ktop-theme");
    assert!(d.is_theme());
    for (name, win, mac, linux) in [
        (
            "a.key",
            NameType::Executable,
            NameType::Document,
            NameType::Document,
        ),
        (
            "a.fon",
            NameType::Executable,
            NameType::OtherBenign,
            NameType::OtherBenign,
        ),
        (
            "a.xhtml",
            NameType::Document,
            NameType::Executable,
            NameType::Document,
        ),
        (
            "a.desktop",
            NameType::Unknown,
            NameType::Unknown,
            NameType::Executable,
        ),
    ] {
        assert_eq!(detect_name_type_for(name, FilenamePlatform::Windows), win);
        assert_eq!(detect_name_type_for(name, FilenamePlatform::Mac), mac);
        assert_eq!(detect_name_type_for(name, FilenamePlatform::Linux), linux);
    }
    assert_eq!(
        sanitize_remote_filename("a\u{202e}b\u{202c}c\u{2068}d"),
        "a_b\u{202c}c\u{2068}d"
    );
    assert_eq!(
        filename_from_user_string("CON.lnk", FilenamePlatform::Windows),
        "_CON.lnk.download"
    );
    assert_eq!(
        filename_from_user_string("LPT10.txt", FilenamePlatform::Windows),
        "LPT10.txt"
    );
    assert_eq!(filename_from_user_string("", FilenamePlatform::Linux), "_");
    assert_eq!(file_extension("a.x/b"), "");
    assert!(file_is_image("", "IMAGE/GIF"));
    assert!(!file_is_image("x.exe", "image/png"));
}
#[test]
fn attribute_order_webm_lottie_and_reapplication() {
    let scope = SessionScope::new();
    let db = NativeRegistry::new();
    let mut a = DocumentData::new(&scope, 1);
    a.size = 100;
    a.set_mime_string("VIDEO/WEBM");
    a.set_attributes(&[attr_sticker(false, 0), attr_video(2, 512, 256)], &db);
    assert_eq!(a.kind, DocumentType::Sticker);
    assert_eq!(a.sticker().unwrap().kind, StickerType::Webm);
    assert_eq!(a.duration(), 1001);
    assert!(a.supports_streaming());
    assert_eq!(a.video_preload_prefix(), 0);
    a.set_attributes(&[], &db);
    assert!(!a.has_duration());
    assert_eq!(a.dimensions, MediaSize::new(512, 256));
    assert!(a.sticker().is_some());
    let mut b = DocumentData::new(&scope, 2);
    b.set_mime_string("video/mp4");
    b.set_attributes(
        &[
            attr_video(2, 512, 256),
            tl::mtp_document_attribute_animated(),
        ],
        &db,
    );
    assert!(b.is_gifv());
    assert!(b.video().is_none());
    let mut c = DocumentData::new(&scope, 3);
    c.set_mime_string("application/x-tgsticker");
    c.set_attributes(&[], &db);
    assert_eq!(c.dimensions, MediaSize::new(512, 512));
    assert!(c.sticker().unwrap().kind.is_lottie());
    c.size = MAX_STICKER_BYTES + 1;
    c.set_attributes(&[], &db);
    assert_eq!(c.kind, DocumentType::File);
}
#[test]
fn video_data_survives_attributes_and_prefix_resets() {
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut d = DocumentData::new(&s, 1);
    d.set_attributes(&[attr_video(2 | 8, 1920, 1080)], &db);
    assert_eq!(d.video_preload_prefix(), 1234);
    assert_eq!(d.video().unwrap().codec, "vp9");
    assert_eq!(d.video().unwrap().start_ts, 1);
    assert!(d.is_silent_video());
    d.video_mut().unwrap().real_video_size = MediaSize::new(1280, 720);
    d.set_attributes(&[attr_video(0, 1920, 1080)], &db);
    assert_eq!(d.video_preload_prefix(), 0);
    assert_eq!(
        d.video().unwrap().real_video_size,
        MediaSize::new(1280, 720)
    );
    assert!(!d.supports_streaming());
    assert!(!d.is_silent_video());
    d.set_not_supports_streaming();
    d.set_maybe_supports_streaming(true);
    assert!(!d.supports_streaming());
    d.set_attributes(&[attr_video(2, 1920, 1080)], &db);
    assert!(d.supports_streaming());
}
#[test]
fn sticker_defaults_premium_and_text_color() {
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut d = DocumentData::new(&s, 1);
    d.set_mime_string("video/webm");
    d.set_attributes(&[attr_sticker(true, 2)], &db);
    assert_eq!(d.dimensions, MediaSize::new(100, 100));
    assert!(d.is_premium_emoji());
    assert!(d.uses_text_color());
    d.set_attributes(&[attr_sticker(true, 1)], &db);
    assert!(!d.is_premium_emoji());
    assert!(!d.uses_text_color());
    d.override_emoji_uses_text_color(true);
    assert!(d.uses_text_color());
    let mut sticker = DocumentData::new(&s, 2);
    sticker.set_mime_string("video/webm");
    sticker.thumbnail.location = image(b'm', 100, 50).location;
    sticker.set_attributes(&[attr_sticker(false, 0)], &db);
    assert_eq!(sticker.dimensions, MediaSize::new(512, 256));
    let set = tl::mtp_input_sticker_set_short_name(b"name".to_vec());
    assert!(sticker_set_identifier(&set).empty());
}
#[test]
fn filename_enforcement_and_thumbnail_erasure() {
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut d = DocumentData::new(&s, 1);
    d.set_mime_string("video/mp4");
    d.set_file_name("clip.exe");
    d.set_attributes(&[attr_video(2, 100, 100)], &db);
    assert_eq!(d.filename(), "clip.exe.mp4");
    assert!(d.is_video_file());
    let mut e = DocumentData::new(&s, 2);
    e.set_mime_string("x/unknown");
    e.thumbnail.location = image(b'm', 100, 50).location;
    e.set_file_name("bad.exe");
    let effects = e.set_attributes(&[attr_video(2, 100, 100)], &db);
    assert_eq!(e.kind, DocumentType::File);
    assert!(!e.thumbnail.location.valid());
    assert!(effects.contains(&DocumentEffect::ClearThumbnails));
    assert!(e
        .update_thumbnails(
            &[1],
            true,
            &image(b'm', 100, 50),
            &Default::default(),
            false
        )
        .is_empty());
    assert!(e.inline_thumbnail_bytes().is_empty());
}
#[test]
fn audio_voice_cover_and_round_waveform() {
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut d = DocumentData::new(&s, 1);
    d.set_mime_string("audio/mp3");
    let audio = tl::mtp_document_attribute_audio(
        tl::tl::Flags::from_raw(0),
        3,
        Some(b"song".to_vec()),
        Some(b"artist".to_vec()),
        Some(vec![255]),
    );
    let effects = d.set_attributes(std::slice::from_ref(&audio), &db);
    assert!(d.is_song());
    assert!(d.is_music_for_profile());
    assert!(d.is_shared_media_music());
    assert!(!d.is_song_with_cover());
    assert_eq!(d.duration(), 3000);
    assert!(d.thumbnail.location.valid());
    assert!(!d.has_thumbnail());
    assert!(effects.contains(&DocumentEffect::LoadThumbnail));
    let mut round = DocumentData::new(&s, 2);
    round.set_attributes(&[attr_video(1, 100, 100), audio], &db);
    assert!(round.is_video_message());
    assert_eq!(round.cache_tag(), 4);
    assert_eq!(round.round().unwrap().waveform, vec![31]);
    let mut voice = DocumentData::new(&s, 3);
    voice.set_attributes(
        &[tl::mtp_document_attribute_audio(
            tl::tl::Flags::from_raw(1 << 10),
            -1,
            None,
            None,
            Some(vec![255, 255]),
        )],
        &db,
    );
    assert!(voice.is_voice_message());
    assert_eq!(voice.cache_tag(), 3);
    assert!(!voice.has_duration());
    assert_eq!(voice.duration(), 0);
    assert_eq!(voice.voice().unwrap().wavemax, 31);
    assert!(voice.supports_streaming());
}
#[test]
fn cache_thresholds_and_predicates() {
    let flags_scope = SessionScope::new();
    let mut flagged = DocumentData::new(&flags_scope, 8);
    flagged.set_attributes(
        &[
            tl::mtp_document_attribute_filename("a\u{202e}.png".as_bytes().to_vec()),
            tl::mtp_document_attribute_has_stickers(),
        ],
        &NativeRegistry::new(),
    );
    assert_eq!(flagged.filename(), "a_.png");
    assert!(flagged.has_attached_stickers());
    flagged.set_attributes(&[], &NativeRegistry::new());
    assert!(!flagged.has_attached_stickers());
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 1);
    d.set_mime_string("audio/mpeg");
    d.set_file_name("sound.mp3");
    assert!(!d.is_audio_file());
    d.set_mime_string("audio/mp4");
    assert!(d.is_audio_file());
    d.set_file_name("x.exe");
    assert!(!d.is_audio_file());
    d.set_mime_string("application/octet-stream");
    d.set_file_name("x.OPUS");
    assert!(d.is_audio_file());
    d.set_file_name("image_x.png");
    d.set_mime_string("image/png");
    d.recount_is_image();
    assert!(d.is_image());
    assert!(d.save_to_cache());
    d.size = MAX_FILE_IN_MEMORY;
    assert!(!d.save_to_cache());
    d.size -= 1;
    assert!(d.save_to_cache());
    d.set_file_name("Image_x.png");
    assert!(!d.save_to_cache());
    d.force_to_cache(true);
    assert!(d.save_to_cache());
    d.force_to_cache(false);
    assert!(!d.save_to_cache());
    d.set_mime_string("image/gif");
    d.set_file_name("");
    assert!(d.is_animation());
    d.set_inapp_playback_failed();
    assert!(!d.is_animation());
    d.set_file_name("a.TDESKTOP-THEME");
    assert!(d.is_theme());
    d.set_forbids_file_save();
    assert!(d.forbids_file_save());
}
#[test]
fn thumbnail_state_machine() {
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 1);
    assert!(!d.good_thumbnail_checked());
    d.set_good_thumbnail_generating();
    assert!(d.good_thumbnail_generating());
    assert!(d.set_good_thumbnail_checked(false).is_empty());
    assert!(d.good_thumbnail_no_data());
    d.set_good_thumbnail_data_ready();
    assert!(!d.good_thumbnail_no_data());
    assert_eq!(
        d.set_good_thumbnail_checked(false),
        vec![DocumentEffect::CheckGoodThumbnail]
    );
    assert!(!d.good_thumbnail_checked());
    d.set_good_thumbnail_checked(true);
    assert!(d.good_thumbnail_checked());
    d.set_good_thumbnail_data_ready();
    assert!(d.good_thumbnail_checked());
    let p = PhotoData::new(&s, 2).key();
    d.set_good_thumbnail_photo(p);
    assert_eq!(d.good_thumbnail_photo(), Some(p));
}
#[test]
fn upload_progress_and_album() {
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 1);
    let mut p = PhotoData::new(&s, 2);
    d.set_waiting_for_album();
    assert!(!d.waiting_for_album());
    let up = UploadState {
        size: 100,
        offset: 150,
        preparing: true,
        ..Default::default()
    };
    d.uploading_data = Some(up.clone());
    p.uploading_data = Some(up);
    assert_eq!(d.uploading_progress(), Some(0.));
    assert_eq!(p.uploading_progress(), Some(1.));
    d.set_waiting_for_album();
    p.set_waiting_for_album();
    assert!(d.waiting_for_album() && p.waiting_for_album());
}
#[test]
fn photo_fallback_preview_video_and_references() {
    let s = SessionScope::new();
    let mut p = PhotoData::new(&s, 10);
    p.set_extended_media_preview(&[1], MediaSize::new(50, 100), Some(0));
    assert!(p.is_null());
    assert_eq!(p.date(), 0);
    assert_eq!(p.extended_media_video_duration(), Some(0));
    let large = image(b'x', 4000, 1000);
    let video = image(b'v', 300, 100);
    p.update_images(
        &[2],
        &Default::default(),
        &ImageWithLocation {
            progressive_part_size: 123,
            ..Default::default()
        },
        &large,
        &Default::default(),
        &video,
        3000,
    );
    assert_eq!(p.inline_thumbnail_bytes(), &[1]);
    assert!(!p.has_exact(PhotoSize::Small));
    assert_eq!(p.valid_size_index(PhotoSize::Small), 2);
    assert_eq!(p.existing_size_index(PhotoSize::Small), 1);
    assert_eq!(p.image_byte_size(PhotoSize::Small), 123);
    assert_eq!(p.size(PhotoSize::Small), Some(MediaSize::new(2560, 640)));
    p.refresh_file_reference(&[9]);
    let DownloadLocation::Storage(l) = &p.images[2].location.file else {
        panic!()
    };
    assert_eq!(l.file_reference, vec![9]);
    let DownloadLocation::Storage(v) = &p.video_location(PhotoSize::Small).unwrap().file else {
        panic!()
    };
    assert!(v.file_reference.is_empty());
    p.refresh_file_reference(&[]);
    let DownloadLocation::Storage(l) = &p.images[2].location.file else {
        panic!()
    };
    assert_eq!(l.file_reference, vec![9]);
    assert!(p.file_reference().is_empty());
    p.set_video_playback_failed();
    assert!(!p.video_can_be_played());
    assert_eq!(p.video_start_position(), 3000);
    p.set_fields(42, true);
    assert_eq!(p.date(), 42);
    assert!(!p.extended_media_preview());
    assert!(p.has_attached_stickers());
    p.set_has_attached_stickers(false);
    assert!(!p.has_attached_stickers());
    assert_eq!(p.height(), 1000);
    p.update_images(
        &[],
        &Default::default(),
        &Default::default(),
        &large,
        &Default::default(),
        &Default::default(),
        0,
    );
    assert!(!p.has_video());
}
#[test]
fn byte_leases_survive_models_and_scope() {
    let (doc, photo) = {
        let s = SessionScope::new();
        let mut d = DocumentData::new(&s, 1);
        let mut p = PhotoData::new(&s, 2);
        let a = d.create_media_view();
        assert!(Rc::ptr_eq(&a, &d.create_media_view()));
        a.borrow_mut().set_bytes(&[1, 2]);
        let b = p.create_media_view();
        assert!(Rc::ptr_eq(&b, &p.create_media_view()));
        b.borrow_mut().set_video(PhotoSize::Large, false, vec![3]);
        b.borrow_mut()
            .set_encoded_image(PhotoSize::Large, PhotoSize::Large, vec![5, 6]);
        (a, b)
    };
    assert_eq!(doc.borrow().bytes(), &[1, 2]);
    doc.borrow_mut().good_thumbnail_wanted();
    assert!(doc.borrow().is_good_thumbnail_wanted());
    doc.borrow_mut().set_video_thumbnail(vec![9]);
    assert_eq!(doc.borrow().video_thumbnail_content(), &[9]);
    doc.borrow_mut().set_bytes(&[]);
    assert_eq!(doc.borrow().bytes(), &[1, 2]);
    assert_eq!(photo.borrow().video_content(PhotoSize::Small, false), &[3]);
    assert_eq!(
        photo.borrow().encoded_image_slot(PhotoSize::Large),
        (&[5, 6][..], PhotoSize::Large)
    );
    photo
        .borrow_mut()
        .set_video(PhotoSize::Small, true, vec![4]);
    assert_eq!(photo.borrow().video_content(PhotoSize::Small, true), &[4]);
    let mut copy = DocumentMedia::default();
    copy.collect_local_bytes(&doc.borrow());
    assert_eq!(copy.bytes(), &[1, 2]);
    assert!(copy.is_good_thumbnail_wanted());
    assert_eq!(copy.video_thumbnail_content(), &[9]);
}
#[test]
fn cloud_metadata_grows_and_preserves_progressive_hints() {
    let mut file = CloudFileMetadata::default();
    let mut small = image(b's', 100, 50);
    small.bytes = vec![1, 2];
    assert_eq!(
        file.update(&small).cache_put_if_empty.unwrap().1,
        vec![1, 2]
    );
    let smaller = image(b'm', 50, 30);
    assert!(!file.update(&smaller).changed);
    assert_eq!(file.location.width, 100);
    file.failed = true;
    assert!(file.update(&image(b'x', 200, 100)).changed);
    assert!(!file.failed);
    let mut progressive = CloudFileMetadata::default();
    progressive.update(&ImageWithLocation {
        progressive_part_size: 50,
        ..Default::default()
    });
    assert_eq!(progressive.progressive_part_size, 50);
    progressive.clear();
    assert_eq!(progressive.progressive_part_size, 0);
}
#[test]
fn tl_photo_selection_progressive_and_empty_updates() {
    let s = SessionScope::new();
    let mut p = PhotoData::new(&s, 10);
    let sizes = vec![
        tl::mtp_photo_size(b"s".to_vec(), 10, 10, 100),
        tl::mtp_photo_size(b"x".to_vec(), 100, 100, 10000),
        tl::mtp_photo_size_progressive(b"y".to_vec(), 200, 100, vec![100, 300, 500]),
        tl::mtp_photo_stripped_size(b"i".to_vec(), vec![1, 2]),
    ];
    let videos = vec![
        tl::mtp_video_size(
            tl::tl::Flags::from_raw(0),
            b"p".to_vec(),
            10,
            10,
            100,
            Some(9.),
        ),
        tl::mtp_video_size(
            tl::tl::Flags::from_raw(0),
            b"v".to_vec(),
            100,
            50,
            200,
            Some(1.2349),
        ),
    ];
    let photo = tl::mtp_photo(
        tl::tl::Flags::from_raw(1),
        10,
        123,
        vec![8],
        42,
        sizes,
        Some(videos),
        4,
    );
    photo_apply_fields(&mut p, &photo);
    assert_eq!(p.date(), 42);
    assert_eq!(p.width(), 200);
    assert_eq!(p.image_byte_size(PhotoSize::Thumbnail), 300);
    assert_eq!(p.image_byte_size(PhotoSize::Large), 500);
    assert!(!p.has_exact(PhotoSize::Small));
    assert!(p.has_video_small());
    assert_eq!(p.video_start_position(), 1234);
    assert_eq!(p.inline_thumbnail_bytes(), &[1, 2]);
    photo_apply_fields(&mut p, &tl::mtp_photo_empty(10));
    assert_eq!(p.date(), 42);
    let invalid = tl::mtp_photo(
        tl::tl::Flags::from_raw(0),
        10,
        0,
        vec![],
        55,
        vec![tl::mtp_photo_size(b"y".to_vec(), 500, 500, 1)],
        None,
        4,
    );
    photo_apply_fields(&mut p, &invalid);
    assert_eq!(p.width(), 200);
    assert_eq!(p.date(), 42);
    let zero_date = tl::mtp_photo(
        tl::tl::Flags::from_raw(0),
        10,
        123,
        vec![],
        0,
        vec![tl::mtp_photo_size(b"y".to_vec(), 500, 500, 1)],
        None,
        4,
    );
    photo_apply_fields(&mut p, &zero_date);
    assert_eq!(p.width(), 200);
    let tied = vec![
        tl::mtp_photo_size(b"x".to_vec(), 5, 5, 1),
        tl::mtp_photo_size(b"y".to_vec(), 5, 5, 2),
    ];
    assert_eq!(
        photo_size_letter(find_document_thumbnail(&tied).unwrap()),
        b'x'
    );
    assert_eq!(video_start_time(-1.), 0);
    assert_eq!(video_start_time(1e30), i32::MAX);
}
#[test]
fn tl_document_mutation_order_thumbnail_inline_and_mime() {
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 10);
    let db = NativeRegistry::new();
    let thumbs = vec![
        tl::mtp_photo_path_size(b"j".to_vec(), vec![3]),
        tl::mtp_photo_stripped_size(b"i".to_vec(), vec![4]),
        tl::mtp_photo_size(b"m".to_vec(), 50, 25, 100),
    ];
    let doc = tl::mtp_document(
        tl::tl::Flags::from_raw(0),
        10,
        123,
        vec![5],
        42,
        b"VIDEO/MP4".to_vec(),
        200,
        Some(thumbs),
        None,
        4,
        vec![attr_video(2, 500, 250)],
    );
    document_apply_fields(&mut d, &doc, &db);
    assert!(d.is_video_file());
    assert!(d.has_thumbnail());
    assert!(d.has_remote_location());
    assert_eq!(d.dc_id(), 4);
    assert!(d.video_preload_location().valid());
    assert!(!d.is_null());
    assert_eq!(d.thumbnail_byte_size(), 100);
    assert!(!d.thumbnail_failed());
    assert!(!d.has_video_thumbnail());
    assert_eq!(d.video_thumbnail_byte_size(), 0);
    assert!(!d.video_thumbnail_failed());
    assert!(!d.video_thumbnail_location().valid());
    let tl::InputDocument::InputDocument(input) = d.mtp_input() else {
        panic!()
    };
    assert_eq!(input.access_hash, 123);
    assert_eq!(input.id, 10);
    assert_eq!(d.duration(), 1001);
    assert_eq!(d.inline_thumbnail_bytes(), &[4]);
    assert!(!d.inline_thumbnail_is_path());
    assert_eq!(d.mime_string(), "video/mp4");
    d.refresh_file_reference(&[9]);
    let DownloadLocation::Storage(t) = &d.thumbnail.location.file else {
        panic!()
    };
    assert_eq!(t.file_reference, vec![9]);
    document_apply_fields(&mut d, &tl::mtp_document_empty(10), &db);
    assert_eq!(d.date, 42);
    let invalid = tl::mtp_document(
        tl::tl::Flags::from_raw(0),
        10,
        0,
        vec![1],
        0,
        b"image/png".to_vec(),
        1,
        None,
        None,
        0,
        vec![],
    );
    document_apply_fields(&mut d, &invalid, &db);
    assert!(d.is_video_file());
    assert_eq!(d.size, 200);
}
#[test]
fn save_name_rules_filters_collisions_and_voice_override() {
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut d = DocumentData::new(&s, 1);
    d.set_mime_string("audio/mp3");
    d.set_attributes(
        &[tl::mtp_document_attribute_audio(
            tl::tl::Flags::from_raw(1 << 10),
            1,
            None,
            None,
            None,
        )],
        &db,
    );
    d.set_file_name("voice.mp3");
    let save = d.file_name_for_save("", &db, "All files (*)");
    assert_eq!(save.name, ".mp3");
    assert_eq!(save.prefix, "audio");
    assert_eq!(save.caption, SaveCaption::Audio);
    assert_eq!(
        d.file_name_for_save("explicit.mp3", &db, "All files (*)")
            .name,
        "explicit.mp3"
    );
    assert_eq!(
        promote_save_filter("x.ogg", "Audio (*.mp3 *.ogg);;All (*)"),
        "Audio (*.ogg *.mp3);;All (*)"
    );
    assert_eq!(
        promote_save_filter("x.wav", "Audio (*.mp3 *.ogg);;All (*)"),
        "Audio (*.wav *.mp3 *.ogg);;All (*)"
    );
    assert_eq!(
        promote_save_filter("x.MP3", "Audio (*.mp3 *.ogg);;All (*)"),
        "Audio (*.mp3 *.ogg);;All (*)"
    );
    assert_eq!(
        promote_save_filter("x.tdesktop-theme", "Theme (*.x);;All (*)"),
        ""
    );
    assert_eq!(promote_save_filter("x.wav", "Audio (*.wav)"), "");
    assert_eq!(promote_save_filter("x", "Any"), "Any");
    assert_eq!(
        available_file_name("/tmp/", "a.tar.gz", |p| p == "/tmp/a.tar.gz"
            || p == "/tmp/a.tar (2).gz"),
        "/tmp/a.tar (3).gz"
    );
    assert_eq!(
        file_name_for_save_platform("x.LNK ", FilenamePlatform::Windows),
        "x.LNK .download"
    );
    assert_eq!(
        file_name_for_save_platform("x.lnk", FilenamePlatform::Linux),
        "x.lnk"
    );
}
#[test]
fn video_variants_dedup_quality_and_closest_choice() {
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut original = DocumentData::new(&s, 1);
    original.size = 1000;
    original.set_attributes(&[attr_video(2, 1920, 1080)], &db);
    let mut a = DocumentData::new(&s, 2);
    a.size = 100;
    a.set_attributes(&[attr_video(2, 1280, 720)], &db);
    let mut b = DocumentData::new(&s, 3);
    b.size = 200;
    b.set_attributes(&[attr_video(2, 1280, 720)], &db);
    let mut c = DocumentData::new(&s, 4);
    c.size = 50;
    c.set_attributes(&[attr_video(2, 640, 360)], &db);
    let av = a.video_variant(720);
    let bv = b.video_variant(720);
    let cv = c.video_variant(360);
    original.set_video_qualities(vec![cv, bv, av]);
    assert_eq!(
        original.video().unwrap().qualities,
        vec![original.key(), a.key(), c.key()]
    );
    assert_eq!(
        original.resolve_video_quality(|k| if k == a.key() { 720 } else { 360 }),
        1080
    );
    original.dimensions = MediaSize::new(4096, 2160);
    assert_eq!(original.resolve_video_quality(|_| 720), 720);
    assert_eq!(original.resolve_original_video_quality(), 2160);
    original.video_mut().unwrap().real_video_size = MediaSize::new(1080, 1920);
    assert_eq!(original.resolve_video_quality(|_| panic!()), 1080);
    let own = original.video_variant(1080);
    assert_eq!(
        original.choose_quality(&[own, av, cv], VideoQuality::new(false, 540, false)),
        c.key()
    );
    assert_eq!(
        original.choose_quality(&[own, av], VideoQuality::new(false, 900, false)),
        a.key()
    );
    assert_eq!(
        original.choose_quality(&[av], VideoQuality::new(false, 720, true)),
        original.key()
    );
    assert_eq!(
        original.initial_playback_video_quality(VideoQuality::default(), "/real/path"),
        VideoQuality::new(true, 1080, true)
    );
    let old = original.video().unwrap().qualities.clone();
    original.set_video_qualities(vec![]);
    assert_eq!(original.video().unwrap().qualities, old);
}
#[test]
fn document_cache_keys_send_rights_wallpaper_and_playback() {
    use tg_core::data::data_chat_participant_status::ChatRestriction as R;
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut d = DocumentData::new(&s, 0x1234_5678_9abc_def0);
    assert_eq!(d.cache_key(), document_cache_key(0, d.id()));
    assert_eq!(
        d.good_thumbnail_cache_key(),
        document_thumb_cache_key(0, d.id())
    );
    assert!(!d.big_file_base_cache_key().valid());
    assert_eq!(d.cache_tag(), 0);
    assert_eq!(d.required_send_right(), R::SendFiles);
    d.set_content_url("https://test/file".into());
    assert_eq!(d.cache_key(), url_cache_key("https://test/file"));
    d.set_web_location(b"url".to_vec(), 1);
    assert_eq!(d.cache_key(), web_document_cache_key(b"url"));
    d.size = 100;
    d.set_attributes(&[attr_video(2, 100, 100)], &db);
    assert_eq!(d.required_send_right(), R::SendVideos);
    assert_eq!(d.location_type(), 0x3d0364ec);
    let mut lease = DocumentMedia::default();
    assert!(!lease.can_be_played(&d, ""));
    lease.set_bytes(&[1]);
    assert!(lease.loaded(""));
    assert!(lease.can_be_played(&d, ""));
    d.set_inapp_playback_failed();
    assert!(!lease.can_be_played(&d, ""));
    d.force_is_streamed_animation();
    assert_eq!(d.required_send_right(), R::SendGifs);
    assert_eq!(d.cache_tag(), 5);
    assert!(d.video().is_none());
    d.set_story_media(true);
    assert!(d.story_media());
    d.set_story_media(false);
    assert!(!d.story_media());
    assert!(d.supports_streaming());
    let mut wallpaper = DocumentData::new(&s, 2);
    wallpaper.dimensions = MediaSize::new(4096, 4096);
    wallpaper.thumbnail.location = image(b'm', 20, 20).location;
    wallpaper.size = MAX_FILE_IN_MEMORY;
    assert!(wallpaper.check_wallpaper_properties());
    assert_eq!(wallpaper.cache_tag(), 1);
    wallpaper.set_mime_string("application/x-tgwallpattern");
    assert!(wallpaper.is_pattern_wallpaper());
    assert!(wallpaper.is_pattern_wallpaper_svg());
    let loc = StorageFileLocation {
        kind: StorageFileType::Document,
        dc_id: 4,
        id: d.id(),
        ..Default::default()
    };
    assert_eq!(
        loc.big_file_base_cache_key(),
        CacheKey::new(0x11234, 0x5678_9abc_def0_0000)
    );
    let mut other = loc.clone();
    other.dc_id = 255;
    assert_eq!(
        loc.big_file_base_cache_key(),
        other.big_file_base_cache_key()
    );
    assert_eq!(document_cache_key(-1, 1), CacheKey::new(0x1ff, 1));
}
#[test]
fn data_and_cache_keeps_inclusive_payload_limit_and_live_lease_only() {
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 1);
    d.force_to_cache(true);
    assert!(d.active_media_view().is_none());
    assert_eq!(
        d.set_data_and_cache(&[7]),
        vec![DocumentEffect::CachePut {
            key: d.cache_key(),
            bytes: vec![7],
            tag: 0
        }]
    );
    assert!(d.active_media_view().is_none());
    let lease = d.create_media_view();
    let bytes = vec![2; MAX_FILE_IN_MEMORY as usize];
    assert_eq!(d.set_data_and_cache(&bytes).len(), 1);
    assert_eq!(lease.borrow().bytes().len(), MAX_FILE_IN_MEMORY as usize);
    d.size = MAX_FILE_IN_MEMORY;
    assert!(d.set_data_and_cache(&[3]).is_empty());
    assert_eq!(lease.borrow().bytes(), &[3]);
}
#[test]
fn metadata_accessors_and_thumbnail_eligibility() {
    let s = SessionScope::new();
    let mut d = DocumentData::new(&s, 1);
    let mut p = PhotoData::new(&s, 2);
    assert_eq!(d.wire_key(), s.document_key(1));
    assert!(d.is_null());
    assert!(!d.video_preload_location().valid());
    assert_eq!(p.wire_key(), s.photo_key(2));
    assert_eq!(d.dc_id(), 0);
    assert_eq!(p.dc_id(), 0);
    assert_eq!(d.media_key(), (0x4e45abe9_00000000, 1));
    assert_eq!(p.media_key(), (0, 2));
    assert!(!d.cancelled());
    d.reset_cancelled();
    assert!(!d.loaded_in_media_cache());
    assert!(!DocumentMedia::may_have_good_thumbnail(&d));
    d.set_file_name("a.svg");
    assert!(d.is_svg_image());
    assert!(DocumentMedia::may_have_good_thumbnail(&d));
    assert_eq!(
        DocumentMedia::video_thumbnail_size(&d),
        MediaSize::new(0, 0)
    );
    p.set_remote_location(-1, 0, &[1]);
    assert_eq!(p.dc_id(), -1);
    let tl::InputPhoto::InputPhoto(input) = p.mtp_input() else {
        panic!()
    };
    assert_eq!(input.id, 2);
    assert_eq!(input.access_hash, 0);
    assert_eq!(input.file_reference, vec![1]);
    assert!(matches!(
        d.mtp_input(),
        tl::InputDocument::InputDocumentEmpty(_)
    ));
    p.images[2].failed = true;
    assert!(p.failed(PhotoSize::Small));
    p.clear_failed(PhotoSize::Small);
    assert!(!p.failed(PhotoSize::Small));
    assert_eq!(p.video_byte_size(PhotoSize::Large), 0);
    assert!(!p.video_loaded(PhotoSize::Small));
    assert!(!p.video_failed(PhotoSize::Small));
    assert!(!p.video_playback_failed());
    assert_eq!(
        tg_core::data::data_photo_media::PhotoMedia::video_size(&p, PhotoSize::Small),
        MediaSize::new(0, 0)
    );
    p.clear_inline_thumbnail_bytes();
    d.clear_inline_thumbnail_bytes();
    assert!(p.inline_thumbnail_bytes().is_empty());
    assert!(d.inline_thumbnail_bytes().is_empty());
    assert!(p.active_media_view().is_none());
}
#[test]
fn location_cache_keys_use_fixed_tags_bit_widths_and_le_digest_slices() {
    let base = StorageFileLocation {
        dc_id: 1,
        local_id: -1,
        volume_id: 100,
        id: 42,
        ..Default::default()
    };
    for (kind, letter, high, low) in [
        (StorageFileType::Legacy, 0, 0xffff_ffff_0301, 100),
        (StorageFileType::PeerPhoto, 0, 0xffff_ffff_0901, 100),
        (StorageFileType::StickerSetThumb, 0, 0xffff_ffff_0a01, 100),
        (StorageFileType::Encrypted, 0, 0x401, 42),
        (StorageFileType::Secure, 0, 0x601, 42),
        (StorageFileType::Document, 0, 0x101, 42),
        (StorageFileType::Document, b'm', 0x6d0501, 42),
        (StorageFileType::Photo, b'm', 0x6d0801, 42),
        (StorageFileType::Takeout, 0, 0x700, 0),
    ] {
        let location = StorageFileLocation {
            kind,
            size_letter: letter,
            ..base.clone()
        };
        assert_eq!(location.cache_key(), CacheKey::new(high, low));
    }
    let stream = StorageFileLocation {
        kind: StorageFileType::GroupCallStream,
        local_id: 1,
        volume_id: 2,
        size_letter: 3,
        ..base.clone()
    };
    assert_eq!(stream.cache_key(), CacheKey::new(0x0300_0000_0021_0b01, 42));
    let sticker = StorageFileLocation {
        kind: StorageFileType::StickerSetThumb,
        dc_id: 4,
        local_id: 2,
        volume_id: 0x1122_3344_5566_7788,
        ..base
    };
    assert_eq!(
        sticker.big_file_base_cache_key(),
        CacheKey::new(0x2080411, 0x2233_4455_6677_8800)
    );
    assert!(!DownloadLocation::default().cache_key().valid());
    assert!(!DownloadLocation::InMemory(vec![1]).cache_key().valid());
    assert!(!DownloadLocation::Url(String::new()).cache_key().valid());
    assert_eq!(
        DownloadLocation::AudioAlbumThumb { document_id: 0 }.cache_key(),
        CacheKey::new(0x300, 0)
    );
    // Independent SHA256 known answers generated with Python hashlib, not Rust.
    assert_eq!(
        web_document_cache_key(b"url"),
        CacheKey::new(0x204_abeb_e528, 0xda63_df37_e2f6_d8d9)
    );
    assert_eq!(
        url_cache_key("https://test/file"),
        CacheKey::new(0x9bc6_3513_d759, 0x19b2_050b_a588_af37)
    );
    assert_eq!(
        web_document_cache_key(&[0, 255]),
        CacheKey::new(0x204_6a7d_eb06, 0x9074_dffb_e519_ee69)
    );
}
#[test]
fn cached_photo_document_video_thumbnail_and_geo_projection() {
    let s = SessionScope::new();
    let db = NativeRegistry::new();
    let mut p = PhotoData::new(&s, 10);
    let photo = tl::mtp_photo(
        tl::tl::Flags::from_raw(0),
        10,
        0,
        vec![1],
        42,
        vec![tl::mtp_photo_cached_size(b"x".to_vec(), 20, 10, vec![3, 4])],
        None,
        4,
    );
    let effects = photo_apply_fields(&mut p, &photo);
    assert_eq!(p.image_byte_size(PhotoSize::Large), 2);
    assert!(
        matches!(&effects[..],[PhotoEffect::CachePutIfEmpty{bytes,tag:1,..}] if bytes == &[3,4])
    );
    let small = tl::mtp_video_size(tl::tl::Flags::from_raw(0), b"p".to_vec(), 10, 10, 0, None);
    let big = tl::mtp_video_size(tl::tl::Flags::from_raw(0), b"f".to_vec(), 50, 50, 20, None);
    let tied = tl::mtp_video_size(tl::tl::Flags::from_raw(0), b"v".to_vec(), 50, 50, 30, None);
    let list = vec![small.clone(), big.clone(), tied];
    let tl::VideoSize::VideoSize(v) = find_document_video_thumbnail(&list).unwrap() else {
        panic!()
    };
    assert_eq!(v.type_, b"f");
    assert!(find_document_video_thumbnail(&[]).is_none());
    assert!(find_document_video_thumbnail(&[tl::mtp_video_size(
        tl::tl::Flags::from_raw(0),
        b"z".to_vec(),
        0,
        10,
        1,
        None
    )])
    .is_none());
    let doc = tl::mtp_document(
        tl::tl::Flags::from_raw(0),
        11,
        123,
        vec![8],
        42,
        b"image/webp".to_vec(),
        300,
        None,
        Some(vec![small, big]),
        4,
        vec![
            tl::mtp_document_attribute_image_size(100, 100),
            attr_sticker(false, 0),
        ],
    );
    let mut d = DocumentData::new(&s, 11);
    document_apply_fields(&mut d, &doc, &db);
    assert!(d.has_video_thumbnail());
    assert_eq!(d.video_thumbnail_byte_size(), 20);
    assert_eq!(
        DocumentMedia::video_thumbnail_size(&d),
        MediaSize::new(50, 50)
    );
    assert!(d.is_premium_sticker());
    assert_eq!(d.cache_tag(), 2);
    assert_eq!(
        d.required_send_right(),
        tg_core::data::data_chat_participant_status::ChatRestriction::SendStickers
    );
    assert!(!d.use_streaming_loader());
    d.refresh_file_reference(&[9]);
    let DownloadLocation::Storage(v) = &d.video_thumbnail_location().file else {
        panic!()
    };
    assert_eq!(v.file_reference, vec![9]);
    let geo = DownloadLocation::Geo(GeoPointLocation {
        lat: 0.,
        lon: 0.,
        width: 256,
        height: 256,
        zoom: 16,
        scale: 1,
        ..Default::default()
    });
    assert_eq!(
        geo.cache_key(),
        CacheKey::new(0x0401_0100_0100, 0x1575_2a00_1575_2a00)
    );
}
