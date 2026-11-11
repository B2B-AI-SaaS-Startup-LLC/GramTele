//! Port of `base/never_freed_pointer.h`.
//!
//! A lazily created holder for global non-trivial values. Rust statics are
//! never dropped, so for a global use it inside a `Mutex`
//! (`static X: Mutex<NeverFreedPointer<T>>`) or `thread_local!` for the
//! main-thread-only globals of the client.

use std::ops::{Deref, DerefMut};

/// `base::NeverFreedPointer<T>`.
#[derive(Debug)]
pub struct NeverFreedPointer<T> {
    value: Option<Box<T>>,
}

impl<T> NeverFreedPointer<T> {
    pub const fn new() -> Self {
        Self { value: None }
    }

    /// `createIfNull(args...)`: `make` runs only if empty.
    pub fn create_if_null(&mut self, make: impl FnOnce() -> T) {
        if self.value.is_none() {
            self.value = Some(Box::new(make()));
        }
    }

    pub fn data(&self) -> Option<&T> {
        self.value.as_deref()
    }

    pub fn data_mut(&mut self) -> Option<&mut T> {
        self.value.as_deref_mut()
    }

    /// Takes the value out without dropping it.
    pub fn release(&mut self) -> Option<Box<T>> {
        self.value.take()
    }

    pub fn reset(&mut self, value: Option<Box<T>>) {
        self.value = value;
    }

    pub fn is_null(&self) -> bool {
        self.value.is_none()
    }

    pub fn clear(&mut self) {
        self.value = None;
    }
}

impl<T> Default for NeverFreedPointer<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// `operator*`: asserts the pointer is set.
impl<T> Deref for NeverFreedPointer<T> {
    type Target = T;
    fn deref(&self) -> &T {
        crate::tg_assert!(self.value.is_some());
        self.value.as_deref().expect("checked above")
    }
}

impl<T> DerefMut for NeverFreedPointer<T> {
    fn deref_mut(&mut self) -> &mut T {
        crate::tg_assert!(self.value.is_some());
        self.value.as_deref_mut().expect("checked above")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_once() {
        let mut p = NeverFreedPointer::<Vec<i32>>::new();
        assert!(p.is_null());
        p.create_if_null(|| vec![1]);
        p.create_if_null(|| vec![2]);
        assert_eq!(*p, vec![1]);
        p.push(3);
        assert_eq!(p.data().map(Vec::len), Some(2));
        let released = p.release().unwrap();
        assert_eq!(*released, vec![1, 3]);
        assert!(p.is_null());
        p.reset(Some(Box::new(vec![])));
        p.clear();
        assert!(p.data().is_none());
    }

    #[test]
    #[should_panic]
    fn deref_of_null_asserts() {
        let p = NeverFreedPointer::<i32>::new();
        let _ = *p;
    }
}
