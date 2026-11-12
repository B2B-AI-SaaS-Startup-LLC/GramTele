//! Port of `base_system_unlock_linux.cpp`.

use crate::system_unlock::{SystemUnlockAvailability, SystemUnlockResult};
use tg_rpl::Producer;

/// `SystemUnlockStatus`: known, not available.
pub fn system_unlock_status(_lookup_details: bool) -> Producer<SystemUnlockAvailability> {
    tg_rpl::single(SystemUnlockAvailability {
        known: true,
        ..Default::default()
    })
}

/// `SuggestSystemUnlock`: always reports `Cancelled`. The `QWidget` parent of
/// the original is not needed here.
pub fn suggest_system_unlock(_text: &str, done: impl FnOnce(SystemUnlockResult)) {
    done(SystemUnlockResult::Cancelled);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_has_no_unlock() {
        let mut result = None;
        suggest_system_unlock("x", |r| result = Some(r));
        assert_eq!(result, Some(SystemUnlockResult::Cancelled));
        let seen = std::rc::Rc::new(std::cell::Cell::new(None));
        let lifetime = tg_rpl::Lifetime::new();
        let s = seen.clone();
        system_unlock_status(true).on_next(move |v| s.set(Some(v)), &lifetime);
        let v = seen.get().unwrap();
        assert!(v.known && !v.available && !v.with_biometrics && !v.with_companion);
    }
}
