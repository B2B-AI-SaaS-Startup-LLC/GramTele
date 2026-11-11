//! Port of `rpl/range.h`: finite producers built from values.
//!
//! Every function here exists in two forms: the plain one fixes the error
//! type to [`NoError`] (what the client uses in practice, and what keeps
//! type inference working in chains), the `_e` one is generic over the
//! error type (`rpl::single<Value, Error>(value)`).

use crate::consumer::{Consumer, NoError};
use crate::lifetime::Lifetime;
use crate::producer::Producer;

/// `rpl::single(value)`: emits `value` once, then done.
///
/// `rpl::single()` (an empty value) is `single(())`.
pub fn single<V: Clone + 'static>(value: V) -> Producer<V> {
    single_e(value)
}

/// `rpl::single<Value, Error>(value)`.
pub fn single_e<V: Clone + 'static, E: 'static>(value: V) -> Producer<V, E> {
    Producer::new(move |consumer: Consumer<V, E>| {
        consumer.put_next(value);
        consumer.put_done();
        Lifetime::new()
    })
}

/// `rpl::vector(values)`: emits all values in order, then done.
pub fn vector<V: Clone + 'static>(values: Vec<V>) -> Producer<V> {
    vector_e(values)
}

/// `rpl::vector<Value, Error>(values)`.
pub fn vector_e<V: Clone + 'static, E: 'static>(values: Vec<V>) -> Producer<V, E> {
    Producer::new(move |consumer: Consumer<V, E>| {
        for value in values {
            consumer.put_next(value);
        }
        consumer.put_done();
        Lifetime::new()
    })
}

/// `rpl::range(range)`: collects the range into a vector, see [`vector`].
pub fn range<I>(range: I) -> Producer<I::Item>
where
    I: IntoIterator,
    I::Item: Clone + 'static,
{
    vector(range.into_iter().collect())
}

/// `rpl::ints(from, till)`: emits `from..till`, then done.
pub fn ints(from: i32, till: i32) -> Producer<i32> {
    assert!(from <= till, "rpl::ints: from > till");
    Producer::new(move |consumer: Consumer<i32, NoError>| {
        for i in from..till {
            consumer.put_next(i);
        }
        consumer.put_done();
        Lifetime::new()
    })
}

/// `rpl::ints(count)`: emits `0..count`, then done.
pub fn ints_count(count: i32) -> Producer<i32> {
    ints(0, count)
}
