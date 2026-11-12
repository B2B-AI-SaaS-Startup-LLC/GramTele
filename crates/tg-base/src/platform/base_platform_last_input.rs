//! Port of `base/platform/base_platform_last_input.h`: the platform source of
//! the last user input time.
//!
//! On Linux the original asks the MIT-SCREEN-SAVER X11 extension, then
//! Mutter's idle monitor over D-Bus (both deferred, see the docs). A platform
//! layer installs its source with [`set_provider`].

use std::sync::{Mutex, OnceLock};
use tg_runtime::Time;

type Provider = Box<dyn Fn() -> Option<Time> + Send + Sync>;

fn slot() -> &'static Mutex<Option<Provider>> {
    static SLOT: OnceLock<Mutex<Option<Provider>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Installs (or removes) the platform source.
pub fn set_provider(provider: Option<Provider>) {
    *slot().lock().unwrap_or_else(|e| e.into_inner()) = provider;
}

/// `Platform::LastUserInputTime()`.
pub fn last_user_input_time() -> Option<Time> {
    slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|provider| provider())
}

/// `LastUserInputTimeSupported()`.
pub fn last_user_input_time_supported() -> bool {
    last_user_input_time().is_some()
}
