//! Port of `rpl/take.h`: `take(count)` and `take_while(predicate)`.

use std::cell::Cell;
use std::rc::Rc;

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::take(count)`: passes the first `count` values, then done.
    ///
    /// As in the original, `take(0)` completes on the first value, not
    /// immediately.
    pub fn take(self, count: usize) -> Producer<V, E> {
        Producer::new(move |consumer: Consumer<V, E>| {
            let state = consumer.make_state(Cell::new(count));
            let initial_consumer = {
                let next = consumer.clone();
                let error = consumer.clone();
                let done = consumer.clone();
                Consumer::new(
                    move |value| {
                        let Some(count) = state.try_get() else {
                            return;
                        };
                        let mut left = count.get();
                        count.set(left.saturating_sub(1));
                        if left != 0 {
                            next.put_next(value);
                            left -= 1;
                        }
                        if left == 0 {
                            next.put_done();
                        }
                    },
                    move |e| error.put_error(e),
                    move || done.put_done(),
                )
            };
            consumer.add_lifetime(Lifetime::from_fn(initial_consumer.terminator()));
            self.start_existing_returning(&initial_consumer)
        })
    }

    /// `| rpl::take_while(predicate)`: passes values while `predicate`
    /// holds; the first value failing it completes the producer.
    pub fn take_while(self, predicate: impl Fn(&V) -> bool + 'static) -> Producer<V, E> {
        let predicate = Rc::new(predicate);
        Producer::new(move |consumer: Consumer<V, E>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value| {
                    if predicate(&value) {
                        next.put_next(value);
                    } else {
                        next.put_done();
                    }
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }
}
