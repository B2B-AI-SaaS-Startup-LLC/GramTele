//! Port of `base/custom_app_icon.h`: forwards to the platform
//! (`platform::base_platform_custom_app_icon`). Only macOS supports custom
//! icons (stage 10); here every call reports "unsupported". The `QImage`
//! overload of `SetCustomAppIcon` comes with `tg-gfx`.

pub use crate::platform::base_platform_custom_app_icon::{
    clear_custom_app_icon, current_custom_app_icon_digest, set_custom_app_icon,
};
