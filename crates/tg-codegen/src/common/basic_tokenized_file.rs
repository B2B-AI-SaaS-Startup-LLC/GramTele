use super::{checked_utf8_string::checked, clean_file::CleanFile, Error, Result};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Name,
    Int,
    Double,
    String,
    Symbol(char),
    End,
}
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    pub value: String,
    pub original: String,
    pub line: usize,
    pub whitespace: bool,
}
pub struct TokenizedFile {
    pub file: String,
    pub tokens: Vec<Token>,
    pub position: usize,
    pub comments: Vec<Vec<u8>>,
    pending_error: Option<Error>,
}
impl TokenizedFile {
    pub fn parse(content: &[u8], file: &str) -> Result<Self> {
        let result = Self::parse_lazy(content, file)?;
        result.finish()?;
        Ok(result)
    }
    /// Delay a later lexical error until the parser reaches it, like C++ getToken.
    pub fn parse_lazy(content: &[u8], file: &str) -> Result<Self> {
        let clean = CleanFile::parse(content, file)?;
        let b = &clean.bytes;
        let (mut i, mut line) = (0, 1);
        let mut tokens = Vec::new();
        let scan = (|| -> Result<()> {
            while i < b.len() {
                let whitespace = b[i].is_ascii_whitespace();
                while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
                    if b[i] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
                if i == b.len() {
                    break;
                }
                let start = i;
                let token_line = line;
                let (kind, value) = if b[i] == b'"' {
                    i += 1;
                    let mut value = Vec::new();
                    loop {
                        if i == b.len() || b[i] == b'\n' {
                            return Err(Error::new(
                                file,
                                line,
                                201,
                                "unterminated string literal.",
                            ));
                        }
                        if b[i] == b'"' {
                            i += 1;
                            break;
                        }
                        if b[i] == b'\\' {
                            i += 1;
                            if i == b.len() || b[i] == b'\n' {
                                return Err(Error::new(
                                    file,
                                    line,
                                    201,
                                    "unterminated string literal.",
                                ));
                            }
                            match b[i] {
                                b'n' => value.push(b'\n'),
                                b't' => value.push(b'\t'),
                                b'"' => value.push(b'"'),
                                b'\\' => value.push(b'\\'),
                                _ => {}
                            }
                        } else {
                            value.push(b[i]);
                        }
                        i += 1;
                    }
                    (
                        Kind::String,
                        checked(&value)
                            .ok_or_else(|| {
                                Error::new(file, line, 202, "incorrect UTF-8 string literal.")
                            })?
                            .to_owned(),
                    )
                } else if b[i].is_ascii_alphanumeric() || b[i] == b'_' {
                    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                        i += 1;
                    }
                    let mut kind = if b[start..i].iter().all(u8::is_ascii_digit) {
                        Kind::Int
                    } else {
                        Kind::Name
                    };
                    if kind == Kind::Int && i < b.len() && b[i] == b'.' {
                        kind = Kind::Double;
                        i += 1;
                        while i < b.len() && b[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    (kind, String::from_utf8_lossy(&b[start..i]).into_owned())
                } else if b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    (
                        Kind::Double,
                        String::from_utf8_lossy(&b[start..i]).into_owned(),
                    )
                } else if b"(){}[]:;,.#+-=|&".contains(&b[i]) {
                    let c = b[i] as char;
                    i += 1;
                    (Kind::Symbol(c), c.to_string())
                } else {
                    return Err(Error::new(
                        file,
                        line,
                        203,
                        format!("incorrect token '{}'", b[i] as char),
                    ));
                };
                tokens.push(Token {
                    kind,
                    value,
                    original: String::from_utf8_lossy(&b[start..i]).into_owned(),
                    line: token_line,
                    whitespace,
                });
            }
            Ok(())
        })();
        tokens.push(Token {
            kind: Kind::End,
            value: String::new(),
            original: String::new(),
            line,
            whitespace: true,
        });
        Ok(Self {
            file: file.into(),
            tokens,
            position: 0,
            comments: clean.comments,
            pending_error: scan.err(),
        })
    }
    pub fn finish(&self) -> Result<()> {
        match &self.pending_error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
    pub fn peek(&self) -> &Token {
        &self.tokens[self.position]
    }
    pub fn take(&mut self) -> Token {
        let result = self.peek().clone();
        if result.kind != Kind::End {
            self.position += 1;
        }
        result
    }
    pub fn eat(&mut self, kind: Kind) -> bool {
        if self.peek().kind == kind {
            self.take();
            true
        } else {
            false
        }
    }
    pub fn expect(&mut self, kind: Kind) -> Result<Token> {
        if self.peek().kind == kind {
            Ok(self.take())
        } else {
            Err(self.unexpected(&match kind {
                Kind::String => "'string'".into(),
                Kind::Name => "'identifier'".into(),
                Kind::Int => "'int'".into(),
                Kind::Double => "'double'".into(),
                Kind::Symbol(c) => format!("'{c}'"),
                Kind::End => "'invalid'".into(),
            }))
        }
    }
    pub fn error(&self, code: u32, message: impl Into<String>) -> Error {
        Error::new(
            &self.file,
            self.tokens[self.position.saturating_sub(1)].line,
            code,
            message,
        )
    }
    pub fn unexpected(&self, expected: &str) -> Error {
        let t = self.peek();
        if t.kind == Kind::End {
            if let Some(error) = &self.pending_error {
                return error.clone();
            }
        }
        Error::new(
            &self.file,
            t.line,
            204,
            if t.kind == Kind::End {
                format!("unexpected token, expected {expected}")
            } else {
                format!("unexpected token '{}', expected {expected}", t.original)
            },
        )
    }
}
