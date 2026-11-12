//! Port of `base/system_unlock.h`: the platform-independent types.
//!
//! The functions are platform code: Linux in
//! `platform::linux::base_system_unlock_linux`; macOS and Windows (biometric
//! prompts) are stage 10. On the other platforms `SuggestSystemUnlock` takes a
//! parent window, which comes with `tg-window`.

/// `base::SystemUnlockAvailability` (bit fields in the original).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemUnlockAvailability {
    pub known: bool,
    pub available: bool,
    pub with_biometrics: bool,
    pub with_companion: bool,
}

/// `base::SystemUnlockResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemUnlockResult {
    Success,
    Cancelled,
    FloodError,
}
