//! Port of `rpl/deferred.h`.

use crate::consumer::Consumer;
use crate::producer::Producer;

/// `rpl::deferred(creator)`: calls `creator` on every start and starts the
/// producer it returns with the same consumer.
pub fn deferred<V, E, F>(creator: F) -> Producer<V, E>
where
    V: 'static,
    E: 'static,
    F: FnOnce() -> Producer<V, E> + Clone + 'static,
{
    Producer::new(move |consumer: Consumer<V, E>| creator().start_existing_returning(&consumer))
}
