//! Port of `lib_tl/tl/tl_basic_types.h` / `tl_basic_types.cpp` together with
//! the `Writer<mtpBuffer>` / `Reader<mtpPrime>` specializations from
//! `mtproto/core_types.h`, which are what make the templates concrete.
//!
//! Mapping of the original classes:
//!
//! | C++ (`tl::`)        | Rust                                   |
//! |---------------------|----------------------------------------|
//! | `int_type`          | `i32`                                  |
//! | `long_type`         | `u64` (the original stores `uint64`)   |
//! | `int64_type`        | `i64`                                  |
//! | `double_type`       | `f64`                                  |
//! | `int128_type`       | [`Int128`] (`l`, `h`)                  |
//! | `int256_type`       | [`Int256`] (`l`, `h`)                  |
//! | `string_type`       | `Vec<u8>` (`v` is a `QByteArray`)      |
//! | `vector_type<T>`    | [`BareVec<T>`] (bare `vector<T>`)      |
//! | `boxed<vector_type>`| `Vec<T>` (boxed `Vector<T>`)           |
//! | `flags_type<F>`     | [`crate::Flags`]                       |
//! | `conditional<T>`    | `Option<&T>` / `Option<T>` + [`Conditional`] |
//! | `LengthCounter`     | [`count_length`]                       |
//!
//! Serialization targets one concrete accumulator, [`Buffer`]
//! (`mtpBuffer = QVector<mtpPrime>`), instead of being generic over the
//! accumulator: the original instantiates every `write` twice (for
//! `mtpBuffer` and for `LengthCounter`), which for the whole schema would
//! double the generated code. [`count_length`] serializes into a scratch
//! buffer instead; it is only used by the MTProto layer when sending.

use crate::error::{Error, Result};

/// `mtpPrime`: one 32-bit word of a TL buffer.
pub type Prime = i32;

/// `mtpTypeId`: a constructor id.
pub type TypeId = u32;

/// `mtpBuffer`: a TL serialization buffer.
pub type Buffer = Vec<Prime>;

/// `tl::id_int`.
pub const ID_INT: TypeId = 0xa850_9bda;
/// `tl::id_long`.
pub const ID_LONG: TypeId = 0x2207_6cba;
/// `tl::id_int128`.
pub const ID_INT128: TypeId = 0x4bb5_362b;
/// `tl::id_int256`.
pub const ID_INT256: TypeId = 0x0929_c32f;
/// `tl::id_double`.
pub const ID_DOUBLE: TypeId = 0x2210_c154;
/// `tl::id_string`.
pub const ID_STRING: TypeId = 0xb528_6e24;
/// `tl::id_vector`.
pub const ID_VECTOR: TypeId = 0x1cb5_c415;
/// `tl::id_bytes`.
pub const ID_BYTES: TypeId = ID_STRING;
/// `tl::id_flags`.
pub const ID_FLAGS: TypeId = ID_INT;
/// `tl::id_flags64`.
pub const ID_FLAGS64: TypeId = ID_LONG;
/// `mtpc_boolFalse` (`boolFalse#bc799737 = Bool;`). `Bool` is mapped to
/// Rust `bool`, so the id lives here instead of on a generated struct.
pub const ID_BOOL_FALSE: TypeId = 0xbc79_9737;
/// `mtpc_boolTrue` (`boolTrue#997275b5 = Bool;`).
pub const ID_BOOL_TRUE: TypeId = 0x9972_75b5;

const PRIME: usize = std::mem::size_of::<u32>();

const fn primes_for_bytes(count: usize) -> usize {
    count.div_ceil(PRIME)
}

/// `Writer<mtpBuffer>::Put`.
#[inline]
pub fn put(to: &mut Buffer, value: u32) {
    to.push(value as Prime);
}

