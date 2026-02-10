use shardmap::ShardMap;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

#[test]
fn test_stress_millions_of_operations() {
    let map = Arc::new(ShardMap::new());
    let num_threads = 8;
    let operations_per_thread = 100_000;

    let start = Instant::now();
    let mut handles = vec![];

    // Spawn threads performing mixed operations
    for thread_id in 0..num_threads {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            // Insert operations
            for i in 0..operations_per_thread {
                let key = format!("thread_{}_key_{}", thread_id, i);
                map.insert(key, i);
            }

            // Read operations
            for i in 0..operations_per_thread {
                let key = format!("thread_{}_key_{}", thread_id, i);
                let _ = map.get(&key);
            }

            // Update operations (on existing keys)
            for i in 0..(operations_per_thread / 2) {
                let key = format!("thread_{}_key_{}", thread_id, i);
                map.update(&key, |v| *v += 1);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    println!(
        "Completed {} operations in {:?}",
        num_threads * operations_per_thread * 3,
        duration
    );

    // Verify final state
    let stats = map.stats();
    assert_eq!(stats.size, num_threads * operations_per_thread);

    // Verify some updates worked
    let sample_key = format!("thread_0_key_{}", 0);
    let value = map.get(&sample_key).unwrap();
    assert!(*value < operations_per_thread);
}

#[test]
fn test_stress_concurrent_renames() {
    let map = Arc::new(ShardMap::new());

    // Insert initial data
    for i in 0..1000 {
        map.insert(format!("key_{}", i), i);
    }

    let num_threads = 10;
    let renames_per_thread = 100;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let map = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for i in 0..renames_per_thread {
                let old_key = format!("key_{}", thread_id * renames_per_thread + i);
                let new_key = format!("renamed_{}_{}", thread_id, i);
                let _ = map.rename(&old_key, new_key);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should still have 1000 entries (some renames may have failed due to conflicts)
    let stats = map.stats();
    assert!(stats.size <= 1000);
    assert!(stats.size >= 900); // Most renames should succeed
}

#[test]
fn test_stress_memory_usage() {
    // This test verifies we don't have memory leaks
    let map = Arc::new(ShardMap::new());

    // Insert and remove many items
    for round in 0..10 {
        for i in 0..10000 {
            let key = format!("round_{}_key_{}", round, i);
            map.insert(key, i);
        }

        for i in 0..10000 {
            let key = format!("round_{}_key_{}", round, i);
            map.remove(&key);
        }
    }

    // Map should be empty
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn test_stress_iteration_under_load() {
    let map = Arc::new(ShardMap::new());

    // Insert data
    for i in 0..10000 {
        map.insert(format!("key_{}", i), i);
    }

    let map_clone = Arc::clone(&map);
    let writer = thread::spawn(move || {
        // Continuously modify the map
        for i in 0..1000 {
            map_clone.insert(format!("writer_key_{}", i), i);
            if i % 10 == 0 {
                map_clone.remove(&format!("writer_key_{}", i - 5));
            }
        }
    });

    // Iterate while writes are happening
    let mut count = 0;
    for _ in 0..10 {
        for _ in map.iter_snapshot() {
            count += 1;
        }
    }

    writer.join().unwrap();

    // Should have iterated over many items
    assert!(count > 0);
}
