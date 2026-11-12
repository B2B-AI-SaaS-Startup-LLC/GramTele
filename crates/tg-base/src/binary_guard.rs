//! Port of `base/binary_guard.h`: a pair of guards that see each other.
//!
//! `let guard = other.make_guard()` makes two linked guards; `alive()` is
//! true on both until either of them is dropped/reset. Used to stop an
//! asynchronous job when its owner goes away and vice versa. Thread-safe.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::weak_ptr::Alive;
use tg_runtime::AsGuard;

#[derive(Debug, Default)]
pub struct BinaryGuard {
    both_alive: Option<Arc<AtomicBool>>,
}

impl BinaryGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// True while this guard and its partner both exist.
    pub fn alive(&self) -> bool {
        self.both_alive
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }

    /// Breaks the previous link (if any) and returns the partner of a new
    /// one.
    pub fn make_guard(&mut self) -> BinaryGuard {
        self.reset();
        let flag = Arc::new(AtomicBool::new(true));
        self.both_alive = Some(flag.clone());
        BinaryGuard {
            both_alive: Some(flag),
        }
    }

    /// `guard = nullptr`.
    pub fn reset(&mut self) {
        if let Some(flag) = self.both_alive.take() {
            flag.store(false, Ordering::SeqCst);
        }
    }
}

impl Drop for BinaryGuard {
    fn drop(&mut self) {
        self.reset();
    }
}

impl Alive for BinaryGuard {
    fn alive(&self) -> bool {
        BinaryGuard::alive(self)
    }
}

/// Token of a [`BinaryGuard`] for `tg_runtime` guards: alive while both
/// sides of the pair exist (`guard_traits<base::binary_guard>::check`).
/// Unlike the original, which moves the guard into the closure, the token
/// shares the pair flag, so the owner keeps the guard itself.
#[derive(Clone, Debug, Default)]
pub struct BinaryGuardToken {
    both_alive: Option<Arc<AtomicBool>>,
}

impl Alive for BinaryGuardToken {
    fn alive(&self) -> bool {
        self.both_alive
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }
}

impl AsGuard for BinaryGuard {
    type Token = BinaryGuardToken;
    fn as_guard(&self) -> BinaryGuardToken {
        BinaryGuardToken {
            both_alive: self.both_alive.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_alive() {
        assert!(!BinaryGuard::new().alive());
    }

    #[test]
    fn partner_drop_kills_both_sides() {
        let mut mine = BinaryGuard::new();
        let theirs = mine.make_guard();
        assert!(mine.alive() && theirs.alive());
        drop(theirs);
        assert!(!mine.alive());
    }

    #[test]
    fn reset_kills_partner() {
        let mut mine = BinaryGuard::new();
        let theirs = mine.make_guard();
        mine.reset();
        assert!(!theirs.alive() && !mine.alive());
    }

    #[test]
    fn make_guard_breaks_previous_link() {
        let mut mine = BinaryGuard::new();
        let first = mine.make_guard();
        let second = mine.make_guard();
        assert!(!first.alive());
        assert!(second.alive() && mine.alive());
    }

    #[test]
    fn move_keeps_link() {
        let mut mine = BinaryGuard::new();
        let theirs = mine.make_guard();
        let moved = mine;
        assert!(theirs.alive() && moved.alive());
    }

    #[test]
    fn works_across_threads() {
        let mut mine = BinaryGuard::new();
        let theirs = mine.make_guard();
        let handle = std::thread::spawn(move || theirs.alive());
        assert!(handle.join().unwrap());
        assert!(!mine.alive());
    }
}
