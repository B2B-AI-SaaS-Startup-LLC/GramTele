//! The global main queue binds the first thread that uses `post_local`, so
//! all scenarios run sequentially in one test.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tg_base::invoke_queued::*;
use tg_base::weak_ptr::{make_weak, HasWeakPtr};

#[test]
fn queued_calls() {
    tg_runtime::init_main_queue(|| {});

    // Runs later, not at once; skipped when the context is dead.
    let owner = HasWeakPtr::new();
    let ran = Rc::new(Cell::new(0));
    let r = ran.clone();
    invoke_queued(&make_weak(&owner), move || r.set(r.get() + 1));
    assert_eq!(ran.get(), 0);
    tg_runtime::process_main_queue();
    assert_eq!(ran.get(), 1);
    let r = ran.clone();
    invoke_queued(&owner, move || r.set(r.get() + 10));
    drop(owner);
    tg_runtime::process_main_queue();
    assert_eq!(ran.get(), 1);

    // Plain main-thread call keeps FIFO order with guarded ones.
    let order = Rc::new(std::cell::RefCell::new(Vec::new()));
    let o = order.clone();
    invoke_queued_main(move || o.borrow_mut().push(1));
    let o = order.clone();
    invoke_queued_main(move || o.borrow_mut().push(2));
    tg_runtime::process_main_queue();
    assert_eq!(*order.borrow(), vec![1, 2]);

    // From another thread.
    let owner = HasWeakPtr::new();
    let weak = make_weak(&owner);
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    std::thread::spawn(move || {
        invoke_queued_send(&weak, move || {
            c.fetch_add(1, Ordering::SeqCst);
        })
    })
    .join()
    .unwrap();
    tg_runtime::process_main_queue();
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // SingleQueuedInvokation: a batch of calls is one run.
    let runs = Arc::new(AtomicUsize::new(0));
    let r = runs.clone();
    let single = SingleQueuedInvokation::new(move || {
        r.fetch_add(1, Ordering::SeqCst);
    });
    single.call();
    single.call();
    single.call();
    assert_eq!(runs.load(Ordering::SeqCst), 0);
    tg_runtime::process_main_queue();
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    single.call();
    tg_runtime::process_main_queue();
    assert_eq!(runs.load(Ordering::SeqCst), 2);
    // Dropped before the queue runs: no call.
    single.call();
    drop(single);
    tg_runtime::process_main_queue();
    assert_eq!(runs.load(Ordering::SeqCst), 2);

    let local = Rc::new(Cell::new(0));
    let l = local.clone();
    let single = SingleQueuedInvokationLocal::new(move || l.set(l.get() + 1));
    single.call();
    single.call();
    tg_runtime::process_main_queue();
    assert_eq!(local.get(), 1);
    single.call();
    drop(single);
    tg_runtime::process_main_queue();
    assert_eq!(local.get(), 1);
}
