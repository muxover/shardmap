use shardmap::{DefaultRouter, Error, RoutingConfig, ShardMap, ShardMapBuilder, ShardRouter};

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
fn test_rename_cross_shard() {
    // With 2 shards, use shard_for_key to pick two keys in different shards, then rename.
    let map = ShardMapBuilder::new()
        .shard_count(2)
        .unwrap()
        .build::<String, &str>()
        .unwrap();
    let mut a = None;
    let mut b = None;
    for s in ["x", "y", "key_0", "key_1", "key_1000"] {
        let k = s.to_string();
        map.insert(k.clone(), "v");
        let shard = map.shard_for_key(&k);
        if a.is_none() {
            a = Some((k, shard));
        } else if let Some((_, sa)) = a {
            if shard != sa {
                b = Some((k, shard));
                break;
            }
        }
    }
    if let (Some((old, _)), Some((new, _))) = (a, b) {
        map.remove(&new);
        map.rename(&old, new.clone()).unwrap();
        assert!(map.get(&old).is_none());
        assert_eq!(*map.get(&new).unwrap(), "v");
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
    // When metrics feature is disabled, op counts are 0; when enabled, total_ops > 0
    let _ = stats
        .operations
        .iter()
        .map(|op| op.reads + op.writes + op.removes)
        .sum::<u64>();
}

#[test]
fn test_shard_loads() {
    let map = ShardMap::new();
    map.insert("a", 1);
    map.insert("b", 2);
    let loads = map.shard_loads();
    assert_eq!(loads.len(), 16);
    assert_eq!(loads.iter().sum::<usize>(), 2);
}

#[test]
fn test_diagnostics() {
    let map = ShardMap::new();
    map.insert("x", 10);
    map.insert("y", 20);
    let diag = map.diagnostics();
    assert_eq!(diag.total_entries, 2);
    assert_eq!(diag.shards.len(), 16);
    assert!(diag.max_load_ratio >= 1.0);
    assert!(diag.avg_load_per_shard >= 0.0);
}

#[test]
fn test_hash_and_by_hash_apis() {
    let map = ShardMap::new();
    map.insert("k", 42);
    let h = map.hash_for_key(&"k");
    assert_eq!(map.shard_for_hash(h), map.shard_for_key(&"k"));
    assert_eq!(*map.get_by_hash(&"k", h).unwrap(), 42);
    assert_eq!(map.remove_by_hash(&"k", h).unwrap().as_ref(), &42);
    assert!(map.get(&"k").is_none());
}

#[test]
fn test_insert_by_hash() {
    let map = ShardMap::new();
    let h = map.hash_for_key(&"pk");
    assert!(map.insert_by_hash("pk", 1, h).is_none());
    assert_eq!(map.insert_by_hash("pk", 2, h).unwrap().as_ref(), &1);
    assert_eq!(*map.get(&"pk").unwrap(), 2);
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
fn test_custom_router() {
    // Use default router explicitly
    let map = ShardMapBuilder::new()
        .shard_count(4)
        .unwrap()
        .routing(RoutingConfig::Default)
        .build::<String, i32>()
        .unwrap();
    map.insert("k".to_string(), 1);
    assert_eq!(*map.get(&"k".to_string()).unwrap(), 1);

    // Custom: route everything to shard 0 (not useful in practice, but tests the API)
    struct AllToZero;
    impl ShardRouter for AllToZero {
        fn route(&self, _key_hash: u64, _shard_count: usize) -> usize {
            0
        }
    }
    let map2 = ShardMapBuilder::new()
        .shard_count(4)
        .unwrap()
        .routing(RoutingConfig::Custom(Box::new(AllToZero)))
        .build::<String, i32>()
        .unwrap();
    map2.insert("a".to_string(), 10);
    map2.insert("b".to_string(), 20);
    let loads = map2.shard_loads();
    assert_eq!(loads[0], 2);
    assert_eq!(loads.iter().sum::<usize>(), 2);
}

#[test]
fn test_default_router() {
    let map = ShardMapBuilder::new()
        .shard_count(8)
        .unwrap()
        .routing(RoutingConfig::Default)
        .build::<&str, i32>()
        .unwrap();
    map.insert("k", 1);
    assert_eq!(
        map.shard_for_key(&"k"),
        DefaultRouter.route(map.hash_for_key(&"k"), 8)
    );
}
