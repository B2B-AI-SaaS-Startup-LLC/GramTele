//! Port of `rpl/producer.h`.
//!
//! A [`Producer`] is a lazy, restartable description of a sequence: a
//! generator that, given a [`Consumer`], starts pushing values to it and
//! returns a [`Lifetime`] that stops the work.
//!
//! Representation: always type-erased (the C++ debug / MSVC configuration,
//! `RPL_PRODUCER_TYPE_ERASED_ALWAYS`). The generator is a boxed `FnOnce`
//! that is also `Clone`:
//! * starting a producer consumes it and *moves* the generator, so values
//!   captured by `single(value)` and friends are moved, not copied, exactly
//!   like `std::move(producer).start(...)`;
//! * cloning a producer (`rpl::duplicate`, copy-construction in C++) clones
//!   the generator together with everything it captured, as copying a
//!   `std::function` does. The cost is one allocation per chained operator.
//!
//! Chaining: C++ `producer | operator(args)` is a method call here:
//! `producer.operator(args)`. Operator methods are defined in the modules
//! named after the original headers (`map.rs`, `filter.rs`, ...). For
//! application-defined operators (C++ functors applied with `|`) use
//! [`Producer::pipe`].

use std::fmt;

use crate::consumer::{Consumer, EmptyValue, NoError};
use crate::lifetime::Lifetime;

trait Generator<V, E> {
    fn call(self: Box<Self>, consumer: Consumer<V, E>) -> Lifetime;
    fn clone_box(&self) -> Box<dyn Generator<V, E>>;
}

impl<V, E, F> Generator<V, E> for F
where
    F: FnOnce(Consumer<V, E>) -> Lifetime + Clone + 'static,
    V: 'static,
    E: 'static,
{
    fn call(self: Box<Self>, consumer: Consumer<V, E>) -> Lifetime {
        (*self)(consumer)
    }

    fn clone_box(&self) -> Box<dyn Generator<V, E>> {
        Box::new(self.clone())
    }
}

