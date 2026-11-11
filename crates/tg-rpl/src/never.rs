//! Port of `rpl/never.h`.

use crate::consumer::Consumer;
use crate::lifetime::Lifetime;
use crate::producer::Producer;

/// `rpl::never<Value>()`: never emits anything, never completes.
pub fn never<V: 'static>() -> Producer<V> {
    never_e()
}

/// `rpl::never<Value, Error>()`.
pub fn never_e<V: 'static, E: 'static>() -> Producer<V, E> {
    Producer::new(|_consumer: Consumer<V, E>| Lifetime::new())
}
