//! One simple load test: concurrent inserts and removes, then verify state and introspection.

use shardmap::ShardMap;
use std::sync::Arc;
use std::thread;

#[test]
fn test_under_load_then_introspect() {
    let map = Arc::new(ShardMap::new());
    let mut handles = vec![];

    for t in 0..4 {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for i in 0..2000 {
                let key = format!("t{}_k{}", t, i);
                map.insert(key, i);
            }
            for i in 0..2000 {
                let key = format!("t{}_k{}", t, i);
                let _ = map.remove(&key);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
    let loads = map.shard_loads();
    assert_eq!(loads.iter().sum::<usize>(), 0);
}
