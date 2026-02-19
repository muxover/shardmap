//! Simple concurrency tests: core map behavior under threads and introspection after load.

use shardmap::ShardMap;
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_inserts() {
    let map = Arc::new(ShardMap::new());
    let mut handles = vec![];

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

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(map.len(), 1000);
}

#[test]
fn test_concurrent_reads() {
    let map = Arc::new(ShardMap::new());
    for i in 0..100 {
        map.insert(format!("key_{}", i), i);
    }

    let mut handles = vec![];
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

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_insert_then_introspect() {
    let map = Arc::new(ShardMap::new());
    let mut handles = vec![];

    for t in 0..4 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for i in 0..500 {
                map.insert(format!("t{}_k{}", t, i), i);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let loads = map.shard_loads();
    assert_eq!(loads.len(), 16);
    assert_eq!(loads.iter().sum::<usize>(), 2000);

    let diag = map.diagnostics();
    assert_eq!(diag.total_entries, 2000);
    assert!(diag.max_load_ratio >= 1.0);
}
