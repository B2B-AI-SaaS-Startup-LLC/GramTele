//! Port of `base/timer_rpl.h`: `base::timer_once` and `base::timer_each` as
//! `rpl` producers.
//!
//! Each start of the producer creates a [`Timer`] kept in the subscription's
//! lifetime; ending the subscription cancels the timer.

use crate::timer::{Timer, TimerHost};
use std::rc::Rc;
use tg_rpl::{Lifetime, Producer};
use tg_runtime::Time;

/// `base::timer_once(delay)`: one value after `delay`, then `done`.
pub fn timer_once(delay: Time) -> Producer {
    timer_once_in(&TimerHost::current(), delay)
}

/// `base::timer_each(delay)`: a value every `delay`, never completes.
pub fn timer_each(delay: Time) -> Producer {
    timer_each_in(&TimerHost::current(), delay)
}

/// [`timer_once`] on an explicit host (virtual time in tests).
pub fn timer_once_in(host: &Rc<TimerHost>, delay: Time) -> Producer {
    let host = Rc::clone(host);
    Producer::new(move |consumer| {
        let result = Lifetime::new();
        let timer = result.make_state(Timer::in_host(&host, None));
        let timer = timer.get();
        timer.set_callback(move || {
            consumer.put_next_copy(&());
            consumer.put_done();
        });
        timer.call_once(delay);
        result
    })
}

/// [`timer_each`] on an explicit host.
pub fn timer_each_in(host: &Rc<TimerHost>, delay: Time) -> Producer {
    let host = Rc::clone(host);
    Producer::new(move |consumer| {
        let result = Lifetime::new();
        let timer = result.make_state(Timer::in_host(&host, None));
        let timer = timer.get();
        timer.set_callback(move || {
            consumer.put_next_copy(&());
        });
        timer.call_each(delay);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::ManualClock;
    use std::cell::Cell;
    use std::sync::Arc;

    fn host() -> (ManualClock, Rc<TimerHost>) {
        let clock = ManualClock::new(0);
        let host = TimerHost::new(Arc::new(clock.clone()));
        (clock, host)
    }

    #[test]
    fn once_emits_then_completes() {
        let (clock, host) = host();
        let values = Rc::new(Cell::new(0));
        let done = Rc::new(Cell::new(false));
        let lifetime = Lifetime::new();
        timer_once_in(&host, 100).start(
            {
                let values = values.clone();
                move |_| values.set(values.get() + 1)
            },
            |_| unreachable!(),
            {
                let done = done.clone();
                move || done.set(true)
            },
            &lifetime,
        );
        clock.set(99);
        host.process();
        assert_eq!(values.get(), 0);
        clock.set(100);
        host.process();
        assert_eq!((values.get(), done.get()), (1, true));
        clock.set(1000);
        host.process();
        assert_eq!(values.get(), 1);
    }

    #[test]
    fn each_emits_until_the_lifetime_ends() {
        let (clock, host) = host();
        let values = Rc::new(Cell::new(0));
        let lifetime = Lifetime::new();
        timer_each_in(&host, 10).on_next(
            {
                let values = values.clone();
                move |_| values.set(values.get() + 1)
            },
            &lifetime,
        );
        for t in [10, 20, 30] {
            clock.set(t);
            host.process();
        }
        assert_eq!(values.get(), 3);
        lifetime.destroy();
        assert_eq!(host.pending(), 0);
        clock.set(100);
        host.process();
        assert_eq!(values.get(), 3);
    }

    #[test]
    fn unsubscribing_before_the_fire_cancels_the_timer() {
        let (clock, host) = host();
        let values = Rc::new(Cell::new(0));
        let lifetime = Lifetime::new();
        timer_once_in(&host, 50).on_next(
            {
                let values = values.clone();
                move |_| values.set(1)
            },
            &lifetime,
        );
        assert_eq!(host.pending(), 1);
        drop(lifetime);
        assert_eq!(host.pending(), 0);
        clock.set(100);
        host.process();
        assert_eq!(values.get(), 0);
    }
}
