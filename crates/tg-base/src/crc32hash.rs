//! Port of `lib_base/base/crc32hash.h` and `crc32hash.cpp`.
//!
//! CRC-32 with the reflected polynomial 0x04C11DB7 (the zlib / IEEE 802.3
//! variant). The table is built the same way as in the original.

use std::sync::LazyLock;

fn reflect(mut value: u32, ch: u32) -> u32 {
    let mut result = 0u32;
    for i in 1..(ch + 1) {
        if value & 1 != 0 {
            result |= 1 << (ch - i);
        }
        value >>= 1;
    }
    result
}

static TABLE: LazyLock<[u32; 256]> = LazyLock::new(|| {
    let poly = 0x04c1_1db7u32;
    let mut data = [0u32; 256];
    for (i, entry) in data.iter_mut().enumerate() {
        *entry = reflect(i as u32, 8) << 24;
        for _ in 0..8 {
            *entry = (*entry << 1) ^ if *entry & (1 << 31) != 0 { poly } else { 0 };
        }
        *entry = reflect(*entry, 32);
    }
    data
});

/// `base::crc32`. Returns the checksum reinterpreted as a signed value, as
/// the original does.
pub fn crc32(data: &[u8]) -> i32 {
    let table = &*TABLE;
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc = (crc >> 8) ^ table[((crc & 0xFF) ^ u32::from(byte)) as usize];
    }
    (crc ^ 0xffff_ffff) as i32
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn check_value() {
        // The standard CRC-32 check value (CRC catalogue, "CRC-32/ISO-HDLC").
        assert_eq!(crc32(b"123456789") as u32, 0xCBF4_3926);
    }

    #[test]
    fn known_vectors() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a") as u32, 0xE8B7_BE43);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog") as u32,
            0x414F_A339
        );
        // Signed result, as in the original.
        assert!(crc32(b"123456789") < 0);
    }

    #[test]
    fn table_matches_zlib() {
        let table = &*super::TABLE;
        assert_eq!(table[0], 0);
        assert_eq!(table[1], 0x7707_3096);
        assert_eq!(table[255], 0x2D02_EF8D);
    }
}
