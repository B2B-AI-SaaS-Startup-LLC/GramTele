//! Port of `base/timer.h` / `timer.cpp`: `base::Timer`, `base::DelayedCallTimer`,
//! `base::CheckLocalTime`.
//!
//! In the original a `Timer` is a `QObject` that asks the Qt event loop of
//! its thread for `QTimerEvent`s. There is no event loop in the port yet, so
//! the loop is split into two abstract parts:
//!
//! * [`Clock`]: the time source, `crl::now()` in production ([`SystemClock`]),
//!   [`ManualClock`] in tests;
//! * [`Alarm`]: "wake me at time T". The loop (or a test) answers it by
//!   calling [`TimerHost::process`] once `clock.now() >= T`.
//!
//! A [`TimerHost`] owns the schedule of one thread: it keeps all pending
//! deadlines, tells its [`Alarm`] the earliest one and fires due timers from
//! `process()`. [`Timer`] and [`DelayedCallTimer`] are clients of a host.
//! The thread-local default host ([`TimerHost::current`]) uses the real
//! clock; [`init_main_timers`] wires it to the `tg-runtime` main queue with a
//! background alarm thread ([`ThreadAlarm`]), so timers fire from
//! `crl::process_main_queue()`.
//!
//! Semantics carried over from the original (all covered by tests on virtual
//! time):
//!
//! * `timerEvent` updates the state first and calls the callback last, so the
//!   callback may `cancel`, restart, `set_callback` or drop the timer;
//! * the callback is copied "onstack" before the call;
//! * `remaining_time()` is `-1` for an inactive timer, otherwise
//!   `max(next - now, 0)`;
//! * after a system sleep ([`TimerHost::adjust`], `Timer::Adjust`) every
//!   active interval timer is flagged "adjusted" and restarts a full
//!   interval after its next fire.
//!
//! Differences: callbacks are `Fn()` (the original copies the `std::function`
//! and calls it through a const reference, so mutable state lives in
//! `Cell`/`RefCell`, as in the rest of the port); a repeating timer is
//! re-armed on its own grid (`deadline + k * interval`, missed periods are
//! skipped), as Qt does internally; the public remaining-time estimate is
//! `_next = now + timeout`, exactly as in the original; the Qt timer type (coarse/precise) is accepted
//! and kept, but the host does not coalesce wake-ups.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tg_runtime::Time;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// `Qt::TimerType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TimerType {
    #[default]
    Precise,
    Coarse,
    VeryCoarse,
}

/// `Timer::DefaultType`: coarse above 240 ms.
pub fn default_type(timeout: Time) -> TimerType {
    const THRESHOLD: Time = 240;
    if timeout > THRESHOLD {
        TimerType::Coarse
    } else {
        TimerType::Precise
    }
}

/// The time source of a host, in `crl::now()` milliseconds.
pub trait Clock: Send + Sync {
    fn now(&self) -> Time;
}

/// `crl::now()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Time {
        tg_runtime::now()
    }
}

/// A clock moved by hand: virtual time for tests, no sleeping.
#[derive(Clone, Debug, Default)]
pub struct ManualClock {
    now: Arc<AtomicI64>,
}

impl ManualClock {
    pub fn new(start: Time) -> Self {
        Self {
            now: Arc::new(AtomicI64::new(start)),
        }
    }

    pub fn set(&self, value: Time) {
        self.now.store(value, Ordering::SeqCst);
    }

