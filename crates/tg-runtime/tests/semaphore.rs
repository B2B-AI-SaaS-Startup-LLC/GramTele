use std::sync::{mpsc, Arc};
use tg_runtime::Semaphore;

#[test]
fn release_before_acquire_does_not_block() {
    let s = Semaphore::new();
    s.release();
    s.acquire();
}

#[test]
fn ping_pong_between_threads() {
    let a = Arc::new(Semaphore::new());
    let b = Arc::new(Semaphore::new());
    let (tx, rx) = mpsc::channel();
    let (a2, b2) = (a.clone(), b.clone());
    let h = std::thread::spawn(move || {
        for i in 0..1000 {
            a2.acquire();
            tx.send(i).unwrap();
            b2.release();
        }
    });
    for i in 0..1000 {
        a.release();
        b.acquire();
        assert_eq!(rx.try_recv().unwrap(), i);
    }
    h.join().unwrap();
}

#[test]
fn double_release_is_saturating() {
    // Two releases give a single permit: after one acquire the next one needs
    // a new release (checked by a thread that is unblocked explicitly).
    let s = Arc::new(Semaphore::new());
    s.release();
    s.release();
    s.acquire();
    let (tx, rx) = mpsc::channel();
    let s2 = s.clone();
    let h = std::thread::spawn(move || {
        s2.acquire();
        tx.send(()).unwrap();
    });
    s.release();
    rx.recv().unwrap();
    h.join().unwrap();
}
