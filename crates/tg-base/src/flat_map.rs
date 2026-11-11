//! Port of `base/flat_map.h`: sorted-vector maps.
//!
//! `FlatMultiMap` keeps `(key, value)` pairs in a `Vec` sorted by key with a
//! user supplied comparator; equal keys keep their insertion order.
//! `FlatMap` is the unique-key flavour (the original derives it privately
//! from the multi map, here it wraps one).
//!
//! C++ iterators are random-access and the client relies on that
//! (`begin() + n`, `lower_bound()` results, `erase(it)` returning the next
//! iterator, `erase(first, last)`). They are translated to `usize` positions:
//! `end()` is `len()`, `begin() + n` is `n`, `it - begin()` is the position
//! itself. Keys are never handed out mutably.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::iter::FromIterator;
use std::ops::Range;

/// Strict weak ordering over `T`; the equivalent of a C++ comparator.
///
/// Lookups accept any `Q` the key can be borrowed as, which corresponds to
/// transparent comparators (`std::less<>`, `is_transparent`): the comparator
/// must also implement `Compare<Q>` and agree with `Compare<K>`.
pub trait Compare<T: ?Sized> {
    /// Returns true if `a` goes strictly before `b`.
    fn less(&self, a: &T, b: &T) -> bool;
}

/// `std::less<>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Less;

/// `std::greater<>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Greater;

impl<T: PartialOrd + ?Sized> Compare<T> for Less {
    fn less(&self, a: &T, b: &T) -> bool {
        a < b
    }
}

impl<T: PartialOrd + ?Sized> Compare<T> for Greater {
    fn less(&self, a: &T, b: &T) -> bool {
        a > b
    }
}

/// Turns a strict-weak "less" into a total `Ordering` for sorting.
pub(crate) fn ordering_by<T: ?Sized, C: Compare<T>>(cmp: &C, a: &T, b: &T) -> Ordering {
    if cmp.less(a, b) {
        Ordering::Less
    } else if cmp.less(b, a) {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

type PairRef<'a, K, V> = (&'a K, &'a V);
type PairMut<'a, K, V> = (&'a K, &'a mut V);

fn as_ref_pair<K, V>(pair: &(K, V)) -> PairRef<'_, K, V> {
    (&pair.0, &pair.1)
}

fn as_mut_pair<K, V>(pair: &mut (K, V)) -> PairMut<'_, K, V> {
    (&pair.0, &mut pair.1)
}

/// Iterator over `(&key, &value)` pairs in key order.
pub type Iter<'a, K, V> =
    std::iter::Map<std::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> PairRef<'a, K, V>>;
/// Iterator over `(&key, &mut value)` pairs in key order.
pub type IterMut<'a, K, V> =
    std::iter::Map<std::slice::IterMut<'a, (K, V)>, fn(&'a mut (K, V)) -> PairMut<'a, K, V>>;

/// `base::flat_multi_map`: sorted vector, repeated keys allowed.
pub struct FlatMultiMap<K, V, C = Less> {
    data: Vec<(K, V)>,
    cmp: C,
}

impl<K, V, C: Default> Default for FlatMultiMap<K, V, C> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            cmp: C::default(),
        }
    }
}

impl<K: Clone, V: Clone, C: Clone> Clone for FlatMultiMap<K, V, C> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            cmp: self.cmp.clone(),
        }
    }
}

impl<K: fmt::Debug, V: fmt::Debug, C> fmt::Debug for FlatMultiMap<K, V, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.data.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

// Like the original, comparisons look at the stored pairs only.
impl<K: PartialEq, V: PartialEq, C> PartialEq for FlatMultiMap<K, V, C> {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl<K: Eq, V: Eq, C> Eq for FlatMultiMap<K, V, C> {}

impl<K: PartialOrd, V: PartialOrd, C> PartialOrd for FlatMultiMap<K, V, C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.data.partial_cmp(&other.data)
    }
}

impl<K, V, C> FlatMultiMap<K, V, C> {
    /// Creates an empty map with an explicit (stateful) comparator.
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

