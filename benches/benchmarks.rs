//! ShardMap benchmarks.
//!
//! Focused on lib identity: scaling by shard count, default (no `metrics`) performance,
//! and pre-hash API (get vs get_by_hash). Run with:
//!
//! ```bash
//! cargo bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shardmap::ShardMapBuilder;
use std::sync::Arc;
use std::thread;

fn bench_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert");

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

    for shard_count in [4, 16, 64] {
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
                for i in 0..1000 {
                    map.insert(i, i);
                }
                b.iter(|| {
                    for i in 0..1000 {
                        black_box(map.get(&i));
                    }
                });
            },
        );
    }

    group.finish();
}

/// get_by_hash vs get: when caller already has a hash, get_by_hash skips shard hashing.
fn bench_get_by_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_by_hash");

    let map = Arc::new(
        ShardMapBuilder::new()
            .shard_count(16)
            .unwrap()
            .build::<usize, usize>()
            .unwrap(),
    );
    for i in 0..1000 {
        map.insert(i, i);
    }

    group.bench_function("get", |b| {
        b.iter(|| {
            for i in 0..1000 {
                black_box(map.get(&i));
            }
        });
    });

    group.bench_function("get_by_hash", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let h = map.hash_for_key(&i);
                black_box(map.get_by_hash(&i, h));
            }
        });
    });

    group.finish();
}

fn bench_concurrent_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_insert");

    let num_threads = 8;
    let ops_per_thread = 10_000;

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
    bench_get_by_hash,
    bench_concurrent_insert,
    bench_mixed_workload
);
criterion_main!(benches);
