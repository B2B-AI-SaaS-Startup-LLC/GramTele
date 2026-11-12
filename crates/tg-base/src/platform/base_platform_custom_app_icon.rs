//! Port of `base/platform/base_platform_custom_app_icon.h` for the platforms
//! without custom icons (everything but macOS, stage 10): "unsupported".

use std::path::Path;

/// `SetCustomAppIcon(path)`: `None` means unsupported or failed. The
/// `QImage` overload belongs to the graphics layer.
pub fn set_custom_app_icon(_path: &Path) -> Option<u64> {
    None
}

pub fn current_custom_app_icon_digest() -> Option<u64> {
    None
}

pub fn clear_custom_app_icon() -> bool {
    false
}
