use std::sync::{mpsc, Arc, Barrier};
use tg_runtime::{adjust_time, now, profile, Time};

#[test]
fn time_is_i64_milliseconds_from_small_origin() {
    let t: Time = now();
    let _: i64 = t;
    assert!(t >= 0);
    // Origin is the first use of the clock, not the unix epoch.
    assert!(t < 24 * 3600 * 1000);
}

#[test]
fn now_is_monotonic_on_every_thread_and_across_handoffs() {
    let barrier = Arc::new(Barrier::new(4));
    let hs: Vec<_> = (0..4)
        .map(|_| {
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                let mut prev = now();
                let mut prev_p = profile();
                for _ in 0..10_000 {
                    let n = now();
                    let p = profile();
                    assert!(n >= prev);
                    assert!(p >= prev_p);
                    prev = n;
                    prev_p = p;
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    // A reading taken after a channel hand-off is not older than the one before.
    let (tx, rx) = mpsc::channel();
    let before = now();
    std::thread::spawn(move || tx.send(now()).unwrap());
    assert!(rx.recv().unwrap() >= before);
}

#[test]
fn profile_is_microseconds_of_the_same_clock() {
    let n1 = now();
    let p = profile();
    let n2 = now();
    assert!(p / 1000 >= n1 && p / 1000 <= n2);
}

#[test]
fn adjust_time_is_false_without_sleep() {
    assert!(!adjust_time());
}
