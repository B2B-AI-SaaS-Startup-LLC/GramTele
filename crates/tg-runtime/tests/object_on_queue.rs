use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::ThreadId;
use tg_runtime::{ObjectOnQueue, ObjectOnThread, WeakOnQueue};

struct Recorder {
    log: Vec<i32>,
    busy: Arc<AtomicBool>,
}

#[test]
fn calls_arrive_in_order_one_at_a_time() {
    let busy = Arc::new(AtomicBool::new(false));
    let b = busy.clone();
    let obj = ObjectOnQueue::new(move || Recorder {
        log: Vec::new(),
        busy: b,
    });
    for i in 0..300 {
        obj.with(move |r| {
            assert!(!r.busy.swap(true, Ordering::SeqCst), "overlap");
            r.log.push(i);
            r.busy.store(false, Ordering::SeqCst);
        });
    }
    let log = obj.with_sync(|r| r.log.clone()).unwrap();
    assert_eq!(log, (0..300).collect::<Vec<_>>());
}

#[test]
fn constructed_on_the_queue_not_on_the_caller() {
    let caller = std::thread::current().id();
    let obj = ObjectOnQueue::new(std::thread::current);
    let built_on: ThreadId = obj.with_sync(|t| t.id()).unwrap();
    assert_ne!(built_on, caller);
}

struct Probe {
    log: Arc<Mutex<Vec<&'static str>>>,
    tx: mpsc::Sender<ThreadId>,
}
impl Drop for Probe {
    fn drop(&mut self) {
        self.log.lock().unwrap().push("drop");
        self.tx.send(std::thread::current().id()).unwrap();
    }
}

#[test]
fn destroyed_after_pending_tasks_and_off_the_caller_thread() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let l = log.clone();
    let obj = ObjectOnQueue::from_value(Probe { log: l, tx });
    obj.with(move |_| gate_rx.recv().unwrap());
    let l = log.clone();
    obj.with(move |_| l.lock().unwrap().push("task"));
    drop(obj);
    // The owner is gone but the queued tasks still run before destruction.
    gate_tx.send(()).unwrap();
    let dropped_on = rx.recv().unwrap();
    assert_ne!(dropped_on, std::thread::current().id());
    assert_eq!(*log.lock().unwrap(), vec!["task", "drop"]);
}

#[test]
fn weak_with_after_destruction_is_a_noop() {
    let (tx, rx) = mpsc::channel();
    let obj = ObjectOnQueue::new(|| 1);
    let weak = obj.weak();
    let tx2 = tx.clone();
    weak.with(move |_| tx2.send("first").unwrap());
    assert_eq!(rx.recv().unwrap(), "first");
    // Make sure destruction has completed before using the weak handle.
    let (dtx, drx) = mpsc::channel();
    struct Sig(mpsc::Sender<()>);
    impl Drop for Sig {
        fn drop(&mut self) {
            self.0.send(()).unwrap();
        }
    }
    let obj2 = ObjectOnQueue::from_value(Sig(dtx));
    let weak2 = obj2.weak();
    drop(obj2);
    drx.recv().unwrap();
    weak2.with(move |_| panic!("must not run"));
    assert!(weak2.with_sync(|_| 1).is_none());
    drop(obj);
    drop(weak);
}

#[test]
fn constructor_with_weak_handle() {
    struct SelfRef {
        weak: WeakOnQueue<SelfRef>,
        hits: Vec<i32>,
    }
    let obj = ObjectOnQueue::new_with_weak(|weak| SelfRef { weak, hits: vec![] });
    let (tx, rx) = mpsc::channel();
    obj.with(move |s| {
        s.hits.push(1);
        // Re-posting through the own weak handle runs later on the queue.
        s.weak.with(move |s| {
            s.hits.push(3);
            tx.send(s.hits.clone()).unwrap();
        });
        s.hits.push(2);
    });
    assert_eq!(rx.recv().unwrap(), vec![1, 2, 3]);
}

#[test]
fn runner_runs_on_the_queue() {
    let obj = ObjectOnQueue::new(|| ());
    let (tx, rx) = mpsc::channel();
    obj.weak().runner().run(move || tx.send(1).unwrap());
    assert_eq!(rx.recv().unwrap(), 1);
}

#[test]
fn destroy_moves_a_value_to_the_queue() {
    struct Sig(mpsc::Sender<ThreadId>);
    impl Drop for Sig {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let obj = ObjectOnQueue::new(|| ());
    let (tx, rx) = mpsc::channel();
    obj.destroy(Sig(tx));
    assert_ne!(rx.recv().unwrap(), std::thread::current().id());
}

#[test]
fn object_on_thread_always_uses_its_own_thread() {
    let caller = std::thread::current().id();
    let obj = ObjectOnThread::new(std::thread::current);
    let first = obj.with_sync(|t| t.id()).unwrap();
    assert_ne!(first, caller);
    let ids = Arc::new(Mutex::new(Vec::new()));
    for _ in 0..50 {
        let ids = ids.clone();
        obj.with(move |_| ids.lock().unwrap().push(std::thread::current().id()));
    }
    obj.with_sync(|_| ()).unwrap();
    assert!(ids.lock().unwrap().iter().all(|id| *id == first));
}

#[test]
fn object_on_thread_is_destroyed_on_its_thread() {
    struct Sig(mpsc::Sender<ThreadId>);
    impl Drop for Sig {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let (tx, rx) = mpsc::channel();
    let obj = ObjectOnThread::from_value(Sig(tx));
    let thread = obj.with_sync(|_| std::thread::current().id()).unwrap();
    drop(obj);
    assert_eq!(rx.recv().unwrap(), thread);
}
