//! Port of `lib_base/base/bytes.h` and `bytes.cpp`.
//!
//! In C++ `bytes::span`, `bytes::const_span`, `bytes::vector` and
//! `bytes::array<N>` are aliases over `gsl::byte`. In Rust they are plain
//! `&mut [u8]`, `&[u8]`, `Vec<u8>` and `[u8; N]`; the aliases below exist only
//! to make the mapping searchable. `make_span`, `make_vector` and
//! `object_as_span` have no counterpart: slices coerce directly, `to_vec()`
//! copies, and `bytemuck::bytes_of` views a POD value as bytes.
//!
//! What stays as functions is the behaviour that differs from the standard
//! library: `copy` allows a longer destination, `compare` orders by size
//! first, `concatenate` joins many parts and `set_random` fills from the
//! cryptographic generator.

use std::cmp::Ordering;

/// `bytes::type`.
pub type Type = u8;

/// `bytes::vector`.
pub type Vector = Vec<u8>;

/// `bytes::array<Size>`.
pub type Array<const SIZE: usize> = [u8; SIZE];

/// `bytes::copy`: copies `source` to the beginning of `destination`.
///
/// Panics if `destination` is shorter than `source` (the original
/// `Expects`). Unlike `<[u8]>::copy_from_slice` the destination may be longer.
pub fn copy(destination: &mut [u8], source: &[u8]) {
    assert!(
        destination.len() >= source.len(),
        "bytes::copy: destination is shorter than source"
    );
    destination[..source.len()].copy_from_slice(source);
}

/// `bytes::move` for overlapping regions in one buffer. The borrow checker
/// cannot express overlapping `&mut [u8]` and `&[u8]`; pass the source range
/// and destination offset instead (`storage_cache_binlog_reader.cpp`).
pub fn move_bytes(buffer: &mut [u8], source: std::ops::Range<usize>, destination: usize) {
    buffer.copy_within(source, destination);
}

/// `bytes::set_with_const`.
pub fn set_with_const(destination: &mut [u8], value: u8) {
    destination.fill(value);
}

/// `bytes::compare`: shorter spans are less, equal sizes compare as `memcmp`.
///
/// This is *not* lexicographic order: `[0xFF]` is less than `[0x00, 0x00]`.
/// Returns -1, 0 or 1 (the C++ version returns the raw `memcmp` value for
/// equal sizes; callers only test its sign).
pub fn compare(a: &[u8], b: &[u8]) -> i32 {
    match a.len().cmp(&b.len()) {
        Ordering::Greater => 1,
        Ordering::Less => -1,
        Ordering::Equal => match a.cmp(b) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        },
    }
}

/// `bytes::concatenate(args...)` and `bytes::concatenate(range)`.
pub fn concatenate<T: AsRef<[u8]>>(parts: &[T]) -> Vector {
    let size = parts.iter().map(|part| part.as_ref().len()).sum();
    let mut result = Vec::with_capacity(size);
    for part in parts {
        result.extend_from_slice(part.as_ref());
    }
    result
}

/// `bytes::set_random`.
pub fn set_random(destination: &mut [u8]) {
    if !destination.is_empty() {
        crate::random::random_fill(destination);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_into_longer_destination() {
        let mut dst = [9u8; 5];
        copy(&mut dst, &[1, 2, 3]);
        assert_eq!(dst, [1, 2, 3, 9, 9]);
    }

    #[test]
    fn move_overlapping_regions() {
        let mut bytes = *b"abcdef";
        move_bytes(&mut bytes, 2..6, 0);
        assert_eq!(&bytes, b"cdefef");
        move_bytes(&mut bytes, 0..4, 2);
        assert_eq!(&bytes, b"cdcdef");
    }

    #[test]
    #[should_panic]
    fn copy_into_shorter_destination_panics() {
        let mut dst = [0u8; 2];
        copy(&mut dst, &[1, 2, 3]);
    }

    #[test]
    fn compare_orders_by_size_first() {
        assert_eq!(compare(&[0xFF], &[0x00, 0x00]), -1);
        assert_eq!(compare(&[0x00, 0x00], &[0xFF]), 1);
        assert_eq!(compare(&[1, 2], &[1, 3]), -1);
        assert_eq!(compare(&[1, 3], &[1, 2]), 1);
        assert_eq!(compare(&[1, 2], &[1, 2]), 0);
        assert_eq!(compare(&[], &[]), 0);
    }

    #[test]
    fn concatenate_parts() {
        let a = [1u8, 2];
        let b = [3u8];
        let c: &[u8] = &[];
        assert_eq!(concatenate(&[&a[..], &b[..], c]), vec![1, 2, 3]);
        assert!(concatenate::<&[u8]>(&[]).is_empty());
    }

    #[test]
    fn set_random_changes_buffer() {
        let mut buffer = [0u8; 64];
        set_random(&mut buffer);
        assert_ne!(buffer, [0u8; 64]);
        set_random(&mut []);
    }
}
