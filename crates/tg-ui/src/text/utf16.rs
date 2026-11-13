//! `QString` is UTF-16 in the original and every text entity offset is
//! counted in UTF-16 code units (the server sends them the same way). In the
//! port the text is a `String` while offsets stay in UTF-16 units; these
//! helpers convert between the two.

/// `QString::size()`.
pub fn len(text: &str) -> i32 {
    text.encode_utf16().count() as i32
}

/// Byte position in `text` for the UTF-16 `offset`.
///
/// An offset past the end gives `text.len()`. An offset inside a surrogate
/// pair (valid for a `QString`) is rounded down to the start of the char.
pub fn byte_index(text: &str, offset: i32) -> usize {
    if offset <= 0 {
        return 0;
    }
    let mut units = 0i32;
    for (index, ch) in text.char_indices() {
        let next = units + ch.len_utf16() as i32;
        if next > offset {
            return index;
        }
        units = next;
    }
    text.len()
}

/// UTF-16 offset of the byte position `index` (a char boundary).
pub fn offset(text: &str, index: usize) -> i32 {
    len(&text[..index])
}

/// `QString::mid(position, n)`; `n < 0` means "till the end".
pub fn mid(text: &str, position: i32, n: i32) -> String {
    let size = len(text);
    let position = position.clamp(0, size);
    let end = if n < 0 || n > size - position {
        size
    } else {
        position + n
    };
    text[byte_index(text, position)..byte_index(text, end)].to_owned()
}

/// `QString::indexOf(needle, from)` in UTF-16 units; `-1` when not found.
pub fn index_of(text: &str, needle: &str, from: i32) -> i32 {
    let start = byte_index(text, from);
    text[start..]
        .find(needle)
        .map_or(-1, |found| offset(text, start + found))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_count_surrogate_pairs_as_two_units() {
        let text = "a😀б";
        assert_eq!(len(text), 4);
        assert_eq!(byte_index(text, 0), 0);
        assert_eq!(byte_index(text, 1), 1);
        assert_eq!(byte_index(text, 2), 1);
        assert_eq!(byte_index(text, 3), 5);
        assert_eq!(byte_index(text, 4), 7);
        assert_eq!(byte_index(text, 99), 7);
        assert_eq!(offset(text, 5), 3);
        assert_eq!(mid(text, 1, 2), "😀");
        assert_eq!(mid(text, 3, -1), "б");
        assert_eq!(mid(text, 3, 100), "б");
        assert_eq!(index_of(text, "б", 0), 3);
        assert_eq!(index_of(text, "б", 4), -1);
        assert_eq!(index_of(text, "x", 0), -1);
    }
}
