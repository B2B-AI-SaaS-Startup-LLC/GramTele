//! Port of `base/flat_set.h`: sorted-vector sets.
//!
//! `FlatMultiSet` allows repeated elements (kept in insertion order among
//! equals), `FlatSet` does not. Positions are `usize` instead of C++
//! iterators, see the notes in [`crate::flat_map`].

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::iter::FromIterator;
use std::ops::Range;

use crate::flat_map::ordering_by;
pub use crate::flat_map::{Compare, Greater, Less};

/// `base::flat_multi_set`.
pub struct FlatMultiSet<T, C = Less> {
    data: Vec<T>,
    cmp: C,
}

impl<T, C: Default> Default for FlatMultiSet<T, C> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            cmp: C::default(),
        }
    }
}

impl<T: Clone, C: Clone> Clone for FlatMultiSet<T, C> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            cmp: self.cmp.clone(),
        }
    }
}

impl<T: fmt::Debug, C> fmt::Debug for FlatMultiSet<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.data.iter()).finish()
    }
}

impl<T: PartialEq, C> PartialEq for FlatMultiSet<T, C> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<T: Eq, C> Eq for FlatMultiSet<T, C> {}

impl<T: PartialOrd, C> PartialOrd for FlatMultiSet<T, C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.data.partial_cmp(&other.data)
    }
}

impl<T, C> FlatMultiSet<T, C> {
    pub fn with_compare(cmp: C) -> Self {
        Self {
            data: Vec::new(),
            cmp,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    /// The sorted elements; `begin() + n` of the original is `as_slice()[n]`.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    pub fn front(&self) -> Option<&T> {
        self.data.first()
    }

    pub fn back(&self) -> Option<&T> {
        self.data.last()
    }

    pub fn get_at(&self, position: usize) -> Option<&T> {
        self.data.get(position)
    }

    /// `erase(iterator)`: returns the position of the next element.
    pub fn erase_at(&mut self, position: usize) -> usize {
        self.data.remove(position);
        position
    }

    /// `erase(first, last)`: returns the position after the erased range.
    pub fn erase_range(&mut self, range: Range<usize>) -> usize {
        let start = range.start;
        self.data.drain(range);
        start
    }

    pub fn lower_bound<Q>(&self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.data
            .partition_point(|item| self.cmp.less(item.borrow(), value))
    }

    pub fn upper_bound<Q>(&self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.data
            .partition_point(|item| !self.cmp.less(value, item.borrow()))
    }

    pub fn equal_range<Q>(&self, value: &Q) -> Range<usize>
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.lower_bound(value)..self.upper_bound(value)
    }

    /// Position of the first equal element, `None` where the original
    /// returns `end()`.
    pub fn find_first<Q>(&self, value: &Q) -> Option<usize>
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        let at = self.lower_bound(value);
        match self.data.get(at) {
            Some(item) if !self.cmp.less(value, item.borrow()) => Some(at),
            _ => None,
        }
    }

    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.find_first(value).is_some()
    }

    pub fn count<Q>(&self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.equal_range(value).len()
    }

    pub fn remove_one<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        match self.find_first(value) {
            Some(at) => {
                self.data.remove(at);
                true
            }
            None => false,
        }
    }

    pub fn remove_all<Q>(&mut self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        let range = self.equal_range(value);
        let count = range.len();
        self.data.drain(range);
        count
    }

    /// `erase(value)`: same as `remove_all`.
    pub fn erase<Q>(&mut self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.remove_all(value)
    }
}

impl<T, C: Compare<T>> FlatMultiSet<T, C> {
    fn sort_data(&mut self) {
        let cmp = &self.cmp;
        self.data.sort_by(|a, b| ordering_by(cmp, a, b));
    }

    pub fn from_iter_with_compare<I: IntoIterator<Item = T>>(iter: I, cmp: C) -> Self {
        let mut result = Self {
            data: iter.into_iter().collect(),
            cmp,
        };
        result.sort_data();
        result
    }

    /// Inserts after all equal elements; returns the position.
    pub fn insert(&mut self, value: T) -> usize {
        if self
            .data
            .last()
            .is_none_or(|back| !self.cmp.less(&value, back))
        {
            self.data.push(value);
            return self.data.len() - 1;
        }
        let at = self.upper_bound::<T>(&value);
        self.data.insert(at, value);
        at
    }

