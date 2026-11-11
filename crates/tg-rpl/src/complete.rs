//! Port of `rpl/complete.h`.

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

/// `rpl::complete<Value>()`: done immediately, no values.
pub fn complete<V: 'static>() -> Producer<V> {
    complete_e()
}

/// `rpl::complete<Value, Error>()`.
pub fn complete_e<V: 'static, E: 'static>() -> Producer<V, E> {
    Producer::new(|consumer: Consumer<V, E>| {
        consumer.put_done();
        Lifetime::new()
    })
}