    /// The sorted pairs; `begin() + n` of the original is `as_slice()[n]`.
    pub fn as_slice(&self) -> &[(K, V)] {
        &self.data
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        self.data
            .iter()
            .map(as_ref_pair as fn(&(K, V)) -> PairRef<'_, K, V>)
    }

    /// Keys stay immutable, only values can be changed.
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        self.data
            .iter_mut()
            .map(as_mut_pair as fn(&mut (K, V)) -> PairMut<'_, K, V>)
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.data.iter().map(|pair| &pair.0)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.data.iter().map(|pair| &pair.1)
    }

    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut V> + ExactSizeIterator {
        self.data.iter_mut().map(|pair| &mut pair.1)
    }

    pub fn front(&self) -> Option<PairRef<'_, K, V>> {
        self.data.first().map(as_ref_pair)
    }

    pub fn back(&self) -> Option<PairRef<'_, K, V>> {
        self.data.last().map(as_ref_pair)
    }

    /// Pair at `position`, `None` if `position == len()` or more.
    pub fn get_at(&self, position: usize) -> Option<PairRef<'_, K, V>> {
        self.data.get(position).map(as_ref_pair)
    }

    pub fn get_at_mut(&mut self, position: usize) -> Option<PairMut<'_, K, V>> {
        self.data.get_mut(position).map(as_mut_pair)
    }

    /// `erase(iterator)`: returns the position of the next pair, which is
    /// the same `position`.
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
}

impl<K, V, C: Compare<K>> FlatMultiMap<K, V, C> {
    fn sort_data(&mut self) {
        let cmp = &self.cmp;
        // Stable, so pairs with equal keys keep the order they came in.
        self.data.sort_by(|a, b| ordering_by(cmp, &a.0, &b.0));
    }

    /// Builds from arbitrary pairs with an explicit comparator.
    pub fn from_iter_with_compare<I: IntoIterator<Item = (K, V)>>(iter: I, cmp: C) -> Self {
        let mut result = Self {
            data: iter.into_iter().collect(),
            cmp,
        };
        result.sort_data();
        result
    }

    /// Inserts after all pairs with an equal key; returns the position.
    pub fn insert(&mut self, value: (K, V)) -> usize {
        if self
            .data
            .last()
            .is_none_or(|back| !self.cmp.less(&value.0, &back.0))
        {
            self.data.push(value);
            return self.data.len() - 1;
        }
        let at = self.upper_bound::<K>(&value.0);
        self.data.insert(at, value);
        at
    }

    pub fn emplace(&mut self, key: K, value: V) -> usize {
        self.insert((key, value))
    }
}

impl<K, V, C> FlatMultiMap<K, V, C> {
    /// Position of the first pair whose key is not less than `key`.
    pub fn lower_bound<Q>(&self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.data
            .partition_point(|pair| self.cmp.less(pair.0.borrow(), key))
    }

    /// Position of the first pair whose key is greater than `key`.
    pub fn upper_bound<Q>(&self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.data
            .partition_point(|pair| !self.cmp.less(key, pair.0.borrow()))
    }

    /// Positions `[lower_bound, upper_bound)`.
    pub fn equal_range<Q>(&self, key: &Q) -> Range<usize>
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.lower_bound(key)..self.upper_bound(key)
    }

    /// Position of the first pair with this key, `None` if there is none
    /// (the original returns `end()`).
    pub fn find_first<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        let at = self.lower_bound(key);
        match self.data.get(at) {
            Some(pair) if !self.cmp.less(key, pair.0.borrow()) => Some(at),
            _ => None,
        }
    }

    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.find_first(key).is_some()
    }

    pub fn count<Q>(&self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.equal_range(key).len()
    }

    /// Removes the first pair with this key.
    pub fn remove_one<Q>(&mut self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        match self.find_first(key) {
            Some(at) => {
                self.data.remove(at);
                true
            }
            None => false,
        }
    }

    /// Removes all pairs with this key and returns how many were removed.
    pub fn remove_all<Q>(&mut self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        let range = self.equal_range(key);
        let count = range.len();
        self.data.drain(range);
        count
    }

    /// `erase(key)`: same as `remove_all`.
    pub fn erase<Q>(&mut self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.remove_all(key)
    }

    /// Removes the first pair equal to `(key, value)`.
    pub fn remove<Q>(&mut self, key: &Q, value: &V) -> bool
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
        V: PartialEq,
    {
        let range = self.equal_range(key);
        for at in range {
            if self.data[at].1 == *value {
                self.data.remove(at);
                return true;
            }
        }
        false
    }
}

