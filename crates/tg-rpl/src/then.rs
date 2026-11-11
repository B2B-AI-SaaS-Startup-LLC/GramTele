//! Port of `rpl/then.h`.

use crate::consumer::Consumer;
use crate::producer::Producer;

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::then(following)`: emits everything from `self`, and when it
    /// is done starts `following` and emits everything from it.
    ///
    /// The C++ version allows a `following` of a "superset" type; the
    /// library itself only defines the identity superset, so both producers
    /// must have the same value and error types here.
    pub fn then(self, following: Producer<V, E>) -> Producer<V, E> {
        Producer::new(move |consumer: Consumer<V, E>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value| {
                    next.put_next(value);
                },
                move |e| error.put_error(e),
                move || {
                    let next = consumer.clone();
                    let error = consumer.clone();
                    let done = consumer.clone();
                    consumer.add_lifetime(following.start_returning(
                        move |value| {
                            next.put_next(value);
                        },
                        move |e| error.put_error(e),
                        move || done.put_done(),
                    ));
                },
            )
        })
    }
}
