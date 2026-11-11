//! Port of `rpl/distinct_until_changed.h`.

use std::cell::RefCell;

use crate::consumer::Consumer;
use crate::producer::Producer;

impl<V: PartialEq + Clone + 'static, E: 'static> Producer<V, E> {
    /// `| rpl::distinct_until_changed()`: drops values equal to the
    /// previous passed value.
    pub fn distinct_until_changed(self) -> Producer<V, E> {
        Producer::new(move |consumer: Consumer<V, E>| {
            let previous = consumer.make_state(RefCell::new(None::<V>));
            let next = consumer.clone();
            let error = consumer.clone();
            self.start_returning(
                move |value: V| {
                    let Some(previous) = previous.try_get() else {
                        return;
                    };
                    let changed = previous.borrow().as_ref() != Some(&value);
                    if changed {
                        *previous.borrow_mut() = Some(value.clone());
                        next.put_next(value);
                    }
                },
                move |e| error.put_error(e),
                move || consumer.put_done(),
            )
        })
    }
}
