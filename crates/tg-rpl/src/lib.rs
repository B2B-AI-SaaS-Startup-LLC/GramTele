//! `tg-rpl`: port of Telegram Desktop `lib_rpl` (`rpl/rpl.h`), the reactive
//! programming library the whole client is built on.
//!
//! Core types:
//! * [`Producer<V, E>`]: a lazy, restartable, cloneable source
//!   (`rpl::producer`). Defaults: `V = ()` (`rpl::empty_value`),
//!   `E = NoError`.
//! * [`Consumer<V, E>`]: the receiving side of a started producer.
//! * [`Lifetime`]: destroy callbacks; dropping it unsubscribes.
//! * [`EventStream<V, E>`]: a hot source with `fire()`.
//! * [`Variable<T, E>`]: a value plus a stream of its changes.
//!
//! Chains use methods instead of `operator|`:
//! `a.events() | rpl::map(f) | rpl::on_next(g, lifetime)` becomes
//! `a.events().map(f).on_next(g, &lifetime)`. Application-defined
//! operators go through [`Producer::pipe`]. The `README.md` of this crate
//! has the full C++ to Rust correspondence table.
//!
//! # Threads
//!
//! The original is single-threaded: `event_stream.h` says "Currently not
//! thread-safe :(", `rpl::lifetime` is a plain `std::vector` of callbacks,
//! the consumer `_terminated` flag is a plain `bool`. The only atomic part
//! is the reference count of `std::shared_ptr`, which is a property of the
//! C++ standard library, not a guarantee of `lib_rpl`: nothing else may be
//! touched from two threads. In the client every `rpl` object is created,
//! started, fired and destroyed on one thread (the main thread, or the
//! thread of an mtproto / audio object); background threads hand results
//! back with `crl::on_main(...)` and fire on the main thread.
//!
//! This port therefore uses `Rc`, `Cell` and `RefCell`, so every type is
//! `!Send` and `!Sync`. That gives exactly the original guarantee
//! (single-threaded use) and turns its violations, which are data races in
//! C++, into compile errors. `Arc` + `Mutex` would promise a thread safety
//! the original never had and would slow down every value delivery.
//!
//! One consequence is stricter than C++: an object holding a `Lifetime` or
//! an `EventStream` cannot be created on one thread and moved to another
//! (the Qt `moveToThread` pattern); such objects must be created on the
//! thread that uses them.
//!
//! ```compile_fail
//! fn assert_send<T: Send>() {}
//! assert_send::<tg_rpl::Lifetime>();
//! ```
//! ```compile_fail
//! fn assert_send<T: Send>() {}
//! assert_send::<tg_rpl::Producer<i32>>();
//! ```
//! ```compile_fail
//! fn assert_send<T: Send>() {}
//! assert_send::<tg_rpl::EventStream<i32>>();
//! ```
//! ```compile_fail
//! fn assert_sync<T: Sync>() {}
//! assert_sync::<tg_rpl::Variable<i32>>();
//! ```
//!
//! # Reentrancy
//!
//! Handlers may do anything the original allows: fire into the stream that
//! is calling them, subscribe and unsubscribe (including themselves),
//! destroy the lifetime they are registered in, drop the event stream that
//! is firing, assign the variable that is notifying them. No `RefCell`
//! borrow is held while a user callback runs, and a running `next` handler
//! is kept alive by an `Rc` clone even if its consumer is terminated
//! inside. See `event_stream.rs` for the exact rules and
//! `tests/reentrancy_tests.rs` for the covered cases.
//!
//! # Closures
//!
//! * Called per value (`next`, `map`, `filter`, ...): `Fn`, shared through
//!   an `Rc`, so they need not be `Clone`. Mutable state goes into `Cell` /
//!   `RefCell` or `make_state`, as a C++ `mutable` lambda in `rpl` never
//!   keeps its mutations either (the handler is copied before each call).
//! * Called once per start (generators, `deferred` creators): `FnOnce +
//!   Clone`, so that starting moves captured values and duplicating a
//!   producer copies them, as with `std::function`.
//! * Called once per subscription (`error`, `done`): `FnOnce`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod after_next;
pub mod before_next;
pub mod combine;
pub mod combine_previous;
pub mod complete;
pub mod conditional;
pub mod consumer;
pub mod deferred;
pub mod distinct_until_changed;
pub mod event_stream;
pub mod fail;
pub mod filter;
pub mod flatten_latest;
pub mod lifetime;
pub mod map;
pub mod merge;
pub mod never;
pub mod producer;
pub mod range;
pub mod skip;
pub mod take;
pub mod then;
pub mod type_erased;
pub mod variable;

pub use combine::{combine, combine_vec, CombineProducers};
pub use complete::{complete, complete_e};
pub use conditional::conditional;
pub use consumer::{make_consumer, Consumer, EmptyError, EmptyValue, NoError, NoValue, EMPTY};
pub use deferred::deferred;
pub use event_stream::EventStream;
pub use fail::fail;
pub use lifetime::{Lifetime, StateRef};
pub use merge::{merge, MergeProducers};
pub use never::{never, never_e};
pub use producer::{duplicate, make_producer, Producer};
pub use range::{ints, ints_count, range, single, single_e, vector, vector_e};
pub use variable::Variable;
