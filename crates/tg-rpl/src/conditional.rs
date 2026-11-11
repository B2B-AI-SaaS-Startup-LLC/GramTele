//! Port of `rpl/conditional.h`.

use crate::combine::combine;
use crate::producer::Producer;

/// `rpl::conditional(test, a, b)`: the latest value of `a` while the latest
/// `test` is `true`, of `b` otherwise. Implemented with `combine`, as in
/// C++, so all three must produce a value before anything is emitted.
pub fn conditional<V, E>(
    test: Producer<bool, E>,
    a: Producer<V, E>,
    b: Producer<V, E>,
) -> Producer<V, E>
where
    V: Clone + 'static,
    E: 'static,
{
    combine((test, a, b)).map(|(test, a, b)| if test { a } else { b })
}
