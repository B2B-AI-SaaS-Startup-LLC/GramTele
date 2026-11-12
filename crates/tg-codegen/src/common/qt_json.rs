//! QJsonDocument-compatible reader for generator input. Strings retain UTF-16,
//! including lone surrogates; object order is QString order, not UTF-8 order.
use super::{Error, Result};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Object(BTreeMap<Vec<u16>, Value>),
    Array(Vec<Value>),
    String(Vec<u16>),
    Number(f64),
    Bool(bool),
    Null,
}
impl Value {
    pub fn as_object(&self) -> Option<&BTreeMap<Vec<u16>, Value>> {
        if let Self::Object(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn utf16(&self) -> Option<&[u16]> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn string(&self) -> Option<String> {
        if let Self::String(v) = self {
            Some(qt_utf8(v))
        } else {
            None
        }
    }
}
/// QString::toUtf8 in the reference Qt drops isolated surrogate code units.
pub fn qt_utf8(s: &[u16]) -> String {
    char::decode_utf16(s.iter().copied())
        .filter_map(std::result::Result::ok)
        .collect()
}
struct Parser<'a> {
    b: &'a [u8],
    at: usize,
}
type Parsed<T> = std::result::Result<T, u32>;
impl Parser<'_> {
    fn space(&mut self) {
        while self.at < self.b.len() && matches!(self.b[self.at], b' ' | b'\t' | b'\r' | b'\n') {
            self.at += 1;
        }
    }
    fn consume(&mut self, c: u8) -> bool {
        self.space();
        if self.b.get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Parsed<Vec<u16>> {
        if !self.consume(b'"') {
            return Err(1);
        }
        let mut result = Vec::new();
        while self.at < self.b.len() {
            let start = self.at;
            match self.b[self.at] {
                b'"' => {
                    self.at += 1;
                    return Ok(result);
                }
                b'\\' => {
                    self.at += 1;
                    let Some(&c) = self.b.get(self.at) else {
                        return Err(10);
                    };
                    self.at += 1;
                    match c {
                        b'"' => result.push(34),
                        b'\\' => result.push(92),
                        b'/' => result.push(47),
                        b'b' => result.push(8),
                        b'f' => result.push(12),
                        b'n' => result.push(10),
                        b'r' => result.push(13),
                        b't' => result.push(9),
                        b'u' => {
                            let Some(digits) = self.b.get(self.at..self.at + 4) else {
                                return Err(8);
                            };
                            let text = std::str::from_utf8(digits).map_err(|_| 8u32)?;
                            let code = u16::from_str_radix(text, 16).map_err(|_| 8u32)?;
                            result.push(code);
                            self.at += 4;
                        }
                        _ => {
                            if c < 128 {
                                result.push(c as u16);
                            } else {
                                return Err(9);
                            }
                        }
                    }
                }
                _ => {
                    while self.at < self.b.len() && !matches!(self.b[self.at], b'"' | b'\\') {
                        self.at += 1;
                    }
                    let text = std::str::from_utf8(&self.b[start..self.at]).map_err(|_| 9u32)?;
                    result.extend(text.encode_utf16());
                }
            }
        }
        Err(10)
    }
    fn value(&mut self, depth: usize) -> Parsed<Value> {
        self.space();
        let Some(&c) = self.b.get(self.at) else {
            return Err(5);
        };
        match c {
            b'{' => {
                if depth >= 1024 {
                    return Err(12);
                }
                self.at += 1;
                let mut map = BTreeMap::new();
                if self.consume(b'}') {
                    return Ok(Value::Object(map));
                }
                loop {
                    if self.at >= self.b.len() {
                        return Err(1);
                    }
                    let key = self.string()?;
                    if !self.consume(b':') {
                        return Err(2);
                    }
                    self.space();
                    if self.at == self.b.len() {
                        return Err(1);
                    }
                    let value = self.value(depth + 1)?;
                    map.insert(key, value);
                    if self.consume(b'}') {
                        break;
                    }
                    if !self.consume(b',') {
                        return Err(1);
                    }
                    self.space();
                    if self.b.get(self.at) == Some(&b'}') {
                        return Err(11);
                    }
                }
                Ok(Value::Object(map))
            }
            b'[' => {
                if depth >= 1024 {
                    return Err(12);
                }
                self.at += 1;
                let mut list = Vec::new();
                if self.consume(b']') {
                    return Ok(Value::Array(list));
                }
                loop {
                    self.space();
                    if self.at >= self.b.len() {
                        return Err(3);
                    }
                    list.push(self.value(depth + 1)?);
                    if self.consume(b']') {
                        break;
                    }
                    if !self.consume(b',') {
                        return Err(3);
                    }
                    self.space();
                    if self.b.get(self.at) == Some(&b']') {
                        return Err(11);
                    }
                }
                Ok(Value::Array(list))
            }
            b'"' => Ok(Value::String(self.string()?)),
            b't' | b'f' | b'n' => {
                let (word, value) = match c {
                    b't' => (b"true".as_slice(), Value::Bool(true)),
                    b'f' => (b"false".as_slice(), Value::Bool(false)),
                    _ => (b"null".as_slice(), Value::Null),
                };
                if self.b.get(self.at..self.at + word.len()) != Some(word) {
                    return Err(5);
                }
                self.at += word.len();
                Ok(value)
            }
            _ => {
                let start = self.at;
                if c == b'-' {
                    self.at += 1;
                }
                if self.b.get(self.at) == Some(&b'0') {
                    self.at += 1;
                } else {
                    let n = self.at;
                    while self.b.get(self.at).is_some_and(u8::is_ascii_digit) {
                        self.at += 1;
                    }
                    if self.at == n {
                        return Err(7);
                    }
                }
                if self.b.get(self.at) == Some(&b'.') {
                    self.at += 1;
                    while self.b.get(self.at).is_some_and(u8::is_ascii_digit) {
                        self.at += 1;
                    }
                }
                if self
                    .b
                    .get(self.at)
                    .is_some_and(|c| matches!(c, b'e' | b'E'))
                {
                    self.at += 1;
                    if self
                        .b
                        .get(self.at)
                        .is_some_and(|c| matches!(c, b'+' | b'-'))
                    {
                        self.at += 1;
                    }
                    while self.b.get(self.at).is_some_and(u8::is_ascii_digit) {
                        self.at += 1;
                    }
                }
                let text = std::str::from_utf8(&self.b[start..self.at]).map_err(|_| 7u32)?;
                let n = text.parse::<f64>().map_err(|_| 7u32)?;
                if !n.is_finite() {
                    return Err(7);
                }
                Ok(Value::Number(n))
            }
        }
    }
}
pub fn parse(bytes: &[u8], file: &str) -> Result<Value> {
    if bytes.is_empty() {
        return Err(Error::new(file, 0, 402, "Bad data: Could not read data."));
    }
    // Qt permits nesting to 1024. A fixed parser thread stack avoids depending on
    // the small stack of a build.rs caller or a Rust test thread.
    let owned = bytes.to_vec();
    let parsed = std::thread::Builder::new()
        .name("codegen-json".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || -> Parsed<Value> {
            let b = owned.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&owned);
            let mut p = Parser { b, at: 0 };
            p.space();
            if !matches!(p.b.get(p.at), Some(b'{' | b'[')) {
                return Err(5);
            }
            let value = p.value(0)?;
            p.space();
            if p.at != p.b.len() {
                return Err(14);
            }
            Ok(value)
        })
        .map_err(|e| Error::new(file, 0, 666, format!("internal - {e}")))?
        .join()
        .map_err(|_| Error::new(file, 0, 666, "internal - JSON parser panicked"))?;
    parsed.map_err(|code| {
        let message = match code {
            1 => "unterminated object",
            2 => "missing name separator",
            3 => "unterminated array",
            4 => "missing value separator",
            5 => "illegal value",
            6 => "invalid termination by number",
            7 => "illegal number",
            8 => "invalid escape sequence",
            9 => "invalid UTF8 string",
            10 => "unterminated string",
            11 => "object is missing after a comma",
            12 => "too deeply nested document",
            _ => "garbage at the end of the document",
        };
        Error::new(
            file,
            0,
            402,
            format!("Bad data: Could not parse data ({code}): {message}"),
        )
    })
}
