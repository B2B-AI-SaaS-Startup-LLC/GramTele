//! Metadata-only TL application from data_session.cpp and
//! ui/image/image_location_factory.cpp. No image decoding or network calls.
use super::{data_document::*, data_image_location::*, data_media_location::*, data_photo::*};
use crate::core::mime_type::MimeDatabase;
use tg_scheme as tl;
fn first_max<T>(items: &[T], score: impl Fn(&T) -> i32) -> Option<&T> {
    items
        .iter()
        .reduce(|a, b| if score(b) > score(a) { b } else { a })
}
pub fn photo_size_letter(size: &tl::PhotoSize) -> u8 {
    use tl::PhotoSize::*;
    match size {
        PhotoSizeEmpty(_) => 0,
        PhotoSize(d) => d.type_.first().copied().unwrap_or(0),
        PhotoCachedSize(d) => d.type_.first().copied().unwrap_or(0),
        PhotoSizeProgressive(d) => d.type_.first().copied().unwrap_or(0),
        PhotoStrippedSize(d) => d.type_.first().copied().unwrap_or(0),
        PhotoPathSize(d) => d.type_.first().copied().unwrap_or(0),
    }
}
fn size_area(size: &tl::PhotoSize) -> i32 {
    use tl::PhotoSize::*;
    match size {
        PhotoSize(d) => d.w.wrapping_mul(d.h),
        PhotoCachedSize(d) => d.w.wrapping_mul(d.h),
        PhotoSizeProgressive(d) => d.w.wrapping_mul(d.h),
        _ => 0,
    }
}
pub fn find_inline_thumbnail(sizes: &[tl::PhotoSize]) -> (&[u8], bool) {
    for s in sizes {
        if let tl::PhotoSize::PhotoStrippedSize(d) = s {
            return (&d.bytes, false);
        }
    }
    for s in sizes {
        if let tl::PhotoSize::PhotoPathSize(d) = s {
            return (&d.bytes, true);
        }
    }
    (&[], false)
}
pub fn find_document_thumbnail(sizes: &[tl::PhotoSize]) -> Option<&tl::PhotoSize> {
    first_max(sizes, size_area).filter(|s| size_area(s) > 0)
}
pub fn find_document_video_thumbnail(sizes: &[tl::VideoSize]) -> Option<&tl::VideoSize> {
    first_max(sizes, |v| {
        if let tl::VideoSize::VideoSize(d) = v {
            d.w.wrapping_mul(d.h)
        } else {
            0
        }
    })
    .filter(|v| matches!(v, tl::VideoSize::VideoSize(d) if d.w.wrapping_mul(d.h) > 0))
}
fn size_by_level<'a>(sizes: &'a [tl::PhotoSize], levels: &[u8]) -> Option<&'a tl::PhotoSize> {
    sizes
        .iter()
        .filter_map(|s| {
            levels
                .iter()
                .position(|&c| c == photo_size_letter(s))
                .map(|i| (i, s))
        })
        .reduce(|a, b| if b.0 < a.0 { b } else { a })
        .map(|(_, s)| s)
}
struct Remote<'a> {
    dc: i32,
    id: u64,
    access: u64,
    reference: &'a [u8],
    kind: StorageFileType,
}
impl Remote<'_> {
    fn location(&self, letter: u8, width: i32, height: i32) -> ImageLocation {
        ImageLocation::new(
            DownloadLocation::Storage(StorageFileLocation {
                dc_id: self.dc as u16,
                kind: self.kind,
                size_letter: letter,
                id: self.id,
                access_hash: self.access,
                file_reference: self.reference.to_vec(),
                ..Default::default()
            }),
            width,
            height,
        )
    }
    fn image(&self, size: &tl::PhotoSize) -> ImageWithLocation {
        if self.kind == StorageFileType::Photo && self.access == 0 && self.reference.is_empty() {
            return ImageWithLocation::default();
        }
        use tl::PhotoSize::*;
        match size {
            PhotoSize(d) => ImageWithLocation {
                location: self.location(photo_size_letter(size), d.w, d.h),
                bytes_count: d.size,
                ..Default::default()
            },
            PhotoCachedSize(d) => ImageWithLocation {
                location: self.location(photo_size_letter(size), d.w, d.h),
                bytes_count: d.bytes.len() as i32,
                bytes: d.bytes.clone(),
                ..Default::default()
            },
            PhotoSizeProgressive(d) => {
                d.sizes
                    .last()
                    .map_or_else(Default::default, |&n| ImageWithLocation {
                        location: self.location(photo_size_letter(size), d.w, d.h),
                        bytes_count: n,
                        ..Default::default()
                    })
            }
            _ => ImageWithLocation::default(),
        }
    }
    fn video(&self, size: &tl::VideoSize) -> ImageWithLocation {
        if let tl::VideoSize::VideoSize(d) = size {
            ImageWithLocation {
                location: self.location(d.type_.first().copied().unwrap_or(0), d.w, d.h),
                bytes_count: d.size,
                ..Default::default()
            }
        } else {
            ImageWithLocation::default()
        }
    }
}
pub fn video_start_time(seconds: f64) -> i32 {
    (seconds * 1000.).floor().clamp(0., f64::from(i32::MAX)) as i32
}
pub fn photo_apply_fields(photo: &mut PhotoData, value: &tl::Photo) -> Vec<PhotoEffect> {
    let tl::Photo::Photo(d) = value else {
        return Vec::new();
    };
    let remote = Remote {
        dc: d.dc_id,
        id: d.id,
        access: d.access_hash,
        reference: &d.file_reference,
        kind: StorageFileType::Photo,
    };
    let progressive = first_max(&d.sizes, |s| {
        if let tl::PhotoSize::PhotoSizeProgressive(_) = s {
            size_area(s)
        } else {
            0
        }
    })
    .filter(|s| matches!(s, tl::PhotoSize::PhotoSizeProgressive(_)));
    let large = progressive
        .or_else(|| size_by_level(&d.sizes, b"ydxcwmbsa"))
        .map_or_else(Default::default, |s| remote.image(s));
    if !large.location.valid() || d.date == 0 {
        return Vec::new();
    }
    let small = if progressive.is_some() {
        ImageWithLocation::default()
    } else {
        size_by_level(&d.sizes, b"sa").map_or_else(Default::default, |s| remote.image(s))
    };
    let thumbnail = if let Some(tl::PhotoSize::PhotoSizeProgressive(s)) = progressive {
        ImageWithLocation {
            progressive_part_size: s.sizes.get(1).copied().unwrap_or_default(),
            ..Default::default()
        }
    } else {
        size_by_level(&d.sizes, b"mbsa").map_or_else(Default::default, |s| remote.image(s))
    };
    let video_sizes = d.video_sizes.as_deref().unwrap_or_default();
    let video_area = |s: &tl::VideoSize| {
        if let tl::VideoSize::VideoSize(s) = s {
            if s.size != 0 {
                s.w.wrapping_mul(s.h)
            } else {
                0
            }
        } else {
            0
        }
    };
    let video_small = video_sizes
        .iter()
        .find(|s| matches!(s, tl::VideoSize::VideoSize(s) if s.type_.first() == Some(&b'p')))
        .filter(|s| video_area(s) > 0);
    let video_large = first_max(video_sizes, video_area).filter(|s| video_area(s) > 0);
    let start = video_large.map_or(0, |s| {
        if let tl::VideoSize::VideoSize(s) = s {
            i64::from(video_start_time(s.video_start_ts.unwrap_or_default()))
        } else {
            0
        }
    });
    let (inline, path) = find_inline_thumbnail(&d.sizes);
    photo.set_remote_location(d.dc_id, d.access_hash, &d.file_reference);
    photo.set_fields(d.date, d.is_has_stickers());
    photo.update_images(
        if path { &[] } else { inline },
        &small,
        &thumbnail,
        &large,
        &video_small.map_or_else(Default::default, |s| remote.video(s)),
        &video_large.map_or_else(Default::default, |s| remote.video(s)),
        start,
    )
}
pub fn document_apply_fields(
    document: &mut DocumentData,
    value: &tl::Document,
    database: &impl MimeDatabase,
) -> Vec<DocumentEffect> {
    let tl::Document::Document(d) = value else {
        return Vec::new();
    };
    if d.date == 0 {
        return Vec::new();
    }
    let remote = Remote {
        dc: d.dc_id,
        id: d.id,
        access: d.access_hash,
        reference: &d.file_reference,
        kind: StorageFileType::Document,
    };
    let sizes = d.thumbs.as_deref().unwrap_or_default();
    let (inline, path) = find_inline_thumbnail(sizes);
    let thumbnail =
        find_document_thumbnail(sizes).map_or_else(Default::default, |s| remote.image(s));
    let video = find_document_video_thumbnail(d.video_thumbs.as_deref().unwrap_or_default());
    let premium = matches!(video, Some(tl::VideoSize::VideoSize(d)) if d.type_ == b"f");
    document.date = d.date;
    document.set_mime_string(&String::from_utf8_lossy(&d.mime_type));
    let mut effects = document.update_thumbnails(
        inline,
        path,
        &thumbnail,
        &video.map_or_else(Default::default, |s| remote.video(s)),
        premium,
    );
    document.size = d.size as i64;
    effects.extend(document.set_attributes(&d.attributes, database));
    document.recount_is_image();
    if d.dc_id != 0 && d.access_hash != 0 {
        document.dc_id = d.dc_id;
        document.access_hash = d.access_hash;
        document.file_reference = d.file_reference.clone();
    }
    effects
}
/// Complete synchronous local-file model phase. The plain metadata helper is
/// also used by pure fixtures; real Documents ownership supplies this service.
pub fn document_apply_fields_with_files(
    document: &mut DocumentData,
    value: &tl::Document,
    database: &impl MimeDatabase,
    files: &dyn crate::core::file_location::FileLocations,
) -> Vec<DocumentEffect> {
    let old_dc = document.dc_id;
    let old_access = document.access_hash;
    let mut effects = document_apply_fields(document, value, database);
    if let tl::Document::Document(d) = value {
        if d.date != 0 && d.dc_id != 0 && d.access_hash != 0 {
            document.dc_id = old_dc;
            document.access_hash = old_access;
            effects.extend(document.set_remote_location(
                d.dc_id,
                d.access_hash,
                &d.file_reference,
                files,
            ));
        }
    }
    effects
}