/// `Writer<mtpBuffer>::PutBytes`: appends `bytes` padded with zero bytes to
/// a whole number of primes. Bytes are laid out in memory order, so on the
/// little-endian hosts the original runs on the result is identical to its
/// `memcpy` into the prime buffer.
pub fn put_bytes(to: &mut Buffer, bytes: &[u8]) {
    to.reserve(primes_for_bytes(bytes.len()));
    let mut chunks = bytes.chunks_exact(PRIME);
    for chunk in &mut chunks {
        to.push(i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    let tail = chunks.remainder();
    if !tail.is_empty() {
        let mut last = [0u8; PRIME];
        last[..tail.len()].copy_from_slice(tail);
        to.push(i32::from_le_bytes(last));
    }
}

/// Converts a prime buffer into the bytes it occupies in memory / on the
/// wire (little-endian), as the original does with
/// `reinterpret_cast<char*>(buffer.data())`.
pub fn primes_to_bytes(primes: &[Prime]) -> Vec<u8> {
    let mut result = Vec::with_capacity(primes.len() * PRIME);
    for prime in primes {
        result.extend_from_slice(&prime.to_le_bytes());
    }
    result
}

/// Converts little-endian bytes into primes. Trailing bytes that do not form
/// a whole prime are dropped, like `span.size() / sizeof(mtpPrime)` in the
/// original callers.
pub fn bytes_to_primes(bytes: &[u8]) -> Buffer {
    bytes
        .chunks_exact(PRIME)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// `Reader<mtpPrime>` together with the `from` / `end` pointer pair the
/// original passes around: a bounds-checked cursor over a prime slice.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    data: &'a [Prime],
    position: usize,
}

impl<'a> Reader<'a> {
    /// A reader over `data` (`from = data.begin(), end = data.end()`).
    pub fn new(data: &'a [Prime]) -> Self {
        Self { data, position: 0 }
    }

    /// Number of primes already consumed.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Number of primes left (`end - from`).
    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }

    /// The unread part of the buffer.
    pub fn rest(&self) -> &'a [Prime] {
        &self.data[self.position..]
    }

    /// `true` if nothing is left.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// `Reader::Has`: at least `primes` primes are left.
    pub fn has(&self, primes: usize) -> bool {
        self.remaining() >= primes
    }

    /// `Reader::HasBytes`: at least `count` bytes (rounded up to primes) are
    /// left.
    pub fn has_bytes(&self, count: usize) -> bool {
        self.remaining() >= primes_for_bytes(count)
    }

    /// `Reader::Get`, checked: reads one prime.
    #[inline]
    pub fn get(&mut self) -> Result<u32> {
        match self.data.get(self.position) {
            Some(&value) => {
                self.position += 1;
                Ok(value as u32)
            }
            None => Err(Error::UnexpectedEnd),
        }
    }

    /// Reads one prime without consuming it.
    pub fn peek(&self) -> Result<u32> {
        self.data
            .get(self.position)
            .map(|&v| v as u32)
            .ok_or(Error::UnexpectedEnd)
    }

    /// Skips `primes` primes.
    pub fn skip(&mut self, primes: usize) -> Result<()> {
        if !self.has(primes) {
            return Err(Error::UnexpectedEnd);
        }
        self.position += primes;
        Ok(())
    }

    /// `Reader::GetBytes`, checked: reads `count` bytes and skips the padding
    /// up to the next prime boundary.
    pub fn get_bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        if !self.has_bytes(count) {
            return Err(Error::UnexpectedEnd);
        }
        let mut result = Vec::with_capacity(count);
        self.append_bytes(count, &mut result)?;
        Ok(result)
    }

    fn append_bytes(&mut self, count: usize, to: &mut Vec<u8>) -> Result<()> {
        let primes = primes_for_bytes(count);
        if !self.has(primes) {
            return Err(Error::UnexpectedEnd);
        }
        let words = &self.data[self.position..self.position + primes];
        let mut left = count;
        for word in words {
            let bytes = word.to_le_bytes();
            let take = left.min(PRIME);
            to.extend_from_slice(&bytes[..take]);
            left -= take;
        }
        self.position += primes;
        Ok(())
    }
}

/// Writes a value in the form it takes as a field of its Rust type: bare for
/// primitives and constructor data, boxed (with the constructor id) for
/// generated enums, `Vec<T>` (`Vector t`) and `bool` (`Bool`).
///
/// The port of the `write(Accumulator &to)` member templates.
pub trait Serialize {
    /// Appends the serialized value to `to`.
    fn write(&self, to: &mut Buffer);
}

/// Reads a value in the same form [`Serialize`] writes it.
///
/// The port of the `read(const Prime *&from, const Prime *end)` member
/// templates. Never panics; on error the reader position is unspecified
/// (the original also leaves `from` partially advanced).
pub trait Deserialize: Sized {
    /// Reads the value from `from`.
    fn read(from: &mut Reader<'_>) -> Result<Self>;
}

/// Serializes `value` into a fresh buffer.
pub fn serialize<T: Serialize + ?Sized>(value: &T) -> Buffer {
    let mut result = Buffer::new();
    value.write(&mut result);
    result
}

/// Reads a `T` from `data`, requiring that the whole slice is consumed.
pub fn deserialize_exact<T: Deserialize>(data: &[Prime]) -> Result<T> {
    let mut reader = Reader::new(data);
    let result = T::read(&mut reader)?;
    if !reader.is_empty() {
        return Err(Error::UnexpectedEnd);
    }
    Ok(result)
}

/// `tl::count_length`: serialized size in bytes.
pub fn count_length<T: Serialize + ?Sized>(value: &T) -> u32 {
    (serialize(value).len() * PRIME) as u32
}

impl<T: Serialize + ?Sized> Serialize for &T {
    fn write(&self, to: &mut Buffer) {
        (**self).write(to);
    }
}

impl<T: Serialize + ?Sized> Serialize for Box<T> {
    fn write(&self, to: &mut Buffer) {
        (**self).write(to);
    }
}

impl<T: Serialize + ?Sized> Serialize for std::sync::Arc<T> {
    fn write(&self, to: &mut Buffer) {
        (**self).write(to);
    }
}

impl<T: Deserialize> Deserialize for Box<T> {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        T::read(from).map(Box::new)
    }
}

