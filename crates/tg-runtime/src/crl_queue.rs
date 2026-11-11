//! `crl::queue` (`crl_common_queue.*`): a serial queue on the pool.
//!
//! Tasks run one after another in FIFO order, never concurrently, but not
//! necessarily on the same thread. Each wake-up processes the whole backlog
//! as a batch on one pool thread.

use crate::crl_executor::Executor;
use crate::crl_list::{List, Push};
use crate::util::{post_and_wait, Task};
use std::cell::Cell;
use std::panic::resume_unwind;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

thread_local! {
    static CURRENT_QUEUE: Cell<u64> = const { Cell::new(0) };
}

struct Shared {
    id: u64,
    list: List,
    executor: Arc<Executor>,
}

impl Shared {
    fn wake(self: &Arc<Self>) {
        let me = Arc::clone(self);
        if !self.executor.spawn(move || me.process()) {
            // The pool is gone: nothing will ever run these tasks.
            self.list.stop();
        }
    }

    fn process(self: &Arc<Self>) {
        struct Restore(u64);
        impl Drop for Restore {
            fn drop(&mut self) {
                CURRENT_QUEUE.with(|c| c.set(self.0));
            }
        }
        let _restore = Restore(CURRENT_QUEUE.with(|c| c.replace(self.id)));
        let batch = self.list.run_batch();
        if batch.rewake {
            self.wake();
        }
        // A panic was already reported by the panic hook; the queue survives.
        drop(batch.panic);
    }
}

pub struct Queue {
    shared: Arc<Shared>,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        Self::with_executor(Arc::clone(Executor::global()))
    }

    pub fn with_executor(executor: Arc<Executor>) -> Self {
        static IDS: AtomicU64 = AtomicU64::new(1);
        Queue {
            shared: Arc::new(Shared {
                id: IDS.fetch_add(1, Ordering::Relaxed),
                list: List::new(),
                executor,
            }),
        }
    }

    /// `queue.async(f)`.
    pub fn async_<F: FnOnce() + Send + 'static>(&self, f: F) {
        self.post(Box::new(f));
    }

    pub(crate) fn post(&self, task: Task) {
        if let Push::Wake = self.shared.list.push(task) {
            self.shared.wake();
        }
    }

    /// `queue.sync(f)`: runs `f` on the queue after everything queued before
    /// and waits for the result. `None` if the executor was shut down.
    ///
    /// Panics instead of deadlocking when called from a task of this queue.
    pub fn sync<R: Send + 'static, F: FnOnce() -> R + Send + 'static>(&self, f: F) -> Option<R> {
        if CURRENT_QUEUE.with(|c| c.get()) == self.shared.id {
            resume_unwind(Box::new("Queue::sync called from the same queue"));
        }
        post_and_wait(|task| self.post(task), f)
    }
}
