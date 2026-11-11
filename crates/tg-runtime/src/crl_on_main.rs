//! The main-thread queue (`crl_common_on_main.*`, `crl_on_main.h`,
//! `dispatch/crl_dispatch_on_main.h`).
//!
//! The queue knows nothing about event loops. The loop gives it a *waker*
//! (called when the queue goes from idle to busy) and calls
//! [`MainQueue::process`] when it wants the backlog drained.
//!
//! Guarantees, all taken from the original common queue:
//!
//! * `on_main` never runs the task inline, even on the main thread;
//! * FIFO order; one `process()` runs exactly the tasks queued before it
//!   started; tasks queued by running tasks wait for the next `process()`;
//! * one waker call per idle-to-busy transition, plus one after a batch if
//!   work arrived during it;
//! * `process()` may be re-entered from a task (nested loops).

use crate::crl_list::{List, Push};
use crate::util::{lock, post_and_wait, Task};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::panic::resume_unwind;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, ThreadId};

/// Called (from any thread) when the queue needs processing.
pub type Waker = Arc<dyn Fn() + Send + Sync + 'static>;
/// `wrap_main_queue`: wraps every `process()` batch (e.g. enter-from-loop).
pub type Wrapper = Arc<dyn Fn(&mut dyn FnMut()) + Send + Sync + 'static>;

type LocalTask = Box<dyn FnOnce() + 'static>;

thread_local! {
    static LOCAL: RefCell<HashMap<u64, VecDeque<LocalTask>>> = RefCell::new(HashMap::new());
}

#[derive(Default)]
struct Hooks {
    waker: Option<Waker>,
    wrapper: Option<Wrapper>,
    main_thread: Option<ThreadId>,
}

struct Shared {
    id: u64,
    list: List,
    hooks: Mutex<Hooks>,
}

#[derive(Clone)]
pub struct MainQueue {
    shared: Arc<Shared>,
}

impl Default for MainQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl MainQueue {
    pub fn new() -> Self {
        static IDS: AtomicU64 = AtomicU64::new(1);
        MainQueue {
            shared: Arc::new(Shared {
                id: IDS.fetch_add(1, Ordering::Relaxed),
                list: List::new(),
                hooks: Mutex::new(Hooks::default()),
            }),
        }
    }

    /// Registers (or removes) the waker. If work is already waiting, the new
    /// waker is called at once.
    pub fn set_waker(&self, waker: Option<Waker>) {
        let call = {
            let mut hooks = lock(&self.shared.hooks);
            hooks.waker = waker.clone();
            waker.filter(|_| self.shared.list.has_pending())
        };
        if let Some(waker) = call {
            waker();
        }
    }

    /// `wrap_main_queue`.
    pub fn set_wrapper(&self, wrapper: Option<Wrapper>) {
        lock(&self.shared.hooks).wrapper = wrapper;
    }

    /// Declares the calling thread to be the main thread.
    pub fn bind_current_thread(&self) {
        lock(&self.shared.hooks).main_thread = Some(thread::current().id());
    }

    pub fn is_main_thread(&self) -> bool {
        lock(&self.shared.hooks).main_thread == Some(thread::current().id())
    }

    fn wake(&self) {
        let waker = lock(&self.shared.hooks).waker.clone();
        if let Some(waker) = waker {
            waker();
        }
    }

    /// `crl::on_main(f)` for any thread. Dropped silently after `shutdown`.
    pub fn post<F: FnOnce() + Send + 'static>(&self, f: F) {
        self.post_task(Box::new(f));
    }

    fn post_task(&self, task: Task) {
        if let Push::Wake = self.shared.list.push(task) {
            self.wake();
        }
    }

    /// Like `post`, but `f` need not be `Send`: only for the main thread,
    /// which is the only one that can run it. Order relative to `post` tasks
    /// is preserved (a `Send` marker occupies the slot in the shared FIFO).
    pub fn post_local<F: FnOnce() + 'static>(&self, f: F) {
        {
            let mut hooks = lock(&self.shared.hooks);
            let me = thread::current().id();
            match hooks.main_thread {
                Some(main) if main != me => {
                    drop(hooks);
                    panic!("post_local must be called on the main thread");
                }
                Some(_) => {}
                None => hooks.main_thread = Some(me),
            }
        }
        if self.shared.list.is_stopped() {
            return;
        }
        let id = self.shared.id;
        LOCAL.with(|l| {
            l.borrow_mut().entry(id).or_default().push_back(Box::new(f));
        });
        self.post_task(Box::new(move || {
            let next = LOCAL.with(|l| l.borrow_mut().get_mut(&id).and_then(|d| d.pop_front()));
            if let Some(task) = next {
                task();
            }
        }));
    }

    /// `crl::on_main_sync(f)`: runs `f` on the main thread, waits for the
    /// result. `None` if the queue was shut down. Panics instead of
    /// deadlocking when called on the main thread itself.
    pub fn post_sync<R: Send + 'static, F: FnOnce() -> R + Send + 'static>(
        &self,
        f: F,
    ) -> Option<R> {
        if self.is_main_thread() {
            resume_unwind(Box::new("on_main_sync called on the main thread"));
        }
        post_and_wait(|task| self.post_task(task), f)
    }

    /// Runs the batch queued so far; returns the number of tasks run.
    /// A panic of a task is re-raised after the batch (other tasks still ran).
    pub fn process(&self) -> usize {
        self.bind_current_thread();
        let wrapper = lock(&self.shared.hooks).wrapper.clone();
        let mut result = None;
        let mut run = || result = Some(self.shared.list.run_batch());
        match wrapper {
            Some(wrapper) => wrapper(&mut run),
            None => run(),
        }
        let Some(batch) = result else { return 0 };
        if batch.rewake {
            self.wake();
        }
        if let Some(payload) = batch.panic {
            resume_unwind(payload);
        }
        batch.ran
    }

    /// Calls `process` until nothing is left (tasks posted by tasks too).
    pub fn process_all(&self) -> usize {
        let mut total = 0;
        while self.shared.list.has_pending() {
            total += self.process();
        }
        total
    }

    /// Destruction of the queue in the original: pending tasks are dropped
    /// unexecuted, later posts are ignored, the waker is released. Local
    /// tasks are dropped only when called on the main thread.
    pub fn shutdown(&self) {
        self.shared.list.stop();
        {
            let mut hooks = lock(&self.shared.hooks);
            hooks.waker = None;
            hooks.wrapper = None;
        }
        let id = self.shared.id;
        let local = LOCAL.with(|l| l.borrow_mut().remove(&id));
        drop(local);
    }

    pub fn is_stopped(&self) -> bool {
        self.shared.list.is_stopped()
    }
}

