use super::{Error, Result};
pub const MAX_SIZE: usize = 10 * 1024 * 1024;
#[derive(Debug)]
pub struct CleanFile {
    pub bytes: Vec<u8>,
    pub comments: Vec<Vec<u8>>,
}
impl CleanFile {
    pub fn read(path: &std::path::Path) -> Result<Self> {
        let name = path.display().to_string();
        if !path.exists() {
            return Err(Error::new(name, 0, 101, ": error: file does not exist."));
        }
        let bytes = std::fs::read(path).map_err(|_| Error::new(&name, 0, 103, "' for read."))?;
        Self::parse(&bytes, &name)
    }
    pub fn parse(content: &[u8], file: &str) -> Result<Self> {
        if content.len() > MAX_SIZE {
            return Err(Error::new(
                file,
                0,
                102,
                format!(
                    "' is too large, size={} > maxsize={MAX_SIZE}",
                    content.len()
                ),
            ));
        }
        let mut out = Vec::new();
        let mut comments = vec![Vec::new()];
        let (mut i, mut line, mut string) = (0, 0, false);
        while i < content.len() {
            let c = content[i];
            if c == b'"'
                && !(i > 0 && content[i - 1] == b'\\' && !(i > 1 && content[i - 2] == b'\\'))
            {
                string = !string;
            }
            if !string && content[i..].starts_with(b"//") {
                let start = i;
                while i < content.len() && content[i] != b'\n' && !content[i..].starts_with(b"\r\n")
                {
                    i += 1;
                }
                if i < content.len() {
                    comments[line] = content[start..i].to_vec();
                    out.push(b' ');
                } else {
                    break;
                }
            } else if !string && content[i..].starts_with(b"/*") {
                i += 2;
                let mut segment = true; // includes the opening /*
                loop {
                    if i >= content.len() {
                        return Err(Error::new(file, 0, 104, ""));
                    }
                    if content[i..].starts_with(b"*/") {
                        out.push(b' ');
                        i += 2;
                        break;
                    }
                    if content[i..].starts_with(b"\r\n") || content[i] == b'\n' {
                        if segment {
                            out.push(b' ');
                        }
                        if content[i] == b'\r' {
                            out.push(b'\r');
                            i += 1;
                        }
                        out.push(b'\n');
                        line += 1;
                        comments.push(Vec::new());
                        segment = false;
                    } else {
                        segment = true;
                    }
                    i += 1;
                }
                continue;
            }
            if i < content.len() {
                let c = content[i];
                out.push(c);
                if c == b'\n' {
                    line += 1;
                    comments.push(Vec::new());
                }
                i += 1;
            }
        }
        Ok(Self {
            bytes: out,
            comments,
        })
    }
}
