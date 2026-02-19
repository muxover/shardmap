//! # ShardMap
//!
//! A **performance-predictable, introspectable concurrent map** for Rust. Built for engineers who
//! care about load behavior: deterministic shard routing, predictable lock isolation, and
//! optional built-in diagnostics.
//!
//! ## Design
//!
//! - **Zero global lock** — Data is split across shards; each shard has its own lock. Operations
//!   on different shards do not block each other.
//! - **Deterministic routing** — The same key always maps to the same shard. You can use
//!   [`shard_for_key`](ShardMap::shard_for_key) or a custom [`ShardRouter`] to control placement.
//! - **Values as `Arc<V>`** — Stored values are reference-counted so readers can clone the `Arc`
//!   and use the value without holding the shard lock (cheap sharing, no copy of `V`).
//!
//! ## When to use ShardMap
//!
//! - You need to **see what your map is doing under load** (e.g. per-shard entry counts, imbalance).
//! - You are **tuning** shard count, capacity, or routing for your workload.
//! - You want **predictable shard isolation** and control over shard count.
//!
//! ## Features
//!
//! | Feature       | Default | Description |
//! |----------------|---------|-------------|
//! | (none)        | ✓       | Performance-first: no metrics overhead. |
//! | `metrics`     | —       | Per-shard read/write/remove and lock-acquisition counters. |
//! | `lock-timing` | —       | Per-shard lock wait time. **Debugging/profiling only**; not for production hot paths. |
//! | `fxhash`      | —       | Use FxHash for shard assignment. |
//!
//! ## Quick example
//!
//! ```rust
//! use shardmap::ShardMap;
//!
//! let map = ShardMap::new();
//! map.insert("key1", "value1");
//!
//! if let Some(v) = map.get(&"key1") {
//!     println!("{}", *v);
//! }
//!
//! // Per-shard entry counts (no feature required)
//! let loads = map.shard_loads();
//! println!("Shard loads: {:?}", loads);
//! ```
//!
//! ## Configuration
//!
//! ```rust
//! use shardmap::{ShardMapBuilder, HashFunction};
//!
//! let map = ShardMapBuilder::new()
//!     .shard_count(32)?
//!     .capacity_per_shard(256)
//!     .hash_function(HashFunction::AHash)
//!     .build::<String, i32>()?;
//! # Ok::<(), shardmap::Error>(())
//! ```
//!
//! ## Introspection
//!
//! - **[`shard_loads()`](ShardMap::shard_loads)** — Per-shard entry counts. Always available.
//! - **[`diagnostics()`](ShardMap::diagnostics)** — Snapshot with `total_entries`, per-shard
//!   stats, `total_operations`, `avg_load_per_shard`, and **`max_load_ratio`** (you decide the
//!   threshold for “imbalance”).
//! - **[`shard_for_key`](ShardMap::shard_for_key)** / **[`shard_for_hash`](ShardMap::shard_for_hash)** — Which shard a key or hash maps to.
//! - **Pre-hashed APIs** — [`hash_for_key`](ShardMap::hash_for_key), [`get_by_hash`](ShardMap::get_by_hash),
//!   [`insert_by_hash`](ShardMap::insert_by_hash), [`remove_by_hash`](ShardMap::remove_by_hash) when you
//!   already have a hash (e.g. from a packet header).
//!
//! ## Custom routing
//!
//! Implement [`ShardRouter`] and pass [`RoutingConfig::Custom(Box::new(your_router))`](RoutingConfig::Custom)
//! to the builder. See [`DefaultRouter`] for the default `hash & (shard_count - 1)` behavior.
//!
//! ## Non-goals
//!
//! Not a drop-in for DashMap or std; no dynamic shard resizing; no built-in eviction or persistence.

#![deny(missing_docs)]
#![warn(clippy::all)]

/// Configuration and builder types.
pub mod config;
/// Error types.
pub mod error;
/// Hash function implementations.
pub mod hash;
/// Iterator implementations.
pub mod iter;
/// Internal shard implementation.
pub mod shard;
/// Main ShardMap implementation.
pub mod shardmap;
/// Statistics and diagnostics types.
pub mod stats;

// Re-export main types
pub use config::{
    Config, DefaultRouter, HashFunction, RoutingConfig, ShardMapBuilder, ShardRouter,
};
pub use error::Error;
pub use shardmap::ShardMap;
pub use stats::{Diagnostics, ShardDiagnostics, ShardOps, Stats};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let map = ShardMap::new();

        assert!(map.insert("key1", "value1").is_none());
        assert_eq!(map.insert("key1", "value2").unwrap().as_ref(), &"value1");
        assert_eq!(map.get(&"key1").unwrap().as_ref(), &"value2");
        assert!(map.get(&"nonexistent").is_none());
        assert_eq!(map.remove(&"key1").unwrap().as_ref(), &"value2");
        assert!(map.get(&"key1").is_none());
    }

    #[test]
    fn test_rename() {
        let map = ShardMap::new();
        map.insert("old_key", "value");
        map.rename(&"old_key", "new_key").unwrap();
        assert!(map.get(&"old_key").is_none());
        assert_eq!(*map.get(&"new_key").unwrap(), "value");
    }

    #[test]
    fn test_update() {
        let map = ShardMap::new();
        map.insert("counter", 0);
        map.update(&"counter", |v| *v += 1);
        assert_eq!(*map.get(&"counter").unwrap(), 1);
    }

    #[test]
    fn test_stats() {
        let map = ShardMap::new();
        map.insert("key1", "value1");
        map.insert("key2", "value2");
        let stats = map.stats();
        assert_eq!(stats.size, 2);
    }

    #[test]
    fn test_builder() {
        let map = ShardMapBuilder::new()
            .shard_count(8)
            .unwrap()
            .build::<String, i32>()
            .unwrap();
        map.insert("test".to_string(), 42);
        assert_eq!(*map.get(&"test".to_string()).unwrap(), 42);
    }

    #[test]
    fn test_shard_loads_and_diagnostics() {
        let map = ShardMap::new();
        map.insert("a", 1);
        map.insert("b", 2);
        let loads = map.shard_loads();
        assert_eq!(loads.len(), 16);
        assert_eq!(loads.iter().sum::<usize>(), 2);
        let diag = map.diagnostics();
        assert_eq!(diag.total_entries, 2);
        assert!(diag.max_load_ratio >= 1.0);
    }

    #[test]
    fn test_hash_and_by_hash() {
        let map = ShardMap::new();
        map.insert("k", 10);
        let h = map.hash_for_key(&"k");
        assert_eq!(map.shard_for_hash(h), map.shard_for_key(&"k"));
        assert_eq!(*map.get_by_hash(&"k", h).unwrap(), 10);
        assert_eq!(map.remove_by_hash(&"k", h).unwrap().as_ref(), &10);
        assert!(map.get(&"k").is_none());
    }
}
