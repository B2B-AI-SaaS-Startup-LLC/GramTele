//! Port of `lib_base/base/unixtime.h` and `unixtime.cpp`.
//!
//! Server time is kept as a shift from the local clock. All functions are
//! thread-safe, as in the original: the shifts are atomics and the message
//! id generator is guarded by a read-write lock.
//!
//! `updates()` must be subscribed on the runtime's main thread. Time updates
//! may originate on any thread; delivery goes through `tg_runtime::on_main`.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// `TimeId` from `basic_types.h`: seconds since the Unix epoch, 32-bit.
pub type TimeId = i32;

const IGNORE_TIME_DIFFERENCE: TimeId = 3;

static VALUE_UPDATED: AtomicBool = AtomicBool::new(false);
static VALUE_SHIFT: AtomicI32 = AtomicI32::new(0);
static HTTP_VALUE_VALID: AtomicBool = AtomicBool::new(false);
static HTTP_VALUE_SHIFT: AtomicI32 = AtomicI32::new(0);

thread_local! {
    static UPDATES: tg_rpl::EventStream<()> = tg_rpl::EventStream::new();
}

/// `updates()`: main-thread producer of applied server-time changes.
pub fn updates() -> tg_rpl::Producer<()> {
    assert!(
        tg_runtime::main_queue().is_main_thread(),
        "unixtime::updates: main thread required"
    );
    UPDATES.with(|stream| stream.events())
}

/// Origin of the monotonic counter. The original reads
/// `CLOCK_MONOTONIC` / `QueryPerformanceCounter` / `mach_absolute_time` and
/// scales it to 2^32 - 2^16 units per second; only differences between two
/// readings are used, so nanoseconds since a fixed `Instant` give the same
/// values on every platform.
static COUNTER_ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);

fn counter() -> u64 {
    COUNTER_ORIGIN.elapsed().as_nanos() as u64
}

/// `GetMultiplier()`: 0xFFFF0000 instead of 0x100000000 makes msg ids grow
/// slightly slower than unixtime, leaving time to reconfigure.
const MULTIPLIER: f64 = 0xFFFF_0000u64 as f64 / 1_000_000_000.;

struct Start {
    start_id: u64,
    start_counter: u64,
}

struct MsgIdManager {
    lock: RwLock<Start>,
    incremented_part: AtomicU32,
    random_part: u64,
}

impl MsgIdManager {
    fn new() -> Self {
        // The original draws a uniform `uint32` from `std::mt19937` seeded by
        // `std::random_device`, then calls `srand()`; only the random value
        // matters to the port.
        let random_part = u64::from(crate::random::random_value::<u32>());
        Self {
            lock: RwLock::new(Self::initial(random_part)),
            incremented_part: AtomicU32::new(0),
            random_part,
        }
    }

    fn initial(random_part: u64) -> Start {
        Start {
            start_counter: counter(),
            start_id: (u64::from(now() as u32) << 32) | random_part,
        }
    }

    fn update(&self) {
        let mut start = self.lock.write().unwrap_or_else(|e| e.into_inner());
        *start = Self::initial(self.random_part);
    }

    fn next(&self) -> u64 {
        let counter = counter();

        let (start_counter, start_id) = {
            let start = self.lock.read().unwrap_or_else(|e| e.into_inner());
            (start.start_counter, start.start_id)
        };

        let incremented_part = self
            .incremented_part
            .fetch_add(4, Ordering::SeqCst)
            .wrapping_add(4);

        let delta = counter.wrapping_sub(start_counter);
        let result = start_id.wrapping_add((delta as f64 * MULTIPLIER).floor() as u64);

        (result & !0x03u64).wrapping_add(u64::from(incremented_part))
    }
}

static GLOBAL_MSG_ID_MANAGER: LazyLock<MsgIdManager> = LazyLock::new(MsgIdManager::new);

