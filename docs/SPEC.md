# Redwood Build System Specification

## Overview

Build system using Datalog for target definitions. Targets are logical clauses in a database. Query the build graph with Datalog. Fast startup, dependency resolution, content-based caching.

## Architecture

HashMap-based Datalog evaluation with semi-naive semantics.

- Facts stored in HashMap with first-argument indexing
- Rules compiled lazily on first query
- Semi-naive evaluation for transitive closure
- BFS optimization for filtered TC queries
- Selective cache invalidation

Performance: 7ms startup for 1K targets, 79ms for 10K targets, <1ms simple queries.

Three core concepts:

1. **Everything is facts** - All data as facts in engine
2. **Datalog evaluation** - Rules derive facts from base facts
3. **BuildKind executes** - Datalog describes WHAT, BuildKind does HOW

### Module Structure

```
src/
├── datalog/             # Complete Datalog system
│   ├── engine.rs        # Query evaluation
│   ├── parser.rs        # Text → AST
│   └── mod.rs           # AST types
├── runtime/             # Fact sources
│   ├── filesystem.rs    # File facts
│   ├── prelude.rs       # Built-in rules
│   └── toolchain_scanner.rs
├── build/               # BuildKind trait
├── sandbox/             # Sandboxing
├── cache/               # Caching
├── cli/                 # CLI
└── format/              # Formatting
```

### Design Goals

1. Fast startup - milliseconds, not seconds
2. Incremental builds - only rebuild what's changed
3. Queryable - ad-hoc queries over build graph
4. Declarative - targets defined by rules
5. Simple - understandable implementation

### Non-Goals

- Full Bazel compatibility
- Remote execution (for now)
- Multi-repo workspaces (for now)

## Core Concepts

### Targets as Facts and Rules

Targets defined as facts:

```datalog
target("//app:server").
kind("//app:server", rust_binary).
sources("//app:server", "src/main.rs").
deps("//app:server", "//lib:http").
```

Derived facts from rules:

```datalog
# Transitive dependencies
transitive_deps(X, Y) :- deps(X, Y).
transitive_deps(X, Z) :- deps(X, Y), transitive_deps(Y, Z).

# Needs rebuild if source hash changed
needs_rebuild(Target) :-
    sources(Target, Source),
    file_hash(Source, Hash),
    cached_hash(Target, Source, OldHash),
    neq(Hash, OldHash).
```

### Build Graph Model

Relations:

- `target(Label)` - target exists
- `sources(Label, Path)` - target depends on source files
- `deps(Label, DepLabel)` - target depends on another target (must be explicit)
- `kind(Label, Kind)` - target has a build kind
- `attr(Label, Key, Value)` - target attributes
- `outputs(Label, Path)` - target produces output files (required for sandbox)

Derived:

- `transitive_deps(Label, DepLabel)` - all dependencies transitively
- `needs_rebuild(Label)` - target needs rebuilding

**Declaration syntax**:

```datalog
# Declare dependencies explicitly
deps("//pkg:target", "//other:dep").

# Declare output files (required for sandboxed builds)
outputs("//pkg:target", "output.exe").
outputs("//pkg:target", "output.so").
```

### Facts Sources

1. **BUILD.datalog files** - target definitions
2. **Lazy filesystem checks** - per-target output existence and source hashes
3. **Toolchain discovery** - available tools on PATH
4. **Cache loading** - cached hashes from previous builds
5. **Prelude rules** - helpers, constraints, rebuild logic

## Language Specification

### Syntax

Datalog subset with build system extensions.

```
head :- body_term1, body_term2, ..., body_termN.
```

- `head` - single predicate
- `body` - comma-separated predicates (all must be true)
- Variables start uppercase
- Atoms/constants start lowercase or are strings

### Built-in Predicates

Filesystem:

```datalog
file_exists(Path)           # file exists
file_modified(Path, Time)   # modification timestamp
file_hash(Path, Hash)       # content hash
glob(Pattern, File)         # pattern match
```

Build system:

```datalog
target(Label)               # target exists
deps(Label, DepLabel)       # dependency
transitive_deps(L, D)       # all dependencies
sources(Label, Path)        # source files
kind(Label, Kind)           # build kind
attr(Label, Key, Value)     # arbitrary attribute
outputs(Label, Path)        # output files
```

**Important**: The `deps()` and `outputs()` predicates must be explicitly declared:

- **deps()**: Dependencies are NOT auto-derived. You must explicitly declare all target dependencies using `deps("//pkg:target", "//other:dep").` This ensures the build system knows the dependency graph for correct build ordering.

