use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use tg_runtime::MainQueue;

fn counting_waker(q: &MainQueue) -> Arc<AtomicUsize> {
    let wakes = Arc::new(AtomicUsize::new(0));
    let w = wakes.clone();
    q.set_waker(Some(Arc::new(move || {
        w.fetch_add(1, Ordering::SeqCst);
    })));
    wakes
}

fn log() -> Arc<Mutex<Vec<i32>>> {
    Arc::new(Mutex::new(Vec::new()))
}

#[test]
fn post_is_deferred_even_on_main_thread_and_fifo() {
    let q = MainQueue::new();
    q.bind_current_thread();
    let l = log();
    for i in 0..5 {
        let l = l.clone();
        q.post(move || l.lock().unwrap().push(i));
    }
    assert!(l.lock().unwrap().is_empty(), "never runs inline");
    assert_eq!(q.process(), 5);
    assert_eq!(*l.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    assert_eq!(q.process(), 0);
}

#[test]
fn tasks_posted_while_processing_wait_for_next_process() {
    let q = MainQueue::new();
    let wakes = counting_waker(&q);
    let l = log();
    {
        let (q2, l1, l2) = (q.clone(), l.clone(), l.clone());
        q.post(move || {
            l1.lock().unwrap().push(1);
            let l3 = l1.clone();
            q2.post(move || l3.lock().unwrap().push(3));
        });
        q.post(move || l2.lock().unwrap().push(2));
    }
    assert_eq!(wakes.load(Ordering::SeqCst), 1, "one wake per idle->busy");
    assert_eq!(q.process(), 2);
    // Nested post ran after the rest of the batch, not inside it.
    assert_eq!(*l.lock().unwrap(), vec![1, 2]);
    assert_eq!(wakes.load(Ordering::SeqCst), 2, "re-woken after the batch");
    assert_eq!(q.process(), 1);
    assert_eq!(*l.lock().unwrap(), vec![1, 2, 3]);
    assert_eq!(wakes.load(Ordering::SeqCst), 2);
    // Idle again: the next post wakes again.
    q.post(|| {});
    assert_eq!(wakes.load(Ordering::SeqCst), 3);
}

#[test]
fn waker_not_called_while_already_scheduled() {
    let q = MainQueue::new();
    let wakes = counting_waker(&q);
    q.post(|| {});
    q.post(|| {});
    q.post(|| {});
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
}

#[test]
fn posts_before_waker_registration_are_delivered() {
    let q = MainQueue::new();
    q.post(|| {});
    let wakes = counting_waker(&q);
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert_eq!(q.process(), 1);
}

#[test]
fn reentrant_process_sees_only_newer_tasks() {
    // Characterisation of the original: a nested process() takes the tasks
    // queued after the outer batch started, so they run before the rest of
    // the outer batch.
    let q = MainQueue::new();
    let l = log();
    let (q1, l1) = (q.clone(), l.clone());
    q.post(move || {
        l1.lock().unwrap().push(1);
        let l = l1.clone();
        q1.post(move || l.lock().unwrap().push(99));
        q1.process();
        l1.lock().unwrap().push(2);
    });
    let l3 = l.clone();
    q.post(move || l3.lock().unwrap().push(3));
    q.process();
    assert_eq!(*l.lock().unwrap(), vec![1, 99, 2, 3]);
}

#[test]
fn local_tasks_keep_global_order_and_need_not_be_send() {
    let q = MainQueue::new();
    let l = Rc::new(RefCell::new(Vec::new()));
    let sl = log();
    {
        let l = l.clone();
        q.post_local(move || l.borrow_mut().push(1));
    }
    {
        let sl = sl.clone();
        q.post(move || sl.lock().unwrap().push(2));
    }
    {
        let (l, sl) = (l.clone(), sl.clone());
        q.post_local(move || {
            l.borrow_mut().push(3);
            sl.lock().unwrap().push(3);
        });
    }
    q.process();
    assert_eq!(*l.borrow(), vec![1, 3]);
    assert_eq!(*sl.lock().unwrap(), vec![2, 3]);
}

#[test]
fn per_thread_order_is_kept_across_threads() {
    let q = MainQueue::new();
    let l = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|t| {
            let (q, l, b) = (q.clone(), l.clone(), barrier.clone());
            std::thread::spawn(move || {
                b.wait();
                for i in 0..100 {
                    let l = l.clone();
                    q.post(move || l.lock().unwrap().push((t, i)));
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(q.process_all(), 400);
    let l = l.lock().unwrap();
    for t in 0..4 {
        let mine: Vec<_> = l.iter().filter(|(x, _)| *x == t).map(|(_, i)| *i).collect();
        assert_eq!(mine, (0..100).collect::<Vec<_>>());
    }
}

#[test]
fn wrapper_surrounds_each_batch() {
    let q = MainQueue::new();
    let l = log();
    let wl = l.clone();
    q.set_wrapper(Some(Arc::new(move |run: &mut dyn FnMut()| {
        wl.lock().unwrap().push(-1);
        run();
        wl.lock().unwrap().push(-2);
    })));
    let l2 = l.clone();
    q.post(move || l2.lock().unwrap().push(1));
    q.process();
    assert_eq!(*l.lock().unwrap(), vec![-1, 1, -2]);
}

#[test]
fn panic_in_task_does_not_lose_the_rest() {
    let q = MainQueue::new();
    let l = log();
    q.post(|| panic!("boom"));
    let l2 = l.clone();
    q.post(move || l2.lock().unwrap().push(1));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| q.process()));
    assert!(result.is_err());
    assert_eq!(*l.lock().unwrap(), vec![1]);
    // The queue is usable afterwards.
    q.post(|| {});
    assert_eq!(q.process(), 1);
}

#[test]
fn post_sync_from_other_thread_returns_value() {
    let q = MainQueue::new();
    q.bind_current_thread();
    let (tx, rx) = mpsc::channel();
    let q2 = q.clone();
    let main_id = std::thread::current().id();
    let h = std::thread::spawn(move || {
        let r = q2.post_sync(move || std::thread::current().id());
        tx.send(r).unwrap();
    });
    // Run the queue until the helper thread's task has been processed.
    let result = loop {
        q.process();
        if let Ok(r) = rx.try_recv() {
            break r;
        }
        std::thread::yield_now();
    };
    h.join().unwrap();
    assert_eq!(result, Some(main_id));
}

#[test]
fn post_sync_on_main_thread_panics_instead_of_deadlocking() {
    let q = MainQueue::new();
    q.bind_current_thread();
    let r = std::panic::catch_unwind(|| q.post_sync(|| 1));
    assert!(r.is_err());
}

struct DropFlag(mpsc::Sender<&'static str>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        let _ = self.0.send("dropped");
    }
}

#[test]
fn shutdown_drops_pending_unexecuted_and_ignores_later_posts() {
    let q = MainQueue::new();
    let wakes = counting_waker(&q);
    let ran = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel();
    for _ in 0..3 {
        let (flag, ran) = (DropFlag(tx.clone()), ran.clone());
        q.post(move || {
            let _keep = &flag;
            ran.fetch_add(1, Ordering::SeqCst);
        });
    }
    let local_flag = DropFlag(tx.clone());
    q.post_local(move || {
        let _keep = &local_flag;
        panic!("must not run");
    });
    q.shutdown();
    assert!(q.is_stopped());
    // All four captured values were released without running.
    for _ in 0..4 {
        assert_eq!(rx.try_recv(), Ok("dropped"));
    }
    let late = DropFlag(tx.clone());
    q.post(move || drop(late));
    assert_eq!(rx.try_recv(), Ok("dropped"), "late post dropped at once");
    assert_eq!(q.process(), 0);
    assert_eq!(ran.load(Ordering::SeqCst), 0);
    let before = wakes.load(Ordering::SeqCst);
    q.post(|| {});
    assert_eq!(wakes.load(Ordering::SeqCst), before, "waker released");
}

#[test]
fn shutdown_from_inside_a_task_drops_the_rest_of_the_batch() {
    let q = MainQueue::new();
    let ran = Arc::new(AtomicUsize::new(0));
    let q2 = q.clone();
    q.post(move || q2.shutdown());
    let r = ran.clone();
    q.post(move || {
        r.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(q.process(), 1);
    assert_eq!(ran.load(Ordering::SeqCst), 0);
}

#[test]
fn update_requests_subscription() {
    let hits = Rc::new(RefCell::new(0));
    let h = hits.clone();
    let sub = tg_runtime::on_main_update_requests(move || *h.borrow_mut() += 1);
    tg_runtime::fire_main_update_requests();
    tg_runtime::fire_main_update_requests();
    assert_eq!(*hits.borrow(), 2);
    drop(sub);
    tg_runtime::fire_main_update_requests();
    assert_eq!(*hits.borrow(), 2);
}
