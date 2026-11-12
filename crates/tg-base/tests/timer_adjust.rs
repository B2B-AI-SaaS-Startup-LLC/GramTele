use std::sync::Arc;
use tg_base::timer::{check_local_time_with, ManualClock, Timer, TimerHost};

#[test]
fn adjustment_invalidates_http_and_rearms_on_virtual_time() {
    let clock = ManualClock::new(0);
    let host = TimerHost::new(Arc::new(clock.clone()));
    let timer = Timer::in_host(&host, None);
    timer.call_each(100);
    clock.set(120);
    host.process();
    // Original estimate is 120+100, Qt schedule is still 200.
    assert_eq!(timer.remaining_time(), 100);
    assert_eq!(host.next_deadline(), Some(200));
    tg_base::unixtime::http_update(tg_base::unixtime::now());
    assert!(!check_local_time_with(|| false));
    assert!(tg_base::unixtime::http_valid());
    assert!(check_local_time_with(|| true));
    assert!(!tg_base::unixtime::http_valid());
    host.process();
    assert_eq!(host.next_deadline(), Some(220));
    clock.set(220);
    host.process();
    assert_eq!(timer.remaining_time(), 100);
    assert_eq!(host.next_deadline(), Some(320));
}
