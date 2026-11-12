//! Output file policy: content is deterministic; unchanged files keep mtime.
use super::{Error, Result};
pub fn write(path: &std::path::Path, text: &str) -> Result<()> {
    if std::fs::read(path).ok().as_deref() == Some(text.as_bytes()) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            Error::new(
                path.display().to_string(),
                0,
                821,
                "can not open path for writing",
            )
        })?;
    }
    std::fs::write(path, text).map_err(|_| {
        Error::new(
            path.display().to_string(),
            0,
            821,
            "can not open file for writing",
        )
    })
}

/// The CMake stamp must become newer even when generated text is unchanged.
pub fn touch_timestamp(base: &std::path::Path) -> Result<()> {
    let path = std::path::PathBuf::from(format!("{}.timestamp", base.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| {
            Error::new(
                path.display().to_string(),
                0,
                821,
                "can not open path for writing",
            )
        })?;
    }
    std::fs::write(&path, b"1").map_err(|_| {
        Error::new(
            path.display().to_string(),
            0,
            821,
            "can not open file for writing",
        )
    })
}