/// The process-wide main queue used by the free functions.
pub fn main_queue() -> &'static MainQueue {
    static MAIN: OnceLock<MainQueue> = OnceLock::new();
    MAIN.get_or_init(MainQueue::new)
}

/// `crl::init_main_queue(processor)`: installs the waker of the global queue
/// and binds the calling thread. Like the original, a second call is an error.
pub fn init_main_queue(waker: impl Fn() + Send + Sync + 'static) {
    let queue = main_queue();
    assert!(
        lock(&queue.shared.hooks).waker.is_none(),
        "init_main_queue called twice"
    );
    queue.bind_current_thread();
    queue.set_waker(Some(Arc::new(waker)));
}

/// `crl::wrap_main_queue(wrapper)`.
pub fn wrap_main_queue(wrapper: impl Fn(&mut dyn FnMut()) + Send + Sync + 'static) {
    main_queue().set_wrapper(Some(Arc::new(wrapper)));
}

/// Drains the global main queue; to be called by the event loop.
pub fn process_main_queue() -> usize {
    main_queue().process()
}

/// `crl::on_main(f)`.
pub fn on_main<F: FnOnce() + Send + 'static>(f: F) {
    main_queue().post(f);
}

/// `crl::on_main(f)` with a non-`Send` closure, from the main thread.
pub fn on_main_local<F: FnOnce() + 'static>(f: F) {
    main_queue().post_local(f);
}

/// `crl::on_main_sync(f)`.
pub fn on_main_sync<R: Send + 'static, F: FnOnce() -> R + Send + 'static>(f: F) -> Option<R> {
    main_queue().post_sync(f)
}

// ---- crl::on_main_update_requests -------------------------------------

type Callback = Rc<RefCell<dyn FnMut()>>;

thread_local! {
    static UPDATE_REQUESTS: RefCell<Vec<(u64, Callback)>> = const { RefCell::new(Vec::new()) };
    static NEXT_SUBSCRIPTION: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

/// Subscription to update requests; unsubscribes on drop. Main thread only.
#[must_use]
pub struct UpdateRequestsSubscription {
    id: u64,
    _not_send: std::marker::PhantomData<Rc<()>>,
}

impl Drop for UpdateRequestsSubscription {
    fn drop(&mut self) {
        let id = self.id;
        UPDATE_REQUESTS.with(|r| r.borrow_mut().retain(|(i, _)| *i != id));
    }
}

/// `crl::on_main_update_requests()` as a callback subscription. The window
/// layer calls [`fire_main_update_requests`] before painting.
pub fn on_main_update_requests(f: impl FnMut() + 'static) -> UpdateRequestsSubscription {
    let id = NEXT_SUBSCRIPTION.with(|n| n.replace(n.get() + 1));
    UPDATE_REQUESTS.with(|r| r.borrow_mut().push((id, Rc::new(RefCell::new(f)))));
    UpdateRequestsSubscription {
        id,
        _not_send: std::marker::PhantomData,
    }
}

/// Notifies subscribers (the original: `Sandbox::widgetUpdateRequests`).
pub fn fire_main_update_requests() {
    let callbacks: Vec<Callback> =
        UPDATE_REQUESTS.with(|r| r.borrow().iter().map(|(_, c)| c.clone()).collect());
    for callback in callbacks {
        // A callback that fires requests itself is not re-entered.
        if let Ok(mut f) = callback.try_borrow_mut() {
            f();
        }
    }
}