    pub fn advance(&self, delta: Time) {
        self.now.fetch_add(delta, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Time {
        self.now.load(Ordering::SeqCst)
    }
}

/// "Wake me at `at`" (in the host's clock terms). The request replaces the
/// previous one; `None` means nothing is pending. The owner of the loop calls
/// [`TimerHost::process`] when the moment comes; an `at` in the past means
/// "as soon as possible". The same contract as the waker of the `tg-runtime`
/// main queue, but with a time attached.
pub trait Alarm {
    fn schedule(&self, at: Option<Time>);
}

/// Records the last request. For tests and for loops that poll
/// [`TimerHost::next_deadline`] themselves.
#[derive(Clone, Debug, Default)]
pub struct ManualAlarm {
    requested: Rc<Cell<Option<Time>>>,
    count: Rc<Cell<usize>>,
}

impl ManualAlarm {
    pub fn new() -> Self {
        Self::default()
    }

    /// The last `schedule` argument.
    pub fn requested(&self) -> Option<Time> {
        self.requested.get()
    }

    /// How many times `schedule` was called.
    pub fn calls(&self) -> usize {
        self.count.get()
    }
}

impl Alarm for ManualAlarm {
    fn schedule(&self, at: Option<Time>) {
        self.requested.set(at);
        self.count.set(self.count.get() + 1);
    }
}

/// What a host fires. `Timer` and `DelayedCallTimer` implement it.
trait Target {
    /// The slot `id` due at `deadline` was removed from the host and fires.
    fn fire(self: Rc<Self>, id: u64, deadline: Time);
    /// The system clock jumped (`Timer::Adjust`).
    fn adjusted(self: Rc<Self>) {}
}

struct Slot {
    deadline: Time,
    seq: u64,
    target: Weak<dyn Target>,
}

#[derive(Default)]
struct State {
    slots: HashMap<u64, Slot>,
    /// `(deadline, seq, id)`: fires in deadline order, then start order.
    queue: BTreeSet<(Time, u64, u64)>,
}

/// Cross-thread part of a host: the "clock was adjusted" flag and a way to
/// wake the owner thread's loop.
struct HostSignal {
    adjust: AtomicBool,
    waker: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

fn registry() -> &'static Mutex<Vec<std::sync::Weak<HostSignal>>> {
    static REGISTRY: Mutex<Vec<std::sync::Weak<HostSignal>>> = Mutex::new(Vec::new());
    &REGISTRY
}

/// The timer schedule of one thread. See the module docs.
pub struct TimerHost {
    clock: Arc<dyn Clock>,
    alarm: RefCell<Option<Rc<dyn Alarm>>>,
    state: RefCell<State>,
    armed: Cell<Option<Time>>,
    next_id: Cell<u64>,
    next_seq: Cell<u64>,
    signal: Arc<HostSignal>,
}

thread_local! {
    static CURRENT: Rc<TimerHost> = TimerHost::new(Arc::new(SystemClock));
}

impl TimerHost {
    pub fn new(clock: Arc<dyn Clock>) -> Rc<Self> {
        let signal = Arc::new(HostSignal {
            adjust: AtomicBool::new(false),
            waker: Mutex::new(None),
        });
        {
            let mut hosts = lock(registry());
            hosts.retain(|host| host.strong_count() > 0);
            hosts.push(Arc::downgrade(&signal));
        }
        Rc::new(Self {
            clock,
            alarm: RefCell::new(None),
            state: RefCell::new(State::default()),
            armed: Cell::new(None),
            next_id: Cell::new(1),
            next_seq: Cell::new(0),
            signal,
        })
    }

    /// The default host of the calling thread (real clock, no alarm until
    /// [`TimerHost::set_alarm`] or [`init_main_timers`]).
    pub fn current() -> Rc<TimerHost> {
        CURRENT.with(Rc::clone)
    }

    pub fn now(&self) -> Time {
        self.clock.now()
    }

    /// Installs the alarm and tells it the current earliest deadline.
    pub fn set_alarm(&self, alarm: Option<Rc<dyn Alarm>>) {
        *self.alarm.borrow_mut() = alarm;
        self.rearm(true);
    }

    /// A thread-safe wake-up for the owner loop, used when another thread
    /// calls [`Timer::adjust_all`]. The loop must call `process()` after it.
    pub fn set_adjust_waker(&self, waker: Option<Arc<dyn Fn() + Send + Sync>>) {
        *lock(&self.signal.waker) = waker;
    }

    /// The earliest pending deadline, for loops without an [`Alarm`].
    pub fn next_deadline(&self) -> Option<Time> {
        self.state.borrow().queue.iter().next().map(|entry| entry.0)
    }

    /// Number of pending slots (a repeating timer counts as one).
    pub fn pending(&self) -> usize {
        self.state.borrow().slots.len()
    }

    fn start_slot(&self, deadline: Time, target: Weak<dyn Target>) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let seq = self.next_seq.get();
        self.next_seq.set(seq + 1);
        {
            let mut state = self.state.borrow_mut();
            state.queue.insert((deadline, seq, id));
            state.slots.insert(
                id,
                Slot {
                    deadline,
                    seq,
                    target,
                },
            );
        }
        self.rearm(false);
        id
    }

    fn kill_slot(&self, id: u64) {
        {
            let mut state = self.state.borrow_mut();
            if let Some(slot) = state.slots.remove(&id) {
                state.queue.remove(&(slot.deadline, slot.seq, id));
            }
        }
        self.rearm(false);
    }

    fn rearm(&self, force: bool) {
        let next = self.next_deadline();
        if !force && next == self.armed.get() {
            return;
        }
        self.armed.set(next);
        let alarm = self.alarm.borrow().clone();
        if let Some(alarm) = alarm {
            alarm.schedule(next);
        }
    }

    /// `Timer::adjust` for every timer of this host: the clock jumped
    /// forward, so the alarm is recomputed and interval timers restart a full
    /// interval after the next fire. Due timers are *not* fired here.
    pub fn adjust(&self) {
        let targets: Vec<_> = self
            .state
            .borrow()
            .slots
            .values()
            .filter_map(|slot| slot.target.upgrade())
            .collect();
        for target in targets {
            target.adjusted();
        }
        self.rearm(true);
    }

    /// Fires every timer that is due *now* (each at most once per call; timers
    /// started or re-armed by callbacks wait for the next call, like the next
    /// loop iteration in Qt). Returns the number of fired timers.
    pub fn process(&self) -> usize {
        if self.signal.adjust.swap(false, Ordering::SeqCst) {
            self.adjust();
        }
        let now = self.clock.now();
        let due: Vec<(Time, u64, u64)> = self
            .state
            .borrow()
            .queue
            .range(..=(now, u64::MAX, u64::MAX))
            .copied()
            .collect();
        let mut fired = 0;
        for (deadline, seq, id) in due {
            let target = {
                let mut state = self.state.borrow_mut();
                match state.slots.get(&id) {
                    // Killed or restarted by an earlier callback: skip.
                    Some(slot) if slot.seq == seq => {
                        let slot = state.slots.remove(&id).expect("checked above");
                        state.queue.remove(&(deadline, seq, id));
                        slot.target.upgrade()
                    }
                    _ => continue,
                }
            };
            if let Some(target) = target {
                fired += 1;
                target.fire(id, deadline);
            }
        }
        self.rearm(false);
        fired
    }
}

/// Raises the "clock was adjusted" flag of all hosts of the process and wakes
/// their loops (`Timer::Adjust`). Each host reacts in its next `process()`.
fn adjust_all_hosts() {
    let hosts: Vec<_> = {
        let mut hosts = lock(registry());
        hosts.retain(|host| host.strong_count() > 0);
        hosts.iter().filter_map(std::sync::Weak::upgrade).collect()
    };
    for host in hosts {
        host.adjust.store(true, Ordering::SeqCst);
        let waker = lock(&host.waker).clone();
        if let Some(waker) = waker {
            waker();
        }
    }
}

/// `base::CheckLocalTime`: if the system slept, moves `crl::now()` forward
/// and re-aims every timer. Returns `true` if the clock was adjusted; the
/// caller must then also invalidate the HTTP time of `unixtime`
/// (`base::unixtime::http_invalidate()`, not part of this module) and call
/// `ConcurrentTimerEnvironment::adjust_all()` for background timers (done
/// here too).
pub fn check_local_time() -> bool {
    check_local_time_with(tg_runtime::adjust_time)
}

/// Injects only the runtime's adjustment decision for deterministic tests.
/// Production uses `tg_runtime::adjust_time`; no second time adjuster exists.
pub fn check_local_time_with(adjust_time: impl FnOnce() -> bool) -> bool {
    let adjusted = adjust_time();
    if adjusted {
        Timer::adjust_all();
        crate::concurrent_timer::ConcurrentTimerEnvironment::adjust_all();
        crate::unixtime::http_invalidate();
    }
    adjusted
}

// ---------------------------------------------------------------------------
// Timer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Repeat {
    Interval,
    SingleShot,
}

