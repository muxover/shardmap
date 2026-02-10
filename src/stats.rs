use std::sync::atomic::{AtomicU64, Ordering};

/// Per-shard operation statistics.
#[derive(Debug, Clone, Default)]
pub struct ShardOps {
    /// Number of read operations on this shard.
    pub reads: u64,
    /// Number of write operations on this shard.
    pub writes: u64,
    /// Number of remove operations on this shard.
    pub removes: u64,
}

/// Thread-safe statistics tracker for a single shard.
pub(crate) struct ShardStats {
    reads: AtomicU64,
    writes: AtomicU64,
    removes: AtomicU64,
}

impl ShardStats {
    pub fn new() -> Self {
        Self {
            reads: AtomicU64::new(0),
            writes: AtomicU64::new(0),
            removes: AtomicU64::new(0),
        }
    }

    pub fn record_read(&self) {
        self.reads.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_write(&self) {
        self.writes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_remove(&self) {
        self.removes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> ShardOps {
        ShardOps {
            reads: self.reads.load(Ordering::Relaxed),
            writes: self.writes.load(Ordering::Relaxed),
            removes: self.removes.load(Ordering::Relaxed),
        }
    }
}

impl Default for ShardStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregate statistics for a ShardMap instance.
#[derive(Debug, Clone)]
pub struct Stats {
    /// Total number of entries across all shards.
    pub size: usize,
    /// Number of entries in each shard.
    pub shard_sizes: Vec<usize>,
    /// Operation counts for each shard.
    pub operations: Vec<ShardOps>,
}
