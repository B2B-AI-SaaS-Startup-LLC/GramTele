//! Port of `base/last_user_input.h` / `last_user_input.cpp`.
//!
//! The original prefers the platform source and otherwise installs a Qt event
//! filter on the application that remembers `crl::now()` at every mouse
//! move, key press, mouse press, touch begin and wheel event. The event
//! filter is the window layer's job (`tg-window`, stage 5): it calls
//! [`note_user_input`] for those events. Everything else is ported.

use crate::platform::base_platform_last_input;
use std::sync::atomic::{AtomicI64, Ordering};
use tg_runtime::Time;

static RESULT: AtomicI64 = AtomicI64::new(0);

/// What the window layer's event filter does on an input event
/// (`result = crl::now()`).
pub fn note_user_input() {
    RESULT.store(tg_runtime::now(), Ordering::Relaxed);
}

/// `base::LastUserInputTime`.
pub fn last_user_input_time() -> Time {
    base_platform_last_input::last_user_input_time().unwrap_or_else(|| RESULT.load(Ordering::Relaxed))
}

/// `base::SinceLastUserInput`.
pub fn since_last_user_input() -> Time {
    tg_runtime::now() - last_user_input_time()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Process-global state: one test touches it.
    #[test]
    fn platform_source_wins_over_noted_input() {
        note_user_input();
        let noted = last_user_input_time();
        assert!(noted >= 0);
        base_platform_last_input::set_provider(Some(Box::new(|| Some(42))));
        assert_eq!(last_user_input_time(), 42);
        assert!(base_platform_last_input::last_user_input_time_supported());
        assert!(since_last_user_input() >= 0);
        base_platform_last_input::set_provider(Some(Box::new(|| None)));
        assert_eq!(last_user_input_time(), noted);
        base_platform_last_input::set_provider(None);
        assert!(!base_platform_last_input::last_user_input_time_supported());
    }
}
