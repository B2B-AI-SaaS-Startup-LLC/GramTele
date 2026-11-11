//! Port of `base/thread_safe_wrap.h`.

use std::collections::VecDeque;
use std::sync::Mutex;

/// `base::thread_safe_wrap<T>`: a value reachable only under a lock.
#[derive(Debug, Default)]
pub struct ThreadSafeWrap<T> {
    value: Mutex<T>,
}

impl<T> ThreadSafeWrap<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Mutex::new(value),
        }
    }

    /// `with(callback)`: runs `callback` with the lock held.
    pub fn with<R>(&self, callback: impl FnOnce(&mut T) -> R) -> R {
        let mut guard = self.value.lock().unwrap_or_else(|e| e.into_inner());
        callback(&mut guard)
    }
}

/// `base::thread_safe_queue<T>`: producers `emplace`, a consumer `take`s
/// everything accumulated so far.
#[derive(Debug, Default)]
pub struct ThreadSafeQueue<T> {
    wrap: ThreadSafeWrap<VecDeque<T>>,
}

impl<T> ThreadSafeQueue<T> {
    pub fn new() -> Self {
        Self {
            wrap: ThreadSafeWrap::new(VecDeque::new()),
        }
    }

    pub fn emplace(&self, value: T) {
        self.wrap.with(|queue| queue.push_back(value));
    }

    pub fn take(&self) -> VecDeque<T> {
        self.wrap.with(std::mem::take)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn wrap_mutates_under_lock() {
        let wrap = ThreadSafeWrap::new(1);
        assert_eq!(
            wrap.with(|v| {
                *v += 1;
                *v
            }),
            2
        );
    }

    #[test]
    fn queue_across_threads() {
        let queue = Arc::new(ThreadSafeQueue::new());
        let handles: Vec<_> = (0..4)
            .map(|t| {
                let queue = queue.clone();
                std::thread::spawn(move || {
                    for i in 0..100 {
                        queue.emplace(t * 100 + i);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let mut all: Vec<_> = queue.take().into_iter().collect();
        all.sort();
        assert_eq!(all, (0..400).collect::<Vec<_>>());
        assert!(queue.take().is_empty());
    }
}
