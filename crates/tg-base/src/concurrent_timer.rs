//! Port of `base/concurrent_timer.h` / `concurrent_timer.cpp`:
//! `base::ConcurrentTimerEnvironment` and `base::ConcurrentTimer`.
//!
//! A `ConcurrentTimer` belongs to an object living on a background queue of
//! `tg-runtime` (`crl::object_on_queue`, `weak_on_queue`). It cannot use the
//! main thread timers, so the original keeps one extra thread with a Qt event
//! loop (`ConcurrentTimerEnvironment`); the timer sends it "call this method
//! in N ms" events, and the method posts the real work back to the owner's
//! queue through a `runner`.
//!
//! The port keeps the structure and replaces the Qt event loop by a small
//! schedule ([`Core`]) that is run either by the environment thread
//! ([`ConcurrentTimerEnvironment::new`]) or by hand with virtual time
//! ([`ConcurrentTimerEnvironment::manual`], used by the tests, no sleeping).
//!
//! Defects of the original that are kept on purpose (see tests):
//!
//! * `call_each` fires **once**: the environment thread kills its Qt timer
//!   before calling the method, and `timerEvent` of an interval timer only
//!   updates `_next`, it never re-arms. Clients (`crl` streaming, storage
//!   cache, round video recorder) only use `call_once`;
//! * `cancel()` assigns an empty guard before asking `is_active()`, so the
//!   "cancel" message to the environment thread is never sent. The pending
//!   method is still harmless: its guard is dead and it returns at once.
//!
//! Differences: the callback is `FnMut() + Send` and runs on the runner's
//! context; the Qt timer type is accepted but not used; `ConcurrentTimer`
//! itself is `Send` (it is stored inside `ObjectOnQueue`).

use crate::binary_guard::BinaryGuard;
use crate::timer::{Clock, SystemClock, TimerType};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;
use tg_runtime::{Policy, Time, WeakAsync};

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// `Fn<void(FnMut<void()>)> runner`: runs a closure in the context of the
/// timer owner (a serial queue, a thread).
pub type Runner = Arc<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;

/// `Qt::TimerType` choice of `ConcurrentTimer::DefaultType`: coarse above 1 s.
pub fn default_type(timeout: Time) -> TimerType {
    const THRESHOLD: Time = 1000;
    if timeout > THRESHOLD {
        TimerType::Coarse
    } else {
        TimerType::Precise
    }
}

type Method = Box<dyn FnOnce() + Send>;
type AdjustFn = Arc<dyn Fn() + Send + Sync>;

struct Pending {
    deadline: Time,
    seq: u64,
    method: Method,
}

struct Object {
    adjust: AdjustFn,
    pending: Option<Pending>,
}

#[derive(Default)]
struct CoreState {
    objects: HashMap<u64, Object>,
    /// `(deadline, seq, object id)`.
    queue: BTreeSet<(Time, u64, u64)>,
    next_id: u64,
    next_seq: u64,
    adjust_requested: bool,
    stop: bool,
}

/// The schedule of the environment: one pending method per `TimerObject`.
struct Core {
    clock: Arc<dyn Clock>,
    state: Mutex<CoreState>,
    condvar: Condvar,
}

impl Core {
    fn new(clock: Arc<dyn Clock>) -> Arc<Self> {
        Arc::new(Core {
            clock,
            state: Mutex::new(CoreState::default()),
            condvar: Condvar::new(),
        })
    }

    /// `ConcurrentTimerEnvironment::createTimer`.
    fn create_object(&self, adjust: AdjustFn) -> u64 {
        let mut state = lock(&self.state);
        state.next_id += 1;
        let id = state.next_id;
        state.objects.insert(
            id,
            Object {
                adjust,
                pending: None,
            },
        );
        id
    }

    /// `~TimerObjectWrap`: the object and its pending method disappear.
    fn destroy_object(&self, id: u64) {
        let removed = {
            let mut state = lock(&self.state);
            let removed = state.objects.remove(&id);
            if let Some(Pending { deadline, seq, .. }) =
                removed.as_ref().and_then(|object| object.pending.as_ref())
            {
                state.queue.remove(&(*deadline, *seq, id));
            }
            removed
        };
        // Dropped outside the lock: the method owns a guard and a runner.
        drop(removed);
    }

