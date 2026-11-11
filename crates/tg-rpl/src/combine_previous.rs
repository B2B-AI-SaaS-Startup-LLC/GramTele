//! Port of `rpl/combine_previous.h`.
//!
//! Order of operations follows the original exactly: the pair is built and
//! passed on first, and only after the downstream handler returns is the
//! stored previous value replaced. A value arriving reentrantly from inside
//! the downstream handler therefore sees the old previous value, and the
//! outer value overwrites whatever the reentrant one stored.
//!
//! The C++ code moves the old previous value into the pair, leaving a
//! moved-from object in the state while the handler runs. Rust has no
//! moved-from state, so the old value is cloned into the pair instead and
//! stays in place (for trivially copyable types, where a move is a copy,
//! this is exactly the C++ behavior). Cost: one extra clone per value.

use std::cell::RefCell;

use crate::consumer::Consumer;
use crate::producer::Producer;

impl<V: Clone + 'static, E: 'static> Producer<V, E> {
    /// `| rpl::combine_previous()`: emits `(previous, current)` pairs,
    /// starting from the second value.
    pub fn combine_previous(self) -> Producer<(V, V), E> {
        Producer::new(move |consumer: Consumer<(V, V), E>| {
            let previous = consumer.make_state(RefCell::new(None::<V>));
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value: V| {
                    let Some(previous) = previous.try_get() else {
                        return;
                    };
                    let old = previous.borrow().clone();
                    match old {
                        Some(old) => {
                            next.put_next((old, value.clone()));
                            *previous.borrow_mut() = Some(value);
                        }
                        None => *previous.borrow_mut() = Some(value),
                    }
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }

    /// `| rpl::combine_previous(value)`: emits `(previous, current)` pairs,
    /// the first value is paired with `value`.
    pub fn combine_previous_with_default(self, value: V) -> Producer<(V, V), E> {
        Producer::new(move |consumer: Consumer<(V, V), E>| {
            let previous = consumer.make_state(RefCell::new(value));
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value: V| {
                    let Some(previous) = previous.try_get() else {
                        return;
                    };
                    let old = previous.borrow().clone();
                    next.put_next((old, value.clone()));
                    *previous.borrow_mut() = value;
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }
}
