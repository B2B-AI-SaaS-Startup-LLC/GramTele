use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use tg_runtime::{guard, guard_fn1, guard_mut, Guardian, MainQueue, MainRef};

#[test]
fn guard_runs_while_alive_and_skips_when_destroyed() {
    let hits = Arc::new(AtomicUsize::new(0));
    let owner = Arc::new(5);
    let h = hits.clone();
    let f = guard(&owner, move || {
        h.fetch_add(1, Ordering::SeqCst);
        1
    });
    assert_eq!(f(), 1);
    let h = hits.clone();
    let g = guard(&owner, move || {
        h.fetch_add(1, Ordering::SeqCst);
        1
    });
    drop(owner);
    assert_eq!(g(), 0, "dead guard returns Default");
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn guard_variants_with_args_and_rc() {
    let owner = Rc::new(());
    let total = Rc::new(RefCell::new(0));
    let t = total.clone();
    let mut add = guard_mut(&owner, move || *t.borrow_mut() += 1);
    add();
    add();
    let t = total.clone();
    let f = guard_fn1(&owner, move |x: i32| *t.borrow_mut() += x);
    f(10);
    drop(owner);
    add();
    f(100);
    assert_eq!(*total.borrow(), 12);
}

#[test]
fn guardian_guards_die_with_the_object() {
    struct Widget {
        guardian: Guardian,
        _not_send: Rc<()>,
    }
    let w = Widget {
        guardian: Guardian::new(),
        _not_send: Rc::new(()),
    };
    let ran = Arc::new(AtomicUsize::new(0));
    let r = ran.clone();
    let f = guard(&w.guardian, move || {
        r.fetch_add(1, Ordering::SeqCst);
    });
    // The guard token itself is Send even though the widget is not.
    let token = w.guardian.guard();
    std::thread::spawn(move || drop(token)).join().unwrap();
    drop(w);
    f();
    assert_eq!(ran.load(Ordering::SeqCst), 0);
}

#[test]
fn on_main_guarded_skips_dead_object_at_run_time() {
    // Uses the global queue: drained explicitly on this thread.
    let owner = Arc::new(());
    let ran = Arc::new(AtomicUsize::new(0));
    let r = ran.clone();
    tg_runtime::on_main_guarded(&owner, move || {
        r.fetch_add(1, Ordering::SeqCst);
    });
    let r = ran.clone();
    tg_runtime::on_main_guarded(&owner, move || {
        r.fetch_add(10, Ordering::SeqCst);
    });
    // Object dies after posting but before the queue runs: nothing runs.
    drop(owner);
    tg_runtime::process_main_queue();
    assert_eq!(ran.load(Ordering::SeqCst), 0);
}

struct Widget {
    value: RefCell<i32>,
}

#[test]
fn main_ref_reaches_a_non_send_object_from_a_background_thread() {
    let q = MainQueue::new();
    q.bind_current_thread();
    let widget = Rc::new(Widget {
        value: RefCell::new(0),
    });
    let handle = MainRef::new(&widget);
    let main_id = std::thread::current().id();
    let (tx, rx) = mpsc::channel();
    let q2 = q.clone();
    std::thread::spawn(move || {
        for i in 1..=3 {
            let tx = tx.clone();
            handle.on_queue(&q2, move |w| {
                *w.value.borrow_mut() += i;
                tx.send(std::thread::current().id()).unwrap();
            });
        }
    })
    .join()
    .unwrap();
    q.process_all();
    assert_eq!(*widget.value.borrow(), 6);
    assert!(rx.try_iter().all(|id| id == main_id));
}

#[test]
fn main_ref_to_destroyed_object_does_not_run() {
    let q = MainQueue::new();
    q.bind_current_thread();
    let widget = Rc::new(Widget {
        value: RefCell::new(0),
    });
    let handle = MainRef::new(&widget);
    let ran = Arc::new(AtomicUsize::new(0));
    let r = ran.clone();
    handle.on_queue(&q, move |_| {
        r.fetch_add(1, Ordering::SeqCst);
    });
    drop(widget);
    q.process_all();
    assert_eq!(ran.load(Ordering::SeqCst), 0);
    assert!(handle.upgrade().is_none());
}

#[test]
fn main_ref_registry_survives_many_dead_entries() {
    let mut keep = Vec::new();
    for i in 0..1000 {
        let w = Rc::new(Widget {
            value: RefCell::new(i),
        });
        let h = MainRef::new(&w);
        if i % 100 == 0 {
            keep.push((w, h));
        }
    }
    for (w, h) in &keep {
        assert!(Rc::ptr_eq(w, &h.upgrade().unwrap()));
    }
}
