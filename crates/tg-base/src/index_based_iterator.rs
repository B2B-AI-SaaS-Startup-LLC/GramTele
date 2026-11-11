//! Port of `base/index_based_iterator.h`.
//!
//! A C++ iterator that survives reallocation of the container because it
//! stores an index. A Rust iterator cannot coexist with mutation of the
//! container, so the cursor here borrows nothing: it remembers the container
//! identity (to keep the original's `Expects(same container)`) and an
//! index, and every access takes the container explicitly. Only the
//! algorithm tests of the original use it.

use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Clone, Copy, Debug)]
pub struct IndexBasedIterator {
    container: usize,
    index: isize,
}

impl IndexBasedIterator {
    #[allow(clippy::ptr_arg)]
    fn id<T>(container: &Vec<T>) -> usize {
        // Address of the `Vec` object itself, stable while it is not moved.
        container as *const Vec<T> as usize
    }

    #[allow(clippy::ptr_arg)]
    pub fn new<T>(container: &Vec<T>, index: usize) -> Self {
        Self {
            container: Self::id(container),
            index: index as isize,
        }
    }

    /// `operator*`.
    pub fn get<'a, T>(&self, container: &'a [T]) -> &'a T {
        &container[self.index as usize]
    }

    pub fn get_mut<'a, T>(&self, container: &'a mut [T]) -> &'a mut T {
        &mut container[self.index as usize]
    }

    /// `base()`: the plain position.
    pub fn base(&self) -> usize {
        self.index as usize
    }

    /// `operator++(int)`: returns the old value.
    pub fn post_inc(&mut self) -> Self {
        let copy = *self;
        self.index += 1;
        copy
    }

    fn check(&self, other: &Self) {
        crate::expects!(self.container == other.container);
    }

    pub fn eq_checked(&self, other: &Self) -> bool {
        self.check(other);
        self.index == other.index
    }

    pub fn lt_checked(&self, other: &Self) -> bool {
        self.check(other);
        self.index < other.index
    }
}

impl PartialEq for IndexBasedIterator {
    fn eq(&self, other: &Self) -> bool {
        self.eq_checked(other)
    }
}

impl PartialOrd for IndexBasedIterator {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.check(other);
        self.index.partial_cmp(&other.index)
    }
}

impl AddAssign<isize> for IndexBasedIterator {
    fn add_assign(&mut self, offset: isize) {
        self.index += offset;
    }
}
impl SubAssign<isize> for IndexBasedIterator {
    fn sub_assign(&mut self, offset: isize) {
        self.index -= offset;
    }
}
impl Add<isize> for IndexBasedIterator {
    type Output = Self;
    fn add(mut self, offset: isize) -> Self {
        self += offset;
        self
    }
}
impl Sub<isize> for IndexBasedIterator {
    type Output = Self;
    fn sub(mut self, offset: isize) -> Self {
        self -= offset;
        self
    }
}
impl Sub for IndexBasedIterator {
    type Output = isize;
    fn sub(self, other: Self) -> isize {
        self.index - other.index
    }
}

#[allow(clippy::ptr_arg)]
/// `index_based_begin(container)`.
pub fn index_based_begin<T>(container: &Vec<T>) -> IndexBasedIterator {
    IndexBasedIterator::new(container, 0)
}

#[allow(clippy::ptr_arg)]
/// `index_based_end(container)`.
pub fn index_based_end<T>(container: &Vec<T>) -> IndexBasedIterator {
    IndexBasedIterator::new(container, container.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `std::remove_if` over index based iterators; the predicate may push
    /// to the vector (the end captured on entry stays valid).
    fn push_back_safe_remove_if(
        v: &mut Vec<i32>,
        mut predicate: impl FnMut(&mut Vec<i32>, i32) -> bool,
    ) {
        let begin = index_based_begin(v);
        let end = index_based_end(v);
        // std::remove_if
        let mut from = begin;
        let mut i = begin;
        while i != end {
            let value = *i.get(v);
            if !predicate(v, value) {
                let kept = *i.get(v);
                *from.get_mut(v) = kept;
                from += 1;
            }
            i += 1;
        }
        if from != end {
            let new_end = index_based_end(v);
            let mut end = end;
            if new_end != end {
                assert!(new_end > end);
                while end != new_end {
                    let value = *end.post_inc().get(v);
                    *from.post_inc().get_mut(v) = value;
                }
            }
            v.drain(from.base()..new_end.base());
        }
    }

    fn sample() -> Vec<i32> {
        vec![1, 2, 3, 4, 5, 4, 3, 2, 1]
    }

    #[test]
    fn allows_to_push_back_from_predicate() {
        let mut v = sample();
        push_back_safe_remove_if(&mut v, |v, value| {
            v.push(value);
            value % 2 == 1
        });
        assert_eq!(v, vec![2, 4, 4, 2, 1, 2, 3, 4, 5, 4, 3, 2, 1]);
    }

    #[test]
    fn allows_to_push_back_while_removing_all() {
        let mut v = sample();
        push_back_safe_remove_if(&mut v, |v, value| {
            if value == 5 {
                v.push(value);
            }
            true
        });
        assert_eq!(v, vec![5]);
    }

    #[allow(clippy::nonminimal_bool, clippy::eq_op)]
    #[test]
    fn arithmetic_and_comparison() {
        let v = vec![10, 20, 30];
        let b = index_based_begin(&v);
        let e = index_based_end(&v);
        assert_eq!(e - b, 3);
        assert_eq!(*(b + 2).get(&v), 30);
        assert_eq!(*(e - 1).get(&v), 30);
        assert!(b < e && e > b && b <= b && b >= b);
        assert_eq!((b + 3).base(), 3);
    }

    #[test]
    #[should_panic]
    fn comparing_different_containers_fails() {
        let a = vec![1];
        let b = vec![1];
        let _ = index_based_begin(&a) == index_based_begin(&b);
    }
}