fn local() -> TimeId {
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        Err(error) => -(error.duration().as_secs_f64().ceil() as i64),
    };
    seconds as TimeId
}

/// `base::unixtime::now()`: local time corrected by the server shift.
#[must_use]
pub fn now() -> TimeId {
    local().wrapping_add(VALUE_SHIFT.load(Ordering::SeqCst))
}

/// `base::unixtime::update(now, force)`: records server time.
///
/// Without `force` only the first meaningful update (a shift of at least 3
/// seconds from the current one) is applied; later ones are ignored.
pub fn update(now: TimeId, force: bool) {
    if force {
        VALUE_UPDATED.store(true, Ordering::SeqCst);
    } else if VALUE_UPDATED.load(Ordering::SeqCst) {
        return;
    }

    let shift = now.wrapping_sub(local());
    let ignore = !force
        && shift
            .wrapping_sub(VALUE_SHIFT.load(Ordering::SeqCst))
            .wrapping_abs()
            < IGNORE_TIME_DIFFERENCE;
    if ignore {
        return;
    }
    if !force
        && VALUE_UPDATED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
    {
        return;
    }
    let old = VALUE_SHIFT.swap(shift, Ordering::SeqCst);

    HTTP_VALUE_SHIFT.store(0, Ordering::SeqCst);
    HTTP_VALUE_VALID.store(false, Ordering::SeqCst);

    if old != shift {
        GLOBAL_MSG_ID_MANAGER.update();
        tg_runtime::on_main(|| UPDATES.with(|stream| stream.fire(())));
    }
}

/// `base::unixtime::parse(value)`: server `TimeId` to a local point in time.
/// `None` stands for the null `QDateTime` returned for `value <= 0`.
#[must_use]
pub fn parse(value: TimeId) -> Option<SystemTime> {
    if value <= 0 {
        return None;
    }
    let seconds = i64::from(value) - i64::from(VALUE_SHIFT.load(Ordering::SeqCst));
    Some(from_secs_since_epoch(seconds))
}

/// `base::unixtime::serialize(date)`: a local point in time to server
/// `TimeId`; `None` (null `QDateTime`) gives 0. Truncated to 32 bits like the
/// original `qint64 -> TimeId` conversion.
#[must_use]
pub fn serialize(date: Option<SystemTime>) -> TimeId {
    match date {
        None => 0,
        Some(date) => {
            let seconds = to_secs_since_epoch(date);
            (seconds + i64::from(VALUE_SHIFT.load(Ordering::SeqCst))) as TimeId
        }
    }
}

fn from_secs_since_epoch(seconds: i64) -> SystemTime {
    if seconds >= 0 {
        UNIX_EPOCH + Duration::from_secs(seconds as u64)
    } else {
        UNIX_EPOCH - Duration::from_secs(seconds.unsigned_abs())
    }
}

/// `QDateTime::toSecsSinceEpoch`: integer division of milliseconds by 1000,
/// truncating toward zero, including for dates before the epoch.
fn to_secs_since_epoch(date: SystemTime) -> i64 {
    match date.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        Err(error) => -(error.duration().as_secs() as i64),
    }
}

/// `base::unixtime::http_valid()`.
#[must_use]
pub fn http_valid() -> bool {
    HTTP_VALUE_VALID.load(Ordering::SeqCst)
}

/// `base::unixtime::http_now()`.
#[must_use]
pub fn http_now() -> TimeId {
    now().wrapping_add(HTTP_VALUE_SHIFT.load(Ordering::SeqCst))
}

/// `base::unixtime::http_update(now)`: time from an HTTP `Date` header.
pub fn http_update(now: TimeId) {
    HTTP_VALUE_SHIFT.store(now.wrapping_sub(self::now()), Ordering::SeqCst);
    HTTP_VALUE_VALID.store(true, Ordering::SeqCst);
}

/// `base::unixtime::http_invalidate()`.
pub fn http_invalidate() {
    HTTP_VALUE_VALID.store(false, Ordering::SeqCst);
}

