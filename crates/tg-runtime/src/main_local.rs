//! A `Send` handle to a value that lives on one thread.
//!
//! Not part of `lib_crl`. In C++ a callback given to a cross-thread API (for
//! example the `done` / `fail` handlers of an MTProto request) freely
//! captures main-thread objects. In Rust such a callback has to be `Send`
//! while the state of the main thread (`Rc`, `RefCell`, `tg-rpl` types) is
//! not. [`MainLocal`] keeps the value in a registry of the thread that
//! created it and hands out only an id, so the handle may travel through
//! other threads; the value itself is reachable, and dropped, only on the
//! owner thread.
use crate::crl_on_main::main_queue;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, ThreadId};

thread_local! {
    static VALUES: RefCell<HashMap<u64, Rc<dyn Any>>> = RefCell::new(HashMap::new());
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn remove(id: u64) -> Option<Rc<dyn Any>> {
    // The value is dropped after the registry borrow is released: its `Drop`
    // may create or destroy other `MainLocal`s.
    VALUES
        .try_with(|values| values.borrow_mut().remove(&id))
        .ok()
        .flatten()
}

/// See the module docs.
pub struct MainLocal<T: 'static> {
    id: u64,
    owner: ThreadId,
    _marker: PhantomData<fn() -> T>,
}

impl<T: 'static> MainLocal<T> {
    /// Stores `value` in the registry of the calling thread.
    pub fn new(value: T) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        VALUES.with(|values| {
            values.borrow_mut().insert(id, Rc::new(value));
        });
        Self {
            id,
            owner: thread::current().id(),
            _marker: PhantomData,
        }
    }

    fn assert_owner(&self) {
        assert!(
            thread::current().id() == self.owner,
            "MainLocal accessed from a thread that does not own the value"
        );
    }

    /// Calls `f` with the value. Panics on a thread other than the owner.
    /// Returns `None` if the value was already taken or dropped (the owner
    /// thread has cleared its registry).
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.assert_owner();
        let value = VALUES.with(|values| values.borrow().get(&self.id).cloned())?;
        value.downcast_ref::<T>().map(f)
    }

    /// Takes the value out. Panics on a thread other than the owner. Returns
    /// `None` if a `with` call further up the stack still uses the value.
    pub fn take(self) -> Option<T> {
        self.assert_owner();
        let value = remove(self.id)?;
        Rc::downcast::<T>(value).ok().and_then(Rc::into_inner)
    }
}

impl<T: 'static> Drop for MainLocal<T> {
    fn drop(&mut self) {
        let id = self.id;
        if thread::current().id() == self.owner {
            drop(remove(id));
        } else {
            // Only the owner may run the destructor of the value. The owner
            // of everything this type is used for is the main thread.
            main_queue().post(move || drop(remove(id)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Flag(Rc<Cell<bool>>);
    impl Drop for Flag {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    fn assert_send<T: Send>(_: &T) {}

    #[test]
    fn handle_is_send_for_a_non_send_value() {
        let local = MainLocal::new(Rc::new(5));
        assert_send(&local);
        assert_eq!(local.with(|value| **value), Some(5));
        assert_eq!(local.take().map(|value| *value), Some(5));
    }

    #[test]
    fn drop_on_the_owner_thread_destroys_the_value_at_once() {
        let dropped = Rc::new(Cell::new(false));
        let local = MainLocal::new(Flag(dropped.clone()));
        assert!(!dropped.get());
        drop(local);
        assert!(dropped.get());
    }

    #[test]
    fn value_may_use_other_locals_while_borrowed() {
        let outer = MainLocal::new(1);
        let result = outer.with(|value| {
            let inner = MainLocal::new(*value + 1);
            inner.take()
        });
        assert_eq!(result, Some(Some(2)));
    }

    #[test]
    fn take_during_with_does_not_free_the_borrowed_value() {
        let dropped = Rc::new(Cell::new(false));
        let local = MainLocal::new(Flag(dropped.clone()));
        let id = local.id;
        let seen = local.with(|_| {
            drop(remove(id));
            dropped.get()
        });
        assert_eq!(seen, Some(false));
        assert!(dropped.get());
        assert!(local.with(|_| ()).is_none());
    }

    #[test]
    fn foreign_thread_may_carry_and_drop_but_not_touch() {
        let dropped = Rc::new(Cell::new(false));
        let local = MainLocal::new(Flag(dropped.clone()));
        let touched = thread::spawn(move || {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                local.with(|_| ());
            }));
            // `local` is dropped here, on the foreign thread.
            caught.is_err()
        })
        .join()
        .unwrap();
        assert!(touched);
        // The value itself was not dropped by the foreign thread.
        assert!(!dropped.get());
    }
}
