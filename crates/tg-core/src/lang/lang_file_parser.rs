//! Parser for runtime/custom .strings files (not the code generator grammar).
use std::{collections::BTreeMap, path::Path};
#[derive(Default, Debug)]
pub struct FileParser {
    pub pairs: Vec<(Vec<u8>, Vec<u8>)>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
impl FileParser {
    pub fn new(content: &[u8]) -> Self {
        let mut result = Self::default();
        let content = tg_base::parse_helper::strip_comments(content);
        if content.is_empty() {
            result.errors.push("Got empty lang file content".into());
            return result;
        }
        let mut p = Parser {
            bytes: &content,
            pos: 0,
        };
        while p.skip() {
            match p.pair() {
                Ok(pair) => result.pairs.push(pair),
                Err(error) => {
                    result.errors.push(error);
                    break;
                }
            }
        }
        result
    }
    pub fn errors(&self) -> String {
        self.errors.join("\n")
    }
    pub fn warnings(&self) -> String {
        self.warnings.join("\n")
    }
    pub fn found(
        &self,
        mut key_index: impl FnMut(&[u8]) -> Option<u16>,
        request: &std::collections::BTreeSet<u16>,
    ) -> BTreeMap<u16, String> {
        self.pairs
            .iter()
            .filter_map(|(k, v)| {
                key_index(k)
                    .filter(|i| request.contains(i))
                    .map(|i| (i, String::from_utf8_lossy(v).into_owned()))
            })
            .collect()
    }
    pub fn read_file(absolute: &Path, relative: &Path) -> Vec<u8> {
        let path = if relative.exists() {
            relative
        } else {
            absolute
        };
        let Ok(data) = std::fs::read(path) else {
            return Vec::new();
        };
        if data.len() < 3 || data.len() > 1024 * 1024 {
            return Vec::new();
        }
        if data.starts_with(&[0xff, 0xfe])
            || data.starts_with(&[0xfe, 0xff])
            || data[1] == 0
            || data[0] == 0
        {
            let bom = data.starts_with(&[0xff, 0xfe]) || data.starts_with(&[0xfe, 0xff]);
            let le = data.starts_with(&[0xff, 0xfe]) || (!bom && data[1] == 0);
            let raw = &data[if bom { 2 } else { 0 }..];
            let units = raw
                .chunks_exact(2)
                .map(|c| {
                    if le {
                        u16::from_le_bytes([c[0], c[1]])
                    } else {
                        u16::from_be_bytes([c[0], c[1]])
                    }
                })
                .collect::<Vec<_>>();
            String::from_utf16_lossy(&units).into_bytes()
        } else if data.starts_with(&[0xef, 0xbb, 0xbf]) {
            data[3..].to_vec()
        } else {
            data
        }
    }
}
struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl Parser<'_> {
    fn skip(&mut self) -> bool {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
        self.pos < self.bytes.len()
    }
    fn pair(&mut self) -> Result<(Vec<u8>, Vec<u8>), String> {
        if self.bytes[self.pos] != b'"' {
            return Err("Expected quote before key name!".into());
        }
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_alphanumeric()
                || matches!(self.bytes[self.pos], b'_' | b'#'))
        {
            self.pos += 1;
        }
        let key = self.bytes[start..self.pos].to_vec();
        let name = String::from_utf8_lossy(&key);
        let eof = || format!("Unexpected end of file in key '{name}'!");
        if self.bytes.get(self.pos) != Some(&b'"') {
            return Err(format!("Expected quote after key name '{name}'!"));
        }
        self.pos += 1;
        if !self.skip() {
            return Err(eof());
        }
        if self.bytes[self.pos] != b'=' {
            return Err(format!("'=' expected in key '{name}'!"));
        }
        self.pos += 1;
        if !self.skip() {
            return Err(eof());
        }
        if self.bytes[self.pos] != b'"' {
            return Err(format!("Expected string after '=' in key '{name}'!"));
        }
        self.pos += 1;
        let mut value = Vec::new();
        loop {
            let ch = *self.bytes.get(self.pos).ok_or_else(eof)?;
            self.pos += 1;
            match ch {
                b'"' => break,
                b'\n' => return Err(format!("Unexpected end of string in key '{name}'!")),
                b'\\' => {
                    let next = *self.bytes.get(self.pos).ok_or_else(eof)?;
                    match next {
                        b'"' | b'\\' => {
                            value.push(next);
                            self.pos += 1;
                        }
                        b'n' => {
                            value.push(b'\n');
                            self.pos += 1;
                        }
                        _ => value.push(ch),
                    }
                }
                _ => value.push(ch),
            }
        }
        if !self.skip() {
            return Err(eof());
        }
        if self.bytes[self.pos] != b';' {
            return Err(format!("';' expected after \"value\" in key '{name}'!"));
        }
        self.pos += 1;
        Ok((key, value))
    }
}
