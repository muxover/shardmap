use crate::stats::ShardStats;
use hashbrown::HashMap;
use parking_lot::RwLock;
use std::hash::Hash;
use std::sync::Arc;

/// A single shard containing a HashMap protected by a read-write lock.
pub(crate) struct Shard<K, V> {
    map: RwLock<HashMap<K, Arc<V>>>,
    stats: ShardStats,
}

impl<K, V> Shard<K, V>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
            stats: ShardStats::new(),
        }
    }

    /// Insert a key-value pair, returning the previous value if any.
    pub fn insert(&self, key: K, value: V) -> Option<Arc<V>> {
        let mut map = self.map.write();
        let result = map.insert(key, Arc::new(value));
        if result.is_none() {
            self.stats.record_write();
        }
        result
    }

    /// Get a value by key, returning an Arc to enable zero-copy access.
    pub fn get(&self, key: &K) -> Option<Arc<V>> {
        let map = self.map.read();
        let result = map.get(key).cloned();
        if result.is_some() {
            self.stats.record_read();
        }
        result
    }

    /// Remove a key-value pair, returning the value if it existed.
    pub fn remove(&self, key: &K) -> Option<Arc<V>> {
        let mut map = self.map.write();
        let result = map.remove(key);
        if result.is_some() {
            self.stats.record_remove();
        }
        result
    }

    /// Update a value using a closure, returning the new value if the key existed.
    ///
    /// Note: This requires `V: Clone` because if the value is shared (multiple
    /// `Arc` references exist), it will clone the value before modifying it.
    pub fn update<F>(&self, key: &K, f: F) -> Option<Arc<V>>
    where
        F: FnOnce(&mut V),
        V: Clone,
    {
        let mut map = self.map.write();
        if let Some(arc_value) = map.get_mut(key) {
            // We need to get a mutable reference, but Arc doesn't allow that.
            // We'll use Arc::make_mut which clones if there are other references.
            // This requires V: Clone.
            let value = Arc::make_mut(arc_value);
            f(value);
            self.stats.record_write();
            Some(arc_value.clone())
        } else {
            None
        }
    }

    /// Get the number of entries in this shard.
    pub fn len(&self) -> usize {
        self.map.read().len()
    }

    /// Check if this shard is empty.
    pub fn is_empty(&self) -> bool {
        self.map.read().is_empty()
    }

    /// Get a snapshot of statistics for this shard.
    pub fn stats(&self) -> crate::stats::ShardOps {
        self.stats.snapshot()
    }

    /// Get a read lock for iteration purposes.
    pub fn read_lock(&self) -> parking_lot::RwLockReadGuard<'_, HashMap<K, Arc<V>>> {
        self.map.read()
    }

    /// Check if a key exists without cloning the value.
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.read().contains_key(key)
    }

    /// Remove a key and return its value, if it exists.
    /// This is an alias for remove, but kept for API clarity.
    #[allow(dead_code)] // Public API method, may be used by external code
    pub fn take(&self, key: &K) -> Option<Arc<V>> {
        self.remove(key)
    }

    /// Atomically rename a key within this shard.
    /// Returns Ok(()) on success, or an error if the old key doesn't exist
    /// or the new key already exists.
    pub fn rename(&self, old_key: &K, new_key: K) -> Result<(), crate::error::Error> {
        let mut map = self.map.write();

        if !map.contains_key(old_key) {
            return Err(crate::error::Error::KeyNotFound);
        }

        if map.contains_key(&new_key) {
            return Err(crate::error::Error::KeyAlreadyExists);
        }

        // Atomic operation: remove and insert in one lock acquisition
        if let Some(value) = map.remove(old_key) {
            map.insert(new_key, value);
            self.stats.record_write();
            Ok(())
        } else {
            Err(crate::error::Error::KeyNotFound)
        }
    }

    /// Insert a value with an existing Arc (used for cross-shard renames).
    pub fn insert_arc(&self, key: K, value: Arc<V>) -> Option<Arc<V>> {
        let mut map = self.map.write();
        let result = map.insert(key, value);
        if result.is_none() {
            self.stats.record_write();
        }
        result
    }
}

impl<K, V> Default for Shard<K, V>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}
