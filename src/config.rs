use crate::error::Error;
use crate::hash::ShardHasher;

/// Which hash function to use for shard assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HashFunction {
    /// Use ahash (default, fast and well-distributed).
    #[default]
    AHash,
    /// Use fxhash (faster but potentially less distributed).
    #[cfg(feature = "fxhash")]
    FxHash,
}

/// Configuration for a ShardMap instance.
#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) shard_count: usize,
    pub(crate) hash_function: HashFunction,
}

impl Config {
    /// Create a new config with defaults (16 shards, ahash).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of shards. Must be a power of two and greater than 0.
    pub fn shard_count(mut self, count: usize) -> Result<Self, Error> {
        if count == 0 || !count.is_power_of_two() {
            return Err(Error::InvalidShardCount);
        }
        self.shard_count = count;
        Ok(self)
    }

    /// Set the hash function to use.
    pub fn hash_function(mut self, hash_fn: HashFunction) -> Self {
        self.hash_function = hash_fn;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shard_count: 16,
            hash_function: HashFunction::AHash,
        }
    }
}

/// Builder for creating a ShardMap with custom configuration.
pub struct ShardMapBuilder {
    config: Config,
}

impl ShardMapBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    /// Set the number of shards. Must be a power of two and greater than 0.
    pub fn shard_count(mut self, count: usize) -> Result<Self, Error> {
        self.config = self.config.shard_count(count)?;
        Ok(self)
    }

    /// Set the hash function to use.
    pub fn hash_function(mut self, hash_fn: HashFunction) -> Self {
        self.config = self.config.hash_function(hash_fn);
        self
    }

    /// Build a ShardMap with the configured settings.
    pub fn build<K, V>(self) -> Result<crate::ShardMap<K, V>, Error>
    where
        K: std::hash::Hash + Eq + Send + Sync,
        V: Send + Sync,
    {
        crate::ShardMap::with_config(self.config)
    }
}

impl Default for ShardMapBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a hash function instance based on the configuration.
pub(crate) fn create_hasher(hash_fn: HashFunction) -> ShardHasher {
    match hash_fn {
        HashFunction::AHash => ShardHasher::AHash,
        #[cfg(feature = "fxhash")]
        HashFunction::FxHash => ShardHasher::FxHash,
    }
}
