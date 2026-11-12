//! Portable logging singleton and executable-location part of base/integration.
//! Event-loop entry points and crash annotations require the launcher/window
//! integration (stage 5/6). There is no second logging singleton.
pub use crate::debug_log::{integration as instance, set_integration as set, Integration};
pub fn exists() -> bool {
    instance().is_some()
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutableInfo {
    directory: String,
    name: String,
}
impl ExecutableInfo {
    pub fn current() -> Self {
        std::env::current_exe()
            .ok()
            .map_or_else(Self::default, |p| Self::from_path(&p))
    }
    pub fn from_path(path: &std::path::Path) -> Self {
        let Ok(path) = std::fs::canonicalize(path) else {
            return Self::default();
        };
        Self {
            directory: path
                .parent()
                .map_or_else(String::new, |p| format!("{}/", p.display())),
            name: path
                .file_name()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
        }
    }
    pub fn executable_dir(&self) -> &str {
        &self.directory
    }
    pub fn executable_name(&self) -> &str {
        &self.name
    }
    pub fn executable_path(&self) -> String {
        format!("{}{}", self.directory, self.name)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_executable() {
        let info = ExecutableInfo::current();
        assert!(!info.executable_name().is_empty());
        assert!(info.executable_dir().ends_with('/'));
        assert_eq!(
            std::path::PathBuf::from(info.executable_path()),
            std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
        );
    }
    #[test]
    fn missing_path_is_empty() {
        assert_eq!(
            ExecutableInfo::from_path(std::path::Path::new("/nonexistent/tg-base-executable")),
            ExecutableInfo::default()
        );
    }
}