impl<K, V, C: Compare<K> + Default> FromIterator<(K, V)> for FlatMultiMap<K, V, C> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self::from_iter_with_compare(iter, C::default())
    }
}

impl<K, V, C: Compare<K> + Default, const N: usize> From<[(K, V); N]> for FlatMultiMap<K, V, C> {
    fn from(value: [(K, V); N]) -> Self {
        value.into_iter().collect()
    }
}

impl<K, V, C: Compare<K>> Extend<(K, V)> for FlatMultiMap<K, V, C> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for pair in iter {
            self.insert(pair);
        }
    }
}

impl<K, V, C> IntoIterator for FlatMultiMap<K, V, C> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a, K, V, C> IntoIterator for &'a FlatMultiMap<K, V, C> {
    type Item = PairRef<'a, K, V>;
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, C> IntoIterator for &'a mut FlatMultiMap<K, V, C> {
    type Item = PairMut<'a, K, V>;
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// `base::flat_map`: sorted vector, unique keys.
pub struct FlatMap<K, V, C = Less> {
    inner: FlatMultiMap<K, V, C>,
}

impl<K, V, C: Default> Default for FlatMap<K, V, C> {
    fn default() -> Self {
        Self {
            inner: FlatMultiMap::default(),
        }
    }
}

impl<K: Clone, V: Clone, C: Clone> Clone for FlatMap<K, V, C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<K: fmt::Debug, V: fmt::Debug, C> fmt::Debug for FlatMap<K, V, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<K: PartialEq, V: PartialEq, C> PartialEq for FlatMap<K, V, C> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<K: Eq, V: Eq, C> Eq for FlatMap<K, V, C> {}

impl<K: PartialOrd, V: PartialOrd, C> PartialOrd for FlatMap<K, V, C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl<K, V, C> FlatMap<K, V, C> {
    pub fn with_compare(cmp: C) -> Self {
        Self {
            inner: FlatMultiMap::with_compare(cmp),
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

    pub fn as_slice(&self) -> &[(K, V)] {
        self.inner.as_slice()
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        self.inner.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        self.inner.iter_mut()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.inner.keys()
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.inner.values()
    }

    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut V> + ExactSizeIterator {
        self.inner.values_mut()
    }

    pub fn front(&self) -> Option<PairRef<'_, K, V>> {
        self.inner.front()
    }

    pub fn back(&self) -> Option<PairRef<'_, K, V>> {
        self.inner.back()
    }

    pub fn get_at(&self, position: usize) -> Option<PairRef<'_, K, V>> {
        self.inner.get_at(position)
    }

    pub fn get_at_mut(&mut self, position: usize) -> Option<PairMut<'_, K, V>> {
        self.inner.get_at_mut(position)
    }

    pub fn erase_at(&mut self, position: usize) -> usize {
        self.inner.erase_at(position)
    }

    pub fn erase_range(&mut self, range: Range<usize>) -> usize {
        self.inner.erase_range(range)
    }

    pub fn lower_bound<Q>(&self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.lower_bound(key)
    }

    pub fn upper_bound<Q>(&self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.upper_bound(key)
    }

    pub fn equal_range<Q>(&self, key: &Q) -> Range<usize>
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.equal_range(key)
    }

    /// `find(key)`: position of the pair, `None` where the original
    /// returns `end()`.
    pub fn find<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.find_first(key)
    }

    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.contains(key)
    }

    /// Value by key (the `find(key)->second` idiom).
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        let at = self.find(key)?;
        Some(&self.inner.data[at].1)
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        let at = self.find(key)?;
        Some(&mut self.inner.data[at].1)
    }

    /// `remove(key)`: true if a pair was removed.
    pub fn remove<Q>(&mut self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.remove_one(key)
    }

    /// `erase(key)`: number of removed pairs (0 or 1).
    pub fn erase<Q>(&mut self, key: &Q) -> usize
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        self.inner.remove_all(key)
    }

    /// Removes the pair and returns its value.
    pub fn take<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        C: Compare<Q>,
        Q: ?Sized,
    {
        let at = self.find(key)?;
        Some(self.inner.data.remove(at).1)
    }
}