type Callback = Rc<dyn Fn()>;

struct TimerShared {
    host: Rc<TimerHost>,
    callback: RefCell<Option<Callback>>,
    next: Cell<Time>,
    timeout: Cell<Time>,
    id: Cell<u64>,
    ty: Cell<TimerType>,
    adjusted: Cell<bool>,
    repeat: Cell<Repeat>,
}

impl TimerShared {
    fn is_active(&self) -> bool {
        self.id.get() != 0
    }

    fn cancel(&self) {
        let id = self.id.replace(0);
        if id != 0 {
            self.host.kill_slot(id);
        }
    }

    fn start(self: &Rc<Self>, timeout: Time, ty: TimerType, repeat: Repeat) {
        crate::expects!(timeout >= 0 && timeout <= Time::from(i32::MAX));

        self.cancel();
        self.ty.set(ty);
        self.repeat.set(repeat);
        self.adjusted.set(false);
        self.timeout.set(timeout);
        let next = self.host.now() + timeout;
        self.arm(next);
    }

    fn arm(self: &Rc<Self>, deadline: Time) {
        let target: Rc<dyn Target> = self.clone();
        self.id
            .set(self.host.start_slot(deadline, Rc::downgrade(&target)));
        self.next.set(deadline);
    }
}

impl Target for TimerShared {
    /// `Timer::timerEvent`.
    fn fire(self: Rc<Self>, _id: u64, deadline: Time) {
        self.id.set(0);
        match self.repeat.get() {
            Repeat::Interval => {
                if self.adjusted.get() {
                    self.start(self.timeout.get(), self.ty.get(), Repeat::Interval);
                } else {
                    let interval = self.timeout.get();
                    let now = self.host.now();
                    let next = if interval <= 0 {
                        now
                    } else {
                        deadline + interval * (1 + (now - deadline).max(0) / interval)
                    };
                    self.arm(next);
                    // Qt keeps its scheduling grid, but Timer::_next is an
                    // estimate measured from delivery, not the Qt deadline.
                    self.next.set(now + interval);
                }
            }
            Repeat::SingleShot => {}
        }
        // `if (const auto onstack = _callback) onstack();`
        let onstack = self.callback.borrow().clone();
        if let Some(callback) = onstack {
            callback();
        }
    }

    /// `Timer::adjust`.
    fn adjusted(self: Rc<Self>) {
        if self.is_active() {
            let remaining = (self.next.get() - self.host.now()).max(0);
            self.cancel();
            self.arm(self.host.now() + remaining);
            self.adjusted.set(true);
        }
    }
}

/// `base::Timer`. Lives on the thread of its host and is not `Send`.
///
/// Dropping the timer cancels it, including from its own callback.
pub struct Timer {
    shared: Rc<TimerShared>,
}

impl Default for Timer {
    fn default() -> Self {
        Self::in_host(&TimerHost::current(), None)
    }
}

impl Timer {
    /// `Timer(callback)` on the default host of this thread.
    pub fn new(callback: impl Fn() + 'static) -> Self {
        Self::in_host(&TimerHost::current(), Some(Rc::new(callback)))
    }