    /// Changes the element at `which` in place and restores the order.
    ///
    /// The re-sorting walk is a literal copy of the original: it swaps the
    /// changed element with its neighbours while keeping `which` fixed, so it
    /// is only correct when the element moves by at most one position.
    pub fn modify<R>(&mut self, which: usize, action: impl FnOnce(&mut T) -> R) -> R {
        let result = action(&mut self.data[which]);
        let cmp = &self.cmp;
        let data = &mut self.data;
        let mut i = which + 1;
        while i < data.len() {
            if cmp.less(&data[i], &data[which]) {
                data.swap(i, which);
                i += 1;
            } else {
                break;
            }
        }
        let mut i = which;
        while i != 0 {
            i -= 1;
            if cmp.less(&data[which], &data[i]) {
                data.swap(i, which);
            } else {
                break;
            }
        }
        result
    }

    /// Adds all elements and re-sorts.
    pub fn merge<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.data.extend(iter);
        self.sort_data();
    }
}

impl<T, C: Compare<T> + Default> FromIterator<T> for FlatMultiSet<T, C> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_iter_with_compare(iter, C::default())
    }
}

impl<T, C: Compare<T> + Default, const N: usize> From<[T; N]> for FlatMultiSet<T, C> {
    fn from(value: [T; N]) -> Self {
        value.into_iter().collect()
    }
}

impl<T, C: Compare<T>> Extend<T> for FlatMultiSet<T, C> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value);
        }
    }
}

impl<T, C> IntoIterator for FlatMultiSet<T, C> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a, T, C> IntoIterator for &'a FlatMultiSet<T, C> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

/// `base::flat_set`.
pub struct FlatSet<T, C = Less> {
    inner: FlatMultiSet<T, C>,
}

impl<T, C: Default> Default for FlatSet<T, C> {
    fn default() -> Self {
        Self {
            inner: FlatMultiSet::default(),
        }
    }
}

impl<T: Clone, C: Clone> Clone for FlatSet<T, C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: fmt::Debug, C> fmt::Debug for FlatSet<T, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: PartialEq, C> PartialEq for FlatSet<T, C> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T: Eq, C> Eq for FlatSet<T, C> {}

impl<T: PartialOrd, C> PartialOrd for FlatSet<T, C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl<T, C> FlatSet<T, C> {
    pub fn with_compare(cmp: C) -> Self {
        Self {
            inner: FlatMultiSet::with_compare(cmp),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional);
    }

    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit();
    }

    pub fn as_slice(&self) -> &[T] {
        self.inner.as_slice()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.inner.iter()
    }

    pub fn front(&self) -> Option<&T> {
        self.inner.front()
    }

    pub fn back(&self) -> Option<&T> {
        self.inner.back()
    }

    pub fn get_at(&self, position: usize) -> Option<&T> {
        self.inner.get_at(position)
    }

    pub fn erase_at(&mut self, position: usize) -> usize {
        self.inner.erase_at(position)
    }

    pub fn erase_range(&mut self, range: Range<usize>) -> usize {
        self.inner.erase_range(range)
    }

    pub fn lower_bound<Q>(&self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.lower_bound(value)
    }

    pub fn upper_bound<Q>(&self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.upper_bound(value)
    }

    pub fn equal_range<Q>(&self, value: &Q) -> Range<usize>
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.equal_range(value)
    }

    /// `find(value)`: position, `None` where the original returns `end()`.
    pub fn find<Q>(&self, value: &Q) -> Option<usize>
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.find_first(value)
    }

    /// Reference to the stored element equal to `value`.
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.get_at(self.find(value)?)
    }

    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.contains(value)
    }

    /// `remove(value)`: true if an element was removed.
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.remove_one(value)
    }

    /// `erase(value)`: number of removed elements (0 or 1).
    pub fn erase<Q>(&mut self, value: &Q) -> usize
    where
        T: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.remove_all(value)
    }
}

impl<T, C: Compare<T>> FlatSet<T, C> {
    fn dedup(&mut self) {
        let cmp = &self.inner.cmp;
        self.inner
            .data
            .dedup_by(|later, earlier| !cmp.less(earlier, later) && !cmp.less(later, earlier));
    }

    pub fn from_iter_with_compare<I: IntoIterator<Item = T>>(iter: I, cmp: C) -> Self {
        let mut result = Self {
            inner: FlatMultiSet::from_iter_with_compare(iter, cmp),
        };
        result.dedup();
        result
    }

