use crate::config::{create_hasher, Config, RoutingConfig};
use crate::error::Error;
use crate::hash::ShardHasher;
use crate::shard::Shard;
use crate::stats::{Diagnostics, ShardDiagnostics, ShardOps, Stats};
use std::borrow::Borrow;
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
    routing: RoutingConfig,
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

    /// Create a new map with the given number of shards (must be a power of two).
    /// Convenience for `ShardMapBuilder::new().shard_count(n).unwrap().build()`.
    pub fn with_shard_count(shard_count: usize) -> Result<Self, Error> {
        Self::with_config(Config::default().shard_count(shard_count)?)
    }

    /// Create a new map with at least this total capacity, spread across shards.
    /// Shard count defaults to 16. For more control use `ShardMapBuilder`.
    pub fn with_capacity(capacity: usize) -> Self {
        let config = Config::default();
        let shard_count = config.shard_count;
        let cap_per_shard = capacity.saturating_add(shard_count - 1) / shard_count;
        let config = config.capacity_per_shard(cap_per_shard);
        Self::with_config(config).unwrap()
    }

    /// Create a new map with custom config.
    pub fn with_config(config: Config) -> Result<Self, Error> {
        if config.shard_count == 0 || !config.shard_count.is_power_of_two() {
            return Err(Error::InvalidShardCount);
        }

        let shard_count = config.shard_count;
        let cap_per_shard = config.capacity_per_shard.unwrap_or(0);
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Shard::with_capacity(cap_per_shard));
        }

        Ok(Self {
            shards,
            shard_mask: shard_count - 1,
            hash: create_hasher(config.hash_function),
            routing: config.routing,
        })
    }

    /// Route a key hash to a shard index.
    #[inline]
    fn route_hash(&self, hash: u64) -> usize {
        match &self.routing {
            RoutingConfig::Default => (hash as usize) & self.shard_mask,
            RoutingConfig::Custom(router) => router.route(hash, self.shards.len()),
        }
    }

    /// Figure out which shard this key belongs to.
    #[inline]
    fn shard_index(&self, key: &K) -> usize {
        let hash = self.hash.hash_key(key);
        self.route_hash(hash)
    }

    /// Returns the hash of a key for shard routing. Use with `shard_for_hash` or `*_by_hash` when you already have a hash.
    #[inline]
    pub fn hash_for_key<Q>(&self, key: &Q) -> u64
    where
        Q: Hash + ?Sized,
    {
        self.hash.hash_key(key)
    }

    /// Returns which shard index the given hash maps to. Use with pre-hashed keys.
    #[inline]
    pub fn shard_for_hash(&self, hash: u64) -> usize {
        self.route_hash(hash)
    }

    /// Returns which shard index the given key maps to.
    ///
    /// Use this for observability, shard-aware logic (e.g. per-shard eviction),
    /// or to interpret `stats().operations[shard_for_key(k)]`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// map.insert("user:42", "data");
    /// let shard = map.shard_for_key(&"user:42");
    /// let stats = map.stats();
    /// println!("Shard {} ops: {:?}", shard, stats.operations[shard]);
    /// ```
    #[inline]
    pub fn shard_for_key<Q>(&self, key: &Q) -> usize
    where
        Q: Hash + ?Sized,
    {
        self.shard_for_hash(self.hash_for_key(key))
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

    /// Get a value by key using a precomputed hash for shard selection (avoids re-hashing for routing).
    pub fn get_by_hash<Q>(&self, key: &Q, key_hash: u64) -> Option<Arc<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let shard_idx = self.shard_for_hash(key_hash);
        self.shards[shard_idx].get(key)
    }

    /// Insert using a precomputed hash for shard selection. Returns the previous value if the key existed.
    pub fn insert_by_hash(&self, key: K, value: V, key_hash: u64) -> Option<Arc<V>> {
        let shard_idx = self.shard_for_hash(key_hash);
        self.shards[shard_idx].insert(key, value)
    }

    /// Remove by key using a precomputed hash for shard selection.
    pub fn remove_by_hash<Q>(&self, key: &Q, key_hash: u64) -> Option<Arc<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let shard_idx = self.shard_for_hash(key_hash);
        self.shards[shard_idx].remove(key)
    }

    /// Returns whether the map contains a value for the given key.
    pub fn contains_key(&self, key: &K) -> bool {
        let shard_idx = self.shard_index(key);
        self.shards[shard_idx].contains_key(key)
    }

    /// Remove all entries from the map.
    pub fn clear(&self) {
        for shard in &self.shards {
            shard.clear();
        }
    }

    /// Retain only entries for which the predicate returns true.
    /// Requires `V: Clone` because values may be cloned when modified in place.
    pub fn retain<F>(&self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
        V: Clone,
    {
        for shard in &self.shards {
            shard.retain(&mut f);
        }
    }

    /// Total capacity across all shards (number of elements that can be stored without reallocating).
    pub fn capacity(&self) -> usize {
        self.shards.iter().map(|s| s.capacity()).sum()
    }

    /// Shrink each shard to fit its current length. Reduces memory use after removals.
    pub fn shrink_to_fit(&self) {
        for shard in &self.shards {
            shard.shrink_to_fit();
        }
    }

    /// Get the value for the key, or insert the value and return a new `Arc<V>`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// let v = map.get_or_insert("counter", 0);
    /// assert_eq!(*v, 0);
    /// map.get_or_insert("counter", 99); // no-op, already present
    /// assert_eq!(*map.get(&"counter").unwrap(), 0);
    /// ```
    pub fn get_or_insert(&self, key: K, value: V) -> Arc<V> {
        let shard_idx = self.shard_index(&key);
        self.shards[shard_idx].get_or_insert(key, value)
    }

    /// Get the value for the key, or compute it with `f` and insert it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// let v = map.get_or_insert_with("expensive", || "computed".to_string());
    /// assert_eq!(v.as_str(), "computed");
    /// ```
    pub fn get_or_insert_with<F>(&self, key: K, f: F) -> Arc<V>
    where
        F: FnOnce() -> V,
    {
        let shard_idx = self.shard_index(&key);
        self.shards[shard_idx].get_or_insert_with(key, f)
    }

    /// Insert the key-value pair only if the key is not present.
    /// Returns `Ok(arc)` with the inserted value, or `Err(arc)` with the existing value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use shardmap::ShardMap;
    ///
    /// let map = ShardMap::new();
    /// assert!(map.try_insert("key", "first").is_ok());
    /// assert!(map.try_insert("key", "second").is_err());
    /// assert_eq!(*map.get(&"key").unwrap(), "first");
    /// ```
    pub fn try_insert(&self, key: K, value: V) -> Result<Arc<V>, Arc<V>> {
        let shard_idx = self.shard_index(&key);
        self.shards[shard_idx].try_insert(key, value)
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

    /// Rename a key to a new key, moving the value without copying.
    ///
    /// **Same shard:** The operation is atomic under that shard's lock: either
    /// both the old key is removed and the new key is inserted, or neither happens.
    ///
    /// **Cross-shard:** Old and new keys map to different shards. This implementation
    /// acquires both shard locks (old then new). The move is atomic from the caller's
    /// view (all-or-nothing), but it is not a single lock — so don't assume the same
    /// atomicity guarantees as within a single shard.
    ///
    /// Returns an error if the old key is missing or the new key already exists.
    /// For cross-shard renames, `K: Clone` is required for conflict recovery.
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

    /// Per-shard entry counts. Works without the `metrics` feature. Use for imbalance detection.
    pub fn shard_loads(&self) -> Vec<usize> {
        self.shards.iter().map(|s| s.len()).collect()
    }

    /// Structured diagnostics snapshot: per-shard stats, total operations, and raw `max_load_ratio` for you to interpret.
    pub fn diagnostics(&self) -> Diagnostics {
        let shards: Vec<ShardDiagnostics> = self
            .shards
            .iter()
            .map(|s| s.diagnostics_snapshot())
            .collect();
        let total_entries: usize = shards.iter().map(|s| s.entries).sum();
        let n = self.shards.len() as f64;
        let avg_load_per_shard = if n > 0.0 {
            total_entries as f64 / n
        } else {
            0.0
        };
        let max_load = shards.iter().map(|s| s.entries).max().unwrap_or(0) as f64;
        let max_load_ratio = if avg_load_per_shard > 0.0 {
            max_load / avg_load_per_shard
        } else {
            1.0
        };
        let total_operations: u64 = shards.iter().map(|s| s.reads + s.writes + s.removes).sum();

        Diagnostics {
            total_entries,
            shards,
            total_operations,
            avg_load_per_shard,
            max_load_ratio,
        }
    }

    /// Get detailed statistics about the map and its shards.
    pub fn stats(&self) -> Stats {
        let shard_sizes = self.shard_loads();
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