    /// `TimerObject::callDelayed`: cancels the previous method, schedules the
    /// new one.
    fn call(&self, id: u64, timeout: Time, method: Method) {
        crate::expects!(timeout >= 0 && timeout < Time::from(i32::MAX));
        let deadline = self.clock.now() + timeout;
        let previous = {
            let mut state = lock(&self.state);
            let seq = state.next_seq;
            state.next_seq += 1;
            let Some(object) = state.objects.get_mut(&id) else {
                drop(state);
                drop(method);
                return;
            };
            let previous = object.pending.replace(Pending {
                deadline,
                seq,
                method,
            });
            if let Some(old) = previous.as_ref() {
                state.queue.remove(&(old.deadline, old.seq, id));
            }
            state.queue.insert((deadline, seq, id));
            previous
        };
        drop(previous);
        self.condvar.notify_all();
    }

    fn request_adjust(&self) {
        lock(&self.state).adjust_requested = true;
        self.condvar.notify_all();
    }

    /// Runs what is due now on the calling thread: methods in deadline order
    /// (`TimerObject::callNow`), then the adjust notifications.
    fn run_due(&self) -> usize {
        let mut ran = 0;
        loop {
            let method = {
                let mut state = lock(&self.state);
                let now = self.clock.now();
                let Some(&(deadline, seq, id)) = state.queue.iter().next() else {
                    break;
                };
                if deadline > now {
                    break;
                }
                state.queue.remove(&(deadline, seq, id));
                state
                    .objects
                    .get_mut(&id)
                    .and_then(|object| object.pending.take())
                    .map(|pending| pending.method)
            };
            if let Some(method) = method {
                ran += 1;
                method();
            }
        }
        let listeners: Vec<AdjustFn> = {
            let mut state = lock(&self.state);
            if !std::mem::take(&mut state.adjust_requested) {
                return ran;
            }
            state
                .objects
                .values()
                .map(|object| Arc::clone(&object.adjust))
                .collect()
        };
        for adjust in listeners {
            adjust();
        }
        ran
    }

    fn run_thread(&self) {
        let mut state = lock(&self.state);
        loop {
            if state.stop {
                return;
            }
            let wait = state
                .queue
                .iter()
                .next()
                .map(|entry| entry.0 - self.clock.now());
            match wait {
                _ if state.adjust_requested => {}
                Some(remaining) if remaining > 0 => {
                    state = self
                        .condvar
                        .wait_timeout(state, Duration::from_millis(remaining as u64))
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                    continue;
                }
                None => {
                    state = self.condvar.wait(state).unwrap_or_else(|e| e.into_inner());
                    continue;
                }
                Some(_) => {}
            }
            drop(state);
            self.run_due();
            state = lock(&self.state);
        }
    }
}

fn environment() -> &'static Mutex<Option<Arc<Core>>> {
    static ENVIRONMENT: Mutex<Option<Arc<Core>>> = Mutex::new(None);
    &ENVIRONMENT
}

/// `base::ConcurrentTimerEnvironment`: the thread that wakes concurrent
/// timers. One global instance, created by the launcher before the first
/// `ConcurrentTimer` (`core/launcher.cpp`) and destroyed last.
pub struct ConcurrentTimerEnvironment {
    core: Arc<Core>,
    thread: Option<std::thread::JoinHandle<()>>,
    global: bool,
}

impl ConcurrentTimerEnvironment {
    /// The global environment with its own thread and `crl::now()` time.
    /// Panics if one already exists (`Expects(Environment == nullptr)`).
    pub fn new() -> Self {
        let core = Core::new(Arc::new(SystemClock));
        {
            let mut global = lock(environment());
            crate::expects!(global.is_none());
            *global = Some(Arc::clone(&core));
        }
        let thread = std::thread::Builder::new()
            .name("Concurrent Timer Thread".into())
            .spawn({
                let core = Arc::clone(&core);
                move || core.run_thread()
            })
            .expect("failed to spawn the concurrent timer thread");
        Self {
            core,
            thread: Some(thread),
            global: true,
        }
    }

    /// An environment without a thread and not global: timers created with
    /// [`ConcurrentTimer::in_environment`] fire when [`Self::process`] is
    /// called with the clock moved by hand. For tests.
    pub fn manual(clock: Arc<dyn Clock>) -> Self {
        Self {
            core: Core::new(clock),
            thread: None,
            global: false,
        }
    }