    /// `Timer(not_null<QThread*>, callback)`: the thread is the one that owns
    /// `host` (the port has no `moveToThread`: create the timer on its thread).
    pub fn in_host(host: &Rc<TimerHost>, callback: Option<Rc<dyn Fn()>>) -> Self {
        Self {
            shared: Rc::new(TimerShared {
                host: Rc::clone(host),
                callback: RefCell::new(callback),
                next: Cell::new(0),
                timeout: Cell::new(0),
                id: Cell::new(0),
                ty: Cell::new(TimerType::Precise),
                adjusted: Cell::new(false),
                repeat: Cell::new(Repeat::Interval),
            }),
        }
    }

    pub fn set_callback(&self, callback: impl Fn() + 'static) {
        *self.shared.callback.borrow_mut() = Some(Rc::new(callback));
    }

    /// `setCallback(nullptr)`.
    pub fn clear_callback(&self) {
        *self.shared.callback.borrow_mut() = None;
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

    /// `-1` when inactive, otherwise the time to the next fire (never
    /// negative).
    pub fn remaining_time(&self) -> Time {
        if !self.is_active() {
            return -1;
        }
        let now = self.shared.host.now();
        let next = self.shared.next.get();
        if next > now {
            next - now
        } else {
            0
        }
    }

    /// `Timer::Adjust()`: the system clock jumped, re-aim the timers of every
    /// host in the process (each one in its own thread's next `process()`).
    pub fn adjust_all() {
        adjust_all_hosts();
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.shared.cancel();
    }
}

// ---------------------------------------------------------------------------
// DelayedCallTimer
// ---------------------------------------------------------------------------

/// Identifier of a delayed call; `0` is "none".
pub type CallId = u64;

struct DelayedShared {
    host: Rc<TimerHost>,
    callbacks: RefCell<HashMap<u64, Box<dyn FnOnce()>>>,
}

impl Target for DelayedShared {
    /// `DelayedCallTimer::timerEvent`.
    fn fire(self: Rc<Self>, id: u64, _deadline: Time) {
        let callback = self.callbacks.borrow_mut().remove(&id);
        if let Some(callback) = callback {
            callback();
        }
    }
}

/// `base::DelayedCallTimer`: many one-shot calls, individually cancellable.
/// Dropping it cancels and drops all pending calls.
pub struct DelayedCallTimer {
    shared: Rc<DelayedShared>,
}

impl Default for DelayedCallTimer {
    fn default() -> Self {
        Self::in_host(&TimerHost::current())
    }
}

impl DelayedCallTimer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn in_host(host: &Rc<TimerHost>) -> Self {
        Self {
            shared: Rc::new(DelayedShared {
                host: Rc::clone(host),
                callbacks: RefCell::new(HashMap::new()),
            }),
        }
    }

    pub fn call(&self, timeout: Time, callback: impl FnOnce() + 'static) -> CallId {
        self.call_typed(timeout, callback, default_type(timeout))
    }

    /// Returns the id to pass to [`DelayedCallTimer::cancel`].
    pub fn call_typed(
        &self,
        timeout: Time,
        callback: impl FnOnce() + 'static,
        _ty: TimerType,
    ) -> CallId {
        crate::expects!(timeout >= 0);

        let target: Rc<dyn Target> = self.shared.clone();
        let id = self
            .shared
            .host
            .start_slot(self.shared.host.now() + timeout, Rc::downgrade(&target));
        self.shared
            .callbacks
            .borrow_mut()
            .insert(id, Box::new(callback));
        id
    }

    /// `cancel(0)` and cancelling a call that already ran do nothing.
    pub fn cancel(&self, call_id: CallId) {
        if call_id != 0 {
            self.shared.host.kill_slot(call_id);
            // Dropped after the borrow is released: the callback's captures
            // may run arbitrary destructors.
            let removed = self.shared.callbacks.borrow_mut().remove(&call_id);
            drop(removed);
        }
    }
}

impl Drop for DelayedCallTimer {
    fn drop(&mut self) {
        let callbacks = std::mem::take(&mut *self.shared.callbacks.borrow_mut());
        for id in callbacks.keys() {
            self.shared.host.kill_slot(*id);
        }
    }
}

// ---------------------------------------------------------------------------
// The real alarm and the main-queue wiring
// ---------------------------------------------------------------------------

struct AlarmState {
    target: Option<Time>,
    stop: bool,
}

struct AlarmShared {
    state: Mutex<AlarmState>,
    condvar: Condvar,
}

