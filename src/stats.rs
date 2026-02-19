//! Statistics and diagnostics types.

#[cfg(feature = "metrics")]
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
    /// Number of lock acquisitions (0 when metrics feature disabled).
    pub lock_acquisitions: u64,
    /// Cumulative lock wait time in nanoseconds (0 when lock-timing disabled).
    pub lock_wait_nanos: u64,
}

/// Thread-safe statistics tracker for a single shard.
#[cfg(feature = "metrics")]
pub(crate) struct ShardStats {
    reads: AtomicU64,
    writes: AtomicU64,
    removes: AtomicU64,
    lock_acquisitions: AtomicU64,
    #[cfg(feature = "lock-timing")]
    lock_wait_nanos: AtomicU64,
}

#[cfg(feature = "metrics")]
impl ShardStats {
    pub fn new() -> Self {
        Self {
            reads: AtomicU64::new(0),
            writes: AtomicU64::new(0),
            removes: AtomicU64::new(0),
            lock_acquisitions: AtomicU64::new(0),
            #[cfg(feature = "lock-timing")]
            lock_wait_nanos: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn record_read(&self) {
        self.reads.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_write(&self) {
        self.writes.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_remove(&self) {
        self.removes.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_lock_acquisition(&self) {
        self.lock_acquisitions.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(feature = "lock-timing")]
    #[inline]
    pub fn record_lock_wait(&self, nanos: u64) {
        self.lock_wait_nanos.fetch_add(nanos, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> ShardOps {
        ShardOps {
            reads: self.reads.load(Ordering::Relaxed),
            writes: self.writes.load(Ordering::Relaxed),
            removes: self.removes.load(Ordering::Relaxed),
            lock_acquisitions: self.lock_acquisitions.load(Ordering::Relaxed),
            #[cfg(feature = "lock-timing")]
            lock_wait_nanos: self.lock_wait_nanos.load(Ordering::Relaxed),
            #[cfg(not(feature = "lock-timing"))]
            lock_wait_nanos: 0,
        }
    }
}

#[cfg(feature = "metrics")]
impl Default for ShardStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Zero-sized placeholder when metrics are disabled.
#[cfg(not(feature = "metrics"))]
pub(crate) struct ShardStats;

#[cfg(not(feature = "metrics"))]
impl ShardStats {
    pub fn new() -> Self {
        ShardStats
    }

    #[inline]
    pub fn record_read(&self) {}

    #[inline]
    pub fn record_write(&self) {}

    #[inline]
    pub fn record_remove(&self) {}

    #[inline]
    pub fn record_lock_acquisition(&self) {}

    #[cfg(feature = "lock-timing")]
    #[inline]
    pub fn record_lock_wait(&self, _nanos: u64) {}

    pub fn snapshot(&self) -> ShardOps {
        ShardOps::default()
    }
}

#[cfg(not(feature = "metrics"))]
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

/// Per-shard diagnostics snapshot.
#[derive(Debug, Clone)]
pub struct ShardDiagnostics {
    /// Number of entries in this shard.
    pub entries: usize,
    /// Read operations (0 when metrics disabled).
    pub reads: u64,
    /// Write operations (0 when metrics disabled).
    pub writes: u64,
    /// Remove operations (0 when metrics disabled).
    pub removes: u64,
    /// Lock acquisitions (0 when metrics disabled).
    pub lock_acquisitions: u64,
    /// Cumulative lock wait time in nanoseconds (0 when lock-timing disabled).
    pub lock_wait_nanos: u64,
}

/// Structured snapshot for performance introspection.
#[derive(Debug, Clone)]
pub struct Diagnostics {
    /// Total number of entries across all shards.
    pub total_entries: usize,
    /// Per-shard diagnostics.
    pub shards: Vec<ShardDiagnostics>,
    /// Total read + write + remove operations (0 when metrics disabled).
    pub total_operations: u64,
    /// Average load (entries) per shard.
    pub avg_load_per_shard: f64,
    /// Max load / avg load ratio. User interprets (e.g. threshold 2.0 for imbalance).
    pub max_load_ratio: f64,
}
