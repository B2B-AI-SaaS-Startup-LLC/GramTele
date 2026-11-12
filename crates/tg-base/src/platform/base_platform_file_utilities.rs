//! Linux non-window file utilities; desktop opening is deferred to stage 6.
#[cfg(target_os = "linux")]
pub use super::linux::base_file_utilities_linux::*;