/// An [`Alarm`] backed by a helper thread: sleeps until the requested time of
/// its clock, then calls `wake` (which must be thread-safe and cheap, for
/// example `crl::on_main(...)`). One thread per alarm, joined on drop.
pub struct ThreadAlarm {
    shared: Arc<AlarmShared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ThreadAlarm {
    pub fn new(clock: Arc<dyn Clock>, wake: impl Fn() + Send + 'static) -> Self {
        let shared = Arc::new(AlarmShared {
            state: Mutex::new(AlarmState {
                target: None,
                stop: false,
            }),
            condvar: Condvar::new(),
        });
        let thread = std::thread::Builder::new()
            .name("tg-timer-alarm".into())
            .spawn({
                let shared = Arc::clone(&shared);
                move || Self::run(&shared, clock.as_ref(), &wake)
            })
            .expect("failed to spawn the alarm thread");
        Self {
            shared,
            thread: Some(thread),
        }
    }

    fn run(shared: &AlarmShared, clock: &dyn Clock, wake: &dyn Fn()) {
        let mut state = lock(&shared.state);
        loop {
            if state.stop {
                return;
            }
            match state.target {
                None => {
                    state = shared
                        .condvar
                        .wait(state)
                        .unwrap_or_else(|e| e.into_inner());
                }
                Some(target) => {
                    let remaining = target - clock.now();
                    if remaining <= 0 {
                        state.target = None;
                        drop(state);
                        wake();
                        state = lock(&shared.state);
                    } else {
                        state = shared
                            .condvar
                            .wait_timeout(state, Duration::from_millis(remaining as u64))
                            .unwrap_or_else(|e| e.into_inner())
                            .0;
                    }
                }
            }
        }
    }
}

impl Alarm for ThreadAlarm {
    fn schedule(&self, at: Option<Time>) {
        lock(&self.shared.state).target = at;
        self.shared.condvar.notify_all();
    }
}

impl Drop for ThreadAlarm {
    fn drop(&mut self) {
        lock(&self.shared.state).stop = true;
        self.shared.condvar.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Fires the due timers of the current thread's default host. This is what
/// the main-queue wake-up runs.
pub fn process_timers() -> usize {
    TimerHost::current().process()
}

/// Wires the default host of the calling (main) thread to the `tg-runtime`
/// main queue: a [`ThreadAlarm`] posts [`process_timers`] with `crl::on_main`
/// at the right moment, and `Timer::adjust_all()` from any thread does the
/// same. Call once on the main thread, after `crl::init_main_queue`.
pub fn init_main_timers() {
    let host = TimerHost::current();
    host.set_alarm(Some(Rc::new(ThreadAlarm::new(
        Arc::new(SystemClock),
        || {
            tg_runtime::on_main(|| {
                process_timers();
            })
        },
    ))));
    host.set_adjust_waker(Some(Arc::new(|| {
        tg_runtime::on_main(|| {
            process_timers();
        })
    })));
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Env {
        clock: ManualClock,
        alarm: ManualAlarm,
        host: Rc<TimerHost>,
    }

    impl Env {
        fn new() -> Self {
            let clock = ManualClock::new(1000);
            let alarm = ManualAlarm::new();
            let host = TimerHost::new(Arc::new(clock.clone()));
            host.set_alarm(Some(Rc::new(alarm.clone())));
            Env { clock, alarm, host }
        }

        fn timer(&self, callback: impl Fn() + 'static) -> Timer {
            Timer::in_host(&self.host, Some(Rc::new(callback)))
        }

        /// Moves virtual time forward step by step to the target moment,
        /// processing at every requested wake-up, like a real loop would.
        fn run_until(&self, target: Time) {
            while let Some(at) = self.host.next_deadline() {
                if at > target {
                    break;
                }
                self.clock.set(at.max(self.clock.now()));
                self.host.process();
            }
            self.clock.set(target.max(self.clock.now()));
        }
    }

    fn log() -> (Rc<RefCell<Vec<String>>>, impl Fn(&str) + Clone) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let push = {
            let log = log.clone();
            move |text: &str| log.borrow_mut().push(text.to_owned())
        };
        (log, push)
    }

    #[test]
    fn default_type_threshold() {
        assert_eq!(default_type(240), TimerType::Precise);
        assert_eq!(default_type(241), TimerType::Coarse);
    }

    #[test]
    fn once_fires_once_at_the_deadline() {
        let env = Env::new();
        let (log, push) = log();
        let timer = env.timer(move || push("fire"));
        assert!(!timer.is_active());
        timer.call_once(100);
        assert!(timer.is_active());
        assert_eq!(env.alarm.requested(), Some(1100));

        env.clock.set(1099);
        assert_eq!(env.host.process(), 0);
        env.clock.set(1100);
        assert_eq!(env.host.process(), 1);
        assert!(!timer.is_active());
        assert_eq!(env.alarm.requested(), None);
        env.run_until(5000);
        assert_eq!(*log.borrow(), vec!["fire"]);
    }

    #[test]
    fn remaining_time_semantics() {
        let env = Env::new();
        let timer = env.timer(|| {});
        assert_eq!(timer.remaining_time(), -1);
        timer.call_once(500);
        assert_eq!(timer.remaining_time(), 500);
        env.clock.advance(120);
        assert_eq!(timer.remaining_time(), 380);
        // Overdue but not yet processed: zero, not negative.
        env.clock.advance(1000);
        assert_eq!(timer.remaining_time(), 0);
        timer.cancel();
        assert_eq!(timer.remaining_time(), -1);
    }

    #[test]
    fn each_repeats_on_its_grid_and_skips_missed_periods() {
        let env = Env::new();
        let (log, push) = log();
        let clock = env.clock.clone();
        let timer = env.timer(move || push(&format!("t={}", clock.now())));
        timer.call_each(100);
        env.run_until(1350);
        assert_eq!(*log.borrow(), vec!["t=1100", "t=1200", "t=1300"]);
        assert!(timer.is_active());
        assert_eq!(timer.remaining_time(), 50);

        // The loop was blocked for a long time: one fire, no catch-up burst,
        // and the next deadline is back on the grid.
        log.borrow_mut().clear();
        env.clock.set(1720);
        assert_eq!(env.host.process(), 1);
        assert_eq!(*log.borrow(), vec!["t=1720"]);
        assert_eq!(timer.remaining_time(), 100);
        assert_eq!(env.host.next_deadline(), Some(1800));
    }

    #[test]
    fn restarting_replaces_the_previous_schedule() {
        let env = Env::new();
        let (log, push) = log();
        let timer = env.timer(move || push("fire"));
        timer.call_once(100);
        env.clock.advance(50);
        timer.call_once(100);
        assert_eq!(env.host.pending(), 1);
        env.run_until(1149);
        assert!(log.borrow().is_empty());
        env.run_until(1150);
        assert_eq!(*log.borrow(), vec!["fire"]);
        // Switching a repeating timer to one-shot.
        timer.call_each(10);
        timer.call_once(10);
        log.borrow_mut().clear();
        env.run_until(1300);
        assert_eq!(log.borrow().len(), 1);
    }

    #[test]
    fn fires_in_deadline_then_start_order() {
        let env = Env::new();
        let (log, push) = log();
        let timers: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|name| {
                let push = push.clone();
                let name = name.to_string();
                env.timer(move || push(&name))
            })
            .collect();
        timers[0].call_once(200);
        timers[1].call_once(100);
        timers[2].call_once(200);
        timers[3].call_once(100);
        env.clock.set(5000);
        assert_eq!(env.host.process(), 4);
        assert_eq!(*log.borrow(), vec!["b", "d", "a", "c"]);
    }

