//! Port of `base/screen_reader_state.h` / `screen_reader_state.cpp`.
//!
//! The interface and the generic implementation (`GeneralScreenReaderState`,
//! a flag set by the toolkit's accessibility activation observer) are
//! ported. Who calls [`GeneralScreenReaderState::accessibility_active_changed`]
//! is the accessibility layer of `tg-window` (stage 5); the Linux-specific
//! detection through `org.a11y.Status` on D-Bus is deferred.

use std::cell::RefCell;
use std::rc::Rc;
use tg_rpl::{Producer, Variable};

/// `base::ScreenReaderState`.
pub trait ScreenReaderState {
    fn active(&self) -> bool;
    fn active_value(&self) -> Producer<bool>;
}

/// `base::GeneralScreenReaderState`.
pub struct GeneralScreenReaderState {
    is_active: Variable<bool>,
}

impl GeneralScreenReaderState {
    /// `initially_active` is `QAccessible::isActive()` at construction.
    pub fn new(initially_active: bool) -> Self {
        Self {
            is_active: Variable::new(initially_active),
        }
    }

    /// `accessibilityActiveChanged`.
    pub fn accessibility_active_changed(&self, active: bool) {
        self.is_active.set(active);
    }
}

impl ScreenReaderState for GeneralScreenReaderState {
    fn active(&self) -> bool {
        self.is_active.current()
    }

    fn active_value(&self) -> Producer<bool> {
        self.is_active.value()
    }
}

thread_local! {
    static INSTANCE: RefCell<Option<Rc<dyn ScreenReaderState>>> = const { RefCell::new(None) };
}

/// Installs what `ScreenReaderState::Instance()` returns (the per-platform
/// object chosen with `#ifdef` in the original).
pub fn set_instance(state: Option<Rc<dyn ScreenReaderState>>) {
    INSTANCE.with(|instance| *instance.borrow_mut() = state);
}

/// `ScreenReaderState::Instance()`: `None` until a platform object is set.
pub fn instance() -> Option<Rc<dyn ScreenReaderState>> {
    INSTANCE.with(|instance| instance.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_state_follows_the_observer() {
        let state = Rc::new(GeneralScreenReaderState::new(false));
        set_instance(Some(state.clone()));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let lifetime = tg_rpl::Lifetime::new();
        let s = seen.clone();
        instance().unwrap().active_value().on_next(move |v| s.borrow_mut().push(v), &lifetime);
        state.accessibility_active_changed(true);
        state.accessibility_active_changed(true);
        assert!(instance().unwrap().active());
        assert_eq!(*seen.borrow(), vec![false, true]);
        set_instance(None);
        assert!(instance().is_none());
    }
}
