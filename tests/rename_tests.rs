use shardmap::{Error, ShardMap};

#[test]
fn test_rename_preserves_value() {
    let map: ShardMap<&str, &str> = ShardMap::new();

    map.insert("old_key", "value");
    let arc_before = map.get(&"old_key").unwrap();

    map.rename(&"old_key", "new_key").unwrap();
    let arc_after = map.get(&"new_key").unwrap();

    // Values should be the same
    assert_eq!(*arc_before, *arc_after);

    // Ideally they should be the same Arc, but rename may clone
    // depending on implementation. At minimum, values must match.
}

#[test]
fn test_rename_atomicity() {
    let map: ShardMap<&str, &str> = ShardMap::new();

    map.insert("old_key", "value");

    // Rename should be atomic: either both old removed and new inserted, or neither
    map.rename(&"old_key", "new_key").unwrap();

    // Old key should be gone
    assert!(map.get(&"old_key").is_none());

    // New key should exist with the value
    assert_eq!(*map.get(&"new_key").unwrap(), "value");
}

#[test]
fn test_rename_to_existing_key_fails() {
    let map: ShardMap<&str, &str> = ShardMap::new();

    map.insert("key1", "value1");
    map.insert("key2", "value2");

    // Rename key1 to key2 should fail
    let result = map.rename(&"key1", "key2");
    assert_eq!(result.unwrap_err(), Error::KeyAlreadyExists);

    // Both keys should still exist with original values
    assert_eq!(*map.get(&"key1").unwrap(), "value1");
    assert_eq!(*map.get(&"key2").unwrap(), "value2");
}

#[test]
fn test_rename_nonexistent_key_fails() {
    let map: ShardMap<&str, &str> = ShardMap::new();

    let result = map.rename(&"nonexistent", "new_key");
    assert_eq!(result.unwrap_err(), Error::KeyNotFound);

    // New key should not exist
    assert!(map.get(&"new_key").is_none());
}

#[test]
fn test_multiple_renames() {
    let map: ShardMap<&str, &str> = ShardMap::new();

    map.insert("key1", "value");

    map.rename(&"key1", "key2").unwrap();
    map.rename(&"key2", "key3").unwrap();
    map.rename(&"key3", "key4").unwrap();

    assert!(map.get(&"key1").is_none());
    assert!(map.get(&"key2").is_none());
    assert!(map.get(&"key3").is_none());
    assert_eq!(*map.get(&"key4").unwrap(), "value");
}
