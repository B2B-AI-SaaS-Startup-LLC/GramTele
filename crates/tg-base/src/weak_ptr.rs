//! Port of `base/weak_ptr.h`, redesigned for the Rust object model.
//!
//! In C++ `base::weak_ptr<T>` is a thread-safe weak pointer to an object
//! deriving `has_weak_ptr`, and `weak.get()` returns `T*`. PORTING.md makes
//! the object graph a set of stores addressed by typed ids, so a weak
//! reference cannot (and must not) hand out a reference to the object. What
//! the client actually needs from `weak_ptr` is *liveness*: "was the object
//! destroyed or invalidated?". That is what this module provides:
//!
//! - [`Alive`]: the interface `tg-runtime`'s `crl::guard` can rely on
//!   (`fn alive(&self) -> bool`); implemented for [`WeakPtr`], `rc::Weak`,
//!   `sync::Weak` and `BinaryGuard`.
//! - [`HasWeakPtr`]: an embeddable field that owns the liveness flag. When
//!   the owner is dropped (or `invalidate_weak_ptrs` is called) every
//!   [`WeakPtr`] made from it turns dead. Cloning gives a fresh, unrelated
//!   flag, as in the original. A bare `HasWeakPtr` field is also the
//!   cancellation token of `invalidate_weak_ptrs(&_guard)`.
//! - [`WeakPtr<T>`]: typed, `Send + Sync`, cheap clone, comparable and
//!   hashable by identity of the flag (usable as a map key). It has no
//!   `get()`/`*`/`->`: after `alive()` the object is reached through the
//!   store with its id (`store.get(id)`), which lives on the main thread.
//!
//! `WeakPtr` is a map key by identity on purpose; clippy's
//! `mutable_key_type` fires for such keys and should be allowed at the use site.
//!
//! Limitations: a `WeakPtr` never gives access to the object; it does not
//! prove the object is still *in a given store* (stores must invalidate on
//! removal by dropping the `HasWeakPtr` inside the object, or calling
//! `invalidate_weak_ptrs`). Objects owned by `Rc`/`Arc` can use
//! `std::rc::Weak`/`sync::Weak` directly.

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// "Is the thing this handle refers to still alive?".
pub trait Alive {
    fn alive(&self) -> bool;
}

impl<T: ?Sized> Alive for std::rc::Weak<T> {
    fn alive(&self) -> bool {
        self.strong_count() > 0
    }
}

impl<T: ?Sized> Alive for std::sync::Weak<T> {
    fn alive(&self) -> bool {
        self.strong_count() > 0
    }
}

#[derive(Debug)]
struct Tracker {
    alive: AtomicBool,
}

/// `base::has_weak_ptr`.
#[derive(Debug, Default)]
pub struct HasWeakPtr {
    tracker: Mutex<Option<Arc<Tracker>>>,
}

