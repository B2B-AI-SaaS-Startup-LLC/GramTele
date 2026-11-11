//! Port of `rpl/type_erased.h`.
//!
//! Producers are always type-erased in this port, so `type_erased` is the
//! identity. It is kept so that `| rpl::type_erased` translates literally
//! (typically to unify the branches of a conditional expression).

use crate::producer::Producer;

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::type_erased` (identity).
    pub fn type_erased(self) -> Producer<V, E> {
        self
    }
}
