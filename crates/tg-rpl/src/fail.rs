//! Port of `rpl/fail.h`.

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

/// `rpl::fail<Value>(error)`: fails immediately with `error`.
pub fn fail<V: 'static, E: Clone + 'static>(error: E) -> Producer<V, E> {
    Producer::new(move |consumer: Consumer<V, E>| {
        consumer.put_error(error);
        Lifetime::new()
    })
}
