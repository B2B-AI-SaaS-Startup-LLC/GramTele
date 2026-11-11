//! `crl::semaphore` (`crl_common_semaphore.h`): a saturating binary event.
//!
//! `release` sets the flag (several releases are the same as one), `acquire`
//! waits for the flag and clears it. This is the non-Apple variant of the
//! original; the dispatch (Apple) one counts releases.

use crate::util::lock;
use std::sync::{Condvar, Mutex};

#[derive(Default)]
pub struct Semaphore {
    signaled: Mutex<bool>,
    condvar: Condvar,
}

impl Semaphore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn acquire(&self) {
        let mut signaled = lock(&self.signaled);
        while !*signaled {
            signaled = self
                .condvar
                .wait(signaled)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *signaled = false;
    }

    pub fn release(&self) {
        let mut signaled = lock(&self.signaled);
        if !*signaled {
            *signaled = true;
            self.condvar.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_saturates() {
        let s = Semaphore::new();
        s.release();
        s.release();
        s.acquire();
        assert!(!*lock(&s.signaled));
    }
}
