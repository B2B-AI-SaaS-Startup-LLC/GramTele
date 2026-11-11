//! Port of `rpl/lifetime.h`.
//!
//! A [`Lifetime`] is a list of destroy callbacks. They run in reverse order
//! of addition when the lifetime is destroyed (explicitly with
//! [`Lifetime::destroy`] or implicitly on `drop`). Subscriptions register
//! their terminators here, so dropping a lifetime unsubscribes everything
//! that was started with it.
//!
//! Unlike the C++ class, all methods take `&self`: the original is almost
//! always reached through a non-owning reference (`widget->lifetime()`,
//! `[&lifetime]` captures), and callbacks may add to or destroy the very
//! lifetime that is running them. Interior mutability makes that sound;
//! no borrow is held while user callbacks run.

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

type Callback = Box<dyn FnOnce()>;

/// Port of `rpl::lifetime`.
#[derive(Default)]
pub struct Lifetime {
    callbacks: RefCell<Vec<Callback>>,
}

impl Lifetime {
    /// `rpl::lifetime()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `rpl::lifetime(destroy)`: a lifetime holding a single callback.
    pub fn from_fn(destroy: impl FnOnce() + 'static) -> Self {
        let result = Self::new();
        result.add(destroy);
        result
    }

    /// `explicit operator bool`, negated: `true` when no callbacks are held.
    pub fn is_empty(&self) -> bool {
        self.callbacks.borrow().is_empty()
    }

    /// `add(Destroy &&destroy)`.
    pub fn add(&self, destroy: impl FnOnce() + 'static) {
        self.callbacks.borrow_mut().push(Box::new(destroy));
    }

    /// `add(lifetime &&other)`: moves all callbacks of `other` to the end
    /// of this lifetime, `other` becomes empty.
    pub fn add_lifetime(&self, other: Lifetime) {
        let callbacks = other.callbacks.take();
        self.callbacks.borrow_mut().extend(callbacks);
    }

    /// `destroy()`: runs all callbacks in reverse order. Callbacks added
    /// while destroying are kept for the next `destroy()` (or drop), as in
    /// the original.
    pub fn destroy(&self) {
        let callbacks = self.callbacks.take();
        for callback in callbacks.into_iter().rev() {
            callback();
        }
    }

    /// `release()`: forgets all callbacks without running them.
    ///
    /// The callbacks themselves are dropped (their captures are released),
    /// but never invoked. Difference from C++: a value stored with
    /// [`Lifetime::make_state`] is dropped here, while the original leaks it
    /// (its raw `delete` callback is discarded). `release()` has no callers
    /// in the client.
    pub fn release(&self) {
        let callbacks = self.callbacks.take();
        drop(callbacks);
    }

    /// `make_state<Type>(args...)`: stores `value` in this lifetime and
    /// returns a non-owning handle to it.
    ///
    /// The value is dropped when the lifetime is destroyed, exactly when the
    /// C++ `delete` runs. The handle is weak: capturing it in handlers that
    /// are themselves registered in this lifetime does not create a
    /// reference cycle. Accessing the handle after the lifetime is destroyed
    /// is a dangling pointer in C++; here [`StateRef::get`] panics and
    /// [`StateRef::try_get`] returns `None`.
    pub fn make_state<T: 'static>(&self, value: T) -> StateRef<T> {
        let strong = Rc::new(value);
        let result = StateRef {
            weak: Rc::downgrade(&strong),
        };
        self.add(move || drop(strong));
        result
    }
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl fmt::Debug for Lifetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lifetime")
            .field("callbacks", &self.callbacks.borrow().len())
            .finish()
    }
}

/// Handle returned by `make_state` (the `Type*` of the original).
///
/// Cheap to clone; does not keep the state alive.
pub struct StateRef<T> {
    weak: Weak<T>,
}

impl<T> StateRef<T> {
    /// A handle to an already destroyed state (the `nullptr` returned by
    /// `consumer.make_state()` for a terminated consumer).
    pub(crate) fn dead() -> Self {
        Self { weak: Weak::new() }
    }

    /// Dereferences the state. Panics if the owning lifetime was destroyed.
    ///
    /// The returned `Rc` keeps the state alive while it is held, so a
    /// handler that destroys its own lifetime can keep using the state
    /// until it returns.
    pub fn get(&self) -> Rc<T> {
        self.weak
            .upgrade()
            .expect("rpl: state used after its lifetime was destroyed")
    }

    /// Dereferences the state if it is still alive.
    pub fn try_get(&self) -> Option<Rc<T>> {
        self.weak.upgrade()
    }

    /// `true` while the owning lifetime holds the state.
    pub fn is_alive(&self) -> bool {
        self.weak.strong_count() > 0
    }
}

impl<T> Clone for StateRef<T> {
    fn clone(&self) -> Self {
        Self {
            weak: self.weak.clone(),
        }
    }
}

impl<T> fmt::Debug for StateRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StateRef")
            .field("alive", &self.is_alive())
            .finish()
    }
}
