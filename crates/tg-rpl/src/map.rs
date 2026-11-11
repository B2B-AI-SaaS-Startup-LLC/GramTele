//! Port of `rpl/map.h`: `map`, `map_to`, `to_empty`, `map_error`,
//! `map_error_to_done`.

use std::rc::Rc;

use crate::consumer::{Consumer, EmptyValue, NoError};
use crate::producer::Producer;

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::map(transform)`.
    ///
    /// C++ mappers taking several arguments from a tuple value
    /// (`map([](int a, int b) {...})`) take the tuple here:
    /// `map(|(a, b)| ...)`. Placeholder mappers (`_1 + _2`, `!_1`) are
    /// written as closures.
    pub fn map<U: 'static>(self, transform: impl Fn(V) -> U + 'static) -> Producer<U, E> {
        let transform = Rc::new(transform);
        Producer::new(move |consumer: Consumer<U, E>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value| {
                    next.put_next(transform(value));
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }

    /// `| rpl::map_to(value)`: replaces every value with a copy of `value`.
    pub fn map_to<U: Clone + 'static>(self, value: U) -> Producer<U, E> {
        self.map(move |_| value.clone())
    }

    /// `| rpl::to_empty`: replaces every value with `rpl::empty_value`.
    pub fn to_empty(self) -> Producer<EmptyValue, E> {
        self.map(|_| ())
    }

    /// `| rpl::map_error(transform)`.
    pub fn map_error<F: 'static>(self, transform: impl Fn(E) -> F + 'static) -> Producer<V, F> {
        let transform = Rc::new(transform);
        Producer::new(move |consumer: Consumer<V, F>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value| {
                    next.put_next(value);
                },
                move |e| error.put_error(transform(e)),
                move || consumer.put_done(),
            )
        })
    }

    /// `| rpl::map_error_to_done()`: an error completes the producer.
    pub fn map_error_to_done(self) -> Producer<V, NoError> {
        Producer::new(move |consumer: Consumer<V, NoError>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value| {
                    next.put_next(value);
                },
                move |_| error.put_done(),
                move || consumer.put_done(),
            )
        })
    }
}