impl<T: Deserialize> Deserialize for std::sync::Arc<T> {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        T::read(from).map(std::sync::Arc::new)
    }
}

// int_type

impl Serialize for i32 {
    #[inline]
    fn write(&self, to: &mut Buffer) {
        put(to, *self as u32);
    }
}

impl Deserialize for i32 {
    #[inline]
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        from.get().map(|v| v as i32)
    }
}

/// `tl::make_int`.
pub const fn make_int(v: i32) -> i32 {
    v
}

// long_type

fn read_u64(from: &mut Reader<'_>) -> Result<u64> {
    if !from.has(2) {
        return Err(Error::UnexpectedEnd);
    }
    let low = u64::from(from.get()?);
    let high = u64::from(from.get()?);
    Ok(low | (high << 32))
}

fn write_u64(to: &mut Buffer, value: u64) {
    put(to, (value & 0xFFFF_FFFF) as u32);
    put(to, (value >> 32) as u32);
}

impl Serialize for u64 {
    #[inline]
    fn write(&self, to: &mut Buffer) {
        write_u64(to, *self);
    }
}

impl Deserialize for u64 {
    #[inline]
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        read_u64(from)
    }
}

/// `tl::make_long`.
pub const fn make_long(v: u64) -> u64 {
    v
}

// int64_type

impl Serialize for i64 {
    fn write(&self, to: &mut Buffer) {
        write_u64(to, *self as u64);
    }
}

impl Deserialize for i64 {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        read_u64(from).map(|v| v as i64)
    }
}

/// `tl::make_int64`.
pub const fn make_int64(v: i64) -> i64 {
    v
}

// int128_type

/// `tl::int128_type`: 128 bits as two little-endian halves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Int128 {
    /// Low 64 bits (written first).
    pub l: u64,
    /// High 64 bits.
    pub h: u64,
}

impl Serialize for Int128 {
    fn write(&self, to: &mut Buffer) {
        write_u64(to, self.l);
        write_u64(to, self.h);
    }
}

impl Deserialize for Int128 {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        if !from.has(4) {
            return Err(Error::UnexpectedEnd);
        }
        Ok(Self {
            l: read_u64(from)?,
            h: read_u64(from)?,
        })
    }
}

/// `tl::make_int128`.
pub const fn make_int128(l: u64, h: u64) -> Int128 {
    Int128 { l, h }
}

// int256_type

/// `tl::int256_type`: 256 bits as two [`Int128`] halves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Int256 {
    /// Low 128 bits (written first).
    pub l: Int128,
    /// High 128 bits.
    pub h: Int128,
}

impl Serialize for Int256 {
    fn write(&self, to: &mut Buffer) {
        self.l.write(to);
        self.h.write(to);
    }
}

impl Deserialize for Int256 {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            l: Int128::read(from)?,
            h: Int128::read(from)?,
        })
    }
}

/// `tl::make_int256`.
pub const fn make_int256(l: Int128, h: Int128) -> Int256 {
    Int256 { l, h }
}

// double_type

impl Serialize for f64 {
    fn write(&self, to: &mut Buffer) {
        write_u64(to, self.to_bits());
    }
}

impl Deserialize for f64 {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        read_u64(from).map(f64::from_bits)
    }
}

/// `tl::make_double`.
pub const fn make_double(v: f64) -> f64 {
    v
}

// string_type / bytes_type

/// Maximum length `string_type::write` accepts (`Expects(v.size() < 0x1000000)`).
pub const MAX_STRING_LENGTH: usize = 0x00FF_FFFF;

