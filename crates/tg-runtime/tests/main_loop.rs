use std::sync::mpsc;
use tg_runtime::{MainLoop, MainQueue};

#[test]
fn background_work_comes_back_on_the_loop_thread() {
    let q = MainQueue::new();
    let lp = MainLoop::new(&q);
    let quit = lp.quitter();
    let loop_thread = std::thread::current().id();
    let (tx, rx) = mpsc::channel();
    let q2 = q.clone();
    let worker = std::thread::spawn(move || {
        for i in 0..3 {
            let tx = tx.clone();
            q2.post(move || tx.send((i, std::thread::current().id())).unwrap());
        }
        q2.post(move || quit.quit());
    });
    lp.run();
    worker.join().unwrap();
    for i in 0..3 {
        assert_eq!(rx.try_recv().unwrap(), (i, loop_thread));
    }
}

#[test]
fn tasks_posted_by_tasks_are_run_by_the_loop() {
    let q = MainQueue::new();
    let lp = MainLoop::new(&q);
    let quit = lp.quitter();
    let (tx, rx) = mpsc::channel();
    let q2 = q.clone();
    q.post(move || {
        tx.send(1).unwrap();
        let tx2 = tx.clone();
        q2.post(move || {
            tx2.send(2).unwrap();
            quit.quit();
        });
    });
    lp.run();
    assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn quit_before_run_returns_after_first_pass() {
    let q = MainQueue::new();
    let lp = MainLoop::new(&q);
    lp.quitter().quit();
    let (tx, rx) = mpsc::channel();
    q.post(move || tx.send(()).unwrap());
    lp.run();
    assert!(rx.try_recv().is_ok());
}