    /// Runs the due methods and pending adjustments on the calling thread.
    /// Only for [`Self::manual`] environments (the others have a thread).
    pub fn process(&self) -> usize {
        self.core.run_due()
    }

    /// `ConcurrentTimerEnvironment::Adjust()`: the system clock jumped; every
    /// concurrent timer re-aims itself on its own context.
    pub fn adjust_all() {
        let core = lock(environment()).clone();
        if let Some(core) = core {
            core.request_adjust();
        }
    }

    /// The same for this environment only.
    pub fn adjust(&self) {
        self.core.request_adjust();
    }
}

impl Default for ConcurrentTimerEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ConcurrentTimerEnvironment {
    fn drop(&mut self) {
        if self.global {
            *lock(environment()) = None;
        }
        lock(&self.core.state).stop = true;
        self.core.condvar.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

// ---------------------------------------------------------------------------
// ConcurrentTimer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Repeat {
    Interval,
    SingleShot,
}

struct Inner {
    next: Time,
    timeout: Time,
    ty: TimerType,
    adjusted: bool,
    repeat: Repeat,
    /// Alive while a scheduled method (or its posted continuation) exists.
    running: BinaryGuard,
}

struct Shared {
    runner: Runner,
    clock: Arc<dyn Clock>,
    callback: Mutex<Option<Box<dyn FnMut() + Send>>>,
    inner: Mutex<Inner>,
    core: Option<(Arc<Core>, u64)>,
}

impl Shared {
    fn is_active(&self) -> bool {
        lock(&self.inner).running.alive()
    }

    fn remaining_time(&self) -> Time {
        let inner = lock(&self.inner);
        if !inner.running.alive() {
            return -1;
        }
        let now = self.clock.now();
        if inner.next > now {
            inner.next - now
        } else {
            0
        }
    }

    fn start(self: &Arc<Self>, timeout: Time, ty: TimerType, repeat: Repeat) {
        crate::expects!(timeout >= 0 && timeout <= Time::from(i32::MAX));

        {
            let mut inner = lock(&self.inner);
            inner.ty = ty;
            inner.repeat = repeat;
            inner.adjusted = false;
            inner.timeout = timeout;
        }
        self.cancel_and_schedule(timeout);
        lock(&self.inner).next = self.clock.now() + timeout;
    }

    /// `ConcurrentTimer::cancelAndSchedule`.
    fn cancel_and_schedule(self: &Arc<Self>, timeout: Time) {
        let guard = lock(&self.inner).running.make_guard();
        let runner = Arc::clone(&self.runner);
        let weak = Arc::downgrade(self);
        // Runs on the environment thread.
        let method: Method = Box::new(move || {
            if !guard.alive() {
                return;
            }
            runner(Box::new(move || {
                if !guard.alive() {
                    return;
                }
                if let Some(shared) = weak.upgrade() {
                    shared.timer_event();
                }
                // `guard` dies here: the timer stops being active.
            }));
        });
        match self.core.as_ref() {
            Some((core, id)) => core.call(*id, timeout, method),
            // No environment: the method is dropped, the guard dies and the
            // timer is inert, as in the original.
            None => drop(method),
        }
    }

    /// `ConcurrentTimer::timerEvent`.
    fn timer_event(self: &Arc<Self>) {
        let (repeat, adjusted, timeout, ty) = {
            let inner = lock(&self.inner);
            (inner.repeat, inner.adjusted, inner.timeout, inner.ty)
        };
        if repeat == Repeat::Interval {
            if adjusted {
                self.start(timeout, ty, repeat);
            } else {
                lock(&self.inner).next = self.clock.now() + timeout;
            }
        } else {
            self.cancel();
        }
        // The callback is taken out while it runs, so it may call
        // `set_callback` on the timer (the new one wins afterwards).
        let taken = lock(&self.callback).take();
        if let Some(mut callback) = taken {
            callback();
            let mut slot = lock(&self.callback);
            if slot.is_none() {
                *slot = Some(callback);
            }
        }
    }