/// `string_type::write`: short form (length byte + data) below 254 bytes,
/// long form (`254`, 24-bit length, data) from 254 bytes on, padded to a
/// whole number of primes.
///
/// # Panics
///
/// On strings of 16 MiB or more, like the `Expects` of the original; such a
/// string cannot be represented in TL at all.
pub fn write_string(to: &mut Buffer, v: &[u8]) {
    assert!(
        v.len() <= MAX_STRING_LENGTH,
        "TL string too long: {}",
        v.len()
    );
    let size = v.len() as u32;
    match v.len() {
        0 => put(to, size),
        1 => put(to, size | (u32::from(v[0]) << 8)),
        2 => put(to, size | (u32::from(v[0]) << 8) | (u32::from(v[1]) << 16)),
        3..=253 => {
            put(
                to,
                size | (u32::from(v[0]) << 8) | (u32::from(v[1]) << 16) | (u32::from(v[2]) << 24),
            );
            put_bytes(to, &v[3..]);
        }
        _ => {
            put(to, (size << 8) | 254);
            put_bytes(to, v);
        }
    }
}

/// `string_type::read`.
pub fn read_string(from: &mut Reader<'_>) -> Result<Vec<u8>> {
    let first = from.get()?;
    let last = (first & 0xFF) as usize;
    let b0 = ((first >> 8) & 0xFF) as u8;
    let b1 = ((first >> 16) & 0xFF) as u8;
    let b2 = ((first >> 24) & 0xFF) as u8;
    match last {
        0 => Ok(Vec::new()),
        1 => Ok(vec![b0]),
        2 => Ok(vec![b0, b1]),
        3..=253 => {
            let remaining = last - 3;
            if !from.has_bytes(remaining) {
                return Err(Error::UnexpectedEnd);
            }
            let mut v = Vec::with_capacity(last);
            v.extend_from_slice(&[b0, b1, b2]);
            from.append_bytes(remaining, &mut v)?;
            Ok(v)
        }
        254 => {
            let length = (first >> 8) as usize;
            if !from.has_bytes(length) {
                return Err(Error::UnexpectedEnd);
            }
            from.get_bytes(length)
        }
        _ => Err(Error::InvalidString),
    }
}

impl Serialize for Vec<u8> {
    fn write(&self, to: &mut Buffer) {
        write_string(to, self);
    }
}

impl Serialize for [u8] {
    fn write(&self, to: &mut Buffer) {
        write_string(to, self);
    }
}

impl Serialize for str {
    fn write(&self, to: &mut Buffer) {
        write_string(to, self.as_bytes());
    }
}

impl Deserialize for Vec<u8> {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        read_string(from)
    }
}

/// `tl::make_string`: UTF-8 bytes of `v` (`QString::toUtf8`).
pub fn make_string(v: impl AsRef<str>) -> Vec<u8> {
    v.as_ref().as_bytes().to_vec()
}

/// `tl::make_bytes`.
pub fn make_bytes(v: impl AsRef<[u8]>) -> Vec<u8> {
    v.as_ref().to_vec()
}

/// `tl::utf16`: decodes TL string bytes (`QString::fromUtf8`). Invalid
/// sequences become U+FFFD.
pub fn utf16(v: &[u8]) -> String {
    String::from_utf8_lossy(v).into_owned()
}

/// `tl::utf8`: the raw bytes.
pub fn utf8(v: &[u8]) -> Vec<u8> {
    v.to_vec()
}

// vector_type

/// `tl::vector_type<T>` used bare: `vector<T>` in the scheme (no
/// `0x1cb5c415` prefix). Boxed `Vector<T>` is plain `Vec<T>`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BareVec<T>(pub Vec<T>);

fn write_vector_items<T: Serialize>(to: &mut Buffer, items: &[T]) {
    put(to, items.len() as u32);
    for item in items {
        item.write(to);
    }
}

fn read_vector_items<T: Deserialize>(from: &mut Reader<'_>) -> Result<Vec<T>> {
    let count = from.get()? as usize;
    // Same anti-allocation guard as the original: every item takes at least
    // one prime, so a longer declared count can't be valid.
    if !from.has(count) {
        return Err(Error::UnexpectedEnd);
    }
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        result.push(T::read(from)?);
    }
    Ok(result)
}

impl<T: Serialize> Serialize for Vec<T> {
    fn write(&self, to: &mut Buffer) {
        put(to, ID_VECTOR);
        write_vector_items(to, self);
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        let cons = from.get()?;
        read_vector_bare(from, cons)
    }
}

/// `vector_type::read(from, end, cons)`: the body of a vector whose
/// constructor id `cons` has already been read.
pub fn read_vector_bare<T: Deserialize>(from: &mut Reader<'_>, cons: TypeId) -> Result<Vec<T>> {
    if cons != ID_VECTOR {
        return Err(Error::UnexpectedConstructor { id: cons });
    }
    read_vector_items(from)
}

