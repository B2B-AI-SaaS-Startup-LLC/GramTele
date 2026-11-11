//! Task list with batch processing (`crl/common/crl_common_list.*`).
//!
//! The original is a lock-free intrusive stack that is reversed on `process`.
//! Here a mutex-protected deque gives the same observable behaviour:
//!
//! * `push` reports whether the list was idle, so exactly one wake-up is
//!   requested per idle-to-busy transition;
//! * `run_batch` takes everything queued so far and runs it in FIFO order;
//!   tasks pushed meanwhile belong to the next batch.

use crate::util::{lock, Panic, Task};
use std::collections::VecDeque;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

struct State {
    tasks: VecDeque<Task>,
    /// A wake-up was requested and the batch has not finished yet.
    scheduled: bool,
}

pub(crate) enum Push {
    /// The list was idle: the caller must request a wake-up.
    Wake,
    /// A wake-up is already pending or a batch is running.
    Queued,
    /// The list is stopped, the task was dropped.
    Stopped,
}

pub(crate) struct Batch {
    pub ran: usize,
    /// First panic of the batch (the other tasks still ran).
    pub panic: Option<Panic>,
    /// Tasks arrived during the batch: another wake-up is needed.
    pub rewake: bool,
}

pub(crate) struct List {
    state: Mutex<State>,
    stopped: AtomicBool,
}

impl List {
    pub fn new() -> Self {
        List {
            state: Mutex::new(State {
                tasks: VecDeque::new(),
                scheduled: false,
            }),
            stopped: AtomicBool::new(false),
        }
    }

    pub fn push(&self, task: Task) -> Push {
        let mut state = lock(&self.state);
        if self.stopped.load(Ordering::Acquire) {
            drop(state);
            // Dropped outside the lock: its destructor may use this list.
            drop(task);
            return Push::Stopped;
        }
        state.tasks.push_back(task);
        if state.scheduled {
            Push::Queued
        } else {
            state.scheduled = true;
            Push::Wake
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    /// True if tasks wait for a batch.
    pub fn has_pending(&self) -> bool {
        !lock(&self.state).tasks.is_empty()
    }

    /// Drops all pending tasks unexecuted and rejects new ones.
    pub fn stop(&self) {
        let dropped = {
            let mut state = lock(&self.state);
            self.stopped.store(true, Ordering::Release);
            state.scheduled = false;
            std::mem::take(&mut state.tasks)
        };
        drop(dropped);
    }

    pub fn run_batch(&self) -> Batch {
        let tasks = std::mem::take(&mut lock(&self.state).tasks);
        let mut ran = 0;
        let mut panic = None;
        for task in tasks {
            if self.is_stopped() {
                // Stopped from inside a task: the rest of the batch is
                // dropped, like the `_alive` check of the original.
                continue;
            }
            ran += 1;
            if let Err(payload) = catch_unwind(AssertUnwindSafe(task)) {
                panic.get_or_insert(payload);
            }
        }
        let mut state = lock(&self.state);
        let rewake = !state.tasks.is_empty();
        if !rewake {
            state.scheduled = false;
        }
        Batch { ran, panic, rewake }
    }
}
