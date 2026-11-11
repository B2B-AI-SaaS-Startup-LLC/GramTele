//! Port of `rpl/before_next.h`.

use crate::producer::Producer;

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::before_next(method)`: calls `method` with each value right
    /// before passing the value on (implemented with `filter`, as in C++).
    pub fn before_next(self, method: impl Fn(&V) + 'static) -> Producer<V, E> {
        self.filter(move |value| {
            method(value);
            true
        })
    }
}
