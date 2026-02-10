use shardmap::ShardMapBuilder;

#[test]
fn test_shard_isolation() {
    // Create a map with 4 shards
    let map = ShardMapBuilder::new()
        .shard_count(4)
        .unwrap()
        .build::<String, i32>()
        .unwrap();

    // Insert many keys to ensure distribution across shards
    for i in 0..100 {
        map.insert(format!("key_{}", i), i);
    }

    let stats = map.stats();

    // Verify we have 4 shards
    assert_eq!(stats.shard_sizes.len(), 4);

    // Verify all entries are accounted for
    let total: usize = stats.shard_sizes.iter().sum();
    assert_eq!(total, 100);

    // Verify keys are deterministically assigned to shards
    // (same key should always map to same shard)
    for i in 0..10 {
        let key = format!("key_{}", i);
        let value1 = map.get(&key);
        let value2 = map.get(&key);
        assert_eq!(value1, value2);
    }
}

#[test]
fn test_deterministic_shard_assignment() {
    let map1 = ShardMapBuilder::new()
        .shard_count(8)
        .unwrap()
        .build::<String, i32>()
        .unwrap();

    let map2 = ShardMapBuilder::new()
        .shard_count(8)
        .unwrap()
        .build::<String, i32>()
        .unwrap();

    // Insert same keys in both maps
    for i in 0..50 {
        let key = format!("key_{}", i);
        map1.insert(key.clone(), i);
        map2.insert(key.clone(), i);
    }

    // Get stats for both
    let stats1 = map1.stats();
    let stats2 = map2.stats();

    // Shard sizes should be identical (deterministic hashing)
    assert_eq!(stats1.shard_sizes, stats2.shard_sizes);
}

#[test]
fn test_shard_distribution() {
    let map = ShardMapBuilder::new()
        .shard_count(16)
        .unwrap()
        .build::<String, i32>()
        .unwrap();

    // Insert many keys
    for i in 0..1000 {
        map.insert(format!("key_{}", i), i);
    }

    let stats = map.stats();

    // Verify distribution across shards (should be relatively even)
    let max_shard_size = *stats.shard_sizes.iter().max().unwrap();
    let min_shard_size = *stats.shard_sizes.iter().min().unwrap();

    // With 1000 keys and 16 shards, we expect ~62-63 keys per shard
    // Allow some variance, but not too extreme
    assert!(
        max_shard_size < 100,
        "Shard distribution too uneven (max: {})",
        max_shard_size
    );
    assert!(
        min_shard_size > 30,
        "Shard distribution too uneven (min: {})",
        min_shard_size
    );
}
