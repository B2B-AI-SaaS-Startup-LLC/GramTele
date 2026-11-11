//! Port of `rpl/event_stream.h`.
//!
//! An [`EventStream`] is a hot source: [`EventStream::events`] returns a
//! producer that registers its consumer in the stream, and `fire*` pushes a
//! value to every registered consumer.
//!
//! Reentrancy is the delicate part, and the algorithm of the original is
//! reproduced literally (including the exact `std::remove_if` compaction
//! with index-based iterators):
//! * `fire` from inside a handler is allowed (nesting depth is tracked, the
//!   list of consumers is only shrunk by the outermost `fire`);
//! * consumers subscribed during a `fire` do not receive the value being
//!   fired, they receive the following ones;
//! * consumers unsubscribed during a `fire` stop receiving at once (the
//!   consumer is terminated) and are removed by the outermost `fire`;
//! * the stream itself may be dropped (or completed with `fire_done`) from
//!   inside a handler: the running `fire` keeps the consumer list alive and
//!   finishes the iteration, the remaining consumers are already terminated.
//!
//! No `RefCell` borrow is held while a handler runs.
//!
//! Like the original (`// Currently not thread-safe :(`), this type is
//! single-threaded; see the crate documentation.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

use crate::consumer::{Consumer, EmptyValue, NoError};
use crate::lifetime::Lifetime;
use crate::producer::Producer;
use crate::range::{single_e, vector_e};

struct Data<V, E> {
    // `None` plays the role of a moved-from C++ consumer.
    consumers: RefCell<Vec<Option<Consumer<V, E>>>>,
    depth: Cell<i32>,
}

impl<V, E> Data<V, E> {
    fn get(&self, index: usize) -> Option<Consumer<V, E>> {
        self.consumers.borrow()[index].clone()
    }

    fn move_entry(&self, from: usize, to: usize) {
        let mut consumers = self.consumers.borrow_mut();
        let taken = consumers[from].take();
        consumers[to] = taken;
    }
}

struct Core<V, E> {
    data: RefCell<Option<Rc<Data<V, E>>>>,
}

impl<V: 'static, E: 'static> Core<V, E> {
    fn make_weak(&self) -> Weak<Data<V, E>> {
        let mut data = self.data.borrow_mut();
        let data = data.get_or_insert_with(|| {
            Rc::new(Data {
                consumers: RefCell::new(Vec::new()),
                depth: Cell::new(0),
            })
        });
        Rc::downgrade(data)
    }

    fn fire_forward(&self, value: V)
    where
        V: Clone,
    {
        let Some(copy) = self.data.borrow().clone() else {
            return;
        };
        let end = copy.consumers.borrow().len();
        if end == 0 {
            return;
        }

        copy.depth.set(copy.depth.get() + 1);

        // Copy value for every consumer except the last.
        //
        // This is `std::remove_if(begin, prev, pred)` written out the way
        // libstdc++ implements it, over index-based iterators: nested fires
        // may compact the vector under our feet and the result must match
        // the original element for element.
        let keep_copy = |index: usize| -> bool {
            match copy.get(index) {
                Some(consumer) => consumer.put_next_copy(&value),
                None => false,
            }
        };
        let prev = end - 1;
        let mut first = 0;
        while first != prev && keep_copy(first) {
            first += 1;
        }
        let mut stale_from = first;
        if first != prev {
            first += 1;
            while first != prev {
                if keep_copy(first) {
                    copy.move_entry(first, stale_from);
                    stale_from += 1;
                }
                first += 1;
            }
        }

        // Perhaps move value for the last consumer.
        let last_alive = match copy.get(prev) {
            Some(consumer) => consumer.put_next(value),
            None => false,
        };
        if last_alive {
            if stale_from != prev {
                copy.move_entry(prev, stale_from);
            }
            stale_from += 1;
        }

        if stale_from != end {
            // Move new consumers.
            let new_end = copy.consumers.borrow().len();
            if new_end != end {
                assert!(new_end > end, "rpl: event_stream consumers shrank");
                let mut consumers = copy.consumers.borrow_mut();
                for i in end..new_end {
                    consumers[stale_from] = consumers[i].clone();
                    stale_from += 1;
                }
            }

            // Erase stale consumers.
            if copy.depth.get() == 1 {
                let removed: Vec<_> = copy.consumers.borrow_mut().drain(stale_from..).collect();
                drop(removed);
            }
        }
        copy.depth.set(copy.depth.get() - 1);
    }

    fn fire_error_forward(&self, error: E)
    where
        E: Clone,
    {
        let Some(data) = self.data.borrow_mut().take() else {
            return;
        };
        let end = data.consumers.borrow().len();
        if end == 0 {
            return;
        }

        // Copy error for every consumer except the last.
        let prev = end - 1;
        for index in 0..prev {
            if let Some(consumer) = data.get(index) {
                consumer.put_error_copy(&error);
            }
        }

        // Perhaps move error for the last consumer.
        if let Some(consumer) = data.get(prev) {
            consumer.put_error(error);
        }

        // Just drop any new consumers.
    }
}

impl<V, E> Core<V, E> {
    fn fire_done(&self) {
        let data = self.data.borrow_mut().take();
        if let Some(data) = data {
            // The original iterates with a range-for, so consumers added
            // from a `done` handler (only possible through an `events()`
            // producer created earlier) are not visited.
            let end = data.consumers.borrow().len();
            for index in 0..end {
                if let Some(consumer) = data.get(index) {
                    consumer.put_done();
                }
            }
        }
    }