    #[test]
    fn cancel_from_own_handler_stops_repetition() {
        let env = Env::new();
        let count = Rc::new(Cell::new(0));
        let timer = Rc::new(Timer::in_host(&env.host, None));
        {
            let count = count.clone();
            let weak = Rc::downgrade(&timer);
            timer.set_callback(move || {
                count.set(count.get() + 1);
                if count.get() == 3 {
                    weak.upgrade().unwrap().cancel();
                }
            });
        }
        timer.call_each(10);
        env.run_until(2000);
        assert_eq!(count.get(), 3);
        assert!(!timer.is_active());
        assert_eq!(env.host.pending(), 0);
        assert_eq!(timer.remaining_time(), -1);
    }

    #[test]
    fn restart_from_own_handler() {
        let env = Env::new();
        let (log, push) = log();
        let timer = Rc::new(Timer::in_host(&env.host, None));
        {
            let weak = Rc::downgrade(&timer);
            let clock = env.clock.clone();
            let runs = Cell::new(0);
            timer.set_callback(move || {
                push(&format!("t={}", clock.now()));
                runs.set(runs.get() + 1);
                if runs.get() == 1 {
                    // A one-shot re-armed with a different timeout.
                    weak.upgrade().unwrap().call_once(300);
                }
            });
        }
        timer.call_once(100);
        env.run_until(5000);
        assert_eq!(*log.borrow(), vec!["t=1100", "t=1400"]);
        assert!(!timer.is_active());
    }

    #[test]
    fn interval_restarted_as_one_shot_from_handler_fires_once_more() {
        let env = Env::new();
        let count = Rc::new(Cell::new(0));
        let timer = Rc::new(Timer::in_host(&env.host, None));
        {
            let count = count.clone();
            let weak = Rc::downgrade(&timer);
            timer.set_callback(move || {
                count.set(count.get() + 1);
                if count.get() == 2 {
                    weak.upgrade().unwrap().call_once(50);
                }
            });
        }
        timer.call_each(100);
        env.run_until(3000);
        assert_eq!(count.get(), 3);
        assert!(!timer.is_active());
    }

    #[test]
    fn drop_from_own_handler() {
        let env = Env::new();
        let fired = Rc::new(Cell::new(0));
        let slot: Rc<RefCell<Option<Timer>>> = Rc::new(RefCell::new(None));
        {
            let fired = fired.clone();
            let slot2 = slot.clone();
            let timer = env.timer(move || {
                fired.set(fired.get() + 1);
                // Destroys the timer and the closure that is running.
                slot2.borrow_mut().take();
            });
            timer.call_each(10);
            *slot.borrow_mut() = Some(timer);
        }
        env.run_until(2000);
        assert_eq!(fired.get(), 1);
        assert!(slot.borrow().is_none());
        assert_eq!(env.host.pending(), 0);
        assert_eq!(env.alarm.requested(), None);
    }

    #[test]
    fn drop_cancels_and_releases_the_callback() {
        let env = Env::new();
        let token = Rc::new(());
        let timer = {
            let token = token.clone();
            env.timer(move || {
                let _ = &token;
            })
        };
        timer.call_once(10);
        assert_eq!(Rc::strong_count(&token), 2);
        drop(timer);
        assert_eq!(Rc::strong_count(&token), 1);
        assert_eq!(env.host.pending(), 0);
        env.run_until(2000);
    }

