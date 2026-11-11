//! Port of `rpl/variable.h`.
//!
//! A [`Variable`] holds a current value and an [`EventStream`] of its
//! changes. Assigning a value equal to the current one does not fire.
//!
//! Equality: the C++ class detects at compile time whether `operator==`
//! exists and, without it, fires on every assignment. Rust has no such
//! detection, so the choice is made at construction: [`Variable::new`]
//! (requires `PartialEq`, compares) or [`Variable::new_non_comparable`]
//! (never compares, fires on every assignment).
//!
//! All methods take `&self`; the value lives in a `RefCell` that is never
//! borrowed while handlers run, so reading `current()` or assigning from a
//! handler of `value()` / `changes()` is fine.
//!
//! Differences from C++:
//! * moving a `Variable` moves its subscribers with it; the C++ move
//!   constructor moves only the value (subscribers stay with the moved-from
//!   object). Write `b.set(a.current())` where the C++ code relies on that;
//! * `Clone` is the C++ copy constructor (value only, no subscribers);
//!   C++ copy assignment `a = b` is `a.set(b.current())`.

use std::cell::{Ref, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

use crate::consumer::NoError;
use crate::event_stream::EventStream;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

// Fields are declared in reverse order of the C++ members, so that they are
// dropped in the same order as C++ destroys them: the producer subscription
// first, then the stream (sending `done`), then the value.
struct Inner<T, E> {
    lifetime: Lifetime,
    changes: EventStream<T, E>,
    data: RefCell<T>,
    equal: Option<fn(&T, &T) -> bool>,
}

impl<T: Clone + 'static, E: Clone + 'static> Inner<T, E> {
    fn assign(&self, value: T) {
        if let Some(equal) = self.equal {
            if equal(&self.data.borrow(), &value) {
                return;
            }
        }
        self.replace_and_fire(value);
    }

    fn replace_and_fire(&self, value: T) {
        let old = std::mem::replace(&mut *self.data.borrow_mut(), value);
        drop(old);
        let copy = self.data.borrow().clone();
        self.changes.fire(copy);
    }

    fn subscribe<PE: 'static>(self: &Rc<Self>, stream: Producer<T, PE>) {
        let weak: Weak<Self> = Rc::downgrade(self);
        stream.on_next(
            move |data| {
                if let Some(strong) = weak.upgrade() {
                    strong.assign(data);
                }
            },
            &self.lifetime,
        );
    }
}

/// Port of `rpl::variable<Type, Error>`.
pub struct Variable<T, E = NoError> {
    inner: Rc<Inner<T, E>>,
}

impl<T: Clone + 'static, E: Clone + 'static> Variable<T, E> {
    fn with_equal(value: T, equal: Option<fn(&T, &T) -> bool>) -> Self {
        Self {
            inner: Rc::new(Inner {
                lifetime: Lifetime::new(),
                changes: EventStream::new(),
                data: RefCell::new(value),
                equal,
            }),
        }
    }

    /// `variable(value)` for a type with `operator==`.
    pub fn new(value: T) -> Self
    where
        T: PartialEq,
    {
        Self::with_equal(value, Some(<T as PartialEq>::eq))
    }

    /// `variable(value)` for a type without `operator==`: every assignment
    /// fires.
    pub fn new_non_comparable(value: T) -> Self {
        Self::with_equal(value, None)
    }

    /// `variable(producer)`: starts with `T::default()` and follows the
    /// values of `stream` (only values; its error / done are ignored).
    pub fn from_producer<PE: 'static>(stream: Producer<T, PE>) -> Self
    where
        T: PartialEq + Default,
    {
        let result = Self::new(T::default());
        result.inner.subscribe(stream);
        result
    }

    /// `variable = value`: stops following a producer assigned earlier and
    /// assigns `value`, firing if it differs from the current one.
    pub fn set(&self, value: T) {
        self.inner.lifetime.destroy();
        self.inner.assign(value);
    }

    /// `variable = std::move(producer)`: stops following the previous
    /// producer and follows `stream`.
    pub fn set_producer<PE: 'static>(&self, stream: Producer<T, PE>) {
        self.inner.lifetime.destroy();
        self.inner.subscribe(stream);
    }

    /// `force_assign(value)`: assigns and fires even if equal.
    pub fn force_assign(&self, value: T) {
        self.inner.lifetime.destroy();
        self.inner.replace_and_fire(value);
    }

    /// `current()`, returning a copy.
    pub fn current(&self) -> T {
        self.inner.data.borrow().clone()
    }

    /// `current()`, returning a reference. Do not hold it across an
    /// assignment to the same variable (that panics).
    pub fn current_ref(&self) -> Ref<'_, T> {
        self.inner.data.borrow()
    }

    /// `value()`: the current value (copied now, at the call), then every
    /// change.
    pub fn value(&self) -> Producer<T, E> {
        self.inner.changes.events_starting_with(self.current())
    }

    /// `changes()`: every change, without the current value.
    pub fn changes(&self) -> Producer<T, E> {
        self.inner.changes.events()
    }

    /// `reset(value)`: assigns without firing, then sends `done` to all
    /// subscribers and unsubscribes them.
    ///
    /// As in C++, a producer assigned earlier keeps being followed.
    pub fn reset(&self, value: T) {
        let old = std::mem::replace(&mut *self.inner.data.borrow_mut(), value);
        drop(old);
        // `_changes = event_stream<Type, Error>();`
        self.inner.changes.fire_done();
    }

    /// `reset()`: `reset(Type())`.
    pub fn reset_default(&self)
    where
        T: Default,
    {
        self.reset(T::default());
    }

    /// `reset_with_error(error)`: sends `error` to all subscribers.
    pub fn reset_with_error(&self, error: E) {
        self.inner.changes.fire_error(error);
    }
}

impl<T: Clone + PartialEq + Default + 'static, E: Clone + 'static> Default for Variable<T, E> {
    /// `variable()`.
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Clone + 'static, E: Clone + 'static> Clone for Variable<T, E> {
    /// `variable(const variable &other)`: copies the value only.
    fn clone(&self) -> Self {
        Self::with_equal(self.current(), self.inner.equal)
    }
}

impl<T: fmt::Debug, E> fmt::Debug for Variable<T, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Variable")
            .field("current", &*self.inner.data.borrow())
            .finish()
    }
}
