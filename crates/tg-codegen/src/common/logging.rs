use std::fmt;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub file: String,
    pub line: usize,
    pub code: u32,
    pub message: String,
}
impl Error {
    pub fn new(
        file: impl Into<String>,
        line: usize,
        code: u32,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.code == 0 {
            return Ok(());
        }
        write!(f, "{}", self.file)?;
        if self.line > 0 {
            write!(f, "({})", self.line)?;
        }
        write!(f, ": error {}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
