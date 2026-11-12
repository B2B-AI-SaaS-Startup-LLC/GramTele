//! `core_types.h`: TL codecs are provided by tg-tl; byte order is explicitly LE.
pub use tg_tl::{Buffer, Int128, Int256, Prime, TypeId};
pub type RequestId = i32;
pub type MsgId = u64;
pub type PingId = u64;
pub type DcId = i32;
pub type ShiftedDcId = i32;
pub const DC_SHIFT: i32 = 10000;
pub const CONFIG_DC_SHIFT: i32 = 1;
pub const LOGOUT_DC_SHIFT: i32 = 2;
pub const UPDATER_DC_SHIFT: i32 = 3;
pub const EXPORT_DC_SHIFT: i32 = 4;
pub const EXPORT_MEDIA_DC_SHIFT: i32 = 5;
pub const GROUP_CALL_STREAM_DC_SHIFT: i32 = 6;
pub const STATS_DC_SHIFT: i32 = 7;
pub const MAX_MEDIA_DC_COUNT: i32 = 0x10;
pub const BASE_DOWNLOAD_DC_SHIFT: i32 = 0x10;
pub const BASE_UPLOAD_DC_SHIFT: i32 = 0x20;
pub const DESTROY_KEY_START_DC_SHIFT: i32 = 0x100;
pub const fn bare_dc_id(id: ShiftedDcId) -> DcId {
    id % DC_SHIFT
}
pub const fn shift_dc_id(id: DcId, shift: i32) -> ShiftedDcId {
    id + DC_SHIFT * shift
}
pub const fn get_dc_id_shift(id: ShiftedDcId) -> i32 {
    id / DC_SHIFT
}
pub const RPC_RESULT: u32 = 0xf35c6d01;
pub const MSG_CONTAINER: u32 = 0x73f1f8dc;
pub const GZIP_PACKED: u32 = 0x3072cfa1;
pub const CORE_MESSAGE: u32 = u32::MAX;
pub const LAYERS: [u32; 18] = [
    0x53835315, 0x289dd1f6, 0xb7475268, 0xdea0d430, 0x417a57ae, 0x3a64d54d, 0xa5be56d3, 0xe9abd9fd,
    0x76715a63, 0x39620c41, 0xa6b88fdf, 0xdda60d3c, 0x427c8ea2, 0x2b9b08fa, 0xb4418b64, 0xcf5f0987,
    0x50858a19, 0x1c900537,
];

/// Injectable source; no protocol operation obtains entropy implicitly.
pub trait Random {
    fn fill(&mut self, data: &mut [u8]);
    fn u32(&mut self) -> u32 {
        let mut b = [0; 4];
        self.fill(&mut b);
        u32::from_le_bytes(b)
    }
    fn u64(&mut self) -> u64 {
        let mut b = [0; 8];
        self.fill(&mut b);
        u64::from_le_bytes(b)
    }
}
pub struct SystemRandom;
impl Random for SystemRandom {
    fn fill(&mut self, data: &mut [u8]) {
        tg_base::random::random_fill(data)
    }
}
/// A snapshot from the owner. Advancing a test never requires sleeping.
#[derive(Clone, Copy, Debug, Default)]
pub struct Time {
    pub monotonic_ms: i64,
    pub unix_seconds: i64,
}
