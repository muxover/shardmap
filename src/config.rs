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

/// User-provided shard selection. Enables stateful or custom routing.
pub trait ShardRouter: Send + Sync {
    /// Return the shard index in `[0, shard_count)` for the given key hash.
    fn route(&self, key_hash: u64, shard_count: usize) -> usize;
}

/// Default routing: `(hash as usize) & (shard_count - 1)`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRouter;

impl ShardRouter for DefaultRouter {
    #[inline]
    fn route(&self, key_hash: u64, shard_count: usize) -> usize {
        (key_hash as usize) & (shard_count - 1)
    }
}

/// Routing strategy for shard selection.
#[derive(Default)]
pub enum RoutingConfig {
    /// Default: hash & (shard_count - 1).
    #[default]
    Default,
    /// User-provided router (e.g. stateful or custom distribution).
    Custom(Box<dyn ShardRouter>),
}

impl std::fmt::Debug for RoutingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoutingConfig::Default => write!(f, "RoutingConfig::Default"),
            RoutingConfig::Custom(_) => write!(f, "RoutingConfig::Custom(...)"),
        }
    }
}

/// Configuration for a ShardMap instance.
#[derive(Debug)]
pub struct Config {
    pub(crate) shard_count: usize,
    pub(crate) hash_function: HashFunction,
    pub(crate) capacity_per_shard: Option<usize>,
    pub(crate) routing: RoutingConfig,
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

    /// Set initial capacity per shard. Total capacity will be approximately
    /// `capacity_per_shard * shard_count`. Omitted by default (HashMap default).
    pub fn capacity_per_shard(mut self, capacity: usize) -> Self {
        self.capacity_per_shard = Some(capacity);
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shard_count: 16,
            hash_function: HashFunction::AHash,
            capacity_per_shard: None,
            routing: RoutingConfig::Default,
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

    /// Set initial capacity per shard. Total capacity ≈ `capacity_per_shard * shard_count`.
    pub fn capacity_per_shard(mut self, capacity: usize) -> Self {
        self.config = self.config.capacity_per_shard(capacity);
        self
    }

    /// Use a custom shard router (e.g. for stateful or custom distribution).
    pub fn routing(mut self, routing: RoutingConfig) -> Self {
        self.config.routing = routing;
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
