//! Minimal built-in loop for tests and console programs: spins the main queue
//! on the current thread.

use crate::crl_on_main::{main_queue, MainQueue};
use crate::util::lock;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct State {
    woken: bool,
    quit: bool,
}

#[derive(Default)]
struct Signal {
    state: Mutex<State>,
    condvar: Condvar,
}

impl Signal {
    fn wake(&self) {
        lock(&self.state).woken = true;
        self.condvar.notify_all();
    }
}

/// Thread-safe handle that stops a running [`MainLoop`].
#[derive(Clone)]
pub struct LoopQuit {
    signal: Arc<Signal>,
}

impl LoopQuit {
    /// `run()` returns after the current batch. Sticky until `run` sees it.
    pub fn quit(&self) {
        lock(&self.signal.state).quit = true;
        self.signal.condvar.notify_all();
    }
}

pub struct MainLoop {
    queue: MainQueue,
    signal: Arc<Signal>,
}

impl MainLoop {
    /// Installs itself as the waker of `queue` and binds the current thread.
    pub fn new(queue: &MainQueue) -> Self {
        let signal = Arc::new(Signal::default());
        let waker = Arc::clone(&signal);
        queue.bind_current_thread();
        queue.set_waker(Some(Arc::new(move || waker.wake())));
        MainLoop {
            queue: queue.clone(),
            signal,
        }
    }

    /// A loop for the global queue.
    pub fn global() -> Self {
        Self::new(main_queue())
    }

    pub fn quitter(&self) -> LoopQuit {
        LoopQuit {
            signal: Arc::clone(&self.signal),
        }
    }

    /// Processes the backlog, then sleeps until woken, until `quit`.
    pub fn run(&self) {
        loop {
            self.queue.process();
            let mut state = lock(&self.signal.state);
            while !state.woken && !state.quit {
                state = self
                    .signal
                    .condvar
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            if state.quit {
                state.quit = false;
                return;
            }
            state.woken = false;
        }
    }
}

impl Drop for MainLoop {
    fn drop(&mut self) {
        self.queue.set_waker(None);
    }
}
