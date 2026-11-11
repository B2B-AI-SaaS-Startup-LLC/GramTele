//! Monotonic time (`crl/crl_time.*`, `linux/`, `mac/`, `winapi/` time files).
//!
//! `Time` is `i64` milliseconds, `ProfileTime` is `i64` microseconds, both
//! counted from the first use of the clock (the original: from static
//! initialisation of the library) plus the sleep adjustment of
//! [`adjust_time`].

use crate::util::lock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// `crl::time`: monotonic milliseconds.
pub type Time = i64;
/// `crl::profile_time`: monotonic microseconds.
pub type ProfileTime = i64;

/// Sleep compensation state (`adjust_time` of the original).
pub(crate) struct Adjuster {
    seconds: AtomicU32,
    /// (last adjustment `now()`, last adjustment unix time in seconds).
    last: Mutex<(Time, i64)>,
}

impl Adjuster {
    pub(crate) fn new(unixtime: i64) -> Self {
        Adjuster {
            seconds: AtomicU32::new(0),
            last: Mutex::new((0, unixtime)),
        }
    }

    pub(crate) fn adjustment_ms(&self) -> Time {
        Time::from(self.seconds.load(Ordering::Relaxed)) * 1000
    }

    /// Compares the wall clock with the monotonic one; if the monotonic clock
    /// stopped (system sleep) the difference is added to all later readings.
    pub(crate) fn adjust(&self, now: Time, unixtime: i64) -> bool {
        let mut last = lock(&self.last);
        let delta = now - last.0;
        let real = unixtime - last.1;
        // Integer division truncates toward zero, as in C++.
        let seconds = (real * 1000 - delta) / 1000;
        last.1 = unixtime;
        last.0 = now;
        if seconds <= 0 {
            return false;
        }
        let max = Time::from(u32::MAX);
        let mut current = self.seconds.load(Ordering::Relaxed);
        loop {
            if Time::from(current) + seconds > max {
                return false;
            }
            let next = current + seconds as u32;
            match self.seconds.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
}

struct Clock {
    start: Instant,
    adjuster: Adjuster,
}

fn unixtime() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn clock() -> &'static Clock {
    static CLOCK: OnceLock<Clock> = OnceLock::new();
    CLOCK.get_or_init(|| Clock {
        start: Instant::now(),
        adjuster: Adjuster::new(unixtime()),
    })
}

/// Fixes the time origin. Optional: call first thing in `main` so that the
/// origin matches the process start as closely as the original static init.
pub fn init_time() {
    let _ = clock();
}

/// `crl::now()`: milliseconds, monotonic.
pub fn now() -> Time {
    let c = clock();
    c.start.elapsed().as_millis() as Time + c.adjuster.adjustment_ms()
}

/// `crl::profile()`: microseconds, monotonic.
pub fn profile() -> ProfileTime {
    let c = clock();
    c.start.elapsed().as_micros() as ProfileTime + c.adjuster.adjustment_ms() * 1000
}

/// `crl::adjust_time()`: returns true if the clock was moved forward to
/// compensate for a system sleep.
pub fn adjust_time() -> bool {
    clock().adjuster.adjust(now(), unixtime())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_adjustment_when_clocks_agree() {
        let a = Adjuster::new(1000);
        assert!(!a.adjust(5000, 1005));
        assert_eq!(a.adjustment_ms(), 0);
    }

    #[test]
    fn sleep_is_compensated() {
        let a = Adjuster::new(1000);
        // 10 real seconds passed, the monotonic clock saw 5.
        assert!(a.adjust(5000, 1010));
        assert_eq!(a.adjustment_ms(), 5000);
        // Deltas are relative to the previous call.
        assert!(!a.adjust(5000 + 3000, 1010 + 3));
        assert_eq!(a.adjustment_ms(), 5000);
        assert!(a.adjust(8000 + 1000, 1013 + 4));
        assert_eq!(a.adjustment_ms(), 8000);
    }

    #[test]
    fn overflow_is_refused() {
        let a = Adjuster::new(0);
        a.seconds.store(u32::MAX - 1, Ordering::Relaxed);
        assert!(!a.adjust(0, 10));
        assert_eq!(a.seconds.load(Ordering::Relaxed), u32::MAX - 1);
    }

    #[test]
    fn wall_clock_going_back_is_ignored() {
        let a = Adjuster::new(1000);
        assert!(!a.adjust(1000, 900));
    }
}
