# Redwood Build System Benchmarks

Performance benchmarks for the Redwood build system using HashMap-based Datalog evaluation.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench scalability
cargo bench --bench startup
cargo bench --bench incremental
cargo bench --bench query_patterns
cargo bench --bench full_tc_small
```

## Benchmark Suite

### scalability.rs
Tests performance at various scales: 1K, 10K, 100K, 1M targets.

**Measures:**
- Fact insertion time
- Rule compilation time
- Query performance
- Memory usage

**Goals:** <10s total time for 1M targets

### startup.rs
Measures cold-start time from BUILD.datalog parsing to ready database.

**Measures:**
- BUILD file parsing
- Initial fact insertion
- First query latency

**Goals:** <1ms startup time

### incremental.rs
Tests incremental update performance (fact insertion, retraction, recompilation).

**Measures:**
- Insert new facts while rules exist
- Retract facts and query
- Recompile rules after fact changes

**Goals:** <10ms for single fact insert+query

**Current Status:** System clears computed cache on every insert

### query_patterns.rs
Benchmarks common query patterns at various scales.

**Patterns Tested:**
1. Base fact lookup
2. Filtered fact query
3. Join query (bottleneck for large graphs)
4. Lazy transitive closure
5. Fanout query
6. Fanin query

**Goals:** Simple queries <1ms, Lazy TC <100ms

**Note:** Full TC removed (doesn't scale). See full_tc_small.rs.

### full_tc_small.rs
Full transitive closure benchmark (limited to ≤100 targets).

**Why limited?** Full TC is O(n³) and doesn't scale.

**Use cases:**
- Whole-graph analysis tools
- Cache warming (questionable value)
- NOT normal builds (use lazy TC instead)

**Goal:** Document that full TC is not a design goal

## Performance Goals (from Spec)

| Metric | Goal | Benchmark |
|--------|------|-----------|
| Startup | <1ms | startup.rs |
| Simple queries | <1ms | query_patterns.rs |
| Complex TC | <100ms | query_patterns.rs |
| 1M targets | <10s total | scalability.rs |

## Implementation Notes

### Current Architecture
- HashMap-based storage with lazy evaluation
- Semi-naive evaluation for transitive closure
- BFS optimization for filtered TC queries
- Simple and fast for build system workloads

### Optimization Priorities
1. Index on first argument of predicates (enables O(1) filtered lookups)
2. Smart cache invalidation (only clear affected TC results)
3. Pre-compute common queries
4. Batch operations

## Interpreting Results

Benchmarks run in release mode with optimizations. Typical results on modern hardware:

- **scalability.rs**: Should scale linearly with target count
- **startup.rs**: Should be <1ms for typical projects
- **query_patterns.rs**: Simple queries <1ms, complex queries <100ms
- **full_tc_small.rs**: Reference only - full TC doesn't scale

If benchmarks show regressions, profile with:
```bash
cargo bench --bench <name> -- --profile-time 10
```
