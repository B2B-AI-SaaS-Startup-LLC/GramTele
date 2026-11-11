// Tests share the global main queue: they serialise on a lock and each
// binds its own thread as the main thread (the harness uses a thread per test).
use std::sync::{Arc, Mutex};

static SERIAL: Mutex<()> = Mutex::new(());

type Step = Box<dyn FnOnce() + Send>;

#[test]
fn steps_run_one_per_turn_in_order() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    tg_runtime::main_queue().bind_current_thread();
    let owner = Arc::new(());
    let log = Arc::new(Mutex::new(Vec::new()));
    let steps: Vec<Step> = (0..3)
        .map(|i| {
            let log = log.clone();
            Box::new(move || log.lock().unwrap().push(i)) as Step
        })
        .collect();
    let extra = {
        let log = log.clone();
        Box::new(move || log.lock().unwrap().push(3)) as Step
    };
    tg_runtime::on_main_queue(&owner, steps)
        .next(extra)
        .dispatch();
    for expected in 1..=4 {
        assert_eq!(tg_runtime::process_main_queue(), 1);
        assert_eq!(log.lock().unwrap().len(), expected);
    }
    assert_eq!(*log.lock().unwrap(), vec![0, 1, 2, 3]);
    assert_eq!(tg_runtime::process_main_queue(), 0);
}

#[test]
fn dead_guard_abandons_the_remaining_steps() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    tg_runtime::main_queue().bind_current_thread();
    let owner = Arc::new(());
    let holder = Arc::new(Mutex::new(Some(owner.clone())));
    let log = Arc::new(Mutex::new(Vec::new()));
    let steps: Vec<Step> = (0..3)
        .map(|i| {
            let (log, holder) = (log.clone(), holder.clone());
            Box::new(move || {
                log.lock().unwrap().push(i);
                if i == 0 {
                    // Destroy the guarded object while the steps are pending.
                    holder.lock().unwrap().take();
                }
            }) as Step
        })
        .collect();
    tg_runtime::on_main_queue(&owner, steps).dispatch();
    drop(owner);
    // `holder` still keeps the object alive until step 0 releases it, and the
    // test's own handle is gone: after step 0 the guard is dead.
    assert_eq!(tg_runtime::process_main_queue(), 1);
    assert_eq!(*log.lock().unwrap(), vec![0]);
    assert_eq!(
        tg_runtime::process_main_queue(),
        1,
        "dead step turn is consumed"
    );
    assert_eq!(*log.lock().unwrap(), vec![0], "steps 1 and 2 never run");
    assert_eq!(tg_runtime::process_main_queue(), 0);
}

#[test]
fn local_steps_need_not_be_send() {
    let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    tg_runtime::main_queue().bind_current_thread();
    let owner = std::rc::Rc::new(());
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let steps: Vec<Box<dyn FnOnce()>> = (0..2)
        .map(|i| {
            let log = log.clone();
            Box::new(move || log.borrow_mut().push(i)) as Box<dyn FnOnce()>
        })
        .collect();
    tg_runtime::on_main_queue_local(&owner, steps).dispatch();
    while tg_runtime::process_main_queue() > 0 {}
    assert_eq!(*log.borrow(), vec![0, 1]);
}
