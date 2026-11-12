//! A separate process owns the global main queue, like the application.
use std::cell::Cell;
use std::rc::Rc;
use tg_base::unixtime;

#[test]
fn worker_time_update_is_delivered_on_main_and_resets_http() {
    tg_runtime::init_main_queue(|| {});
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    let lifetime = unixtime::updates().on_next_returning(move |_| seen.set(seen.get() + 1));
    let old = unixtime::now();
    unixtime::http_update(old + 50);
    assert!(unixtime::http_valid());
    std::thread::spawn(move || unixtime::update(old + 100, true))
        .join()
        .unwrap();
    assert_eq!(count.get(), 0);
    tg_runtime::process_main_queue();
    assert_eq!(count.get(), 1);
    assert!(!unixtime::http_valid());
    assert!((unixtime::now() - old - 100).abs() <= 1);
    assert_eq!(unixtime::serialize(unixtime::parse(old)), old);
    drop(lifetime);
}
