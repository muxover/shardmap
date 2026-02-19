# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2025-02-19

### Added

- **Optional metrics** — New `metrics` feature (off by default). When enabled, per-shard counters for reads, writes, removes, and lock acquisitions. Performance-first default: no overhead unless you opt in.
- **Lock timing** — New `lock-timing` feature (depends on `metrics`). Measures cumulative lock wait time per shard. Intended for debugging and profiling only; not for production hot paths.
- **Diagnostics API** — `diagnostics()` returns a structured snapshot: `Diagnostics` with `total_entries`, per-shard `ShardDiagnostics`, `total_operations`, `avg_load_per_shard`, and raw `max_load_ratio` (you interpret; no hardcoded threshold).
- **Shard loads** — `shard_loads() -> Vec<usize>` returns per-shard entry counts. Works without the `metrics` feature; use for imbalance detection in minimal builds.
- **Custom shard routing** — `ShardRouter` trait and `RoutingConfig` (Default | Custom(Box<dyn ShardRouter>)). `DefaultRouter` for default `hash & (n-1)` behavior. Builder: `.routing(routing)`.
- **Pre-hashed key APIs** — `hash_for_key(key) -> u64`, `shard_for_hash(hash) -> usize`, `get_by_hash(key, hash)`, `insert_by_hash(key, value, hash)`, `remove_by_hash(key, hash)` for hot paths where you already have a hash.
- **ShardOps extensions** — `lock_acquisitions` and `lock_wait_nanos` fields (0 when features disabled).

### Changed

- **Default features** — `default = []`. No metrics by default. Use `features = ["metrics"]` for v0.1-style always-on stats.
- **Config** — Added `routing: RoutingConfig` and `capacity_per_shard`. Config no longer implements `Clone` (routing can be custom).
- **Positioning** — Crate repositioned as a performance-predictable, introspectable concurrent map for engineers who care about load behavior.

### Fixed

- None in this release.

### Migration from 0.1.x

- Bump dependency to `shardmap = "0.2"`.
- To retain always-on stats: add `features = ["metrics"]`.
- For minimal overhead: use `default-features = false`.
- `stats()` and `ShardOps` remain; new fields are additive. See [README](README.md) for usage.

---

## [0.1.0] - Initial release

### Added

- Concurrent sharded map: `ShardMap<K, V>` with per-shard locks, no global lock.
- Core API: `insert`, `get`, `remove`, `update`, `rename`, `contains_key`, `len`, `is_empty`, `clear`, `retain`, `capacity`, `shrink_to_fit`.
- Convenience: `get_or_insert`, `get_or_insert_with`, `try_insert`.
- Iteration: `iter_snapshot()`, `iter_concurrent()`.
- Configuration: `ShardMapBuilder`, `Config`, `HashFunction` (AHash default, FxHash optional).
- Statistics: `stats() -> Stats` with per-shard sizes and operation counts (reads, writes, removes).
- `shard_for_key(key)` for observability.
- Values stored as `Arc<V>` for zero-copy sharing.

[0.2.0]: https://github.com/muxover/shardmap/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/muxover/shardmap/releases/tag/v0.1.0