    /// Inserts if absent. Returns the position and whether it was inserted.
    pub fn insert(&mut self, value: T) -> (usize, bool) {
        let data = &mut self.inner.data;
        let cmp = &self.inner.cmp;
        if data.last().is_none_or(|back| cmp.less(back, &value)) {
            data.push(value);
            return (data.len() - 1, true);
        }
        let at = data.partition_point(|item| cmp.less(item, &value));
        if cmp.less(&value, &data[at]) {
            data.insert(at, value);
            (at, true)
        } else {
            (at, false)
        }
    }

    /// `emplace(args...)`: same as `insert`.
    pub fn emplace(&mut self, value: T) -> (usize, bool) {
        self.insert(value)
    }

    /// Changes the element at `which` in place and restores the order;
    /// if it became equal to a neighbour it is erased.
    ///
    /// Literal copy of the original walk (see [`FlatMultiSet::modify`]):
    /// correct only when the element moves by at most one position.
    pub fn modify(&mut self, which: usize, action: impl FnOnce(&mut T)) {
        action(&mut self.inner.data[which]);
        let cmp = &self.inner.cmp;
        let data = &mut self.inner.data;
        let mut i = which + 1;
        while i < data.len() {
            if cmp.less(&data[i], &data[which]) {
                data.swap(i, which);
                i += 1;
            } else if !cmp.less(&data[which], &data[i]) {
                data.remove(which);
                return;
            } else {
                break;
            }
        }
        let mut i = which;
        while i != 0 {
            i -= 1;
            if cmp.less(&data[which], &data[i]) {
                data.swap(i, which);
            } else if !cmp.less(&data[i], &data[which]) {
                data.remove(which);
                return;
            } else {
                break;
            }
        }
    }

    /// Adds all elements, re-sorts and drops duplicates.
    pub fn merge<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.inner.merge(iter);
        self.dedup();
    }
}

impl<T, C: Compare<T> + Default> FromIterator<T> for FlatSet<T, C> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_iter_with_compare(iter, C::default())
    }
}

impl<T, C: Compare<T> + Default, const N: usize> From<[T; N]> for FlatSet<T, C> {
    fn from(value: [T; N]) -> Self {
        value.into_iter().collect()
    }
}

impl<T, C: Compare<T>> Extend<T> for FlatSet<T, C> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.insert(value);
        }
    }
}