    #[test]
    fn dropping_another_timer_from_a_handler_prevents_its_fire() {
        let env = Env::new();
        let (log, push) = log();
        let victim: Rc<RefCell<Option<Timer>>> = Rc::new(RefCell::new(None));
        let killer = {
            let victim = victim.clone();
            let push = push.clone();
            env.timer(move || {
                push("killer");
                victim.borrow_mut().take();
            })
        };
        *victim.borrow_mut() = Some(env.timer(move || push("victim")));
        killer.call_once(100);
        victim.borrow().as_ref().unwrap().call_once(100);
        env.clock.set(1100);
        // Both are due in the same `process()`; the victim was collected into
        // the batch but must not fire after it is gone.
        assert_eq!(env.host.process(), 1);
        assert_eq!(*log.borrow(), vec!["killer"]);
    }

    #[test]
    fn set_callback_inside_handler_applies_next_time() {
        let env = Env::new();
        let (log, push) = log();
        let timer = Rc::new(Timer::in_host(&env.host, None));
        {
            let weak = Rc::downgrade(&timer);
            let push2 = push.clone();
            timer.set_callback(move || {
                push2("first");
                let push3 = push2.clone();
                weak.upgrade()
                    .unwrap()
                    .set_callback(move || push3("second"));
            });
        }
        timer.call_each(10);
        env.run_until(1035);
        assert_eq!(*log.borrow(), vec!["first", "second", "second"]);
    }

    #[test]
    fn no_callback_is_fine() {
        let env = Env::new();
        let timer = Timer::in_host(&env.host, None);
        timer.call_once(10);
        env.run_until(2000);
        assert!(!timer.is_active());
        let timer = env.timer(|| unreachable!());
        timer.clear_callback();
        timer.call_once(10);
        env.run_until(3000);
    }

    #[test]
    fn timers_started_by_a_handler_wait_for_the_next_process() {
        let env = Env::new();
        let (log, push) = log();
        let second = Rc::new(env.timer({
            let push = push.clone();
            move || push("second")
        }));
        let first = {
            let second = second.clone();
            env.timer(move || {
                push("first");
                second.call_once(0);
            })
        };
        first.call_once(10);
        env.clock.set(1010);
        assert_eq!(env.host.process(), 1);
        assert_eq!(*log.borrow(), vec!["first"]);
        // The alarm is asked to wake as soon as possible.
        assert_eq!(env.alarm.requested(), Some(1010));
        assert_eq!(env.host.process(), 1);
        assert_eq!(*log.borrow(), vec!["first", "second"]);
    }

    #[test]
    fn zero_interval_fires_once_per_process() {
        let env = Env::new();
        let count = Rc::new(Cell::new(0));
        let timer = {
            let count = count.clone();
            env.timer(move || count.set(count.get() + 1))
        };
        timer.call_each(0);
        for expected in 1..=3 {
            assert_eq!(env.host.process(), 1);
            assert_eq!(count.get(), expected);
        }
    }

    #[test]
    fn alarm_follows_the_earliest_deadline() {
        let env = Env::new();
        let a = env.timer(|| {});
        let b = env.timer(|| {});
        assert_eq!(env.alarm.requested(), None);
        a.call_once(500);
        assert_eq!(env.alarm.requested(), Some(1500));
        b.call_once(100);
        assert_eq!(env.alarm.requested(), Some(1100));
        let calls = env.alarm.calls();
        // A later timer does not bother the alarm.
        let c = env.timer(|| {});
        c.call_once(900);
        assert_eq!(env.alarm.calls(), calls);
        b.cancel();
        assert_eq!(env.alarm.requested(), Some(1500));
        a.cancel();
        c.cancel();
        assert_eq!(env.alarm.requested(), None);
    }

    #[test]
    fn adjust_makes_interval_restart_a_full_period() {
        let env = Env::new();
        let (log, push) = log();
        let clock = env.clock.clone();
        let timer = env.timer(move || push(&format!("t={}", clock.now())));
        timer.call_each(1000);
        // Ordinary firing keeps the grid.
        env.run_until(2100);
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(timer.remaining_time(), 900);

        // The system slept for 30 s: `crl::now()` jumped forward.
        env.clock.advance(30_000);
        env.host.adjust();
        // The alarm is recomputed even though the deadline is unchanged.
        let calls = env.alarm.calls();
        env.host.adjust();
        assert!(env.alarm.calls() > calls);
        assert_eq!(env.alarm.requested(), Some(32100));
        assert_eq!(timer.remaining_time(), 0);

        log.borrow_mut().clear();
        assert_eq!(env.host.process(), 1);
        assert_eq!(*log.borrow(), vec!["t=32100"]);
        // Fresh full interval from the fire, not the old grid (which would
        // have been due at 33000 already, then 34000...).
        assert_eq!(timer.remaining_time(), 1000);
        assert_eq!(env.alarm.requested(), Some(33_100));
        // And the flag is cleared: the grid goes on from the new deadline.
        env.clock.set(33_100);
        env.host.process();
        assert_eq!(timer.remaining_time(), 1000);
    }

    #[test]
    fn adjust_does_not_touch_one_shot_or_inactive_timers() {
        let env = Env::new();
        let once = env.timer(|| {});
        let idle = env.timer(|| {});
        once.call_once(500);
        env.clock.advance(100);
        env.host.adjust();
        assert_eq!(once.remaining_time(), 400);
        assert_eq!(idle.remaining_time(), -1);
        env.run_until(2000);
        assert!(!once.is_active());
    }

