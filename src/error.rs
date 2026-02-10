/// Errors that can occur when operating on a ShardMap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The requested key was not found in the map.
    KeyNotFound,
    /// The target key already exists (used in rename operations).
    KeyAlreadyExists,
    /// The shard count is invalid (must be a power of two and greater than 0).
    InvalidShardCount,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::KeyNotFound => write!(f, "key not found"),
            Error::KeyAlreadyExists => write!(f, "key already exists"),
            Error::InvalidShardCount => {
                write!(f, "shard count must be a power of two and greater than 0")
            }
        }
    }
}

impl std::error::Error for Error {}