- **outputs()**: Output files must be explicitly declared using `outputs("//pkg:target", "output.exe").` The sandbox needs this information to know which files to copy back from the isolated build directory to the workspace. Without `outputs()` declarations, your build artifacts will not be preserved.

Toolchain:

```datalog
tool_available(Tool, Path, Version)  # discovered tool
requires_tool(Target, Tool)          # target needs tool
toolchain(Target, Tool, Path)        # resolved tool path
```

Comparisons:

```datalog
gt(X, Y)   # greater than
lt(X, Y)   # less than
neq(X, Y)  # not equal
```

Negation:

```datalog
needs_rebuild(Target) :-
    target(Target),
    not(output_exists(Target)).
```

### Variables and Constants

Variables: uppercase first letter (`X`, `Label`, `Path`)
Constants: lowercase or strings (`rust_binary`, `"//app:cli"`)

```datalog
# All Rust targets
rust_target(X) :- kind(X, rust_binary).

# Specific target
my_app :- target("//app:cli").
```

## System Predicates

### Filesystem Predicates

Filesystem facts inserted at startup:

```datalog
file_exists("src/main.rs").
file_modified("src/main.rs", 1234567890).
file_hash("src/main.rs", "abc123...").
```

Glob patterns expand to multiple facts:

```datalog
# In BUILD.datalog
sources(Target, File) :-
    target(Target),
    glob("src/**/*.rs", File).

# Expands to
sources("//app:cli", "src/main.rs").
sources("//app:cli", "src/lib.rs").
```

### Toolchain Predicates

System scans PATH for tools:

```datalog
tool_available("cargo", "/usr/bin/cargo", "1.70.0").
tool_available("rustc", "/usr/bin/rustc", "1.70.0").
tool_available("gcc", "/usr/bin/gcc", "12.2.0").
```

Build kinds declare requirements:

```datalog
requires_tool(Target, "cargo") :- kind(Target, rust_binary).
requires_tool(Target, "gcc") :- kind(Target, c_binary).
```

Prelude resolves requirements to available tools:

```datalog
toolchain(Target, Tool, Path) :-
    requires_tool(Target, Tool),
    tool_available(Tool, Path, _).
```

Override with explicit declarations:

```datalog
toolchain_override("cargo", "/home/user/.cargo/bin/cargo").
```

### Rebuild Logic

Prelude defines when targets need rebuilding:

```datalog
# No cached output
needs_rebuild(Target) :-
    target(Target),
    not(cached_output(Target)).

# Source hash changed
needs_rebuild(Target) :-
    sources(Target, Source),
    file_hash(Source, Hash),
    cached_hash(Target, Source, OldHash),
    neq(Hash, OldHash).

# Dependency needs rebuild
needs_rebuild(Target) :-
    deps(Target, Dep),
    needs_rebuild(Dep).
```

## Build Kinds

Single build kind: **system_tool**. Queries attributes from database to construct command invocation.

```rust
impl BuildKind for SystemTool {
    fn plan(&self, target: &TargetLabel, engine: &mut Engine)
        -> Result<BuildPlan, String>
    {
        let tool = query_attr(engine, target, "tool")?;
        let attrs = query_all_attrs(engine, target, &tool);
        let sources = query_sources(engine, target);

        check_constraints(engine, target)?;  // Fails if constraint_failed facts exist

        let args = build_args_from_attrs(attrs, sources, outputs);

        Ok(BuildPlan {
            command: tool_path,
            args,
            inputs: sources,
            outputs: query_outputs(engine, target),
            ...
        })
    }
}
```

All functionality for different tools implemented in datalog prelude via helpers:

- `cargo_binary(Target)` - cargo build --release
- `cargo_binary_debug(Target)` - cargo build
- `cargo_lib(Target)` - cargo build --release --lib
- `cargo_test(Target)` - cargo test
- `system_cc(Target)` - gcc with -fPIC, -Wall
- `system_cxx(Target)` - g++ with -fPIC, -Wall, -std=c++17
- `system_clang(Target)` - clang with -fPIC, -Wall
- `system_clangxx(Target)` - clang++ with -fPIC, -Wall, -std=c++17
- `system_go(Target)` - go with -v
- `system_python(Target)` - python3 with -u, -Werror
- `system_node(Target)` - node with --trace-warnings

Helpers automatically derive:
- `target(Target)` - target existence
- `kind(Target, system_tool)` - build kind
- `attr(Target, "tool", ToolName)` - tool selection
- `attr(Target, Key, Value)` - default flags for tool

## Constraint Checking

Constraints validate targets using datalog rules:

```datalog
constraint_failed(Target, "requires at least one source") :-
    system_cc(Target),
    not(has_sources(Target)).

has_sources(Target) :- sources(Target, Path).
```

BuildKind queries `constraint_failed(Target, Message)` before planning. If any facts exist, build fails with error message.

Constraints fully evaluated in datalog. No special-casing in rust code. Add new constraints by writing rules.

## Build Execution

### Dependency Resolution

Topological sort with cycle detection:

1. Query `transitive_deps(Target, Dep)` for all targets
2. Build dependency graph
3. DFS to compute topological order
4. Detect cycles during traversal
5. Build in dependency order

### Incremental Builds

Content-based caching:

1. Hash all source files with xxhash
2. Query `needs_rebuild(Target)` from prelude rules
3. Skip if false, build if true
4. Store output hashes in `.redwood-cache/`

Cache format:

```json
{
  "target": "//app:cli",
  "inputs": {
    "src/main.rs": "abc123...",
    "src/lib.rs": "def456..."
  },
  "outputs": {
    "target/release/cli": "789abc..."
  },
  "timestamp": 1234567890
}
```

On next build, load cached hashes as `cached_hash` facts and compare.

### Build Execution Flow

```
1. Parse BUILD.datalog → insert facts
2. Compile prelude rules → defines helpers, constraints, rebuild logic
3. Topological sort → dependency order
4. For each target:
   a. Check output existence → insert file_exists facts for outputs
   b. Hash source files → insert file_hash facts for sources
   c. Load cached hashes → insert cached_hash facts
   d. Query needs_rebuild(Target) → decide if rebuild needed
   e. Skip if up-to-date
   f. Query build kind and check constraints
   g. Plan build (query attrs, sources, deps, toolchain)
   h. Execute build
   i. Store output hashes
```

Lazy filesystem scanning: only checks files relevant to current target. Scans ~2 files per target (outputs + sources) instead of entire workspace (15k+ files).

## Datalog Engine Implementation

The engine lives in [src/datalog/engine.rs](src/datalog/engine.rs).

### HashMap Storage

```rust
struct Engine {
    base_facts: HashMap<String, Vec<Rc<Fact>>>,
    rules: HashMap<String, Vec<Rule>>,
    computed: HashMap<String, Vec<Rc<Fact>>>,
    tc_cache: HashMap<String, HashMap<String, Vec<Rc<Fact>>>>,
    indices: Vec<(RefCell<Option<Index>>, RefCell<Option<ReverseIndex>>)>,
}
```

Facts stored with reference counting to avoid expensive cloning. First-argument indexing provides O(1) lookup for filtered queries. Reverse index enables O(1) fact retraction.

### Query Evaluation

```rust
fn query(&mut self, predicate: &str, filters: &[Option<&str>]) -> Vec<Fact> {
    if let Some(first_filter) = filters.get(0) {
        return self.query_indexed(predicate, first_filter);
    }

    if !self.rules.contains_key(predicate) {
        return self.base_facts.get(predicate).cloned().unwrap_or_default();
    }

    if self.is_tc_predicate(predicate) && has_first_filter(filters) {
        return self.query_tc_bfs(predicate, filters);
    }
    self.evaluate_predicate(predicate)
}
```

### Semi-Naive Evaluation

For rules like:

```datalog
tc(X, Y) :- edge(X, Y).
tc(X, Z) :- edge(X, Y), tc(Y, Z).
```

Algorithm:

1. Start with base facts (edges)
2. Apply rules to derive new facts
3. Repeat until no new facts derived (fixpoint)
4. Track "delta" facts to avoid redundant work

### BFS Optimization

For transitive closure queries filtered by starting node:

```datalog
# Query: transitive_deps("//app:cli", X)
```

Use BFS instead of full semi-naive:

1. Start from `"//app:cli"`
2. Find immediate dependencies
3. Recursively explore dependencies
4. Cache result per starting node

Faster for single-target queries. Full TC doesn't scale.

### Cache Invalidation

When facts change:

1. Mark predicates that directly use changed facts
2. Recursively mark predicates that depend on marked predicates
3. Clear computed results for marked predicates
4. Invalidate TC cache entries for affected sources

Selective invalidation avoids full recomputation.

## CLI Interface

### Build Command

```bash
redwood build TARGET...
```

Builds specified targets and dependencies.

Options:
- `--explain` - show why targets need rebuilding (future)
- `--parallel` - parallel execution (future)

### Query Command

```bash
redwood query QUERY
```

Ad-hoc Datalog queries:

```bash
# All targets
redwood query "target(X)"

# All Rust targets
redwood query "kind(X, rust_binary)"

# Dependencies of a target
redwood query "deps(\"//app:cli\", X)"

# Transitive dependencies
redwood query "transitive_deps(\"//app:cli\", X)"

# Source files for target
redwood query "sources(\"//lib:parser\", X)"
```

### Format Command

```bash
redwood format [PATH]
```

Format BUILD.datalog files:

- Consistent indentation
- One fact per line
- Rule bodies indented
- Preserves comments

### Watch Command (Future)

```bash
redwood watch TARGET...
```

Watch filesystem for changes, rebuild automatically.

## Configuration

### Attributes

Attributes configure build behavior:

```datalog
# Custom output directory
attr("//app:cli", "output_dir", "dist/bin").

# Debug build
attr("//app:cli", "profile", "debug").

# Specific binary name for cargo
attr("//app:cli", "bin", "cli").

# Compiler flags
attr("//lib:math", "flag_opt", "-O3").
attr("//lib:math", "flag_march", "-march=native").
```

Build kinds query attributes when planning builds.

### Toolchain Overrides

Override discovered tools:

```datalog
toolchain_override("cargo", "/opt/rust/bin/cargo").
toolchain_override("gcc", "/usr/local/gcc-13/bin/gcc").
```

## Future Work

### Priority 1 - Correctness

**Sandboxing** - Symlink-based build isolation

Create clean directory, symlink declared inputs, execute. Undeclared reads fail naturally.

```rust
fn execute_sandboxed(plan: &BuildPlan) -> Result<()> {
    let sandbox = create_sandbox_dir()?;
    for input in &plan.inputs {
        symlink(input, sandbox.join(input))?;
    }
    execute_in_dir(&plan.command, &plan.args, &sandbox)?;
    Ok(())
}
```

### Priority 2 - Performance

**Parallel execution** - Build independent targets concurrently

Use dependency graph to identify parallelizable targets. Execute with rayon.

**Watch mode** - Rebuild on file changes

Use notify crate. Retract old file facts, insert new ones. Query `needs_rebuild` and rebuild affected targets.

### Priority 3 - Features

**Environment predicates** - Platform-specific builds

```datalog
target("//platform:linux") :- os("linux").
target("//platform:macos") :- os("macos").

arch_flags(Target, "-march=arm64") :-
    target(Target),
    arch("aarch64").
```

**Explain command** - Debug rebuild decisions

```bash
redwood explain //app:cli
```

Show which constraints triggered `needs_rebuild`.

**Multi-repo** - External dependencies

```datalog
deps("//app:cli", "//third_party/lib//core:http").
```

Path format: `//path/to/repo//target:name`

## Implementation Notes

### Why HashMap Not Differential Dataflow?

Profiled both. HashMap 1.5-4x faster on build workloads:

- String conversion overhead in DD
- Thread coordination overhead
- Materialization overhead
- Collection creation overhead

Build systems don't need millisecond update latency. 7ms startup for 1K targets is plenty fast.

Simplicity matters more than asymptotic complexity. HashMap implementation is simple, debuggable, no external runtime.

### Semi-Naive vs Naive Evaluation

Naive evaluation recomputes everything from scratch each iteration. Exponentially expensive for transitive closure.

Semi-naive tracks delta (new facts since last iteration) and only computes with deltas. Linear in rule applications.

For transitive closure with 1K edges, naive takes 500ms, semi-naive takes 5ms.

### BFS Optimization

Full transitive closure computes all pairs. O(n³) doesn't scale.

When querying for single starting node, BFS is faster:

```
Full TC: O(n³) for all pairs
BFS: O(n+e) for single source
```

For `transitive_deps("//app:cli", X)` with 100 deps, BFS takes 200μs vs 50ms for full TC.

Cache BFS results per starting node. Invalidate on edge changes.

### First-Argument Indexing

Most queries filter by first argument:

```datalog
sources("//app:cli", X)    # Filter by target
deps("//app:cli", X)       # Filter by target
```

Index facts by first argument for O(1) lookup:

```rust
first_arg_index: HashMap<predicate, HashMap<first_arg, Vec<Fact>>>
```

Query time: O(1) lookup + O(m) iteration over matching facts, instead of O(n) scan.

## Code Ownership

Path-based ownership system for determining who owns code and who should review changes.

### Declaration

Define ownership using filesystem paths:

```datalog
owner(PathPrefix, Owner)      # Path-based ownership
target_path(Target, Path)     # Map target labels to filesystem paths
```

**Deepest match wins** - more specific paths override less specific ones:

```datalog
owner("src", "core-team").            # Owns all of src/
owner("src/datalog", "datalog-expert"). # Overrides for src/datalog/
```

