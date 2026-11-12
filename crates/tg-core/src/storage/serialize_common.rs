//! Shared Qt_5_1 stream; no second QByteArray/QString implementation.
pub use tg_base::qt_stream::{Reader as ByteArrayReader, Writer as ByteArrayWriter};
pub fn string_size(value: Option<&[u16]>) -> usize {
    4 + value.map_or(0, |s| s.len() * 2)
}
pub fn bytearray_size(value: &[u8]) -> usize {
    4 + value.len()
}
pub fn bytes_size(value: &[u8]) -> usize {
    bytearray_size(value)
}
pub const fn color_size() -> usize {
    4
}
pub const fn date_time_size() -> usize {
    13
}
pub fn write_color(stream: &mut ByteArrayWriter, color: [u8; 4]) {
    stream.i32(u32::from_le_bytes(color) as i32);
}
pub fn read_color(stream: &mut ByteArrayReader<'_>) -> Option<[u8; 4]> {
    Some((stream.i32()? as u32).to_le_bytes())
}
/// Serialize::bytes(span) writes an empty span as null QByteArray.
pub fn write_bytes(stream: &mut ByteArrayWriter, bytes: &[u8]) {
    stream.nullable_bytes(if bytes.is_empty() { None } else { Some(bytes) });
}
