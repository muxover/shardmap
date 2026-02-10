use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dashmap::DashMap;
use hashbrown::HashMap;
use parking_lot::RwLock;
use shardmap::ShardMapBuilder;
use std::sync::Arc;
use std::thread;

fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert");

    // Single-lock HashMap baseline
    group.bench_function("single_lock_hashmap", |b| {
        let map = Arc::new(RwLock::new(HashMap::new()));
        b.iter(|| {
            for i in 0..1000 {
                map.write().insert(i, i);
            }
        });
    });

    // DashMap
    group.bench_function("dashmap", |b| {
        let map = Arc::new(DashMap::new());
        b.iter(|| {
            for i in 0..1000 {
                map.insert(i, i);
            }
        });
    });

    // ShardMap with different shard counts
    for shard_count in [4, 8, 16, 32, 64] {
        group.bench_with_input(
            BenchmarkId::new("shardmap", shard_count),
            &shard_count,
            |b, &shard_count| {
                let map = Arc::new(
                    ShardMapBuilder::new()
                        .shard_count(shard_count)
                        .unwrap()
                        .build::<usize, usize>()
                        .unwrap(),
                );
                b.iter(|| {
                    for i in 0..1000 {
                        map.insert(i, i);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");

    // Prepare data
    let single_map = Arc::new(RwLock::new(HashMap::new()));
    let dashmap = Arc::new(DashMap::new());
    let shardmap_4 = Arc::new(
        ShardMapBuilder::new()
            .shard_count(4)
            .unwrap()
            .build::<usize, usize>()
            .unwrap(),
    );
    let shardmap_16 = Arc::new(
        ShardMapBuilder::new()
            .shard_count(16)
            .unwrap()
            .build::<usize, usize>()
            .unwrap(),
    );
    let shardmap_64 = Arc::new(
        ShardMapBuilder::new()
            .shard_count(64)
            .unwrap()
            .build::<usize, usize>()
            .unwrap(),
    );

    for i in 0..1000 {
        single_map.write().insert(i, i);
        dashmap.insert(i, i);
        shardmap_4.insert(i, i);
        shardmap_16.insert(i, i);
        shardmap_64.insert(i, i);
    }

    // Single-lock HashMap
    group.bench_function("single_lock_hashmap", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(single_map.read().get(&i));
            }
        });
    });

    // DashMap
    group.bench_function("dashmap", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(dashmap.get(&i));
            }
        });
    });

    // ShardMap variants
    group.bench_function("shardmap_4", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(shardmap_4.get(&i));
            }
        });
    });

    group.bench_function("shardmap_16", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(shardmap_16.get(&i));
            }
        });
    });

    group.bench_function("shardmap_64", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(shardmap_64.get(&i));
            }
        });
    });

    group.finish();
}

fn bench_concurrent_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_insert");

    let num_threads = 8;
    let ops_per_thread = 10_000;

    // Single-lock HashMap
    group.bench_function("single_lock_hashmap", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let map = Arc::new(RwLock::new(HashMap::new()));
                let mut handles = vec![];

                for thread_id in 0..num_threads {
                    let map = Arc::clone(&map);
                    let handle = thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let key = thread_id * ops_per_thread + i;
                            map.write().insert(key, key);
                        }
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    handle.join().unwrap();
                }
            }
            start.elapsed()
        });
    });

    // DashMap
    group.bench_function("dashmap", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let map = Arc::new(DashMap::new());
                let mut handles = vec![];

                for thread_id in 0..num_threads {
                    let map = Arc::clone(&map);
                    let handle = thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let key = thread_id * ops_per_thread + i;
                            map.insert(key, key);
                        }
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    handle.join().unwrap();
                }
            }
            start.elapsed()
        });
    });

    // ShardMap with different shard counts
    for shard_count in [4, 16, 64] {
        group.bench_with_input(
            BenchmarkId::new("shardmap", shard_count),
            &shard_count,
            |b, &shard_count| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();
                    for _ in 0..iters {
                        let map = Arc::new(
                            ShardMapBuilder::new()
                                .shard_count(shard_count)
                                .unwrap()
                                .build::<usize, usize>()
                                .unwrap(),
                        );
                        let mut handles = vec![];

                        for thread_id in 0..num_threads {
                            let map = Arc::clone(&map);
                            let handle = thread::spawn(move || {
                                for i in 0..ops_per_thread {
                                    let key = thread_id * ops_per_thread + i;
                                    map.insert(key, key);
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.join().unwrap();
                        }
                    }
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    let num_threads = 8;
    let ops_per_thread = 5_000;

    // Single-lock HashMap
    group.bench_function("single_lock_hashmap", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let map = Arc::new(RwLock::new(HashMap::new()));
                let mut handles = vec![];

                for thread_id in 0..num_threads {
                    let map = Arc::clone(&map);
                    let handle = thread::spawn(move || {
                        // 70% reads, 30% writes
                        for i in 0..ops_per_thread {
                            if i % 10 < 3 {
                                // Write
                                let key = thread_id * ops_per_thread + i;
                                map.write().insert(key, key);
                            } else {
                                // Read
                                let key = (thread_id * ops_per_thread + i)
                                    % (num_threads * ops_per_thread);
                                black_box(map.read().get(&key));
                            }
                        }
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    handle.join().unwrap();
                }
            }
            start.elapsed()
        });
    });

    // DashMap
    group.bench_function("dashmap", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            for _ in 0..iters {
                let map = Arc::new(DashMap::new());
                let mut handles = vec![];

                for thread_id in 0..num_threads {
                    let map = Arc::clone(&map);
                    let handle = thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            if i % 10 < 3 {
                                let key = thread_id * ops_per_thread + i;
                                map.insert(key, key);
                            } else {
                                let key = (thread_id * ops_per_thread + i)
                                    % (num_threads * ops_per_thread);
                                black_box(map.get(&key));
                            }
                        }
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    handle.join().unwrap();
                }
            }
            start.elapsed()
        });
    });

    // ShardMap
    for shard_count in [16, 64] {
        group.bench_with_input(
            BenchmarkId::new("shardmap", shard_count),
            &shard_count,
            |b, &shard_count| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();
                    for _ in 0..iters {
                        let map = Arc::new(
                            ShardMapBuilder::new()
                                .shard_count(shard_count)
                                .unwrap()
                                .build::<usize, usize>()
                                .unwrap(),
                        );
                        let mut handles = vec![];

                        for thread_id in 0..num_threads {
                            let map = Arc::clone(&map);
                            let handle = thread::spawn(move || {
                                for i in 0..ops_per_thread {
                                    if i % 10 < 3 {
                                        let key = thread_id * ops_per_thread + i;
                                        map.insert(key, key);
                                    } else {
                                        let key = (thread_id * ops_per_thread + i)
                                            % (num_threads * ops_per_thread);
                                        black_box(map.get(&key));
                                    }
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.join().unwrap();
                        }
                    }
                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_insert,
    bench_get,
    bench_concurrent_insert,
    bench_mixed_workload
);
criterion_main!(benches);