    /// `ConcurrentTimer::cancel`, with the original order of operations: the
    /// guard is reset first, so `is_active()` is already false and the
    /// environment is not told (see the module docs).
    fn cancel(&self) {
        let mut inner = lock(&self.inner);
        inner.running = BinaryGuard::new();
    }

    /// `ConcurrentTimer::adjust`.
    fn adjust(self: &Arc<Self>) {
        let remaining = self.remaining_time();
        if remaining >= 0 {
            self.cancel_and_schedule(remaining);
            lock(&self.inner).adjusted = true;
        }
    }
}

/// `base::ConcurrentTimer`.
pub struct ConcurrentTimer {
    shared: Arc<Shared>,
}

impl ConcurrentTimer {
    /// `ConcurrentTimer(runner, callback)` in the global environment. Without
    /// an environment the timer never fires (as in the original).
    pub fn new(runner: Runner, callback: impl FnMut() + Send + 'static) -> Self {
        Self::create(runner, Some(Box::new(callback)), None)
    }

    /// `ConcurrentTimer(runner)` without a callback.
    pub fn without_callback(runner: Runner) -> Self {
        Self::create(runner, None, None)
    }

    /// `ConcurrentTimer(weak_async, callback)`: the runner is the weak
    /// object's queue.
    pub fn from_weak<P: Policy, T: Send + 'static>(
        weak: &WeakAsync<P, T>,
        callback: impl FnMut() + Send + 'static,
    ) -> Self {
        let queue_runner = weak.runner();
        Self::new(Arc::new(move |method| queue_runner.run(method)), callback)
    }

    /// Like [`Self::new`] in a given (non-global) environment.
    pub fn in_environment(
        environment: &ConcurrentTimerEnvironment,
        runner: Runner,
        callback: Option<Box<dyn FnMut() + Send>>,
    ) -> Self {
        Self::create(runner, callback, Some(Arc::clone(&environment.core)))
    }

    fn create(
        runner: Runner,
        callback: Option<Box<dyn FnMut() + Send>>,
        core: Option<Arc<Core>>,
    ) -> Self {
        let core = core.or_else(|| lock(environment()).clone());
        let clock: Arc<dyn Clock> = match core.as_ref() {
            Some(core) => Arc::clone(&core.clock),
            None => Arc::new(SystemClock),
        };
        let shared = Arc::new_cyclic(|weak: &Weak<Shared>| {
            // `createAdjuster`: runs on the environment thread, hops to the
            // owner's context through the runner, and only if the timer
            // still exists.
            let adjust: AdjustFn = {
                let runner = Arc::clone(&runner);
                let weak = weak.clone();
                Arc::new(move || {
                    let weak = weak.clone();
                    runner(Box::new(move || {
                        if let Some(shared) = weak.upgrade() {
                            shared.adjust();
                        }
                    }));
                })
            };
            let core = core.map(|core| {
                let id = core.create_object(adjust);
                (core, id)
            });
            Shared {
                runner: Arc::clone(&runner),
                clock,
                callback: Mutex::new(callback),
                inner: Mutex::new(Inner {
                    next: 0,
                    timeout: 0,
                    ty: TimerType::Precise,
                    adjusted: false,
                    repeat: Repeat::Interval,
                    running: BinaryGuard::new(),
                }),
                core,
            }
        });
        Self { shared }
    }

    pub fn set_callback(&self, callback: impl FnMut() + Send + 'static) {
        *lock(&self.shared.callback) = Some(Box::new(callback));
    }

    pub fn call_once(&self, timeout: Time) {
        self.call_once_typed(timeout, default_type(timeout));
    }

    pub fn call_each(&self, timeout: Time) {
        self.call_each_typed(timeout, default_type(timeout));
    }

    pub fn call_once_typed(&self, timeout: Time, ty: TimerType) {
        self.shared.start(timeout, ty, Repeat::SingleShot);
    }

    pub fn call_each_typed(&self, timeout: Time, ty: TimerType) {
        self.shared.start(timeout, ty, Repeat::Interval);
    }

    pub fn is_active(&self) -> bool {
        self.shared.is_active()
    }

    pub fn cancel(&self) {
        self.shared.cancel();
    }

    /// `-1` when inactive, otherwise the time to the fire (never negative).
    pub fn remaining_time(&self) -> Time {
        self.shared.remaining_time()
    }
}