    fn has_consumers(&self) -> bool {
        match &*self.data.borrow() {
            Some(data) => !data.consumers.borrow().is_empty(),
            None => false,
        }
    }
}

/// Port of `rpl::event_stream<Value, Error>`.
pub struct EventStream<V = EmptyValue, E = NoError> {
    core: Rc<Core<V, E>>,
}

impl<V: 'static, E: 'static> EventStream<V, E> {
    /// `event_stream()`.
    pub fn new() -> Self {
        Self {
            core: Rc::new(Core {
                data: RefCell::new(None),
            }),
        }
    }

    /// `fire(Value &&)` / `fire_forward`.
    pub fn fire(&self, value: V)
    where
        V: Clone,
    {
        self.core.fire_forward(value);
    }

    /// `fire_copy(const Value &)`.
    pub fn fire_copy(&self, value: &V)
    where
        V: Clone,
    {
        self.core.fire_forward(value.clone());
    }

    /// `fire_error(Error &&)` / `fire_error_forward`.
    ///
    /// All current consumers get the error; the stream forgets them, and a
    /// later `events()` starts a fresh list.
    pub fn fire_error(&self, error: E)
    where
        E: Clone,
    {
        self.core.fire_error_forward(error);
    }

    /// `fire_error_copy(const Error &)`.
    pub fn fire_error_copy(&self, error: &E)
    where
        E: Clone,
    {
        self.core.fire_error_forward(error.clone());
    }

    /// `fire_done()`: completes all current consumers and forgets them.
    /// Also called on drop.
    pub fn fire_done(&self) {
        self.core.fire_done();
    }

    /// `events()`.
    pub fn events(&self) -> Producer<V, E> {
        let weak = self.core.make_weak();
        Producer::new(move |consumer: Consumer<V, E>| {
            if let Some(strong) = weak.upgrade() {
                let weak = weak.clone();
                let registered = consumer.clone();
                let result = Lifetime::from_fn(move || {
                    if let Some(strong) = weak.upgrade() {
                        let found = strong
                            .consumers
                            .borrow()
                            .iter()
                            .flatten()
                            .find(|candidate| **candidate == registered)
                            .cloned();
                        if let Some(found) = found {
                            found.terminate();
                        }
                    }
                });
                strong.consumers.borrow_mut().push(Some(consumer));
                return result;
            }
            Lifetime::new()
        })
    }

    /// `events_starting_with(Value &&)`.
    pub fn events_starting_with(&self, value: V) -> Producer<V, E>
    where
        V: Clone,
    {
        single_e::<V, E>(value).then(self.events())
    }

    /// `events_starting_with_copy(const Value &)`.
    pub fn events_starting_with_copy(&self, value: &V) -> Producer<V, E>
    where
        V: Clone,
    {
        self.events_starting_with(value.clone())
    }

    /// `has_consumers()`. Counts consumers that unsubscribed but were not
    /// yet removed by a `fire`, as the original does.
    pub fn has_consumers(&self) -> bool {
        self.core.has_consumers()
    }

    fn weak(&self) -> Weak<Core<V, E>> {
        Rc::downgrade(&self.core)
    }
}

impl<V: 'static, E: 'static> Default for EventStream<V, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V, E> Drop for EventStream<V, E> {
    fn drop(&mut self) {
        // `~event_stream() { fire_done(); }`.
        self.core.fire_done();
    }
}

impl<V, E> fmt::Debug for EventStream<V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let consumers = self
            .core
            .data
            .borrow()
            .as_ref()
            .map_or(0, |data| data.consumers.borrow().len());
        f.debug_struct("EventStream")
            .field("consumers", &consumers)
            .finish()
    }
}

impl<V: 'static, E: 'static> Producer<V, E> {
    /// `| rpl::start_to_stream(stream, alive_while)`: forwards values and
    /// errors (not `done`) to `stream`.
    ///
    /// The C++ helper captures the stream by reference; here a weak handle
    /// is captured, so firing into an already dropped stream is a no-op.
    pub fn start_to_stream(self, stream: &EventStream<V, E>, alive_while: &Lifetime)
    where
        V: Clone,
        E: Clone,
    {
        let next = stream.weak();
        let error = stream.weak();
        self.on_next_error(
            move |value| {
                if let Some(core) = next.upgrade() {
                    core.fire_forward(value);
                }
            },
            move |e| {
                if let Some(core) = error.upgrade() {
                    core.fire_error_forward(e);
                }
            },
            alive_while,
        );
    }

    /// `| rpl::start_spawning(alive_while)`: starts the producer now,
    /// keeping it alive in `alive_while`, and returns a producer that
    /// replays the values produced synchronously during the start and then
    /// follows the live values.
    pub fn start_spawning(self, alive_while: &Lifetime) -> Producer<V, E>
    where
        V: Clone,
        E: Clone,
    {
        let stream = alive_while.make_state(EventStream::<V, E>::new()).get();
        let values = Rc::new(RefCell::new(Vec::new()));
        let maybe_error = Rc::new(RefCell::new(None));
        let collecting = {
            let values = values.clone();
            let maybe_error = maybe_error.clone();
            stream.events().start_returning(
                move |value| values.borrow_mut().push(value),
                move |error| *maybe_error.borrow_mut() = Some(error),
                || {},
            )
        };
        self.start_to_stream(&stream, alive_while);
        collecting.destroy();

        if let Some(error) = maybe_error.take() {
            return Producer::new(move |consumer: Consumer<V, E>| {
                consumer.put_error(error);
                Lifetime::new()
            });
        }
        vector_e::<V, E>(values.take()).then(stream.events())
    }
}
