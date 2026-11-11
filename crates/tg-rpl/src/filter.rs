//! Port of `rpl/filter.h`: `filter(predicate)`, `filter(producer<bool>)`,
//! `filter_optional()`, `filter_nullptr()`.

use std::rc::Rc;

use crate::combine::combine;
use crate::consumer::Consumer;
use crate::producer::Producer;

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::filter(predicate)`. The predicate sees the value by
    /// reference (`const auto &` in C++); tuple values are destructured as
    /// `filter(|(a, b)| ...)` with `a`, `b` being references.
    pub fn filter(self, predicate: impl Fn(&V) -> bool + 'static) -> Producer<V, E> {
        let predicate = Rc::new(predicate);
        Producer::new(move |consumer: Consumer<V, E>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value| {
                    if predicate(&value) {
                        next.put_next(value);
                    }
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }

    /// `| rpl::filter(filterer)` with a `producer<bool>`: passes values
    /// while the latest value of `filterer` is `true`.
    ///
    /// Implemented as in C++: `combine(self, filterer) | filter(_2) |
    /// map(_1_of_two)`, so nothing passes until both have produced a value,
    /// and a change of `filterer` to `true` re-emits the latest value.
    pub fn filter_producer(self, filterer: Producer<bool, E>) -> Producer<V, E>
    where
        V: Clone,
    {
        combine((self, filterer))
            .filter(|(_, pass)| *pass)
            .map(|(value, _)| value)
    }
}

impl<V: 'static, E: 'static> Producer<Option<V>, E> {
    /// `| rpl::filter_optional()`: drops `None`, unwraps `Some`.
    pub fn filter_optional(self) -> Producer<V, E> {
        Producer::new(move |consumer: Consumer<V, E>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value: Option<V>| {
                    if let Some(value) = value {
                        next.put_next(value);
                    }
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }

    /// `| rpl::filter_nullptr()`: in C++ drops null pointers and yields
    /// `not_null`. Nullable pointers are `Option` in Rust, so this is the
    /// same as [`Producer::filter_optional`].
    pub fn filter_nullptr(self) -> Producer<V, E> {
        self.filter_optional()
    }
}
