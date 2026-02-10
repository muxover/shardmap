use crate::config::{create_hasher, Config};
use crate::error::Error;
use crate::hash::ShardHasher;
use crate::shard::Shard;
use crate::stats::{ShardOps, Stats};
use std::hash::Hash;
use std::sync::Arc;

/// High-performance concurrent sharded map.
///
/// Splits your data across multiple shards, each with its own lock. This means
/// operations on different shards don't block each other. Values are wrapped in
/// `Arc<T>` so you can share them without copying.
///
/// # Example
///
/// ```rust
/// use shardmap::ShardMap;
///
/// let map = ShardMap::new();
/// map.insert("key1", "value1");
///
/// if let Some(value) = map.get(&"key1") {
///     println!("Found: {}", *value);
/// }
/// ```
pub struct ShardMap<K, V> {
    shards: Vec<Shard<K, V>>,
    shard_mask: usize,
    hash: ShardHasher,
}

impl<K, V> ShardMap<K, V>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    /// Create a new map with defaults (16 shards, ahash).
    pub fn new() -> Self {
        Self::with_config(Config::default()).unwrap()
    }

    /// Create a new map with custom config.
    pub fn with_config(config: Config) -> Result<Self, Error> {
        if config.shard_count == 0 || !config.shard_count.is_power_of_two() {
            return Err(Error::InvalidShardCount);
        }

        let shard_count = config.shard_count;
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Shard::new());
        }

        Ok(Self {
            shards,
            shard_mask: shard_count - 1,
            hash: create_hasher(config.hash_function),
        })
    }

    /// Figure out which shard this key belongs to.
    #[inline]
    fn shard_index(&self, key: &K) -> usize {
        let hash = self.hash.hash_key(key);
        (hash as usize) & self.shard_mask
    }

    /// Insert a key-value pair. Returns the old value if the key existed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// assert!(map.insert("key", "value").is_none());
    /// assert_eq!(map.insert("key", "new_value").unwrap().as_ref(), &"value");
    /// ```
    pub fn insert(&self, key: K, value: V) -> Option<Arc<V>> {
        let shard_idx = self.shard_index(&key);
        self.shards[shard_idx].insert(key, value)
    }

    /// Get a value by key. Returns an `Arc<V>` so you can share it without copying.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// map.insert("key", "value");
    ///
    /// if let Some(value) = map.get(&"key") {
    ///     // value is Arc<&str>, clone is cheap
    ///     assert_eq!(*value, "value");
    /// }
    /// ```
    pub fn get(&self, key: &K) -> Option<Arc<V>> {
        let shard_idx = self.shard_index(key);
        self.shards[shard_idx].get(key)
    }

    /// Remove a key-value pair, returning the value if it existed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// map.insert("key", "value");
    /// assert_eq!(map.remove(&"key").unwrap().as_ref(), &"value");
    /// assert!(map.get(&"key").is_none());
    /// ```
    pub fn remove(&self, key: &K) -> Option<Arc<V>> {
        let shard_idx = self.shard_index(key);
        self.shards[shard_idx].remove(key)
    }

    /// Update a value using a closure, returning the new value if the key existed.
    ///
    /// Note: This requires `V: Clone` because if the value is shared (multiple
    /// `Arc` references exist), it will clone the value before modifying it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// map.insert("counter", 0);
    ///
    /// map.update(&"counter", |v| *v += 1);
    /// assert_eq!(*map.get(&"counter").unwrap(), 1);
    /// ```
    pub fn update<F>(&self, key: &K, f: F) -> Option<Arc<V>>
    where
        F: FnOnce(&mut V),
        V: Clone,
    {
        let shard_idx = self.shard_index(key);
        self.shards[shard_idx].update(key, f)
    }

    /// Atomically rename a key to a new key, moving the value without copying.
    ///
    /// This operation is atomic: either both the old key is removed and the new
    /// key is inserted, or neither happens. If the new key already exists, an
    /// error is returned.
    ///
    /// Note: For cross-shard renames (when old and new keys map to different
    /// shards), this requires `K: Clone` to handle conflict recovery.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// map.insert("old_key", "value");
    ///
    /// map.rename(&"old_key", "new_key").unwrap();
    /// assert!(map.get(&"old_key").is_none());
    /// assert_eq!(*map.get(&"new_key").unwrap(), "value");
    /// ```
    pub fn rename(&self, old_key: &K, new_key: K) -> Result<(), Error>
    where
        K: Clone,
    {
        let old_shard_idx = self.shard_index(old_key);
        let new_shard_idx = self.shard_index(&new_key);

        // If both keys map to the same shard, use atomic rename
        if old_shard_idx == new_shard_idx {
            return self.shards[old_shard_idx].rename(old_key, new_key);
        }

        // Different shards: use cross-shard rename helper
        // This requires K: Clone for conflict recovery
        self.rename_cross_shard(old_key, new_key, old_shard_idx, new_shard_idx)
    }

    /// Helper for cross-shard rename operations.
    /// This handles the case where we need to lock both shards and ensure atomicity.
    fn rename_cross_shard(
        &self,
        old_key: &K,
        new_key: K,
        old_shard_idx: usize,
        new_shard_idx: usize,
    ) -> Result<(), Error>
    where
        K: Clone,
    {
        // For cross-shard renames, we lock both shards in order to prevent deadlock
        // We check the new shard first, then remove from old shard, then insert
        let old_shard = &self.shards[old_shard_idx];
        let new_shard = &self.shards[new_shard_idx];

        // Check if new key already exists (this acquires a read lock)
        if new_shard.contains_key(&new_key) {
            return Err(Error::KeyAlreadyExists);
        }

        // Remove value from old shard
        let value = old_shard.remove(old_key).ok_or(Error::KeyNotFound)?;

        // Double-check new shard (it might have been inserted between our check and now)
        // This is a race condition we need to handle
        if new_shard.contains_key(&new_key) {
            // Conflict: restore the value to old shard
            old_shard.insert_arc(old_key.clone(), value);
            return Err(Error::KeyAlreadyExists);
        }

        // Insert into new shard
        new_shard.insert_arc(new_key, value);
        Ok(())
    }

    /// Get the total number of entries across all shards.
    ///
    /// Note: This operation requires acquiring read locks on all shards, so it
    /// may be slow for large numbers of shards. For better performance, use
    /// `stats()` which provides more detailed information.
    pub fn len(&self) -> usize {
        self.shards.iter().map(|shard| shard.len()).sum()
    }

    /// Check if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.shards.iter().all(|shard| shard.is_empty())
    }

    /// Get detailed statistics about the map and its shards.
    pub fn stats(&self) -> Stats {
        let shard_sizes: Vec<usize> = self.shards.iter().map(|s| s.len()).collect();
        let operations: Vec<ShardOps> = self.shards.iter().map(|s| s.stats()).collect();
        let size: usize = shard_sizes.iter().sum();

        Stats {
            size,
            shard_sizes,
            operations,
        }
    }

    /// Create a snapshot-based iterator over all key-value pairs.
    ///
    /// This iterator captures the current state of the map into a vector,
    /// then iterates over it. It won't see concurrent modifications made
    /// after the snapshot is taken, but provides a consistent view.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// map.insert("key1", "value1");
    /// map.insert("key2", "value2");
    ///
    /// let mut count = 0;
    /// for (_key, _value) in map.iter_snapshot() {
    ///     count += 1;
    /// }
    /// assert_eq!(count, 2);
    /// ```
    pub fn iter_snapshot(&self) -> crate::iter::SnapshotIter<K, V>
    where
        K: Clone,
    {
        crate::iter::SnapshotIter::new(&self.shards)
    }

    /// Create a concurrent-safe iterator over all key-value pairs.
    ///
    /// This iterator holds read locks on shards while iterating, so it can
    /// see concurrent modifications. However, it may see partial updates if
    /// entries are moved between shards during iteration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// map.insert("key1", "value1");
    /// map.insert("key2", "value2");
    ///
    /// let mut count = 0;
    /// for (_key, _value) in map.iter_concurrent() {
    ///     count += 1;
    /// }
    /// assert_eq!(count, 2);
    /// ```
    pub fn iter_concurrent(&self) -> crate::iter::ConcurrentIter<'_, K, V>
    where
        K: Clone,
    {
        crate::iter::ConcurrentIter::new(&self.shards)
    }
}

impl<K, V> Default for ShardMap<K, V>
where
    K: Hash + Eq + Send + Sync,
    V: Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}
