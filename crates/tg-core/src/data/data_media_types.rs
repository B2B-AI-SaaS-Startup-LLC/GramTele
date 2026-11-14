//! Scalar media contracts from data_types.h and pure document/photo helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaSize {
    pub width: i32,
    pub height: i32,
}
impl Default for MediaSize {
    fn default() -> Self {
        Self::INVALID
    }
}
impl MediaSize {
    pub const INVALID: Self = Self {
        width: -1,
        height: -1,
    };
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
    pub fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }
    /// QSize::scaled(KeepAspectRatio), integer arithmetic (including zero sides).
    pub fn fit(self, bound: Self) -> Self {
        if self.width == 0 || self.height == 0 {
            return bound;
        }
        let use_height = i64::from(bound.height) * i64::from(self.width) / i64::from(self.height)
            <= i64::from(bound.width);
        if use_height {
            Self::new(
                (i64::from(bound.height) * i64::from(self.width) / i64::from(self.height)) as i32,
                bound.height,
            )
        } else {
            Self::new(
                bound.width,
                (i64::from(bound.width) * i64::from(self.height) / i64::from(self.width)) as i32,
            )
        }
    }
    pub fn photo_size(self) -> Option<Self> {
        if self.is_empty() {
            return None;
        }
        if self.width <= 2560 && self.height <= 2560 {
            return Some(self);
        }
        let r = self.fit(Self::new(2560, 2560));
        Some(Self::new(r.width.max(1), r.height.max(1)))
    }
}
pub fn good_sticker_dimensions(width: i32, height: i32) -> bool {
    width > 0 && height > 0 && i64::from(width) * i64::from(height) <= 2560 * 2560
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum DocumentType {
    #[default]
    File = 0,
    Video = 1,
    Song = 2,
    Sticker = 3,
    Animated = 4,
    Voice = 5,
    RoundVideo = 6,
    Wallpaper = 7,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i8)]
pub enum FileStatus {
    DownloadFailed = -2,
    UploadFailed = -1,
    #[default]
    Ready = 1,
}
/// media_common.h's bitfields. `height` is masked when constructed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoQuality {
    pub manual: bool,
    pub height: u32,
    pub original: bool,
}
impl VideoQuality {
    pub fn new(manual: bool, height: u32, original: bool) -> Self {
        Self {
            manual,
            height: height & 0x3fff_ffff,
            original,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UploadState {
    pub offset: i64,
    pub size: i64,
    pub waiting_for_album: bool,
    pub preparing: bool,
    pub prepare_progress: f64,
}
impl UploadState {
    pub fn new(size: i64) -> Self {
        Self {
            size,
            ..Self::default()
        }
    }
    pub fn progress(&self, document: bool) -> f64 {
        if self.size <= 0 || (document && self.preparing) {
            0.
        } else {
            (self.offset as f64 / self.size as f64).clamp(0., 1.)
        }
    }
}
pub fn milliseconds_from_seconds(seconds: f64) -> i64 {
    let ms = seconds * 1000.;
    if ms.is_finite() && ms.abs() < i64::MAX as f64 {
        ms.round() as i64
    } else {
        0
    }
}
pub fn resolve_attribute_vs_transcode_quality(attributes: i32, max: i32) -> i32 {
    if max > 0 && (attributes < max || f64::from(attributes) > f64::from(max) * 1.5) {
        max
    } else {
        attributes
    }
}
/// The C++ implementation reads unaligned host uint16s. Explicit little endian
/// preserves the pinned x86 format without unaligned/overrun accesses.
pub fn document_waveform_decode(bytes: &[u8]) -> Vec<i8> {
    (0..bytes.len() * 8 / 5)
        .map(|i| {
            let start = i * 5 / 8;
            let word = u16::from(bytes[start])
                | (u16::from(bytes.get(start + 1).copied().unwrap_or(0)) << 8);
            ((word >> (i * 5 % 8)) & 31) as i8
        })
        .collect()
}
pub fn document_waveform_encode_5bit(waveform: &[i8]) -> Vec<u8> {
    let len = (waveform.len() * 5).div_ceil(8);
    let mut out = vec![0; len + 1];
    for (i, &v) in waveform.iter().enumerate() {
        let word = ((v as u16) & 31) << (i * 5 % 8);
        out[i * 5 / 8] |= word as u8;
        out[i * 5 / 8 + 1] |= (word >> 8) as u8;
    }
    out.truncate(len);
    out
}
