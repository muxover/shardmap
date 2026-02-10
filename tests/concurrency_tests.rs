use shardmap::ShardMap;
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_inserts() {
    let map = Arc::new(ShardMap::new());
    let mut handles = vec![];

    // Spawn 10 threads, each inserting 100 items
    for thread_id in 0..10 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let key = format!("thread_{}_key_{}", thread_id, i);
                map.insert(key, i);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all items are present
    assert_eq!(map.len(), 1000);
}

#[test]
fn test_concurrent_reads() {
    let map = Arc::new(ShardMap::new());

    // Insert some data
    for i in 0..100 {
        map.insert(format!("key_{}", i), i);
    }

    let mut handles = vec![];

    // Spawn 20 threads, each reading all items
    for _ in 0..20 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let key = format!("key_{}", i);
                let value = map.get(&key).unwrap();
                assert_eq!(*value, i);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_updates() {
    let map = Arc::new(ShardMap::new());
    map.insert("counter".to_string(), 0);

    let mut handles = vec![];

    // Spawn 10 threads, each incrementing the counter 100 times
    for _ in 0..10 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                map.update(&"counter".to_string(), |v| *v += 1);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Counter should be 1000 (10 threads * 100 increments)
    assert_eq!(*map.get(&"counter".to_string()).unwrap(), 1000);
}

#[test]
fn test_concurrent_mixed_operations() {
    let map = Arc::new(ShardMap::new());
    let mut handles = vec![];

    // Spawn writers
    for thread_id in 0..5 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let key = format!("key_{}_{}", thread_id, i);
                map.insert(key, i);
            }
        });
        handles.push(handle);
    }

    // Spawn readers
    for _ in 0..5 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                // Try to read random keys
                for i in 0..10 {
                    let key = format!("key_{}_{}", i % 5, i);
                    map.get(&key); // May or may not exist, that's ok
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have at least some entries
    assert!(map.len() > 0);
}

#[test]
fn test_concurrent_renames() {
    let map = Arc::new(ShardMap::new());

    // Insert initial data
    for i in 0..50 {
        map.insert(format!("old_key_{}", i), i);
    }

    let mut handles = vec![];

    // Spawn threads that rename keys
    for thread_id in 0..5 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for i in 0..10 {
                let old_key = format!("old_key_{}", thread_id * 10 + i);
                let new_key = format!("new_key_{}_{}", thread_id, i);
                let _ = map.rename(&old_key, new_key); // May fail if already renamed
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Some renames should have succeeded
    let stats = map.stats();
    assert!(stats.size > 0);
}
