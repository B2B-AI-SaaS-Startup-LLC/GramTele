//! Port of `rpl/consumer.h`.
//!
//! A [`Consumer`] is the receiving end of a started producer: a shared,
//! reference counted set of handlers (`on_next`, `on_error`, `on_done`)
//! plus a [`Lifetime`] that is destroyed when the consumer terminates.
//! Copies of a consumer (C++ copies the `shared_ptr` to the handlers) are
//! `Clone`s here and compare equal.
//!
//! Handler semantics follow `consumer_handlers`:
//! * `next` may be invoked many times and reentrantly. The C++ code copies
//!   the handler before invoking it, so a mutable lambda never observes its
//!   own mutations; here `next` is an `Fn`, kept alive by an `Rc` clone for
//!   the duration of the call (the consumer may be terminated, and the
//!   handler dropped, from inside the call).
//! * `error` and `done` run at most once and terminate the consumer after
//!   they return. While they run the consumer is not yet terminated, as in
//!   the original.
//!
//! Difference from C++: on termination the handlers are dropped right after
//! the consumer lifetime is destroyed. The original keeps them until the
//! last copy of the consumer goes away. Dropping early releases captures
//! deterministically and breaks reference cycles through handlers.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use crate::lifetime::{Lifetime, StateRef};

/// `rpl::no_value`: a value type that can never be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoValue {}

/// `rpl::no_error`: an error type that can never be produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoError {}

/// `rpl::empty_value`. `rpl::producer<>` is `Producer<()>`.
pub type EmptyValue = ();

/// `rpl::empty`.
pub const EMPTY: EmptyValue = ();

/// `rpl::empty_error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct EmptyError;

type NextHandler<V> = Rc<dyn Fn(V)>;
type ErrorHandler<E> = Box<dyn FnOnce(E)>;
type DoneHandler = Box<dyn FnOnce()>;

struct Handlers<V, E> {
    next: RefCell<Option<NextHandler<V>>>,
    error: RefCell<Option<ErrorHandler<E>>>,
    done: RefCell<Option<DoneHandler>>,
    lifetime: Lifetime,
    terminated: Cell<bool>,
}

/// Port of `rpl::consumer<Value, Error>` (always type-erased).
pub struct Consumer<V = EmptyValue, E = NoError> {
    handlers: Rc<Handlers<V, E>>,
}

impl<V: 'static, E: 'static> Consumer<V, E> {
    /// `make_consumer<Value, Error>(next, error, done)`.
    pub fn new(
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
    ) -> Self {
        Self {
            handlers: Rc::new(Handlers {
                next: RefCell::new(Some(Rc::new(next))),
                error: RefCell::new(Some(Box::new(error))),
                done: RefCell::new(Some(Box::new(done))),
                lifetime: Lifetime::new(),
                terminated: Cell::new(false),
            }),
        }
    }

    /// `terminator()`: a callback that terminates this consumer.
    pub fn terminator(&self) -> impl FnOnce() + Clone + 'static {
        let consumer = self.clone();
        move || consumer.terminate()
    }
}

impl<V, E> Consumer<V, E> {
    /// `put_next(Value &&)`. Returns `false` if the consumer is terminated.
    pub fn put_next(&self, value: V) -> bool {
        if self.handlers.terminated.get() {
            return false;
        }
        // Same as `auto handler = this->_next;` in the original: the call
        // must survive the consumer being terminated from inside.
        let handler = self.handlers.next.borrow().clone();
        if let Some(handler) = handler {
            handler(value);
        }
        true
    }

    /// `put_next_copy(const Value &)`.
    pub fn put_next_copy(&self, value: &V) -> bool
    where
        V: Clone,
    {
        if self.handlers.terminated.get() {
            return false;
        }
        self.put_next(value.clone())
    }

    /// `put_error(Error &&)`.
    pub fn put_error(&self, error: E) {
        if self.handlers.terminated.get() {
            return;
        }
        let handler = self.handlers.error.borrow_mut().take();
        if let Some(handler) = handler {
            handler(error);
            self.terminate();
        }
    }

    /// `put_error_copy(const Error &)`.
    pub fn put_error_copy(&self, error: &E)
    where
        E: Clone,
    {
        if !self.handlers.terminated.get() {
            self.put_error(error.clone());
        }
    }

    /// `put_done()`.
    pub fn put_done(&self) {
        if self.handlers.terminated.get() {
            return;
        }
        let handler = self.handlers.done.borrow_mut().take();
        if let Some(handler) = handler {
            handler();
            self.terminate();
        }
    }

    /// `add_lifetime(lifetime &&)`. If the consumer is already terminated
    /// the lifetime is destroyed immediately and `false` is returned.
    pub fn add_lifetime(&self, lifetime: Lifetime) -> bool {
        if self.handlers.terminated.get() {
            lifetime.destroy();
            return false;
        }
        self.handlers.lifetime.add_lifetime(lifetime);
        true
    }

    /// `make_state<Type>(args...)`: state that lives until the consumer
    /// terminates. For an already terminated consumer the C++ code returns
    /// `nullptr`; here the value is dropped at once and a dead handle is
    /// returned.
    pub fn make_state<T: 'static>(&self, value: T) -> StateRef<T> {
        if self.handlers.terminated.get() {
            return StateRef::dead();
        }
        self.handlers.lifetime.make_state(value)
    }

    /// `terminate()`: marks the consumer terminated, destroys its lifetime
    /// and drops the handlers.
    pub fn terminate(&self) {
        let handlers = &self.handlers;
        if handlers.terminated.replace(true) {
            return;
        }
        handlers.lifetime.destroy();
        let next = handlers.next.borrow_mut().take();
        let error = handlers.error.borrow_mut().take();
        let done = handlers.done.borrow_mut().take();
        drop((next, error, done));
    }

    /// `true` once the consumer has received error / done or was terminated.
    pub fn is_terminated(&self) -> bool {
        self.handlers.terminated.get()
    }
}

impl<V, E> Clone for Consumer<V, E> {
    fn clone(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
        }
    }
}

/// `operator==` compares the shared handlers (`comparable()`).
impl<V, E> PartialEq for Consumer<V, E> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.handlers, &other.handlers)
    }
}

impl<V, E> Eq for Consumer<V, E> {}

impl<V, E> fmt::Debug for Consumer<V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Consumer")
            .field("handlers", &Rc::as_ptr(&self.handlers))
            .field("terminated", &self.handlers.terminated.get())
            .finish()
    }
}

/// `rpl::make_consumer<Value, Error>(next, error, done)`.
pub fn make_consumer<V: 'static, E: 'static>(
    next: impl Fn(V) + 'static,
    error: impl FnOnce(E) + 'static,
    done: impl FnOnce() + 'static,
) -> Consumer<V, E> {
    Consumer::new(next, error, done)
}