    #[test]
    fn adjust_flag_set_from_another_thread_is_applied_by_process() {
        let env = Env::new();
        let timer = env.timer(|| {});
        timer.call_each(100);
        let signal = env.host.signal.clone();
        std::thread::spawn(move || signal.adjust.store(true, Ordering::SeqCst))
            .join()
            .unwrap();
        let calls = env.alarm.calls();
        env.host.process();
        assert!(env.alarm.calls() > calls);
        assert!(timer.shared.adjusted.get());
    }

    #[test]
    fn delayed_calls_run_once_in_order_and_can_be_cancelled() {
        let env = Env::new();
        let timer = DelayedCallTimer::in_host(&env.host);
        let (log, push) = log();
        let ids: Vec<_> = [(300, "c"), (100, "a"), (200, "b"), (100, "a2")]
            .iter()
            .map(|(delay, name)| {
                let push = push.clone();
                let name = name.to_string();
                timer.call(*delay, move || push(&name))
            })
            .collect();
        assert!(ids.iter().all(|id| *id != 0));
        timer.cancel(ids[2]);
        timer.cancel(0);
        env.run_until(1100);
        assert_eq!(*log.borrow(), vec!["a", "a2"]);
        // Cancelling a finished call is harmless.
        timer.cancel(ids[1]);
        env.run_until(5000);
        assert_eq!(*log.borrow(), vec!["a", "a2", "c"]);
        assert_eq!(env.host.pending(), 0);
    }

    #[test]
    fn delayed_call_may_schedule_and_cancel_from_its_callback() {
        let env = Env::new();
        let timer = Rc::new(DelayedCallTimer::in_host(&env.host));
        let (log, push) = log();
        let pending = Rc::new(Cell::new(0));
        let timer2 = timer.clone();
        let pending2 = pending.clone();
        let push2 = push.clone();
        timer.call(10, move || {
            push2("first");
            let push3 = push2.clone();
            pending2.set(timer2.call(10, move || push3("second")));
            let push4 = push2.clone();
            let id = timer2.call(10, move || push4("never"));
            timer2.cancel(id);
        });
        env.run_until(1100);
        assert_eq!(*log.borrow(), vec!["first", "second"]);
        assert_ne!(pending.get(), 0);
    }

    #[test]
    fn dropping_delayed_timer_drops_callbacks_and_slots() {
        let env = Env::new();
        let token = Rc::new(());
        let timer = DelayedCallTimer::in_host(&env.host);
        for delay in [10, 20] {
            let token = token.clone();
            timer.call(delay, move || {
                let _ = &token;
            });
        }
        assert_eq!(Rc::strong_count(&token), 3);
        drop(timer);
        assert_eq!(Rc::strong_count(&token), 1);
        assert_eq!(env.host.pending(), 0);
    }

    #[test]
    fn dropping_delayed_timer_from_its_callback() {
        let env = Env::new();
        let slot: Rc<RefCell<Option<DelayedCallTimer>>> = Rc::new(RefCell::new(None));
        let ran = Rc::new(Cell::new(0));
        let timer = DelayedCallTimer::in_host(&env.host);
        {
            let slot = slot.clone();
            let ran1 = ran.clone();
            timer.call(10, move || {
                ran1.set(ran1.get() + 1);
                slot.borrow_mut().take();
            });
            let ran = ran.clone();
            timer.call(10, move || ran.set(ran.get() + 100));
        }
        *slot.borrow_mut() = Some(timer);
        env.run_until(1100);
        // The second call was cancelled by the destruction.
        assert_eq!(ran.get(), 1);
        assert_eq!(env.host.pending(), 0);
    }

    #[test]
    fn timer_outliving_its_host_handle_is_safe() {
        let clock = ManualClock::new(0);
        let timer;
        {
            let host = TimerHost::new(Arc::new(clock.clone()));
            timer = Timer::in_host(&host, None);
            timer.call_once(10);
        }
        // The timer keeps the host alive; dropping it is clean.
        assert!(timer.is_active());
        drop(timer);
    }

    #[test]
    fn thread_alarm_wakes_at_the_requested_time_of_its_clock() {
        // The clock is virtual: the thread only sleeps while the target is in
        // the future, and a past target fires at once. No tuned delays.
        let clock = ManualClock::new(0);
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = Mutex::new(tx);
        let alarm = ThreadAlarm::new(Arc::new(clock.clone()), move || {
            let _ = lock(&tx).send(());
        });
        alarm.schedule(Some(1_000_000));
        // A later request replaces the earlier one.
        clock.set(500);
        alarm.schedule(Some(400));
        rx.recv_timeout(Duration::from_secs(30)).expect("woken");
        // `None` clears the request.
        alarm.schedule(Some(10_000_000));
        alarm.schedule(None);
        drop(alarm);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn main_queue_integration_end_to_end() {
        let env = Env::new();
        let queue = tg_runtime::MainQueue::new();
        queue.bind_current_thread();
        let fired = Rc::new(Cell::new(0));
        let timer = env.timer({
            let fired = fired.clone();
            move || fired.set(fired.get() + 1)
        });
        timer.call_once(5);
        env.clock.advance(5);
        let host = env.host.clone();
        queue.post_local(move || {
            host.process();
        });
        assert_eq!(fired.get(), 0);
        queue.process();
        assert_eq!(fired.get(), 1);
    }
}
