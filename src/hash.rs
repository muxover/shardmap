use std::hash::{Hash, Hasher};

/// Hash function implementation for shard assignment.
/// Uses an enum to avoid trait object limitations with generics.
pub enum ShardHasher {
    /// AHash implementation (default, fast and well-distributed).
    AHash,
    /// FxHash implementation (faster but potentially less distributed).
    #[cfg(feature = "fxhash")]
    FxHash,
}

impl ShardHasher {
    /// Hash a key to determine which shard it belongs to.
    pub fn hash_key<K: Hash>(&self, key: &K) -> u64 {
        match self {
            ShardHasher::AHash => {
                let mut hasher = ahash::AHasher::default();
                key.hash(&mut hasher);
                hasher.finish()
            }
            #[cfg(feature = "fxhash")]
            ShardHasher::FxHash => {
                let mut hasher = fxhash::FxHasher::default();
                key.hash(&mut hasher);
                hasher.finish()
            }
        }
    }
}

impl Default for ShardHasher {
    fn default() -> Self {
        ShardHasher::AHash
    }
}
