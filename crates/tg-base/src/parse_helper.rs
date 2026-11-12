//! Port of `lib_base/base/parse_helper.h` and `parse_helper.cpp`.
//!
//! The C++ helpers move a `const char *&from` pointer towards `end`. In Rust
//! the pointer is an index into a byte slice.

use std::borrow::Cow;

#[derive(Clone, Copy, PartialEq, Eq)]
enum InsideComment {
    None,
    SingleLine,
    MultiLine,
}

/// `base::parse::stripComments`: replaces every C-style comment with a single
/// space, keeping the line breaks that end or are inside comments.
///
/// Returns the input unchanged (borrowed) when there was nothing to strip,
/// like the original returns `content` itself.
pub fn strip_comments(content: &[u8]) -> Cow<'_, [u8]> {
    let end = content.len();
    let at = |index: usize| content.get(index).copied().unwrap_or(0);

    let mut inside_comment = InsideComment::None;
    let mut inside_string = false;
    let mut result: Vec<u8> = Vec::new();
    let mut offset = 0usize;

    let feed_content = |result: &mut Vec<u8>, offset: &mut usize, ch: usize| {
        if ch > *offset {
            result.extend_from_slice(&content[*offset..ch]);
            *offset = ch;
        }
    };
    let feed_comment = |result: &mut Vec<u8>, offset: &mut usize, ch: usize| {
        if ch > *offset {
            result.push(b' ');
            *offset = ch;
        }
    };

    let mut ch = 0usize;
    while ch != end {
        let current = content[ch];
        let next = at(ch + 1);

        if inside_comment == InsideComment::None && current == b'"' {
            // The original reads `*(ch - 2)` even when `ch == begin + 1`,
            // which is outside the buffer; a missing byte counts as "not a
            // backslash" here.
            let escaped =
                ch > 0 && content[ch - 1] == b'\\' && (ch < 2 || content[ch - 2] != b'\\');
            if !escaped {
                inside_string = !inside_string;
            }
        }
        if inside_string {
            ch += 1;
            continue;
        }

        if inside_comment == InsideComment::None && current == b'/' && next == b'/' {
            feed_content(&mut result, &mut offset, ch);
            inside_comment = InsideComment::SingleLine;
            ch += 2;
        } else if inside_comment == InsideComment::SingleLine && current == b'\r' && next == b'\n' {
            feed_comment(&mut result, &mut offset, ch);
            ch += 2;
            inside_comment = InsideComment::None;
        } else if inside_comment == InsideComment::SingleLine && current == b'\n' {
            feed_comment(&mut result, &mut offset, ch);
            ch += 1;
            inside_comment = InsideComment::None;
        } else if inside_comment == InsideComment::None && current == b'/' && next == b'*' {
            feed_content(&mut result, &mut offset, ch);
            ch += 2;
            inside_comment = InsideComment::MultiLine;
        } else if inside_comment == InsideComment::MultiLine && current == b'*' && next == b'/' {
            ch += 2;
            feed_comment(&mut result, &mut offset, ch);
            inside_comment = InsideComment::None;
        } else if inside_comment == InsideComment::MultiLine && current == b'\r' && next == b'\n' {
            feed_comment(&mut result, &mut offset, ch);
            ch += 2;
            feed_content(&mut result, &mut offset, ch);
        } else if inside_comment == InsideComment::MultiLine && current == b'\n' {
            feed_comment(&mut result, &mut offset, ch);
            ch += 1;
            feed_content(&mut result, &mut offset, ch);
        } else {
            ch += 1;
        }
    }

    if inside_comment == InsideComment::None && end > offset {
        if result.is_empty() {
            return Cow::Borrowed(content);
        }
        result.extend_from_slice(&content[offset..end]);
    }
    Cow::Owned(result)
}

/// `base::parse::skipWhitespaces`: advances `from` over ' ', '\n', '\t',
/// '\r'; returns whether anything is left.
pub fn skip_whitespaces(data: &[u8], from: &mut usize) -> bool {
    assert!(*from <= data.len());
    while *from != data.len() && matches!(data[*from], b' ' | b'\n' | b'\t' | b'\r') {
        *from += 1;
    }
    *from != data.len()
}

/// `base::parse::readName`: reads `[a-zA-Z0-9_]*` starting at `from`.
pub fn read_name<'a>(data: &'a [u8], from: &mut usize) -> &'a str {
    assert!(*from <= data.len());
    let start = *from;
    while *from != data.len() && (data[*from].is_ascii_alphanumeric() || data[*from] == b'_') {
        *from += 1;
    }
    std::str::from_utf8(&data[start..*from]).expect("ASCII subset")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip(text: &str) -> String {
        String::from_utf8(strip_comments(text.as_bytes()).into_owned()).unwrap()
    }

    #[test]
    fn no_comments_borrows() {
        assert!(matches!(strip_comments(b"a: 1;"), Cow::Borrowed(_)));
        assert_eq!(strip(""), "");
    }

    #[test]
    fn single_line() {
        assert_eq!(strip("a // x\nb"), "a  \nb");
        assert_eq!(strip("a // x\r\nb"), "a  \r\nb");
        assert_eq!(strip("// only"), "");
        assert_eq!(strip("abc// x\n"), "abc \n");
    }

    #[test]
    fn multi_line() {
        assert_eq!(strip("a /* x */ b"), "a   b");
        assert_eq!(strip("/* x */abc"), " abc");
        assert_eq!(strip("a/* x\ny */b"), "a \n b");
        assert_eq!(strip("a/* x\r\ny */b"), "a \r\n b");
        // Unterminated comment: the tail is dropped.
        assert_eq!(strip("a /* x"), "a ");
    }

    #[test]
    fn strings_are_kept() {
        assert_eq!(strip("\"// not\" // yes\n"), "\"// not\"  \n");
        assert_eq!(strip("\"a\\\"// still\" //x"), "\"a\\\"// still\" ");
        assert_eq!(strip("\"a\\\\\" //x"), "\"a\\\\\" ");
        assert_eq!(strip("\\\" //x"), "\\\" ");
    }

    #[test]
    fn cursor_helpers() {
        let data = b" \t\r\nname_1: value";
        let mut from = 0;
        assert!(skip_whitespaces(data, &mut from));
        assert_eq!(from, 4);
        assert_eq!(read_name(data, &mut from), "name_1");
        assert_eq!(data[from], b':');
        let mut end = data.len();
        assert!(!skip_whitespaces(data, &mut end));
        assert_eq!(read_name(data, &mut end), "");
    }
}