impl<K, V, C: Compare<K>> FlatMap<K, V, C> {
    fn dedup(&mut self) {
        let cmp = &self.inner.cmp;
        // Keeps the first of each run of equivalent keys.
        self.inner.data.dedup_by(|later, earlier| {
            !cmp.less(&earlier.0, &later.0) && !cmp.less(&later.0, &earlier.0)
        });
    }

    /// Builds from arbitrary pairs; for repeated keys the first one wins.
    pub fn from_iter_with_compare<I: IntoIterator<Item = (K, V)>>(iter: I, cmp: C) -> Self {
        let mut result = Self {
            inner: FlatMultiMap::from_iter_with_compare(iter, cmp),
        };
        result.dedup();
        result
    }

    /// `insert(value)`: keeps the existing pair if the key is present.
    /// Returns the position and whether the pair was inserted.
    pub fn insert(&mut self, value: (K, V)) -> (usize, bool) {
        let data = &mut self.inner.data;
        let cmp = &self.inner.cmp;
        if data.last().is_none_or(|back| cmp.less(&back.0, &value.0)) {
            data.push(value);
            return (data.len() - 1, true);
        }
        let at = data.partition_point(|pair| cmp.less(&pair.0, &value.0));
        if cmp.less(&value.0, &data[at].0) {
            data.insert(at, value);
            (at, true)
        } else {
            (at, false)
        }
    }

    /// `emplace(key, args...)`: does not overwrite an existing value.
    pub fn emplace(&mut self, key: K, value: V) -> (usize, bool) {
        self.insert((key, value))
    }

    /// `try_emplace(key, args...)`: same as `emplace` (the value is
    /// dropped when the key is already present).
    pub fn try_emplace(&mut self, key: K, value: V) -> (usize, bool) {
        self.insert((key, value))
    }

    /// `insert_or_assign(key, value)`: overwrites the value if present.
    pub fn insert_or_assign(&mut self, key: K, value: V) -> (usize, bool) {
        let mut value = Some(value);
        let (at, inserted) = self.try_emplace_with(key, || value.take().expect("value is set"));
        if let Some(value) = value {
            self.inner.data[at].1 = value;
        }
        (at, inserted)
    }

    /// `emplace_or_assign(key, args...)`.
    pub fn emplace_or_assign(&mut self, key: K, value: V) -> (usize, bool) {
        self.insert_or_assign(key, value)
    }

    /// `operator[]`: the value for `key`, default-inserted if absent.
    pub fn get_or_default(&mut self, key: K) -> &mut V
    where
        V: Default,
    {
        let (at, _) = self.try_emplace_with(key, V::default);
        &mut self.inner.data[at].1
    }

    /// Lazy `try_emplace`: `make` runs only if the key is absent.
    pub fn try_emplace_with(&mut self, key: K, make: impl FnOnce() -> V) -> (usize, bool) {
        let data = &mut self.inner.data;
        let cmp = &self.inner.cmp;
        if data.last().is_none_or(|back| cmp.less(&back.0, &key)) {
            data.push((key, make()));
            return (data.len() - 1, true);
        }
        let at = data.partition_point(|pair| cmp.less(&pair.0, &key));
        if cmp.less(&key, &data[at].0) {
            data.insert(at, (key, make()));
            (at, true)
        } else {
            (at, false)
        }
    }
}

