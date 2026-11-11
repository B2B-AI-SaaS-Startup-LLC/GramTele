//! Port of `rpl/after_next.h`.

use std::rc::Rc;

use crate::consumer::Consumer;
use crate::producer::Producer;

impl<V: Clone + 'static, E: 'static> Producer<V, E> {
    /// `| rpl::after_next(method)`: passes a copy of each value on, then
    /// calls `method` with the value.
    pub fn after_next(self, method: impl Fn(V) + 'static) -> Producer<V, E> {
        let method = Rc::new(method);
        Producer::new(move |consumer: Consumer<V, E>| {
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value: V| {
                    let copy = method.clone();
                    next.put_next_copy(&value);
                    copy(value);
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }
}
