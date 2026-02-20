# ShardMap

<div align="center">

[![Crates.io](https://img.shields.io/crates/v/shardmap.svg)](https://crates.io/crates/shardmap)
[![Documentation](https://docs.rs/shardmap/badge.svg)](https://docs.rs/shardmap)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

**Performance-predictable, introspectable concurrent map for Rust.**

[Features](#-features) • [Quick Start](#-quick-start) • [Documentation](https://docs.rs/shardmap) • [Configuration](#️-configuration) • [API Overview](#-api-overview) • [Benchmarks](#-benchmarks) • [Non-goals](#-non-goals) • [Contributing](#-contributing) • [License](#-license)

</div>

---

ShardMap is a concurrent map for engineers who care about **load behavior**: deterministic shard routing, predictable lock isolation, and optional built-in diagnostics.

## ✨ Features

- 🔒 **Zero global lock** — Data is split across shards; each shard has its own lock. Operations on different shards do not block each other.
- 🎯 **Deterministic routing** — The same key always maps to the same shard. Custom routers are supported.
- 📊 **Optional diagnostics** — Enable the `metrics` feature for per-shard read/write/remove and lock counts. `shard_loads()` works without any feature.
- ⚡ **Pre-hashed APIs** — When you already have a hash (e.g. from a packet header), use `get_by_hash`, `insert_by_hash`, `remove_by_hash` to avoid re-hashing for shard selection.

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
shardmap = "0.2"
```

**Optional features:**

| Feature       | Description |
|--------------|-------------|
| `metrics`    | Per-shard read/write/remove and lock-acquisition counters. Enables op counts in `diagnostics()`. |
| `lock-timing` | Per-shard lock wait time. **For debugging and profiling only** — not for production hot paths. |
| `fxhash`     | Use FxHash for shard assignment. |

```toml
# With diagnostics
shardmap = { version = "0.2", features = ["metrics"] }

# Minimal overhead (no metrics)
shardmap = { version = "0.2", default-features = false }
```

## 🚀 Quick Start

```rust
use shardmap::ShardMap;

let map = ShardMap::new();
map.insert("user:1", "Alice");
map.insert("user:2", "Bob");

// Read: get returns Arc<V>, so you can use the value without holding the lock
if let Some(name) = map.get(&"user:1") {
    println!("{}", *name);
}

// Introspection: per-shard entry counts, no feature required
let loads = map.shard_loads();
println!("Entries per shard: {:?}", loads);
```

## ✨ When to use ShardMap

- You need to **see what your map is doing under load** (shard loads, imbalance, hot shards).
- You are **tuning** shard count, capacity, or routing for your CPU and workload.
- You want **predictable shard isolation** and control over shard count (scaling is relative to how many shards you use).
- You are building **rate limiters**, **caches**, or **session stores** that benefit from per-shard visibility or `rename` (atomic within a shard; cross-shard rename acquires two shard locks).

## 📋 API Overview

### Map operations

| Method | Description |
|--------|-------------|
| `insert`, `get`, `remove` | Core operations. |
| `get_or_insert`, `get_or_insert_with`, `try_insert` | Convenience. |
| `update`, `rename` | In-place update; rename is atomic within one shard (cross-shard acquires two locks). |
| `contains_key`, `len`, `is_empty`, `clear`, `retain` | Queries and bulk ops. |
| `capacity`, `shrink_to_fit` | Capacity control. |

### Introspection

| Method | Description |
|--------|-------------|
| `shard_loads()` | Per-shard entry counts. No feature required. |
| `diagnostics()` | Snapshot: `total_entries`, per-shard stats, `total_operations`, `avg_load_per_shard`, **`max_load_ratio`** (you interpret). |
| `stats()` | Per-shard sizes and op counts. |
| `shard_for_key(key)` | Shard index for a key. |
| `hash_for_key(key)` | Hash used for routing. |
| `shard_for_hash(hash)` | Shard index for a precomputed hash. |
| `get_by_hash(key, hash)` | Get using precomputed hash for shard selection. |
| `insert_by_hash(key, value, hash)` | Insert with precomputed hash. |
| `remove_by_hash(key, hash)` | Remove with precomputed hash. |

### Iteration

- **`iter_snapshot()`** — Copies current entries then iterates; consistent view, no lock held during iteration.
- **`iter_concurrent()`** — Iterates while holding shard locks; can see concurrent writes but may see partial state.

## ⚙️ Configuration

```rust
use shardmap::{ShardMapBuilder, HashFunction, RoutingConfig};

// Full control
let map = ShardMapBuilder::new()
    .shard_count(32)?
    .capacity_per_shard(256)
    .hash_function(HashFunction::AHash)
    .routing(RoutingConfig::Default)
    .build::<String, i32>()?;

// Convenience
let map = ShardMap::with_capacity(4096);  // capacity spread across default 16 shards
let map = ShardMap::with_shard_count(64)?;
```

Shard count must be a power of two (2, 4, 8, 16, 32, 64, …). Start with 16 and tune from there.

## 📊 Diagnostics and imbalance

Use **`diagnostics()`** to detect hot shards or imbalance. It returns **`max_load_ratio`** (max shard load ÷ average). There is no built-in threshold — you decide (e.g. alert when `max_load_ratio > 2.0`).

```rust
let diag = map.diagnostics();
println!("Total entries: {}", diag.total_entries);
println!("Max load ratio: {}", diag.max_load_ratio);
for (i, s) in diag.shards.iter().enumerate() {
    if s.entries > 0 {
        println!("  Shard {}: {} entries", i, s.entries);
    }
}
```

Without the `metrics` feature, `diagnostics()` still provides `total_entries`, `shards[].entries`, `avg_load_per_shard`, and `max_load_ratio`; op counts are 0.

## 🔀 Custom shard routing

Implement the `ShardRouter` trait and pass it to the builder:

```rust
use shardmap::{ShardMapBuilder, ShardRouter, RoutingConfig};

struct MyRouter;
impl ShardRouter for MyRouter {
    fn route(&self, key_hash: u64, shard_count: usize) -> usize {
        (key_hash as usize) % shard_count  // or your logic
    }
}

let map = ShardMapBuilder::new()
    .shard_count(16)
    .unwrap()
    .routing(RoutingConfig::Custom(Box::new(MyRouter)))
    .build::<String, i32>()
    .unwrap();
```

Default behavior is `hash & (shard_count - 1)` via `DefaultRouter`.

## 🏁 Benchmarks

Run with:

```bash
cargo bench
```

All ShardMap benchmarks use the **default** build (no `metrics` feature).

## 🏗️ Design

- **Locks** — `parking_lot::RwLock` per shard; no global lock.
- **Storage** — `hashbrown::HashMap` per shard. Values are **`Arc<V>`**: readers clone the `Arc` and use the value without holding the lock (no copy of `V`).
- **Shard count** — Power of two so routing is a bitmask (`hash & (n - 1)`), no division.

## 🚫 Non-goals

ShardMap is focused. The following are explicitly **not** goals:

- **Drop-in for other maps** — Not a replacement for DashMap or std HashMap; different tradeoffs and API.
- **Read-heavy specialization** — Not tuned specifically for read-heavy workloads (consider evmap or similar if that’s your main use case).
- **Dynamic sharding** — No background rebalancing or dynamic shard resizing; shard count is fixed at build time.
- **Eviction or persistence** — No built-in eviction, LRU, or persistence; use with other crates if needed.

## 🤝 Contributing

Contributions are welcome. Please open an [issue](https://github.com/muxover/shardmap/issues) or [pull request](https://github.com/muxover/shardmap) on GitHub.

## 📄 License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)

## 🔗 Links

- **Crates.io**: https://crates.io/crates/shardmap
- **Documentation**: https://docs.rs/shardmap
- **Repository**: https://github.com/muxover/shardmap
- **Issues**: https://github.com/muxover/shardmap/issues
- **Changelog**: [CHANGELOG.md](CHANGELOG.md)

---

<div align="center">

Made with ❤️ by Jax (@muxover)

[⭐ Star us on GitHub](https://github.com/muxover/shardmap) if you find this project useful!

</div>
