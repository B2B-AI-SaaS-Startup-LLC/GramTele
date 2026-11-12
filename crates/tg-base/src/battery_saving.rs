//! Port of `base/battery_saving.h` / `battery_saving.cpp`.
//!
//! The generic holder is ported; the Linux helper (`GPowerProfileMonitor`
//! through GLib) is deferred, so [`create_battery_saving`] reports "not
//! supported" until a platform helper is installed.

use std::rc::Rc;
use tg_rpl::{Producer, Variable};

/// `Platform::AbstractBatterySaving`.
pub trait AbstractBatterySaving {
    fn enabled(&self) -> Option<bool>;
}

/// `Platform::CreateBatterySaving(changeCallback)` for platforms without a
/// helper: `enabled()` is always `None`.
struct Unsupported;
impl AbstractBatterySaving for Unsupported {
    fn enabled(&self) -> Option<bool> {
        None
    }
}

/// Factory of a platform helper; the callback must be called on change.
pub type HelperFactory = Box<dyn FnOnce(Box<dyn Fn()>) -> Rc<dyn AbstractBatterySaving>>;

/// `base::BatterySaving`.
pub struct BatterySaving {
    helper: Rc<dyn AbstractBatterySaving>,
    value: Rc<Variable<Option<bool>>>,
    _helper_slot: Rc<std::cell::RefCell<Option<Rc<dyn AbstractBatterySaving>>>>,
}

/// Explicit unsupported helper. Linux GLib detection must be supplied by the
/// future platform backend; this factory does not perform platform detection.
pub fn unsupported_factory() -> HelperFactory {
    Box::new(|_| Rc::new(Unsupported))
}

impl BatterySaving {
    pub fn new(factory: HelperFactory) -> Self {
        let value = Rc::new(Variable::new(None));
        let slot: Rc<std::cell::RefCell<Option<Rc<dyn AbstractBatterySaving>>>> = Rc::default();
        let changed = {
            let value = Rc::downgrade(&value);
            let slot = Rc::downgrade(&slot);
            Box::new(move || {
                if let (Some(value), Some(slot)) = (value.upgrade(), slot.upgrade()) {
                    let helper = slot.borrow().clone();
                    if let Some(helper) = helper {
                        value.set(helper.enabled());
                    }
                }
            })
        };
        let helper = factory(changed);
        *slot.borrow_mut() = Some(helper.clone());
        value.set(helper.enabled());
        Self {
            helper,
            value,
            _helper_slot: slot,
        }
    }

    pub fn supported(&self) -> bool {
        self.enabled().is_some()
    }

    pub fn enabled(&self) -> Option<bool> {
        self.value.current()
    }

    /// `value()`: unsupported counts as `false`.
    pub fn value(&self) -> Producer<bool> {
        self.value.value().map(|maybe| maybe.unwrap_or(false))
    }

    #[doc(hidden)]
    pub fn helper(&self) -> &Rc<dyn AbstractBatterySaving> {
        &self.helper
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Fake(Cell<Option<bool>>);
    impl AbstractBatterySaving for Fake {
        fn enabled(&self) -> Option<bool> {
            self.0.get()
        }
    }

    #[test]
    fn unsupported_is_false_and_unsupported() {
        let saving = BatterySaving::new(unsupported_factory());
        assert!(!saving.supported());
        assert_eq!(saving.enabled(), None);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let lifetime = tg_rpl::Lifetime::new();
        let s = seen.clone();
        saving
            .value()
            .on_next(move |v| s.borrow_mut().push(v), &lifetime);
        assert_eq!(*seen.borrow(), vec![false]);
    }

    #[test]
    fn stored_change_callback_does_not_retain_the_helper() {
        struct Helper {
            _changed: Box<dyn Fn()>,
        }
        impl AbstractBatterySaving for Helper {
            fn enabled(&self) -> Option<bool> {
                Some(false)
            }
        }
        let weak_slot = Rc::new(RefCell::new(None));
        let output = weak_slot.clone();
        let saving = BatterySaving::new(Box::new(move |changed| {
            let helper = Rc::new(Helper { _changed: changed });
            *output.borrow_mut() = Some(Rc::downgrade(&helper));
            helper
        }));
        assert!(weak_slot.borrow().as_ref().unwrap().upgrade().is_some());
        drop(saving);
        assert!(weak_slot.borrow().as_ref().unwrap().upgrade().is_none());
    }

    #[test]
    fn follows_the_helper_on_change_callback() {
        let fake = Rc::new(Fake(Cell::new(Some(false))));
        type Notify = Rc<RefCell<Option<Box<dyn Fn()>>>>;
        let notify: Notify = Rc::default();
        let (f, n) = (fake.clone(), notify.clone());
        let saving = BatterySaving::new(Box::new(move |changed| {
            *n.borrow_mut() = Some(changed);
            f
        }));
        assert!(saving.supported());
        assert_eq!(saving.enabled(), Some(false));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let lifetime = tg_rpl::Lifetime::new();
        let s = seen.clone();
        saving
            .value()
            .on_next(move |v| s.borrow_mut().push(v), &lifetime);
        fake.0.set(Some(true));
        (notify.borrow().as_ref().unwrap())();
        fake.0.set(None);
        (notify.borrow().as_ref().unwrap())();
        assert_eq!(*seen.borrow(), vec![false, true, false]);
        assert!(!saving.supported());
    }
}