File at `src/datalog/parser.rs` → owner is `datalog-expert`, not `core-team`.

### Target Path Mapping

Map target labels to filesystem paths (explicit for now):

```datalog
target_path("//src/datalog:parser", "src/datalog").
target_path("//app/api:server", "app/api").
```

Future: automatic derivation from target labels using string builtins.

### Resolution Algorithm

```datalog
# Find all owners whose prefix matches this target's path
potential_reviewers(T, Owner) :-
    target_path(T, P),
    owner(Prefix, Owner),
    prefix(P, Prefix).

# Deepest match wins - eliminate if a longer prefix exists
target_owner(T, Owner) :-
    potential_reviewers(T, Owner),
    not(has_deeper_owner(T, Owner)).
```

Uses negation to eliminate non-maximal matches. Datalog-idiomatic stratification.

### Business Rules

Flexible predicates for different notification policies:

```datalog
direct_owner(T, Owner)             # Owns changed code
impact_owner(T, Owner)             # Owns immediate downstream dependencies
transitive_impact_owner(T, Owner)  # Owns all downstream dependencies
```

Users customize by querying different predicates:
- **Conservative**: `unique_direct_owner(X)` - only changed code owners
- **Standard**: `unique_impact_owner(X)` - include immediate dependencies
- **Aggressive**: `unique_transitive_owner(X)` - all downstream
- **Combined**: `unique_all_owners(X)` - direct + impact owners

### Deduplication

Datalog's set semantics naturally deduplicate:

```datalog
# Even if expert owns 3 changed targets...
unique_direct_owner(Owner) :- direct_owner(_, Owner).
# Returns expert once (set semantics)
```

No `unique()` builtin needed - native datalog semantics handle it.

### Query Examples

```bash
# Who owns this target?
redwood query 'target_owner("//app:server", X)'

# What does alice own?
redwood query 'target_owner(T, "alice")'

# All potential owners (with prefix matches)
redwood query 'potential_reviewers(T, Owner)'

# Direct owners of changes
redwood query 'unique_direct_owner(X)'

# Impact owners (downstream dependencies)
redwood query 'unique_impact_owner(X)'

# All owners (direct + impact)
redwood query 'unique_all_owners(X)'
```

### Custom Business Rules

Users can define their own policies:

```datalog
# Only notify for breaking changes
notify_for_breaking(Owner) :-
    breaking_change(T),
    impact_owner(T, Owner).

# Only notify if > 10 lines changed
notify_high_churn(Owner) :-
    directly_changed(T),
    changed_lines(T, N),
    gt(N, 10),
    target_owner(T, Owner).

# Exclude experimental code
notify_production(Owner) :-
    directly_changed(T),
    not(prefix(T, "//experimental/")),
    target_owner(T, Owner).
```

Then query: `redwood query 'notify_production(X)'`

### Integration with Git Diff

Code ownership integrates with existing git diff infrastructure:

```datalog
# From git_diff.datalog:
directly_changed(Target)        # Targets with direct changes
affected_by_changes(Target)     # Targets needing rebuild (transitive)

# Ownership resolution:
direct_owner(T, Owner) :-
    directly_changed(T),
    target_owner(T, Owner).

impact_owner(T, Owner) :-
    directly_changed(ChangedTarget),
    deps(T, ChangedTarget),
    target_owner(T, Owner).
```

External tools can query ownership and implement notification logic:

```bash
# Get owners to notify
redwood query 'unique_direct_owner(X)' > direct_owners.txt
redwood query 'unique_impact_owner(X)' > impact_owners.txt

# Post to GitHub, Slack, etc. using external scripts
```

### Performance

- **Prefix matching**: O(owners × targets) but only for changed targets
- **Deepest match**: O(matching prefixes per target) - typically 1-3
- **Deduplication**: O(1) via datalog set semantics (free)
- **Impact analysis**: Leverages existing git_diff predicates

No performance concerns for realistic workloads (< 10K targets, < 100 ownership paths).

### Design Philosophy

Pure datalog implementation:
- No Rust changes (except prelude registration)
- Uses existing `prefix()` builtin
- No `unique()` builtin needed
- Explicit `target_path()` declarations
- Flexible business rules via query predicates
- Set semantics provide natural deduplication

The system computes ownership facts. External tools consume these facts and implement notification logic (GitHub, Slack, email, etc.).

## Summary

Build system based on Datalog with HashMap evaluation. Fast startup, dependency resolution, content-based caching. Simple implementation, good performance for typical build workloads.

Next work: sandboxing and fix incremental builds. Then parallel execution. Then features.
