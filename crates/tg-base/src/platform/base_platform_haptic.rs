//! Port of the constant part of `base/platform/base_platform_haptic.h`.
//! `Haptic()` itself (feedbackd over D-Bus on Linux) is deferred.

/// Linux: swipe-back is always enabled (`IsSwipeBackEnabled`).
pub fn is_swipe_back_enabled() -> bool {
    true
}
