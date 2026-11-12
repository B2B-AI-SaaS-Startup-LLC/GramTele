//! Port of `base/network_reachability.h` / `network_reachability.cpp`.
//!
//! The holder is ported. The original takes the state from the platform
//! helper (macOS, Windows) or from `QNetworkInformation`
//! (Linux: NetworkManager / GLib network monitor over D-Bus). That second
//! source is deferred: on Linux nothing reports changes, `available` stays
//! `true` (the value the original starts with) until a source is plugged in
//! with [`NetworkReachability::set_available`].

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use tg_rpl::{Lifetime, Producer, Variable};

use crate::platform::base_platform_network_reachability as platform;

/// `base::NetworkReachability`.
pub struct NetworkReachability {
    available: Variable<bool>,
    _lifetime: Lifetime,
}

thread_local! {
    static GLOBAL: RefCell<Weak<NetworkReachability>> = const { RefCell::new(Weak::new()) };
}

impl NetworkReachability {
    pub fn new() -> Self {
        let result = Self {
            available: Variable::new(true),
            _lifetime: Lifetime::new(),
        };
        if let Some(helper) = platform::create() {
            // The helper's stream is copied into the variable.
            result.available.set_producer(helper.available_value());
        }
        result
    }

    /// `Instance()`: shared while somebody holds it.
    pub fn instance() -> Rc<NetworkReachability> {
        GLOBAL.with(|global| {
            if let Some(result) = global.borrow().upgrade() {
                return result;
            }
            let result = Rc::new(Self::new());
            *global.borrow_mut() = Rc::downgrade(&result);
            result
        })
    }

    pub fn available(&self) -> bool {
        self.available.current()
    }

    pub fn available_changes(&self) -> Producer<bool> {
        self.available.changes()
    }

    pub fn available_value(&self) -> Producer<bool> {
        self.available.value()
    }

    /// Entry point for a platform source (not in the original, which wires
    /// the sources inside the constructor).
    pub fn set_available(&self, value: bool) {
        self.available.set(value);
    }
}

impl Default for NetworkReachability {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn starts_available_and_reports_changes() {
        let reach = NetworkReachability::new();
        assert!(reach.available());
        let changes = Rc::new(Cell::new(0));
        let lifetime = Lifetime::new();
        let c = changes.clone();
        reach.available_changes().on_next(move |_| c.set(c.get() + 1), &lifetime);
        reach.set_available(true);
        assert_eq!(changes.get(), 0);
        reach.set_available(false);
        assert!(!reach.available());
        assert_eq!(changes.get(), 1);
    }

    #[test]
    fn instance_is_shared_while_held() {
        let a = NetworkReachability::instance();
        let b = NetworkReachability::instance();
        assert!(Rc::ptr_eq(&a, &b));
        drop((a, b));
        let c = NetworkReachability::instance();
        assert!(c.available());
    }
}
