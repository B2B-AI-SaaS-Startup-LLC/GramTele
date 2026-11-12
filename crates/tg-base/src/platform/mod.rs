//! Port of `Telegram/lib_base/base/platform/` (common headers).
//!
//! Only what does not depend on Qt or the window system is implemented; the
//! rest is listed in `docs/timers-and-process.md`. Platform implementations
//! live in `linux/` (`mac/` and `win/` belong to stage 10).

pub mod base_platform_custom_app_icon;
pub mod base_platform_file_utilities;
pub mod base_platform_haptic;
pub mod base_platform_last_input;
pub mod base_platform_layout_switch;
pub mod base_platform_network_reachability;
pub mod base_platform_power_save_blocker;

#[cfg(target_os = "linux")]
pub mod linux;