/// Port of `rpl::producer<Value, Error>`.
#[must_use = "producers are lazy and do nothing unless started"]
pub struct Producer<V = EmptyValue, E = NoError> {
    generator: Option<Box<dyn Generator<V, E>>>,
}

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `rpl::producer<Value, Error>(generator)` /
    /// `rpl::make_producer<Value, Error>(generator)`.
    ///
    /// The generator receives the consumer by value and returns the
    /// lifetime of the work it started. It is called once per start; it
    /// must be `Clone` so the producer can be duplicated (in C++ the lambda
    /// must be copyable to be stored in `std::function`).
    pub fn new<F>(generator: F) -> Self
    where
        F: FnOnce(Consumer<V, E>) -> Lifetime + Clone + 'static,
    {
        Self {
            generator: Some(Box::new(generator)),
        }
    }

    /// `explicit operator bool`, negated: `true` for a default-constructed
    /// producer without a generator. Such a producer never emits anything.
    pub fn is_empty(&self) -> bool {
        self.generator.is_none()
    }

    /// `start_existing(consumer, alive_while)`.
    pub fn start_existing(self, consumer: &Consumer<V, E>, alive_while: &Lifetime) {
        alive_while.add(consumer.terminator());
        let lifetime = match self.generator {
            Some(generator) => generator.call(consumer.clone()),
            None => Lifetime::new(),
        };
        consumer.add_lifetime(lifetime);
    }

    /// `start_existing(consumer)` returning the lifetime.
    pub fn start_existing_returning(self, consumer: &Consumer<V, E>) -> Lifetime {
        let result = Lifetime::new();
        self.start_existing(consumer, &result);
        result
    }

    /// `start(next, error, done, alive_while)`.
    pub fn start(
        self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
        alive_while: &Lifetime,
    ) {
        self.start_existing(&Consumer::new(next, error, done), alive_while);
    }

    /// `start(next, error, done)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn start_returning(
        self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
    ) -> Lifetime {
        let result = Lifetime::new();
        self.start(next, error, done, &result);
        result
    }

    /// `start_copy(next, error, done, alive_while) const &`.
    pub fn start_copy(
        &self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
        alive_while: &Lifetime,
    ) {
        self.clone().start(next, error, done, alive_while);
    }

    /// `start_copy(next, error, done) const &` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn start_copy_returning(
        &self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
    ) -> Lifetime {
        self.clone().start_returning(next, error, done)
    }

    /// `| rpl::start(alive_while)`: start without handlers.
    pub fn start_no_handlers(self, alive_while: &Lifetime) {
        self.start(|_| {}, |_| {}, || {}, alive_while);
    }

    /// `| rpl::start()`: start without handlers, returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn start_no_handlers_returning(self) -> Lifetime {
        self.start_returning(|_| {}, |_| {}, || {})
    }

    /// `| rpl::on_next(next, alive_while)`.
    pub fn on_next(self, next: impl Fn(V) + 'static, alive_while: &Lifetime) {
        self.start(next, |_| {}, || {}, alive_while);
    }

    /// `| rpl::on_next(next)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn on_next_returning(self, next: impl Fn(V) + 'static) -> Lifetime {
        self.start_returning(next, |_| {}, || {})
    }

    /// `| rpl::on_error(error, alive_while)`.
    pub fn on_error(self, error: impl FnOnce(E) + 'static, alive_while: &Lifetime) {
        self.start(|_| {}, error, || {}, alive_while);
    }

    /// `| rpl::on_error(error)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn on_error_returning(self, error: impl FnOnce(E) + 'static) -> Lifetime {
        self.start_returning(|_| {}, error, || {})
    }

    /// `| rpl::on_done(done, alive_while)`.
    pub fn on_done(self, done: impl FnOnce() + 'static, alive_while: &Lifetime) {
        self.start(|_| {}, |_| {}, done, alive_while);
    }

    /// `| rpl::on_done(done)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn on_done_returning(self, done: impl FnOnce() + 'static) -> Lifetime {
        self.start_returning(|_| {}, |_| {}, done)
    }

    /// `| rpl::on_next_error(next, error, alive_while)`.
    pub fn on_next_error(
        self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        alive_while: &Lifetime,
    ) {
        self.start(next, error, || {}, alive_while);
    }

    /// `| rpl::on_next_error(next, error)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn on_next_error_returning(
        self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
    ) -> Lifetime {
        self.start_returning(next, error, || {})
    }

    /// `| rpl::on_error_done(error, done, alive_while)`.
    pub fn on_error_done(
        self,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
        alive_while: &Lifetime,
    ) {
        self.start(|_| {}, error, done, alive_while);
    }

    /// `| rpl::on_error_done(error, done)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn on_error_done_returning(
        self,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
    ) -> Lifetime {
        self.start_returning(|_| {}, error, done)
    }

    /// `| rpl::on_next_done(next, done, alive_while)`.
    pub fn on_next_done(
        self,
        next: impl Fn(V) + 'static,
        done: impl FnOnce() + 'static,
        alive_while: &Lifetime,
    ) {
        self.start(next, |_| {}, done, alive_while);
    }

    /// `| rpl::on_next_done(next, done)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn on_next_done_returning(
        self,
        next: impl Fn(V) + 'static,
        done: impl FnOnce() + 'static,
    ) -> Lifetime {
        self.start_returning(next, |_| {}, done)
    }

    /// `| rpl::on_next_error_done(next, error, done, alive_while)`.
    pub fn on_next_error_done(
        self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
        alive_while: &Lifetime,
    ) {
        self.start(next, error, done, alive_while);
    }

    /// `| rpl::on_next_error_done(next, error, done)` returning the lifetime.
    #[must_use = "dropping the lifetime unsubscribes immediately"]
    pub fn on_next_error_done_returning(
        self,
        next: impl Fn(V) + 'static,
        error: impl FnOnce(E) + 'static,
        done: impl FnOnce() + 'static,
    ) -> Lifetime {
        self.start_returning(next, error, done)
    }

    /// Applies an application-defined operator: C++ `producer | Helper()`
    /// where `Helper()` returns a functor taking the producer.
    pub fn pipe<R>(self, operator: impl FnOnce(Self) -> R) -> R {
        operator(self)
    }
}

impl<V, E> Clone for Producer<V, E> {
    fn clone(&self) -> Self {
        Self {
            generator: self.generator.as_ref().map(|g| g.clone_box()),
        }
    }
}

impl<V, E> Default for Producer<V, E> {
    /// `rpl::producer<Value, Error>()`: no generator, never emits.
    fn default() -> Self {
        Self { generator: None }
    }
}

impl<V, E> fmt::Debug for Producer<V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Producer")
            .field("empty", &self.generator.is_none())
            .finish()
    }
}

/// `rpl::make_producer<Value, Error>(generator)`.
pub fn make_producer<V, E, F>(generator: F) -> Producer<V, E>
where
    V: 'static,
    E: 'static,
    F: FnOnce(Consumer<V, E>) -> Lifetime + Clone + 'static,
{
    Producer::new(generator)
}

/// `rpl::duplicate(producer)`: a copy that can be started independently.
pub fn duplicate<V, E>(value: &Producer<V, E>) -> Producer<V, E> {
    value.clone()
}
