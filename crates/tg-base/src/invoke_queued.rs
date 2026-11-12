//! Port of `base/invoke_queued.h`: `InvokeQueued` and `SingleQueuedInvokation`.
//!
//! `InvokeQueued(context, lambda)` posts a call to the event loop of the
//! context's thread; the call is dropped if the context died meanwhile. In
//! the port that is the `tg-runtime` main queue plus a guard:
//!
//! | C++ | Rust |
//! |---|---|
//! | `InvokeQueued(this, [=] { ... })` | [`invoke_queued`]`(&self.weak, move \|\| ...)` |
//! | `InvokeQueued(qApp, [=] { ... })` | [`invoke_queued_main`]`(move \|\| ...)` |
//! | from a background thread | [`invoke_queued_send`] (closure and token are `Send`) |
//!
//! Difference: Qt delivers queued calls in the event loop, interleaved with
//! other events; here the call runs in `crl::process_main_queue()`.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use tg_runtime::{on_main_guarded, on_main_local, on_main_local_guarded, AsGuard};

/// `InvokeQueued(context, lambda)` from the main thread.
pub fn invoke_queued<G>(context: &G, lambda: impl FnOnce() + 'static)
where
    G: AsGuard + ?Sized,
    G::Token: 'static,
{
    on_main_local_guarded(context, lambda);
}

/// `InvokeQueued(context, lambda)` from any thread.
pub fn invoke_queued_send<G>(context: &G, lambda: impl FnOnce() + Send + 'static)
where
    G: AsGuard + ?Sized,
    G::Token: Send + 'static,
{
    on_main_guarded(context, lambda);
}

/// `InvokeQueued(qApp, lambda)`: the context never dies.
pub fn invoke_queued_main(lambda: impl FnOnce() + 'static) {
    on_main_local(lambda);
}

struct SendInner {
    pending: AtomicBool,
    callback: Mutex<Box<dyn FnMut() + Send>>,
}

/// `SingleQueuedInvokation`: `call()` any number of times, the callback runs
/// once per batch on the main queue. `call()` is thread-safe. Dropping the
/// object drops a call that has not run yet.
pub struct SingleQueuedInvokation {
    inner: Arc<SendInner>,
}

impl SingleQueuedInvokation {
    pub fn new(callback: impl FnMut() + Send + 'static) -> Self {
        Self {
            inner: Arc::new(SendInner {
                pending: AtomicBool::new(false),
                callback: Mutex::new(Box::new(callback)),
            }),
        }
    }

    pub fn call(&self) {
        if self
            .inner
            .pending
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let weak: Weak<SendInner> = Arc::downgrade(&self.inner);
            tg_runtime::on_main(move || {
                let Some(inner) = weak.upgrade() else { return };
                if inner
                    .pending
                    .compare_exchange(true, false, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    (inner.callback.lock().unwrap_or_else(|e| e.into_inner()))();
                }
            });
        }
    }
}

/// [`SingleQueuedInvokation`] for main-thread callbacks that are not `Send`
/// (they capture `Rc`s). `call()` must be made on the main thread.
pub struct SingleQueuedInvokationLocal {
    pending: Rc<Cell<bool>>,
    callback: Rc<dyn Fn()>,
    alive: Rc<()>,
}

impl SingleQueuedInvokationLocal {
    pub fn new(callback: impl Fn() + 'static) -> Self {
        Self {
            pending: Rc::new(Cell::new(false)),
            callback: Rc::new(callback),
            alive: Rc::new(()),
        }
    }

    pub fn call(&self) {
        if !self.pending.replace(true) {
            let pending = self.pending.clone();
            let callback = self.callback.clone();
            let weak = Rc::downgrade(&self.alive);
            on_main_local(move || {
                if weak.strong_count() > 0 && pending.replace(false) {
                    callback();
                }
            });
        }
    }
}
