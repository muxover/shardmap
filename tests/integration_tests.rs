use shardmap::{Error, ShardMap, ShardMapBuilder};
use std::sync::Arc;

#[test]
fn test_basic_insert_get() {
    let map = ShardMap::new();

    assert!(map.insert("key1", "value1").is_none());
    assert_eq!(*map.get(&"key1").unwrap(), "value1");

    // Overwrite
    assert_eq!(*map.insert("key1", "value2").unwrap(), "value1");
    assert_eq!(*map.get(&"key1").unwrap(), "value2");
}

#[test]
fn test_remove() {
    let map = ShardMap::new();

    map.insert("key1", "value1");
    assert_eq!(*map.remove(&"key1").unwrap(), "value1");
    assert!(map.get(&"key1").is_none());
    assert!(map.remove(&"key1").is_none());
}

#[test]
fn test_update() {
    let map = ShardMap::new();

    map.insert("counter", 0);
    map.update(&"counter", |v| *v += 1);
    assert_eq!(*map.get(&"counter").unwrap(), 1);

    map.update(&"counter", |v| *v += 10);
    assert_eq!(*map.get(&"counter").unwrap(), 11);
}

#[test]
fn test_rename_same_shard() {
    let map = ShardMap::new();

    map.insert("old_key", "value");
    map.rename(&"old_key", "new_key").unwrap();

    assert!(map.get(&"old_key").is_none());
    assert_eq!(*map.get(&"new_key").unwrap(), "value");
}

#[test]
fn test_rename_different_shards() {
    // Create a map with 2 shards to increase chance of cross-shard rename
    let map = ShardMapBuilder::new()
        .shard_count(2)
        .unwrap()
        .build::<String, &str>()
        .unwrap();

    // Try to find keys that map to different shards
    let mut old_key = None;
    let mut new_key = None;

    for i in 0..100 {
        let key1 = format!("key_{}", i);
        let key2 = format!("key_{}", i + 1000);

        map.insert(key1.clone(), "value1");
        map.insert(key2.clone(), "value2");

        // Check if they're in different shards by checking stats
        let stats = map.stats();
        if stats.shard_sizes[0] > 0 && stats.shard_sizes[1] > 0 {
            old_key = Some(key1);
            new_key = Some(key2);
            break;
        }
    }

    if let (Some(old), Some(new)) = (old_key, new_key) {
        map.remove(&new); // Remove the new key first
        map.rename(&old, new.clone()).unwrap();
        assert!(map.get(&old).is_none());
        assert_eq!(*map.get(&new).unwrap(), "value1");
    }
}

#[test]
fn test_rename_errors() {
    let map = ShardMap::new();

    // Rename non-existent key
    assert_eq!(
        map.rename(&"nonexistent", "new").unwrap_err(),
        Error::KeyNotFound
    );

    // Rename to existing key
    map.insert("key1", "value1");
    map.insert("key2", "value2");
    assert_eq!(
        map.rename(&"key1", "key2").unwrap_err(),
        Error::KeyAlreadyExists
    );
}

#[test]
fn test_len_and_is_empty() {
    let map = ShardMap::new();

    assert!(map.is_empty());
    assert_eq!(map.len(), 0);

    map.insert("key1", "value1");
    assert!(!map.is_empty());
    assert_eq!(map.len(), 1);

    map.insert("key2", "value2");
    assert_eq!(map.len(), 2);

    map.remove(&"key1");
    assert_eq!(map.len(), 1);
}

#[test]
fn test_stats() {
    let map = ShardMap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.get(&"key1");
    map.get(&"key2");
    map.remove(&"key1");

    let stats = map.stats();
    assert_eq!(stats.size, 1);
    assert_eq!(stats.shard_sizes.len(), 16); // Default 16 shards
    assert_eq!(stats.operations.len(), 16);

    // At least one shard should have operations
    let total_ops: u64 = stats
        .operations
        .iter()
        .map(|op| op.reads + op.writes + op.removes)
        .sum();
    assert!(total_ops > 0);
}

#[test]
fn test_iter_snapshot() {
    let map = ShardMap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.insert("key3", "value3");

    let mut entries: Vec<_> = map.iter_snapshot().collect();
    entries.sort_by_key(|(k, _)| *k);

    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].0, "key1");
    assert_eq!(entries[1].0, "key2");
    assert_eq!(entries[2].0, "key3");
}

#[test]
fn test_iter_concurrent() {
    let map = ShardMap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");
    map.insert("key3", "value3");

    let mut entries: Vec<_> = map.iter_concurrent().collect();
    entries.sort_by_key(|(k, _)| *k);

    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].0, "key1");
    assert_eq!(entries[1].0, "key2");
    assert_eq!(entries[2].0, "key3");
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
fn test_builder_invalid_shard_count() {
    // Not a power of two
    assert!(ShardMapBuilder::new().shard_count(7).is_err());

    // Zero
    assert!(ShardMapBuilder::new().shard_count(0).is_err());
}

#[test]
fn test_arc_sharing() {
    let map = ShardMap::new();

    map.insert("key", "value");
    let arc1 = map.get(&"key").unwrap();
    let arc2 = map.get(&"key").unwrap();

    // Both should point to the same value
    assert!(Arc::ptr_eq(&arc1, &arc2));
    assert_eq!(*arc1, *arc2);
}
