//! Port of `base/network_reachability.h` / `network_reachability.cpp`.
//!
//! The holder is ported. The original takes the state from the platform
//! helper (macOS, Windows) or from `QNetworkInformation`
//! (Linux: NetworkManager / GLib network monitor over D-Bus). That second
//! source is deferred. The constructor requires an explicit producer instead
//! of pretending to detect connectivity. The platform backend is deferred.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use tg_rpl::{Lifetime, Producer, Variable};

/// `base::NetworkReachability`.
pub struct NetworkReachability {
    available: Variable<bool>,
    _lifetime: Lifetime,
}

thread_local! {
    static GLOBAL: RefCell<Weak<NetworkReachability>> = const { RefCell::new(Weak::new()) };
}

impl NetworkReachability {
    /// Requires a real source: the missing Qt reachability backend is not
    /// silently replaced by permanent "online" state.
    pub fn new(source: Producer<bool>) -> Self {
        let result = Self {
            available: Variable::new(true),
            _lifetime: Lifetime::new(),
        };
        result.available.set_producer(source);
        result
    }

    /// `Instance()`: shared while somebody holds it.
    pub fn instance(source: Producer<bool>) -> Rc<NetworkReachability> {
        GLOBAL.with(|global| {
            if let Some(result) = global.borrow().upgrade() {
                return result;
            }
            let result = Rc::new(Self::new(source));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn starts_available_and_reports_changes() {
        let reach = NetworkReachability::new(tg_rpl::single(true));
        assert!(reach.available());
        let changes = Rc::new(Cell::new(0));
        let lifetime = Lifetime::new();
        let c = changes.clone();
        reach
            .available_changes()
            .on_next(move |_| c.set(c.get() + 1), &lifetime);
        reach.set_available(true);
        assert_eq!(changes.get(), 0);
        reach.set_available(false);
        assert!(!reach.available());
        assert_eq!(changes.get(), 1);
    }

    #[test]
    fn instance_is_shared_while_held() {
        let a = NetworkReachability::instance(tg_rpl::single(true));
        let b = NetworkReachability::instance(tg_rpl::single(true));
        assert!(Rc::ptr_eq(&a, &b));
        drop((a, b));
        let c = NetworkReachability::instance(tg_rpl::single(true));
        assert!(c.available());
    }
}