impl Drop for ConcurrentTimer {
    fn drop(&mut self) {
        // `_running` dies first (members are destroyed in reverse order), so
        // a method that is already on its way sees a dead guard.
        self.shared.cancel();
        if let Some((core, id)) = self.shared.core.as_ref() {
            core.destroy_object(*id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::ManualClock;
    use std::sync::atomic::AtomicUsize;

    /// A runner that queues closures; the test drains them, like a serial
    /// queue of the owner.
    #[derive(Clone, Default)]
    struct Queue {
        tasks: Arc<Mutex<Vec<Method>>>,
    }

    impl Queue {
        fn runner(&self) -> Runner {
            let tasks = Arc::clone(&self.tasks);
            Arc::new(move |task| lock(&tasks).push(task))
        }

        fn drain(&self) -> usize {
            let mut count = 0;
            loop {
                let tasks = std::mem::take(&mut *lock(&self.tasks));
                if tasks.is_empty() {
                    return count;
                }
                for task in tasks {
                    count += 1;
                    task();
                }
            }
        }
    }

    struct Env {
        clock: ManualClock,
        env: ConcurrentTimerEnvironment,
        queue: Queue,
    }

    impl Env {
        fn new() -> Self {
            let clock = ManualClock::new(10_000);
            Env {
                env: ConcurrentTimerEnvironment::manual(Arc::new(clock.clone())),
                clock,
                queue: Queue::default(),
            }
        }

        fn timer(&self, callback: impl FnMut() + Send + 'static) -> ConcurrentTimer {
            ConcurrentTimer::in_environment(
                &self.env,
                self.queue.runner(),
                Some(Box::new(callback)),
            )
        }

        /// One cycle of the world: the environment thread fires due methods,
        /// the owner queue runs what they posted.
        fn step(&self, to: Time) {
            self.clock.set(to);
            self.env.process();
            self.queue.drain();
        }
    }

    fn counter() -> (Arc<AtomicUsize>, impl FnMut() + Send + 'static) {
        let count = Arc::new(AtomicUsize::new(0));
        let inner = Arc::clone(&count);
        (count, move || {
            inner.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
    }

    #[test]
    fn once_runs_on_the_owner_queue_not_on_the_environment() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        assert!(!timer.is_active());
        assert_eq!(timer.remaining_time(), -1);
        timer.call_once(500);
        assert!(timer.is_active());
        assert_eq!(timer.remaining_time(), 500);

        env.step(10_499);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(timer.remaining_time(), 1);

        env.clock.set(10_500);
        // The environment thread fires the method, which only posts to the
        // owner queue: the callback has not run yet and the timer is active.
        assert_eq!(env.env.process(), 1);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(timer.is_active());
        env.queue.drain();
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!timer.is_active());
        assert_eq!(timer.remaining_time(), -1);
        env.step(20_000);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn restart_replaces_the_pending_method() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        timer.call_once(100);
        env.clock.advance(50);
        timer.call_once(100);
        env.step(10_100);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        env.step(10_150);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn cancel_before_the_deadline_prevents_the_call() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        timer.call_once(100);
        timer.cancel();
        assert!(!timer.is_active());
        assert_eq!(timer.remaining_time(), -1);
        env.step(11_000);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn cancel_after_the_method_fired_but_before_the_owner_ran_it() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        timer.call_once(100);
        env.clock.set(10_100);
        env.env.process();
        // The continuation is already queued on the owner's context.
        timer.cancel();
        env.queue.drain();
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn dropping_the_timer_prevents_the_call() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        timer.call_once(100);
        env.clock.set(10_100);
        env.env.process();
        drop(timer);
        env.queue.drain();
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        // And a drop before the environment fires: nothing is left behind.
        let timer = env.timer(|| {});
        timer.call_once(100);
        drop(timer);
        assert_eq!(env.env.process(), 0);
    }

    #[test]
    fn restart_from_the_callback() {
        let env = Env::new();
        let count = Arc::new(AtomicUsize::new(0));
        let timer: Arc<Mutex<Option<ConcurrentTimer>>> = Arc::new(Mutex::new(None));
        {
            let count = Arc::clone(&count);
            let slot = Arc::clone(&timer);
            let made = env.timer(move || {
                if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 {
                    lock(&slot).as_ref().unwrap().call_once(100);
                }
            });
            *lock(&timer) = Some(made);
        }
        lock(&timer).as_ref().unwrap().call_once(100);
        env.step(10_100);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(lock(&timer).as_ref().unwrap().is_active());
        env.step(10_200);
        env.step(10_300);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 3);
        assert!(!lock(&timer).as_ref().unwrap().is_active());
    }

    #[test]
    fn drop_from_the_callback() {
        let env = Env::new();
        let count = Arc::new(AtomicUsize::new(0));
        let slot: Arc<Mutex<Option<ConcurrentTimer>>> = Arc::new(Mutex::new(None));
        {
            let count = Arc::clone(&count);
            let slot2 = Arc::clone(&slot);
            let made = env.timer(move || {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let dying = lock(&slot2).take();
                drop(dying);
            });
            made.call_once(10);
            *lock(&slot) = Some(made);
        }
        env.step(10_010);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(lock(&slot).is_none());
        env.step(30_000);
    }

    #[test]
    fn call_each_fires_only_once_like_the_original() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        timer.call_each(100);
        for t in 1..=10 {
            env.step(10_000 + 100 * t);
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!timer.is_active());
    }

    #[test]
    fn adjust_reschedules_and_marks_the_timer_adjusted() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        timer.call_once(10_000);
        // The system slept: the clock jumped by 9.5 s.
        env.clock.advance(9_500);
        env.env.adjust();
        // The adjust message goes through the owner's queue.
        assert_eq!(env.env.process(), 0);
        env.queue.drain();
        assert!(timer.shared.inner.lock().unwrap().adjusted);
        assert_eq!(timer.remaining_time(), 500);
        env.step(19_999);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        env.step(20_000);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn adjust_of_an_inactive_timer_does_nothing() {
        let env = Env::new();
        let timer = env.timer(|| {});
        env.env.adjust();
        env.env.process();
        env.queue.drain();
        assert!(!timer.shared.inner.lock().unwrap().adjusted);
        assert!(!timer.is_active());
    }

    #[test]
    fn adjust_after_the_timer_is_gone_is_ignored() {
        let env = Env::new();
        let timer = env.timer(|| {});
        timer.call_once(100);
        env.env.adjust();
        drop(timer);
        env.env.process();
        env.queue.drain();
    }

    #[test]
    fn zero_timeout_fires_on_the_next_environment_pass() {
        let env = Env::new();
        let (count, callback) = counter();
        let timer = env.timer(callback);
        timer.call_once(0);
        assert_eq!(timer.remaining_time(), 0);
        env.step(10_000);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn several_timers_fire_in_deadline_order() {
        let env = Env::new();
        let order = Arc::new(Mutex::new(Vec::new()));
        let make = |name: &'static str, delay: Time| {
            let order = Arc::clone(&order);
            let timer = env.timer(move || lock(&order).push(name));
            timer.call_once(delay);
            timer
        };
        let _a = make("a", 300);
        let _b = make("b", 100);
        let _c = make("c", 200);
        env.step(11_000);
        assert_eq!(*lock(&order), vec!["b", "c", "a"]);
    }

    #[test]
    fn no_environment_means_an_inert_timer() {
        let queue = Queue::default();
        let (count, callback) = counter();
        // No global environment exists in this test binary at this point
        // unless another test made one; use the explicit constructor.
        let timer = ConcurrentTimer::create(queue.runner(), Some(Box::new(callback)), None);
        // `create` falls back to the global one; make sure there is none.
        if timer.shared.core.is_none() {
            timer.call_once(10);
            assert!(!timer.is_active());
            queue.drain();
            assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn runtime_queue_runner_with_virtual_clock() {
        let clock = ManualClock::new(0);
        let env = ConcurrentTimerEnvironment::manual(Arc::new(clock.clone()));
        let queue = Arc::new(tg_runtime::Queue::new());
        let runner_queue = queue.clone();
        let count = Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        let timer = ConcurrentTimer::in_environment(
            &env,
            Arc::new(move |task| runner_queue.async_(task)),
            Some(Box::new(move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            })),
        );
        timer.call_once(5);
        clock.set(4);
        env.process();
        queue.sync(|| ());
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
        clock.set(5);
        env.process();
        queue.sync(|| ());
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