impl<T, C> IntoIterator for FlatSet<T, C> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a, T, C> IntoIterator for &'a FlatSet<T, C> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct IntWrap {
        value: i32,
    }
    #[derive(Default, Clone, Copy)]
    struct IntWrapComparator;
    impl Compare<IntWrap> for IntWrapComparator {
        fn less(&self, a: &IntWrap, b: &IntWrap) -> bool {
            a.value < b.value
        }
    }
    impl Compare<i32> for IntWrapComparator {
        fn less(&self, a: &i32, b: &i32) -> bool {
            a < b
        }
    }
    impl Borrow<i32> for IntWrap {
        fn borrow(&self) -> &i32 {
            &self.value
        }
    }

    #[test]
    fn flat_sets_should_keep_items_sorted() {
        let mut v = FlatSet::<i32>::default();
        v.insert(0);
        v.insert(5);
        v.insert(4);
        v.insert(2);
        assert!(v.contains(&4));
        assert_eq!(v.len(), 4);
        assert!(v.as_slice().windows(2).all(|p| p[0] < p[1]));
        // adding item puts it in the right position
        v.insert(3);
        assert_eq!(v.len(), 5);
        assert!(v.find(&3).is_some());
        assert_eq!(v.as_slice(), &[0, 2, 3, 4, 5]);
    }

    #[test]
    fn flat_sets_with_custom_comparators() {
        let mut v = FlatSet::<IntWrap, IntWrapComparator>::default();
        for value in [0, 5, 4, 2] {
            v.insert(IntWrap { value });
        }
        assert!(v.find(&4).is_some());
        assert_eq!(v.len(), 4);
        let sorted = |v: &FlatSet<IntWrap, IntWrapComparator>| {
            v.as_slice().windows(2).all(|p| p[0].value < p[1].value)
        };
        assert!(sorted(&v));
        v.insert(IntWrap { value: 3 });
        assert_eq!(v.len(), 5);
        assert!(v.find(&3).is_some());
        assert!(sorted(&v));
    }

    #[test]
    fn insert_reports_duplicates() {
        let mut v = FlatSet::<i32>::default();
        assert_eq!(v.insert(2), (0, true));
        assert_eq!(v.insert(1), (0, true));
        assert_eq!(v.insert(2), (1, false));
        assert_eq!(v.emplace(3), (2, true));
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn remove_erase_and_ranges() {
        let mut v: FlatSet<i32> = (0..10).collect();
        assert!(v.remove(&3));
        assert!(!v.remove(&3));
        assert_eq!(v.erase(&4), 1);
        assert_eq!(v.erase_range(0..2), 0);
        assert_eq!(v.as_slice(), &[2, 5, 6, 7, 8, 9]);
        assert_eq!(v.lower_bound(&6), 2);
        assert_eq!(v.upper_bound(&6), 3);
        assert_eq!(v.equal_range(&3), 1..1);
        assert_eq!(v.front(), Some(&2));
        assert_eq!(v.back(), Some(&9));
        assert_eq!(v.get(&7), Some(&7));
    }

    #[test]
    fn construction_dedups() {
        let v: FlatSet<i32> = FlatSet::from([3, 1, 3, 2, 1]);
        assert_eq!(v.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn merge_sorts_and_dedups() {
        let mut v: FlatSet<i32> = FlatSet::from([1, 5]);
        v.merge([5, 3, 1, 4]);
        assert_eq!(v.as_slice(), &[1, 3, 4, 5]);
        v.merge(Vec::new());
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn modify_by_one_step() {
        let mut v: FlatSet<i32> = FlatSet::from([1, 5, 9]);
        v.modify(1, |x| *x = 7);
        assert_eq!(v.as_slice(), &[1, 7, 9]);
        v.modify(1, |x| *x = 10);
        assert_eq!(v.as_slice(), &[1, 9, 10]);
        v.modify(2, |x| *x = 3);
        assert_eq!(v.as_slice(), &[1, 3, 9]);
        // Becoming equal to a neighbour erases the element.
        v.modify(1, |x| *x = 9);
        assert_eq!(v.as_slice(), &[1, 9]);
        v.modify(0, |x| *x = 9);
        assert_eq!(v.as_slice(), &[9]);
    }

    #[test]
    fn modify_longer_move_is_unsorted_like_original() {
        // Documents the quirk of the original walk (kept on purpose).
        let mut v: FlatSet<i32> = FlatSet::from([1, 5, 6]);
        v.modify(0, |x| *x = 10);
        assert_eq!(v.as_slice(), &[5, 10, 6]);
    }

    #[test]
    fn multi_set_semantics() {
        let mut m = FlatMultiSet::<i32>::default();
        for x in [3, 1, 3, 2, 3] {
            m.insert(x);
        }
        assert_eq!(m.as_slice(), &[1, 2, 3, 3, 3]);
        assert_eq!(m.count(&3), 3);
        assert!(m.remove_one(&3));
        assert_eq!(m.remove_all(&3), 2);
        assert_eq!(m.erase(&3), 0);
        m.merge([0, 2]);
        assert_eq!(m.as_slice(), &[0, 1, 2, 2]);
        let r = m.modify(0, |x| {
            *x = 1;
            "done"
        });
        assert_eq!(r, "done");
        assert_eq!(m.as_slice(), &[1, 1, 2, 2]);
    }

    #[test]
    fn string_sets_look_up_by_str() {
        let mut v = FlatSet::<String>::default();
        v.insert("b".into());
        v.insert("a".into());
        assert!(v.contains("a") && !v.contains("c"));
        assert_eq!(v.find("b"), Some(1));
    }

    #[test]
    fn descending_and_pairs_ordering() {
        let v: FlatSet<i32, Greater> = FlatSet::from([1, 3, 2]);
        assert_eq!(v.as_slice(), &[3, 2, 1]);
        let a: FlatSet<i32> = FlatSet::from([1, 2]);
        let b: FlatSet<i32> = FlatSet::from([1, 3]);
        assert!(a < b && a != b);
    }

    #[test]
    fn empty_set_edge_cases() {
        let mut v = FlatSet::<i32>::default();
        assert_eq!(v.front(), None);
        assert!(!v.contains(&1));
        assert!(!v.remove(&1));
        assert_eq!(v.lower_bound(&1), 0);
        v.insert(1);
        v.clear();
        assert!(v.is_empty());
    }
}
