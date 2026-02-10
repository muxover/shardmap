//! # ShardMap
//!
//! A high-performance concurrent sharded map for extreme workloads.
//!
//! ShardMap splits your data across multiple shards, each with its own lock.
//! This means operations on different shards don't block each other, giving
//! you much better performance under contention. Values are stored behind
//! `Arc<T>` so you can share them without copying.
//!
//! ## Features
//!
//! - **High Performance**: Sharded design minimizes lock contention
//! - **Thread-Safe**: All operations are safe for concurrent access
//! - **Zero-Copy Reads**: Values stored as `Arc<T>` for efficient sharing
//! - **Deterministic**: Same key always maps to the same shard
//! - **Configurable**: Choose shard count and hash function
//! - **Statistics**: Per-shard operation tracking
//!
//! ## Example
//!
//! ```rust
//! use shardmap::ShardMap;
//!
//! let map = ShardMap::new();
//!
//! // Insert values
//! map.insert("key1", "value1");
//! map.insert("key2", "value2");
//!
//! // Read values (zero-copy via Arc)
//! if let Some(value) = map.get(&"key1") {
//!     println!("Found: {}", *value);
//! }
//!
//! // Update values
//! map.update(&"key1", |v| {
//!     // Modify value in place
//! });
//!
//! // Rename keys atomically
//! map.rename(&"key1", "new_key1").unwrap();
//!
//! // Iterate over entries
//! for (key, value) in map.iter_snapshot() {
//!     println!("{}: {}", key, *value);
//! }
//!
//! // Get statistics
//! let stats = map.stats();
//! println!("Total entries: {}", stats.size);
//! ```
//!
//! ## Configuration
//!
//! ```rust
//! use shardmap::{ShardMapBuilder, HashFunction};
//!
//! let map = ShardMapBuilder::new()
//!     .shard_count(32)?  // Must be power of two
//!     .hash_function(HashFunction::AHash)
//!     .build::<String, i32>()?;
//! # Ok::<(), shardmap::Error>(())
//! ```

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
/// Statistics and metrics collection.
pub mod stats;

// Re-export main types
pub use config::{Config, HashFunction, ShardMapBuilder};
pub use error::Error;
pub use shardmap::ShardMap;
pub use stats::{ShardOps, Stats};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let map = ShardMap::new();

        // Insert
        assert!(map.insert("key1", "value1").is_none());
        assert_eq!(map.insert("key1", "value2").unwrap().as_ref(), &"value1");

        // Get
        assert_eq!(map.get(&"key1").unwrap().as_ref(), &"value2");
        assert!(map.get(&"nonexistent").is_none());

        // Remove
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
}