/// `base::unixtime::mtproto_msg_id()`: next MTProto message id
/// (`unixtime << 32 | fraction`, low two bits replaced by a counter step).
#[must_use]
pub fn mtproto_msg_id() -> u64 {
    GLOBAL_MSG_ID_MANAGER.next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The state is global, tests that change it run one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn reset() {
        VALUE_UPDATED.store(false, Ordering::SeqCst);
        VALUE_SHIFT.store(0, Ordering::SeqCst);
        HTTP_VALUE_VALID.store(false, Ordering::SeqCst);
        HTTP_VALUE_SHIFT.store(0, Ordering::SeqCst);
    }

    #[test]
    fn small_shift_is_ignored_until_forced() {
        let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        update(local() + 1, false);
        assert_eq!(VALUE_SHIFT.load(Ordering::SeqCst), 0);
        // The ignored update does not consume the "first update" slot.
        assert!(!VALUE_UPDATED.load(Ordering::SeqCst));

        update(local() + 100, false);
        let shift = VALUE_SHIFT.load(Ordering::SeqCst);
        assert!((99..=101).contains(&shift));
        assert!(VALUE_UPDATED.load(Ordering::SeqCst));

        // Later non-forced updates are ignored.
        update(local() + 1000, false);
        assert_eq!(VALUE_SHIFT.load(Ordering::SeqCst), shift);

        // Forced updates always apply, even small ones.
        update(local() - 1, true);
        let forced = VALUE_SHIFT.load(Ordering::SeqCst);
        assert!((-2..=0).contains(&forced));
        reset();
    }

    #[test]
    fn http_shift() {
        let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        assert!(!http_valid());
        http_update(now() + 50);
        assert!(http_valid());
        let delta = http_now() - now();
        assert!((49..=51).contains(&delta));
        http_invalidate();
        assert!(!http_valid());
        // A server time update resets the HTTP shift.
        http_update(now() + 50);
        update(local() + 10, true);
        assert!(!http_valid());
        assert!((http_now() - now()).abs() <= 1);
        reset();
    }

    #[test]
    fn parse_and_serialize() {
        let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        assert_eq!(parse(0), None);
        assert_eq!(parse(-5), None);
        assert_eq!(serialize(None), 0);
        VALUE_SHIFT.store(10, Ordering::SeqCst);
        let date = parse(1_000_000).unwrap();
        assert_eq!(date, UNIX_EPOCH + Duration::from_secs(999_990));
        assert_eq!(serialize(Some(date)), 1_000_000);
        // Fractions are dropped toward zero, including before the epoch.
        assert_eq!(
            to_secs_since_epoch(UNIX_EPOCH + Duration::from_millis(1500)),
            1
        );
        assert_eq!(
            to_secs_since_epoch(UNIX_EPOCH - Duration::from_millis(1500)),
            -1
        );
        reset();
    }

    #[test]
    fn msg_ids_grow_and_are_multiple_of_four_steps() {
        let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        GLOBAL_MSG_ID_MANAGER.update();
        let first = mtproto_msg_id();
        let second = mtproto_msg_id();
        assert!(second > first);
        let seconds = (first >> 32) as i64;
        assert!((seconds - i64::from(now())).abs() <= 2);
    }

    #[test]
    fn msg_id_formula() {
        let manager = MsgIdManager {
            lock: RwLock::new(Start {
                start_id: 0x5000_0000_0000_0003,
                start_counter: counter(),
            }),
            incremented_part: AtomicU32::new(u32::MAX - 3),
            random_part: 3,
        };
        // The counter part wraps to 0, the low two bits are cleared.
        let id = manager.next();
        assert_eq!(id >> 32, 0x5000_0000);
        assert_eq!(id & 3, 0);
        // Next step adds 4.
        let next = manager.next();
        assert_eq!(next & 3, 0);
    }
}
