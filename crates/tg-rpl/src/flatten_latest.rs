//! Port of `rpl/flatten_latest.h`.

use std::cell::Cell;

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

struct State {
    alive: Lifetime,
    finished: Cell<bool>,
}

impl<V: 'static, E: 'static> Producer<Producer<V, E>, E> {
    /// `| rpl::flatten_latest()`: follows the latest inner producer,
    /// unsubscribing from the previous one. Done when both the outer
    /// producer and the current inner producer are done.
    pub fn flatten_latest(self) -> Producer<V, E> {
        Producer::new(move |consumer: Consumer<V, E>| {
            let state = consumer.make_state(State {
                alive: Lifetime::new(),
                finished: Cell::new(false),
            });
            let outer_next = consumer.clone();
            let outer_error = consumer.clone();
            let outer_done = consumer.clone();
            let next_state = state.clone();
            self.start_returning(
                move |inner: Producer<V, E>| {
                    let Some(strong) = next_state.try_get() else {
                        return;
                    };
                    strong.finished.set(false);
                    // `state->alive = lifetime();`
                    strong.alive.destroy();
                    let next = outer_next.clone();
                    let error = outer_next.clone();
                    let done = outer_next.clone();
                    let done_state = next_state.clone();
                    inner.start(
                        move |value| {
                            next.put_next(value);
                        },
                        move |e| error.put_error(e),
                        move || {
                            if let Some(state) = done_state.try_get() {
                                if state.finished.get() {
                                    done.put_done();
                                } else {
                                    state.finished.set(true);
                                }
                            }
                        },
                        &strong.alive,
                    );
                },
                move |e| outer_error.put_error(e),
                move || {
                    if let Some(state) = state.try_get() {
                        if state.finished.get() {
                            outer_done.put_done();
                        } else {
                            state.finished.set(true);
                        }
                    }
                },
            )
        })
    }
}
