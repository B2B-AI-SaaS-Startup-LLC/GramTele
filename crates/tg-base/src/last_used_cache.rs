//! Port of `base/last_used_cache.h`: LRU order of entries.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

/// `base::last_used_cache`: `up` marks an entry as most recently used,
/// `take_lowest` removes the least recently used one.
#[derive(Debug)]
pub struct LastUsedCache<Entry> {
    // Tick -> entry, oldest first; and entry -> its tick.
    queue: BTreeMap<u64, Entry>,
    map: HashMap<Entry, u64>,
    next: u64,
}

impl<Entry> Default for LastUsedCache<Entry> {
    fn default() -> Self {
        Self {
            queue: BTreeMap::new(),
            map: HashMap::new(),
            next: 0,
        }
    }
}

impl<Entry: Clone + Eq + Hash> LastUsedCache<Entry> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn up(&mut self, entry: Entry) {
        if let Some((_, back)) = self.queue.iter().next_back() {
            if *back == entry {
                return;
            }
        }
        let tick = self.next;
        self.next += 1;
        if let Some(old) = self.map.insert(entry.clone(), tick) {
            self.queue.remove(&old);
        }
        self.queue.insert(tick, entry);
    }

    pub fn remove(&mut self, entry: &Entry) {
        if let Some(tick) = self.map.remove(entry) {
            self.queue.remove(&tick);
        }
    }

    pub fn clear(&mut self) {
        self.queue.clear();
        self.map.clear();
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<Entry: Clone + Eq + Hash + Default> LastUsedCache<Entry> {
    /// Removes and returns the least recently used entry; like the
    /// original returns `Entry::default()` when empty.
    pub fn take_lowest(&mut self) -> Entry {
        match self.queue.pop_first() {
            Some((_, entry)) => {
                self.map.remove(&entry);
                entry
            }
            None => Entry::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let mut cache = LastUsedCache::new();
        for i in [1, 2, 3] {
            cache.up(i);
        }
        cache.up(1);
        cache.up(1);
        assert_eq!(cache.take_lowest(), 2);
        assert_eq!(cache.take_lowest(), 3);
        assert_eq!(cache.take_lowest(), 1);
        assert_eq!(cache.take_lowest(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn remove_and_clear() {
        let mut cache = LastUsedCache::new();
        cache.up(1);
        cache.up(2);
        cache.remove(&1);
        cache.remove(&9);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.take_lowest(), 2);
        cache.up(5);
        cache.clear();
        assert_eq!(cache.take_lowest(), 0);
    }
}