impl Clone for HasWeakPtr {
    /// Copies do not share weak pointers.
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl HasWeakPtr {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Arc<Tracker>>> {
        self.tracker.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn tracker(&self) -> Arc<Tracker> {
        self.lock()
            .get_or_insert_with(|| {
                Arc::new(Tracker {
                    alive: AtomicBool::new(true),
                })
            })
            .clone()
    }

    fn invalidate(&self) {
        if let Some(tracker) = self.lock().take() {
            tracker.alive.store(false, Ordering::SeqCst);
        }
    }

    /// `invalidate_weak_ptrs(object)`: all existing weak pointers die,
    /// pointers made afterwards are alive again.
    pub fn invalidate_weak_ptrs(&self) {
        self.invalidate();
    }

    /// `weak_ptrs_count(object)`: number of live `WeakPtr` clones.
    pub fn weak_ptrs_count(&self) -> usize {
        // The owner holds one reference itself.
        self.lock()
            .as_ref()
            .map_or(0, |tracker| Arc::strong_count(tracker) - 1)
    }
}

impl Drop for HasWeakPtr {
    fn drop(&mut self) {
        self.invalidate();
    }
}

/// Implemented by types that embed a [`HasWeakPtr`] (the C++ base class).
pub trait AsHasWeakPtr {
    fn has_weak_ptr(&self) -> &HasWeakPtr;
}

impl AsHasWeakPtr for HasWeakPtr {
    fn has_weak_ptr(&self) -> &HasWeakPtr {
        self
    }
}

/// `base::make_weak(object)`.
pub fn make_weak<T: AsHasWeakPtr + ?Sized>(object: &T) -> WeakPtr<T> {
    WeakPtr {
        tracker: Some(object.has_weak_ptr().tracker()),
        _marker: PhantomData,
    }
}

/// `base::weak_ptr<T>`, liveness only (see the module docs).
pub struct WeakPtr<T: ?Sized> {
    tracker: Option<Arc<Tracker>>,
    _marker: PhantomData<fn() -> Box<T>>,
}

impl<T: ?Sized> WeakPtr<T> {
    /// Null pointer (`weak_ptr()`).
    pub fn new() -> Self {
        Self {
            tracker: None,
            _marker: PhantomData,
        }
    }

    /// Never set (`null()` in the original), as opposed to dead.
    pub fn is_null(&self) -> bool {
        self.tracker.is_none()
    }

    /// Null or dead (`empty()`; `!weak` and `weak == nullptr`).
    pub fn is_empty(&self) -> bool {
        !self.alive()
    }

    pub fn reset(&mut self) {
        self.tracker = None;
    }

    /// Re-types the pointer, the `weak_ptr<Derived>` to `weak_ptr<Base>`
    /// conversion; only liveness is shared so it is always sound.
    pub fn cast<U: ?Sized>(self) -> WeakPtr<U> {
        WeakPtr {
            tracker: self.tracker,
            _marker: PhantomData,
        }
    }

    fn address(&self) -> usize {
        self.tracker
            .as_ref()
            .map_or(0, |tracker| Arc::as_ptr(tracker) as usize)
    }
}

impl<T: ?Sized> Alive for WeakPtr<T> {
    fn alive(&self) -> bool {
        self.tracker
            .as_ref()
            .is_some_and(|tracker| tracker.alive.load(Ordering::SeqCst))
    }
}

impl<T: ?Sized> Default for WeakPtr<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized> Clone for WeakPtr<T> {
    fn clone(&self) -> Self {
        Self {
            tracker: self.tracker.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: ?Sized> std::fmt::Debug for WeakPtr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WeakPtr(alive: {})", self.alive())
    }
}

// Identity comparison of the flag, like the defaulted `<=>` of the original
// (note: unlike `== nullptr`, which compares `get()`).
impl<T: ?Sized> PartialEq for WeakPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.address() == other.address()
    }
}
impl<T: ?Sized> Eq for WeakPtr<T> {}
impl<T: ?Sized> PartialOrd for WeakPtr<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T: ?Sized> Ord for WeakPtr<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.address().cmp(&other.address())
    }
}
impl<T: ?Sized> Hash for WeakPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Object {
        weak: HasWeakPtr,
    }
    impl AsHasWeakPtr for Object {
        fn has_weak_ptr(&self) -> &HasWeakPtr {
            &self.weak
        }
    }

    #[test]
    fn dies_with_owner() {
        let object = Object::default();
        let weak = make_weak(&object);
        let copy = weak.clone();
        assert!(weak.alive() && copy.alive() && !weak.is_empty());
        assert_eq!(object.weak.weak_ptrs_count(), 2);
        drop(object);
        assert!(!weak.alive() && copy.is_empty());
        assert!(!weak.is_null());
    }

    #[test]
    fn null_is_not_alive() {
        let weak = WeakPtr::<Object>::new();
        assert!(weak.is_null() && weak.is_empty() && !weak.alive());
    }

    #[allow(clippy::mutable_key_type)]
    #[test]
    fn same_object_same_identity() {
        let a = Object::default();
        let b = Object::default();
        assert_eq!(make_weak(&a), make_weak(&a));
        assert_ne!(make_weak(&a), make_weak(&b));
        let mut map = std::collections::HashMap::new();
        map.insert(make_weak(&a), 1);
        assert_eq!(map.get(&make_weak(&a)), Some(&1));
        assert_eq!(map.get(&make_weak(&b)), None);
    }

    #[test]
    fn invalidate_kills_old_pointers_only() {
        let guard = HasWeakPtr::new();
        let old = make_weak(&guard);
        guard.invalidate_weak_ptrs();
        assert!(!old.alive());
        assert_eq!(guard.weak_ptrs_count(), 0);
        let fresh = make_weak(&guard);
        assert!(fresh.alive() && !old.alive());
        assert_ne!(old, fresh);
        assert_eq!(guard.weak_ptrs_count(), 1);
        drop(fresh);
        assert_eq!(guard.weak_ptrs_count(), 0);
    }

    #[test]
    fn clone_of_owner_has_own_flag() {
        let a = Object::default();
        let weak = make_weak(&a);
        let b = Object {
            weak: a.weak.clone(),
        };
        drop(a);
        assert!(!weak.alive());
        assert!(make_weak(&b).alive());
    }

    #[test]
    fn cast_keeps_liveness() {
        let guard = HasWeakPtr::new();
        let typed = make_weak(&guard);
        let erased: WeakPtr<dyn std::any::Any> = typed.clone().cast();
        assert!(erased.alive());
        drop(guard);
        assert!(!erased.alive() && !typed.alive());
    }

    #[test]
    fn checked_from_other_thread() {
        let object = Object::default();
        let weak = make_weak(&object);
        let handle = std::thread::spawn({
            let weak = weak.clone();
            move || weak.alive()
        });
        assert!(handle.join().unwrap());
        drop(object);
        assert!(!std::thread::spawn(move || weak.alive()).join().unwrap());
    }

    #[test]
    fn std_weak_is_alive() {
        let rc = std::rc::Rc::new(1);
        let weak = std::rc::Rc::downgrade(&rc);
        assert!(weak.alive());
        drop(rc);
        assert!(!weak.alive());
        let arc = Arc::new(1);
        let weak = Arc::downgrade(&arc);
        assert!(Alive::alive(&weak));
        drop(arc);
        assert!(!Alive::alive(&weak));
    }
}
