//! Port of `base/platform/base_platform_layout_switch.h`.

#[cfg(target_os = "linux")]
pub use super::linux::base_layout_switch_linux::switch_keyboard_layout_to_english;