impl<K, V, C: Compare<K> + Default> FromIterator<(K, V)> for FlatMap<K, V, C> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self::from_iter_with_compare(iter, C::default())
    }
}

impl<K, V, C: Compare<K> + Default, const N: usize> From<[(K, V); N]> for FlatMap<K, V, C> {
    fn from(value: [(K, V); N]) -> Self {
        value.into_iter().collect()
    }
}

impl<K, V, C: Compare<K>> Extend<(K, V)> for FlatMap<K, V, C> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for pair in iter {
            self.insert(pair);
        }
    }
}

impl<K, V, C> IntoIterator for FlatMap<K, V, C> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a, K, V, C> IntoIterator for &'a FlatMap<K, V, C> {
    type Item = PairRef<'a, K, V>;
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, C> IntoIterator for &'a mut FlatMap<K, V, C> {
    type Item = PairMut<'a, K, V>;
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Borrow;

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
    impl Borrow<i32> for IntWrap {
        fn borrow(&self) -> &i32 {
            &self.value
        }
    }
    impl Compare<i32> for IntWrapComparator {
        fn less(&self, a: &i32, b: &i32) -> bool {
            a < b
        }
    }

    fn check_sorted<K: PartialOrd, V>(map: &FlatMap<K, V, impl Compare<K>>) {
        for pair in map.as_slice().windows(2) {
            assert!(pair[0].0 < pair[1].0);
        }
    }

    fn sample() -> FlatMap<i32, String> {
        let mut v = FlatMap::new_for_test();
        v.emplace(0, "a".into());
        v.emplace(5, "b".into());
        v.emplace(4, "d".into());
        v.emplace(2, "e".into());
        v
    }

    impl<K, V, C: Default> FlatMap<K, V, C> {
        fn new_for_test() -> Self {
            Self::default()
        }
    }

    #[test]
    fn flat_maps_should_keep_items_sorted_by_key() {
        let mut v = sample();
        assert_eq!(v.len(), 4);
        check_sorted(&v);
        // adding item puts it in the right position
        v.emplace(3, "c".into());
        assert_eq!(v.len(), 5);
        assert!(v.find(&3).is_some());
        check_sorted(&v);
    }

    #[test]
    fn copy_constructor_and_assignment() {
        let mut v = FlatMap::<i32, String>::default();
        v.emplace(0, "a".into());
        v.emplace(2, "b".into());
        let u = v.clone();
        assert_eq!(u.len(), 2);
        assert_eq!(u.find(&0), Some(0));
        assert_eq!(u.find(&2), Some(u.len() - 1));
        let mut w = FlatMap::default();
        assert!(w.is_empty());
        w.clone_from(&v);
        assert_eq!(w, v);
        assert_eq!(w.find(&2), Some(1));
    }

    #[test]
    fn custom_comparator() {
        let mut v = FlatMap::<IntWrap, String, IntWrapComparator>::default();
        v.emplace(IntWrap { value: 0 }, "a".into());
        v.emplace(IntWrap { value: 5 }, "b".into());
        v.emplace(IntWrap { value: 4 }, "d".into());
        v.emplace(IntWrap { value: 2 }, "e".into());
        assert_eq!(v.len(), 4);
        let sorted = |v: &FlatMap<IntWrap, String, IntWrapComparator>| {
            v.as_slice().windows(2).all(|p| p[0].0.value < p[1].0.value)
        };
        assert!(sorted(&v));
        v.emplace(IntWrap { value: 3 }, "c".into());
        assert_eq!(v.len(), 5);
        assert!(v.find(&IntWrap { value: 3 }).is_some());
        // Transparent lookup by the bare integer.
        assert!(v.find(&3).is_some());
        assert!(v.contains(&4) && !v.contains(&7));
        assert!(sorted(&v));
    }

    #[test]
    fn structured_binding_iteration() {
        let mut v = FlatMap::<i32, Box<f64>>::default();
        v.emplace(0, Box::new(0.));
        v.emplace(1, Box::new(1.));
        for (key, value) in &v {
            assert_eq!(*key, value.round() as i32);
        }
        let second: FlatMap<i32, i32> = FlatMap::from([(1, 1), (2, 2), (2, 3), (3, 3)]);
        assert_eq!(second.len(), 3);
        for (a, b) in &second {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn emplace_does_not_overwrite_but_insert_or_assign_does() {
        let mut v = FlatMap::<i32, &str>::default();
        assert_eq!(v.emplace(1, "a"), (0, true));
        assert_eq!(v.emplace(1, "b"), (0, false));
        assert_eq!(v.get(&1), Some(&"a"));
        assert_eq!(v.insert_or_assign(1, "c"), (0, false));
        assert_eq!(v.insert_or_assign(0, "z"), (0, true));
        assert_eq!(v.get(&1), Some(&"c"));
        assert_eq!(v.try_emplace(1, "q"), (1, false));
        assert_eq!(v.get(&1), Some(&"c"));
    }

    #[test]
    fn try_emplace_with_is_lazy() {
        let mut v = FlatMap::<i32, i32>::default();
        v.emplace(1, 10);
        let mut calls = 0;
        v.try_emplace_with(1, || {
            calls += 1;
            0
        });
        assert_eq!(calls, 0);
        v.try_emplace_with(2, || {
            calls += 1;
            20
        });
        assert_eq!(calls, 1);
    }

    #[test]
    fn index_operator_inserts_default() {
        let mut v = FlatMap::<i32, Vec<i32>>::default();
        v.get_or_default(5).push(1);
        v.get_or_default(3).push(2);
        v.get_or_default(5).push(3);
        assert_eq!(v.as_slice(), &[(3, vec![2]), (5, vec![1, 3])]);
    }

    #[test]
    fn take_remove_and_erase() {
        let mut v = sample();
        assert_eq!(v.take(&4).as_deref(), Some("d"));
        assert_eq!(v.take(&4), None);
        assert!(v.remove(&0));
        assert!(!v.remove(&0));
        assert_eq!(v.erase(&5), 1);
        assert_eq!(v.erase(&5), 0);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn bounds_and_positional_erase() {
        let mut v: FlatMap<i32, i32> = (0..10).map(|i| (i * 2, i)).collect();
        assert_eq!(v.lower_bound(&5), 3);
        assert_eq!(v.upper_bound(&6), 4);
        assert_eq!(v.lower_bound(&100), v.len());
        assert_eq!(v.lower_bound(&-1), 0);
        assert_eq!(v.equal_range(&6), 3..4);
        assert_eq!(v.equal_range(&7), 4..4);
        // Erase while walking, like `i = map.erase(i)` loops.
        let mut i = 0;
        while i < v.len() {
            if v.get_at(i).unwrap().0 % 4 == 0 {
                i = v.erase_at(i);
            } else {
                i += 1;
            }
        }
        assert_eq!(
            v.keys().copied().collect::<Vec<_>>(),
            vec![2, 6, 10, 14, 18]
        );
        assert_eq!(v.erase_range(1..3), 1);
        assert_eq!(v.keys().copied().collect::<Vec<_>>(), vec![2, 14, 18]);
        assert_eq!(v.front(), Some((&2, &1)));
        assert_eq!(v.back(), Some((&18, &9)));
    }

    #[test]
    fn descending_comparator_and_float_keys() {
        let v: FlatMap<i32, i32, Greater> = FlatMap::from([(1, 1), (3, 3), (2, 2)]);
        assert_eq!(v.keys().copied().collect::<Vec<_>>(), vec![3, 2, 1]);
        assert_eq!(v.find(&2), Some(1));
        let f: FlatMap<f64, &str> = FlatMap::from([(0.5, "a"), (0.25, "b")]);
        assert_eq!(f.get(&0.25), Some(&"b"));
    }

    #[test]
    fn string_keys_look_up_by_str() {
        let mut v = FlatMap::<String, i32>::default();
        v.emplace("b".to_string(), 2);
        v.emplace("a".to_string(), 1);
        assert_eq!(v.get("a"), Some(&1));
        assert!(!v.contains("c"));
    }

    #[test]
    fn construction_from_duplicates_keeps_first() {
        let v: FlatMap<i32, &str> = FlatMap::from([(2, "first"), (1, "x"), (2, "second")]);
        assert_eq!(v.as_slice(), &[(1, "x"), (2, "first")]);
    }

    #[test]
    fn iter_mut_changes_values_only() {
        let mut v: FlatMap<i32, i32> = FlatMap::from([(1, 1), (2, 2)]);
        for (_, value) in &mut v {
            *value *= 10;
        }
        assert_eq!(v.values().copied().collect::<Vec<_>>(), vec![10, 20]);
        *v.get_mut(&2).unwrap() = 5;
        assert_eq!(v.get(&2), Some(&5));
    }

    #[test]
    fn comparison_operators_compare_pairs() {
        let a: FlatMap<i32, i32> = FlatMap::from([(1, 1)]);
        let b: FlatMap<i32, i32> = FlatMap::from([(1, 2)]);
        assert!(a < b && a != b);
        assert_eq!(a, FlatMap::from([(1, 1)]));
    }

    #[test]
    fn multi_map_keeps_insertion_order_of_equal_keys() {
        let mut m = FlatMultiMap::<i32, &str>::default();
        m.emplace(2, "a");
        m.emplace(1, "x");
        m.emplace(2, "b");
        m.emplace(2, "c");
        m.emplace(0, "z");
        assert_eq!(
            m.as_slice(),
            &[(0, "z"), (1, "x"), (2, "a"), (2, "b"), (2, "c")]
        );
        assert_eq!(m.count(&2), 3);
        assert_eq!(m.equal_range(&2), 2..5);
        assert_eq!(m.find_first(&2), Some(2));
        assert_eq!(m.find_first(&3), None);
    }

    #[test]
    fn multi_map_removal() {
        let mut m: FlatMultiMap<i32, i32> = FlatMultiMap::from([(1, 1), (1, 2), (1, 3), (2, 9)]);
        assert!(m.remove(&1, &2));
        assert!(!m.remove(&1, &2));
        assert!(m.remove_one(&1));
        assert_eq!(m.as_slice(), &[(1, 3), (2, 9)]);
        assert_eq!(m.remove_all(&1), 1);
        assert_eq!(m.remove_all(&1), 0);
        assert_eq!(m.erase(&2), 1);
        assert!(m.is_empty());
    }

    #[test]
    fn multi_map_from_unsorted_is_stable() {
        let m: FlatMultiMap<i32, i32> = FlatMultiMap::from([(2, 1), (1, 1), (2, 2), (1, 2)]);
        assert_eq!(m.as_slice(), &[(1, 1), (1, 2), (2, 1), (2, 2)]);
    }

    #[test]
    fn multi_map_expiring_idiom() {
        // `_expiring` of Data::Stories: drop everything up to `now`.
        let mut m: FlatMultiMap<i32, u32> = FlatMultiMap::default();
        for (t, id) in [(30, 1), (10, 2), (20, 3), (10, 4), (40, 5)] {
            m.emplace(t, id);
        }
        let now = 20;
        let end = m.upper_bound(&now);
        let expired: Vec<_> = m.as_slice()[..end].iter().map(|p| p.1).collect();
        assert_eq!(m.erase_range(0..end), 0);
        assert_eq!(expired, vec![2, 4, 3]);
        assert_eq!(m.front(), Some((&30, &1)));
    }

    #[test]
    fn empty_map_edge_cases() {
        let mut m = FlatMap::<i32, i32>::default();
        assert_eq!(m.front(), None);
        assert_eq!(m.find(&1), None);
        assert_eq!(m.lower_bound(&1), 0);
        assert!(!m.remove(&1));
        assert_eq!(m.take(&1), None);
        m.clear();
        assert_eq!(m.get_at(0), None);
    }
}