impl<T: Serialize> Serialize for BareVec<T> {
    fn write(&self, to: &mut Buffer) {
        write_vector_items(to, &self.0);
    }
}

impl<T: Deserialize> Deserialize for BareVec<T> {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        read_vector_items(from).map(BareVec)
    }
}

/// `tl::make_vector`.
pub fn make_vector<T>(v: impl Into<Vec<T>>) -> Vec<T> {
    v.into()
}

// Bool

impl Serialize for bool {
    fn write(&self, to: &mut Buffer) {
        put(to, if *self { ID_BOOL_TRUE } else { ID_BOOL_FALSE });
    }
}

impl Deserialize for bool {
    fn read(from: &mut Reader<'_>) -> Result<Self> {
        let cons = from.get()?;
        read_bool_bare(cons)
    }
}

/// `MTPbool::read(from, end, cons)`.
pub fn read_bool_bare(cons: TypeId) -> Result<bool> {
    match cons {
        ID_BOOL_TRUE => Ok(true),
        ID_BOOL_FALSE => Ok(false),
        id => Err(Error::UnexpectedConstructor { id }),
    }
}

// conditional

/// `tl::conditional<T>::value_or` / `value_or_empty` for the `Option`s the
/// generated conditional getters return.
///
/// Like the original, only types with an inner `.v` value have it: numbers,
/// strings / bytes (as `&[u8]`) and vectors (as `&[T]`).
pub trait Conditional {
    /// The unwrapped value (`decltype(T::v)`).
    type Inner;
    /// The value, or `fallback` if the field is absent.
    fn value_or(self, fallback: Self::Inner) -> Self::Inner;
    /// The value, or the empty / zero value if the field is absent.
    fn value_or_empty(self) -> Self::Inner;
}

macro_rules! conditional_copy {
    ($($t:ty),*) => {$(
        impl Conditional for Option<$t> {
            type Inner = $t;
            fn value_or(self, fallback: $t) -> $t {
                self.unwrap_or(fallback)
            }
            fn value_or_empty(self) -> $t {
                self.unwrap_or_default()
            }
        }
    )*};
}

conditional_copy!(i32, u64, i64, f64);

impl<'a, T> Conditional for Option<&'a Vec<T>> {
    type Inner = &'a [T];
    fn value_or(self, fallback: &'a [T]) -> &'a [T] {
        self.map_or(fallback, Vec::as_slice)
    }
    fn value_or_empty(self) -> &'a [T] {
        self.map_or(&[], Vec::as_slice)
    }
}

// Helpers used by the generated code.

/// Writes a conditional field whose flag bit is `present`.
///
/// Mirrors the original: the flag decides, the stored value is only the
/// payload. A set bit with no stored value writes `T::default()` — the
/// original would write the default-constructed placeholder it was given.
#[inline]
pub fn write_conditional<T: Serialize + Default>(
    to: &mut Buffer,
    present: bool,
    value: &Option<T>,
) {
    if present {
        match value {
            Some(value) => value.write(to),
            None => T::default().write(to),
        }
    }
}

/// Reads a conditional field: present iff its flag bit is set.
#[inline]
pub fn read_conditional<T: Deserialize>(from: &mut Reader<'_>, present: bool) -> Result<Option<T>> {
    if present {
        T::read(from).map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_bytes_pads_with_zeros() {
        let mut b = Buffer::new();
        put_bytes(&mut b, &[1, 2, 3, 4, 5]);
        assert_eq!(primes_to_bytes(&b), vec![1, 2, 3, 4, 5, 0, 0, 0]);
    }

    #[test]
    fn get_bytes_checks_bounds() {
        let data = [0x0403_0201];
        let mut r = Reader::new(&data);
        assert_eq!(r.get_bytes(5), Err(Error::UnexpectedEnd));
        assert_eq!(r.get_bytes(usize::MAX), Err(Error::UnexpectedEnd));
        assert_eq!(r.position(), 0);
        assert_eq!(r.get_bytes(3).unwrap(), vec![1, 2, 3]);
        assert!(r.is_empty());
    }

    #[test]
    fn conditional_values() {
        assert_eq!(Some(5i32).value_or(-1), 5);
        assert_eq!(None::<i32>.value_or(-1), -1);
        assert_eq!(None::<u64>.value_or_empty(), 0);
        let v = vec![1u8, 2];
        assert_eq!(Some(&v).value_or_empty(), &[1, 2]);
        assert!(None::<&Vec<u8>>.value_or_empty().is_empty());
    }
}
