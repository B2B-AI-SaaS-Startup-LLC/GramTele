//! Small internal helpers shared by the modules.

use std::any::Any;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// A type-erased unit of work that can be moved between threads.
pub type Task = Box<dyn FnOnce() + Send + 'static>;

/// Payload of a caught panic.
pub(crate) type Panic = Box<dyn Any + Send + 'static>;

/// Locks a mutex ignoring poisoning: a panicking task must not wedge a queue.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Posts `f` through `post` and waits for its result.
///
/// Returns `None` if the task was dropped without running (the target was shut
/// down). A panic inside `f` is re-raised in the caller.
pub(crate) fn post_and_wait<R, P, F>(post: P, f: F) -> Option<R>
where
    R: Send + 'static,
    P: FnOnce(Task),
    F: FnOnce() -> R + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    post(Box::new(move || {
        let result = catch_unwind(AssertUnwindSafe(f));
        // The receiver is gone only if the waiting thread died; nothing to do.
        let _ = sender.send(result);
    }));
    match receiver.recv() {
        Ok(Ok(value)) => Some(value),
        Ok(Err(payload)) => resume_unwind(payload),
        Err(_) => None,
    }
}
