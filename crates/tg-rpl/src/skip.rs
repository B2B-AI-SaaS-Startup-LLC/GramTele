//! Port of `rpl/skip.h`.

use std::cell::Cell;

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::skip(count)`: drops the first `count` values.
    pub fn skip(self, count: usize) -> Producer<V, E> {
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
                        if count.get() != 0 {
                            count.set(count.get() - 1);
                        } else {
                            next.put_next(value);
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
}
