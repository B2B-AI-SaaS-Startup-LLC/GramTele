use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use tg_runtime::{Executor, Queue};

#[test]
fn tasks_run_in_order_and_never_overlap() {
    let q = Queue::new();
    let busy = Arc::new(AtomicBool::new(false));
    let log = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    const N: usize = 500;
    for i in 0..N {
        let (busy, log, tx) = (busy.clone(), log.clone(), tx.clone());
        q.async_(move || {
            assert!(!busy.swap(true, Ordering::SeqCst), "overlap");
            log.lock().unwrap().push(i);
            busy.store(false, Ordering::SeqCst);
            if i == N - 1 {
                tx.send(()).unwrap();
            }
        });
    }
    rx.recv().unwrap();
    assert_eq!(*log.lock().unwrap(), (0..N).collect::<Vec<_>>());
}

#[test]
fn enqueueing_from_many_threads_keeps_per_thread_order() {
    let q = Arc::new(Queue::new());
    let log = Arc::new(Mutex::new(Vec::new()));
    let hs: Vec<_> = (0..4)
        .map(|t| {
            let (q, log) = (q.clone(), log.clone());
            std::thread::spawn(move || {
                for i in 0..100 {
                    let log = log.clone();
                    q.async_(move || log.lock().unwrap().push((t, i)));
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    q.sync(|| ()).unwrap();
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 400);
    for t in 0..4 {
        let mine: Vec<_> = log
            .iter()
            .filter(|(x, _)| *x == t)
            .map(|(_, i)| *i)
            .collect();
        assert_eq!(mine, (0..100).collect::<Vec<_>>());
    }
}

#[test]
fn sync_waits_for_earlier_tasks_and_returns_value() {
    let q = Queue::new();
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let done = Arc::new(AtomicBool::new(false));
    let d = done.clone();
    q.async_(move || {
        gate_rx.recv().unwrap();
        d.store(true, Ordering::SeqCst);
    });
    gate_tx.send(()).unwrap();
    let d = done.clone();
    let seen = q.sync(move || d.load(Ordering::SeqCst)).unwrap();
    assert!(seen, "sync ran after the earlier task");
}

#[test]
fn sync_from_same_queue_panics_instead_of_deadlocking() {
    let q = Arc::new(Queue::new());
    let q2 = q.clone();
    let result = q.sync(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| q2.sync(|| ()))).is_err()
    });
    assert_eq!(result, Some(true));
}

#[test]
fn panic_in_task_does_not_wedge_the_queue() {
    let q = Queue::new();
    q.async_(|| panic!("expected test panic"));
    assert_eq!(q.sync(|| 7), Some(7));
}

#[test]
fn sync_propagates_panic_to_caller() {
    let q = Queue::new();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        q.sync(|| -> i32 { panic!("expected test panic") })
    }));
    assert!(r.is_err());
}

#[test]
fn global_async_and_sync() {
    let (tx, rx) = mpsc::channel();
    tg_runtime::async_(move || tx.send(5).unwrap());
    assert_eq!(rx.recv().unwrap(), 5);
    assert_eq!(tg_runtime::sync(|| 6), Some(6));
}

struct DropSignal(mpsc::Sender<()>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

#[test]
fn executor_shutdown_drops_pending_queue_tasks() {
    let exec = Arc::new(Executor::new());
    let q = Arc::new(Queue::with_executor(exec.clone()));
    let ran = Arc::new(AtomicUsize::new(0));
    let (drop_tx, drop_rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let (q2, ran2) = (q.clone(), ran.clone());
    q.async_(move || {
        // Queue two more tasks: they belong to the next batch.
        for _ in 0..2 {
            let (sig, ran) = (DropSignal(drop_tx.clone()), ran2.clone());
            q2.async_(move || {
                let _keep = &sig;
                ran.fetch_add(1, Ordering::SeqCst);
            });
        }
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    started_rx.recv().unwrap();
    exec.shutdown();
    assert!(exec.is_stopped());
    release_tx.send(()).unwrap();
    // Both pending tasks are dropped without running.
    drop_rx.recv().unwrap();
    drop_rx.recv().unwrap();
    assert_eq!(ran.load(Ordering::SeqCst), 0);
    // Later calls are ignored and sync reports it.
    q.async_(|| panic!("must not run"));
    assert_eq!(q.sync(|| 1), None);
}
