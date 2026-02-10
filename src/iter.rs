use crate::shard::Shard;
use std::hash::Hash;
use std::sync::Arc;

/// Snapshot-based iterator that captures the current state of the map.
///
/// This iterator collects all entries into a vector first, then iterates over
/// them. This means it won't see concurrent modifications made after the
/// snapshot is taken, but it's guaranteed to see a consistent view of the map
/// at the time of creation.
pub struct SnapshotIter<K, V> {
    entries: Vec<(K, Arc<V>)>,
    index: usize,
}

impl<K, V> SnapshotIter<K, V>
where
    K: Hash + Eq + Send + Sync + Clone,
    V: Send + Sync,
{
    pub(crate) fn new(shards: &[Shard<K, V>]) -> Self {
        let mut entries = Vec::new();

        // Collect all entries from all shards
        for shard in shards {
            let map = shard.read_lock();
            for (key, value) in map.iter() {
                entries.push((key.clone(), value.clone()));
            }
        }

        Self { entries, index: 0 }
    }
}

impl<K, V> Iterator for SnapshotIter<K, V>
where
    K: Clone,
{
    type Item = (K, Arc<V>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.entries.len() {
            let item = self.entries[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.entries.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<K, V> ExactSizeIterator for SnapshotIter<K, V> where K: Clone {}

/// Concurrent-safe iterator that iterates over shards with read locks.
///
/// This iterator collects entries from each shard one at a time while holding
/// a read lock, so it can see concurrent modifications. However, it may see
/// partial updates if entries are moved between shards during iteration.
///
/// Note: This implementation collects entries from each shard into a buffer
/// to avoid lifetime issues with holding locks across iterator calls.
pub struct ConcurrentIter<'a, K, V> {
    shards: &'a [Shard<K, V>],
    current_shard: usize,
    buffer: Vec<(K, Arc<V>)>,
    buffer_index: usize,
}

impl<'a, K, V> ConcurrentIter<'a, K, V>
where
    K: Hash + Eq + Send + Sync + Clone,
    V: Send + Sync,
{
    pub(crate) fn new(shards: &'a [Shard<K, V>]) -> Self {
        Self {
            shards,
            current_shard: 0,
            buffer: Vec::new(),
            buffer_index: 0,
        }
    }

    /// Fill the buffer with entries from the current shard.
    fn fill_buffer(&mut self) -> bool {
        // Clear the buffer and reset index
        self.buffer.clear();
        self.buffer_index = 0;

        // Try to get entries from current shard
        while self.current_shard < self.shards.len() {
            let shard = &self.shards[self.current_shard];
            let guard = shard.read_lock();

            // Collect entries from this shard
            for (key, value) in guard.iter() {
                self.buffer.push((key.clone(), value.clone()));
            }

            // If we got entries, we're done
            if !self.buffer.is_empty() {
                self.current_shard += 1;
                return true;
            }

            // Empty shard, move to next
            drop(guard);
            self.current_shard += 1;
        }

        false
    }
}

impl<'a, K, V> Iterator for ConcurrentIter<'a, K, V>
where
    K: Hash + Eq + Send + Sync + Clone,
    V: Send + Sync,
{
    type Item = (K, Arc<V>);

    fn next(&mut self) -> Option<Self::Item> {
        // If buffer is empty or exhausted, try to fill it
        if self.buffer_index >= self.buffer.len() && !self.fill_buffer() {
            return None;
        }

        // Get next item from buffer
        let item = self.buffer[self.buffer_index].clone();
        self.buffer_index += 1;
        Some(item)
    }
}
