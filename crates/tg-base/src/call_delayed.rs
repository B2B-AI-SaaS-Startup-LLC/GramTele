//! Port of `base/call_delayed.h` / `call_delayed.cpp`.
//!
//! `base::call_delayed(delay, [guard,] callable)` runs a closure once after
//! `delay` ms on the main thread, through one process-wide
//! [`DelayedCallTimer`] (the one of the main thread's default
//! [`TimerHost`]). With a guard (`this`, `base::weak_ptr`, `binary_guard`;
//! anything `tg_runtime::AsGuard`) the call is skipped if the guard died.
//!
//! The original ties the global timer to `QCoreApplication`: after
//! `aboutToQuit` new calls are ignored, on `destroyed` the timer is deleted.
//! Here the application calls [`finish`] and [`destroy`] at the same moments.

use crate::timer::{DelayedCallTimer, TimerHost};
use std::cell::{Cell, RefCell};
use tg_runtime::{AsGuard, GuardToken, Time};

thread_local! {
    static GLOBAL: RefCell<Option<DelayedCallTimer>> = const { RefCell::new(None) };
    static FINISHED: Cell<bool> = const { Cell::new(false) };
}

/// `call_delayed(delay, callable)`.
pub fn call_delayed(delay: Time, callable: impl FnOnce() + 'static) {
    if FINISHED.with(Cell::get) {
        return;
    }
    call_delayed_in(&TimerHost::current(), delay, callable);
}

/// The same on an explicit host (virtual time in tests). The global timer is
/// created for the first host used by the thread.
pub fn call_delayed_in(
    host: &std::rc::Rc<TimerHost>,
    delay: Time,
    callable: impl FnOnce() + 'static,
) {
    if FINISHED.with(Cell::get) {
        return;
    }
    GLOBAL.with(|global| {
        let mut global = global.borrow_mut();
        let timer = global.get_or_insert_with(|| DelayedCallTimer::in_host(host));
        timer.call(delay, callable);
    });
}

/// `call_delayed(delay, guard, callable)`.
pub fn call_delayed_guarded<G>(delay: Time, guard: &G, callable: impl FnOnce() + 'static)
where
    G: AsGuard + ?Sized,
    G::Token: 'static,
{
    let token = guard.as_guard();
    call_delayed(delay, move || {
        if token.alive() {
            callable();
        }
    });
}

/// `fn_delayed(delay, guard, callable)`: a function that, each time it is
/// called, schedules a guarded call of `callable`.
pub fn fn_delayed<G>(delay: Time, guard: &G, callable: impl Fn() + 'static) -> impl Fn()
where
    G: AsGuard + ?Sized,
    G::Token: Clone + 'static,
{
    let token = guard.as_guard();
    let callable = std::rc::Rc::new(callable);
    move || {
        let token = token.clone();
        let callable = callable.clone();
        call_delayed(delay, move || {
            if token.alive() {
                callable();
            }
        });
    }
}

/// `QCoreApplication::aboutToQuit`: later `call_delayed` calls are ignored.
pub fn finish() {
    FINISHED.with(|finished| finished.set(true));
}

/// `QCoreApplication::destroyed`: drops the global timer and its calls.
pub fn destroy() {
    FINISHED.with(|finished| finished.set(true));
    let timer = GLOBAL.with(|global| global.borrow_mut().take());
    drop(timer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::ManualClock;
    use crate::weak_ptr::{make_weak, HasWeakPtr};
    use std::rc::Rc;
    use std::sync::Arc;

    // The global timer is thread-local: each test thread has its own.
    fn host() -> (ManualClock, Rc<TimerHost>) {
        let clock = ManualClock::new(0);
        (clock.clone(), TimerHost::new(Arc::new(clock)))
    }

    #[test]
    fn runs_once_after_the_delay() {
        let (clock, host) = host();
        let count = Rc::new(Cell::new(0));
        let c = count.clone();
        call_delayed_in(&host, 100, move || c.set(c.get() + 1));
        clock.set(99);
        host.process();
        assert_eq!(count.get(), 0);
        clock.set(100);
        host.process();
        clock.set(500);
        host.process();
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn guard_skips_the_call_after_death() {
        let (clock, host) = host();
        let owner = HasWeakPtr::new();
        let weak = make_weak(&owner);
        let count = Rc::new(Cell::new(0));
        for delay in [10, 20] {
            let c = count.clone();
            let token = weak.clone();
            call_delayed_in(&host, delay, move || {
                if tg_runtime::GuardToken::alive(&token) {
                    c.set(c.get() + 1);
                }
            });
        }
        clock.set(15);
        host.process();
        drop(owner);
        clock.set(25);
        host.process();
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn weak_ptr_and_binary_guard_are_guards() {
        let owner = HasWeakPtr::new();
        let weak = make_weak(&owner);
        let f = tg_runtime::guard_fn(&weak, || 7);
        assert_eq!(f(), 7);
        let f2 = tg_runtime::guard_fn(&owner, || 8);
        assert_eq!(f2(), 8);
        let mut mine = crate::binary_guard::BinaryGuard::new();
        let theirs = mine.make_guard();
        let f3 = tg_runtime::guard_fn(&mine, || 9);
        assert_eq!(f3(), 9);
        drop(theirs);
        assert_eq!(f3(), 0);
        drop(owner);
        assert_eq!(f(), 0);
        assert_eq!(f2(), 0);
    }

    #[test]
    fn finish_ignores_new_calls_and_destroy_drops_pending() {
        let (clock, host) = host();
        let token = Rc::new(());
        let t = token.clone();
        call_delayed_in(&host, 10, move || drop(t));
        assert_eq!(Rc::strong_count(&token), 2);
        destroy();
        assert_eq!(Rc::strong_count(&token), 1);
        let ran = Rc::new(Cell::new(false));
        let r = ran.clone();
        call_delayed_in(&host, 1, move || r.set(true));
        clock.set(100);
        host.process();
        assert!(!ran.get());
    }
}
