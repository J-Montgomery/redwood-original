# Redwood Build System Tutorial

Learn Redwood by example. Run these commands on the redwood repository itself to see the build system in action.

## Prerequisites

```bash
# Clone and build redwood
git clone <repo>
cd redwood
cargo build --release
export PATH=$PWD/target/release:$PATH
```

## Part 1: Query Your Build

###Example 1: List All Targets

```bash
redwood query "target(X)"
```

Shows all targets declared in BUILD.datalog. Datalog's set semantics automatically deduplicate.

**Output**:
```
target("//redwood:redwood")
target("//redwood:fake_tool")
target("//bootstrap:redwood")
target("//src/datalog:datalog")
target("//src/runtime:runtime")
target("//src/cache:cache")
target("//src/format:format")
target("//src/build:build")
target("//src/sandbox:sandbox")
target("//src/cli:cli")
```

### Example 2: Find Dependencies

```bash
# Direct dependencies
redwood query "deps(\"//src/cli:cli\", X)"

# Reverse dependencies (what depends on X?)
redwood query "deps(X, \"//src/datalog:datalog\")"

# Transitive closure
redwood query "transitive_deps(\"//src/cli:cli\", X)"
```

**Key insight**: Build graph is queryable data, not opaque internals.

**Try it**:
```bash
# Count how many things depend on datalog
redwood query "deps(X, \"//src/datalog:datalog\")" | wc -l
```

### Example 3: Count Dependencies

```bash
# Count transitive dependencies using shell tools
redwood query "transitive_deps(\"//src/cli:cli\", X)" | wc -l
```

Shows the number of transitive dependencies for the cli module. Note: Datalog's `count()` aggregation counts facts of a predicate, not pattern matches, so we use shell tools for this type of counting.

## Part 2: Build Targets

### Example 4: Build with Cargo

```bash
redwood build //redwood:redwood
```

Uses cargo_binary() helper from prelude. Runs: `cargo build --release`

This is the production build path - fast, reliable, uses cargo's incremental compilation.

### Example 5: Bootstrap Build with rustc

```bash
# First, generate external .rlib files with cargo
cargo build --release

# Now build redwood modules with rustc (not cargo!)
redwood build //bootstrap:redwood
```

**What's happening**:
1. Redwood resolves `//bootstrap:redwood` → `//src/cli:cli` (via alias)
2. Builds dependency chain: datalog → runtime/cache/format/build → sandbox → cli
3. For each module, invokes rustc with --extern flags for dependencies
4. Parallel builds for independent modules (cache, format build concurrently after datalog)
5. Incremental compilation via cache (only rebuilds changed modules)

**Key insight**: Fine-grained module builds, parallel compilation, incremental caching. Same performance characteristics as cargo, but with explicit control over module boundaries.

Individual modules can also be built:
```bash
redwood build //src/datalog:datalog
redwood build //src/runtime:runtime
```

### Example 6: Incremental Builds

```bash
# First build
redwood build //src/datalog:datalog

# Touch a file
touch src/datalog/parser.rs

# Rebuild - uses cache, sees parser.rs changed
redwood build //src/datalog:datalog
```

Cache key = hash of (sources + deps + attributes + toolchain). Uses xxhash for speed.

**Only changed modules rebuild**. If you change datalog, runtime rebuilds but cache/format don't (they're independent).

##Part 3: Git Diff Integration

### Example 7: What Changed?

```bash
# Make a change and commit it
echo "# Test comment" >> src/datalog/parser.rs
git add . && git commit -m "test change"

# What targets changed?
redwood query "directly_changed(X)"
```

**Expected output**:
```
directly_changed("//src/datalog:datalog")
```

Compares BUILD.datalog at HEAD vs HEAD~1 using shell() builtin. Detects:
- New targets
- Changed sources
- Changed dependencies

### Example 8: Impact Analysis

```bash
# What needs rebuilding due to changes?
redwood query "affected_by_changes(X)"
```

**Expected output**:
```
affected_by_changes("//src/datalog:datalog")
affected_by_changes("//src/runtime:runtime")
affected_by_changes("//src/cache:cache")
affected_by_changes("//src/format:format")
affected_by_changes("//src/build:build")
affected_by_changes("//src/sandbox:sandbox")
affected_by_changes("//src/cli:cli")
affected_by_changes("//bootstrap:redwood")
```

Shows transitive impact. datalog changed → everything that depends on it needs rebuild.

**Key insight**: Other build systems compute this in CI scripts. Redwood computes it in datalog, making it queryable and composable.

**Use case**: Incremental CI
```bash
# Only test affected targets
TARGETS=$(redwood query "affected_by_changes(X)" | cut -d'(' -f2 | cut -d')' -f1 | tr '\n' ' ')
redwood build $TARGETS
```

## Part 4: License Validation

### Example 9: Detect License Conflicts

License validation happens automatically during build planning.

If you had declared conflicting licenses:
```datalog
license("//myapp:server", "PROPRIETARY").
deps("//myapp:server", "//some:gpl_lib").
license("//some:gpl_lib", "GPL-3.0").
```

**Result**: Build fails before compilation:
```
constraint_failed("//myapp:server", "depends on GPL/AGPL library but is proprietary")
```

Validation runs BEFORE any compilation. Saves time catching license issues early.

### Example 10: Query License Dependencies

```bash
# What licenses does a target depend on?
redwood query "has_license(\"//redwood:redwood\", License)"
```

has_license/2 computes transitive license closure. Shows all licenses in the dependency tree (target + all transitive dependencies).

**More examples**:
```bash
# Find all GPL dependencies
redwood query "has_license(T, \"GPL-3.0\")"

# Find proprietary targets
redwood query "license(T, \"PROPRIETARY\")"
```

**Key insight**: License compliance as database query, not manual audits or external tools.

## Part 5: Code Ownership

### Example 11: Who Owns What?

```bash
# All ownerships
redwood query "target_owner(T, Owner)"
```

**Expected output**:
```
target_owner("//src/datalog:datalog", "datalog-expert")
target_owner("//src/runtime:runtime", "runtime-team")
target_owner("//src/build:build", "build-team")
target_owner("//src/cache:cache", "core-team")
target_owner("//src/format:format", "core-team")
target_owner("//src/sandbox:sandbox", "core-team")
target_owner("//src/cli:cli", "core-team")
```

Path-based ownership with deepest match wins:
- `owner("src", "core-team")` - owns all of src/
- `owner("src/datalog", "datalog-expert")` - overrides for src/datalog/


```bash
redwood query 'target_owner("//src/cli:cli", Owner)'
```

```bash
query("core-team").
```

### Example 12: Who to Notify for Changes?

```bash
# After making changes (directly_changed facts exist from git diff)
redwood query "unique_direct_owner(Owner)"
```

Shows owners of changed code. Deduplicated automatically via datalog set semantics.

```bash
# Include impact owners (downstream dependencies)
redwood query "unique_all_owners(Owner)"
```

Shows owners of changed code + owners of code that depends on changed code.

**Key insight**: Code review notifications as database query. Business rules in datalog, not scripts:

```datalog
# Custom policy: notify for breaking changes only
notify_breaking(Owner) :-
    directly_changed(T),
    attr(T, "breaking", "true"),
    impact_owner(T, Owner).
```

## Part 6: Writing BUILD.datalog

### Basic Rust Binary

```datalog
cargo_binary("//myapp:server").
sources("//myapp:server", "src/main.rs").
license("//myapp:server", "MIT").
```

That's it. cargo_binary() expands to:
```datalog
target("//myapp:server").
kind("//myapp:server", system_tool).
attr("//myapp:server", "tool", "cargo").
attr("//myapp:server", "0", "build").
attr("//myapp:server", "1", "--release").
attr("//myapp:server", "no_sandbox", "true").
```

The prelude (prelude/toolchain_discovery.datalog) defines these helpers so you don't have to.

### With Dependencies

```datalog
cargo_binary("//myapp:server").
sources("//myapp:server", "src/main.rs").
deps("//myapp:server", "//lib:auth").
license("//myapp:server", "MIT").

cargo_lib("//lib:auth").
sources("//lib:auth", "lib/auth.rs").
license("//lib:auth", "MIT").
```

Redwood topologically sorts dependencies. Builds //lib:auth before //myapp:server.

### C Binary

```datalog
system_cc("//native:parser").
sources("//native:parser", "native/parser.c").
sources("//native:parser", "native/lexer.c").
outputs("//native:parser", "target/gcc/parser").
attr("//native:parser", "-o", "{output}").
attr("//native:parser", "-O3", "").
license("//native:parser", "MIT").
```

system_cc() expands to gcc invocation with sources as arguments. Template {output} gets replaced with actual output path.

### Custom Tool

```datalog
target("//proto:api").
kind("//proto:api", system_tool).
sources("//proto:api", "proto/api.proto").
outputs("//proto:api", "gen/api.rs").
attr("//proto:api", "tool", "protoc").
attr("//proto:api", "0", "--rust_out=gen").
attr("//proto:api", "1", "{sources}").
license("//proto:api", "MIT").
```

Runs: `protoc --rust_out=gen proto/api.proto`

Attributes with numeric keys (0, 1, 2...) become positional arguments.
Attributes with string keys become flags.
Templates {sources}, {output}, {target} get expanded.

## Part 7: Advanced Queries

### Find Targets by Type

```bash
# All cargo binaries
redwood query "cargo_binary(X)"

# All rustc modules
redwood query "system_rustc(X)"

# All C/C++ targets
redwood query "system_cc(X)"
```

### Find Targets Without Tests

Add a custom rule to your BUILD.datalog:
```datalog
has_test(Target) :-
    cargo_test(TestTarget),
    deps(TestTarget, Target).

no_test(Target) :-
    target(Target),
    not(has_test(Target)).
```

Query:
```bash
redwood query "no_test(X)"
```

Shows all targets that don't have test coverage.

### Find Large Dependency Fans

Add a custom rule:
```datalog
many_dependents(Target) :-
    target(Target),
    count(deps(_, Target), N),
    gt(N, 5).
```

Query:
```bash
redwood query "many_dependents(X)"
```

Finds targets with more than 5 reverse dependencies (things that depend on them).

### Find Where Targets Are Defined

```bash
# Find all target definitions and their locations
redwood query "source_location(Target, File, Line)"

# Find where a specific target is defined
redwood query "source_location(\"//src/cli:cli\", File, Line)"
```

Shows the BUILD.datalog file and line number where each target is defined. Useful for navigating large codebases.

### Debug Attribute Values

```bash
redwood query "attr(\"//src/datalog:datalog\", Key, Value)"
```

Shows all attributes for a target. Useful for debugging what command will be executed.

**Example output**:
```
attr("//src/datalog:datalog", "tool", "rustc")
attr("//src/datalog:datalog", "0", "--crate-type")
attr("//src/datalog:datalog", "1", "lib")
attr("//src/datalog:datalog", "2", "--edition")
attr("//src/datalog:datalog", "3", "2021")
attr("//src/datalog:datalog", "4", "--crate-name")
attr("//src/datalog:datalog", "5", "datalog")
attr("//src/datalog:datalog", "6", "src/datalog/mod.rs")
attr("//src/datalog:datalog", "-o", "target/rustc/libdatalog.rlib")
```

From these attributes, you can reconstruct the exact command:
```bash
rustc --crate-type lib --edition 2021 --crate-name datalog src/datalog/mod.rs -o target/rustc/libdatalog.rlib
```

### Visualize Dependency Graph

```bash
# Query deps and convert to DOT format
echo "digraph deps {" > deps.dot
redwood query "deps(X, Y)" | sed 's/query("\([^"]*\)", "\([^"]*\)")\./  "\1" -> "\2";/' >> deps.dot
echo "}" >> deps.dot

# Render with graphviz
dot -Tpng deps.dot -o deps.png
```

Or use the built-in graph generator if configured:
```bash
redwood build //graph:deps
```

## Part 8: Performance and Caching

### How Caching Works

Redwood caches build outputs based on a hash of:
1. All source files (content hash)
2. All dependency outputs (transitive)
3. All attributes (flags, tool, etc.)
4. Toolchain version

Cache key = xxhash3 of all these inputs.

**This means**:
- Change a source file → cache miss → rebuild
- Change a dependency → cache miss → rebuild
- Change compile flags → cache miss → rebuild
- Change nothing → cache hit → skip

### Incremental Builds

```bash
# First build (cold cache)
time redwood build //bootstrap:redwood

# Touch unrelated file
touch README.md

# Rebuild (hot cache - nothing changed)
time redwood build //bootstrap:redwood  # Should be instant

# Touch source file in one module
touch src/datalog/parser.rs

# Rebuild (partial cache hit)
time redwood build //bootstrap:redwood
# Only rebuilds: datalog (changed) + everything that depends on it
# Does NOT rebuild: independent external dependencies
```

### Parallel Builds

Redwood uses rayon for parallel builds. Independent targets build concurrently:

```
datalog (builds first)
├── runtime (depends on datalog)
├── cache (depends on datalog, builds in parallel with runtime)
└── format (depends on datalog, builds in parallel with runtime/cache)
```

Use RUST_LOG to see parallelism:
```bash
RUST_LOG=debug redwood build //bootstrap:redwood 2>&1 | grep "Building"
```

## Conclusion

Redwood = build system + query engine.

**Traditional build systems**:
- Opaque DAG
- Limited introspection
- Scripted constraints
- External tools for analysis

**Redwood**:
- Queryable database
- Full introspection via datalog
- Declarative constraints (license, ownership)
- Extensible in datalog, not scripts

### Try Writing Custom Rules

The real power is composability. Mix and match predicates:

```datalog
# Find GPL code without proper review
unreviewed_gpl(T) :-
    license(T, License),
    strong_copyleft(License),
    not(attr(T, "reviewed", "true")).

# Find high-risk changes (GPL + directly changed)
high_risk_change(T) :-
    directly_changed(T),
    unreviewed_gpl(T).

# Notify security team for high-risk changes
notify_security(Owner) :-
    high_risk_change(T),
    target_owner(T, "security-team").
```

Query:
```bash
redwood query "notify_security(X)"
```

**Everything is data. Query it.**

## Next Steps

1. Read [SPEC.md](SPEC.md) for language reference
2. Read [LICENSE_VALIDATION.md](LICENSE_VALIDATION.md) for license system details
3. Explore prelude/*.datalog to see helper rules
4. Write your own BUILD.datalog for your project
5. Extend the prelude with custom predicates for your workflow

## Troubleshooting

### "Target not found"
- Check spelling of target labels
- Ensure target is declared in BUILD.datalog

### "Tool not found"
- Run `redwood query "tool_available(Tool, Path, Version)"`to see discovered tools
- Install missing tool or use toolchain_override

### "Constraint failed"
- Read the error message (tells you which constraint and why)
- Common: missing license declaration, missing sources, GPL conflicts

### "Incremental build slower than expected"
- Check if cache directory has permissions
- Check if sources are on network filesystem (slower hashing)
- Use RUST_LOG=debug to see what's being rebuilt

### Build fails with rustc errors
- First try with cargo: `cargo build --release`
- If cargo works but rustc fails, check:
  - External deps were built first (cargo build --release)
  - paths match (lib*.rlib files exist)
  - Module exports (pub use) are correct

### Common Sandbox Issues

Redwood uses sandboxing to ensure builds only access declared inputs. This can surface issues that might be hidden in non-sandboxed builds.

#### "File exists" error
**Cause**: Duplicate files declared in `sources()`.

**Fix**: Check your BUILD.datalog for duplicate source declarations:
```bash
redwood query "sources(\"//your:target\", X)" | sort | uniq -d
```
Remove duplicate entries.

#### "Missing output" error
**Cause**: Output file not declared in `outputs()` predicate.

**Fix**: Add the missing output to your BUILD.datalog:
```datalog
outputs("//your:target", "path/to/output.exe").
```

The sandbox needs explicit `outputs()` declarations to know which files to copy back from the isolated build directory.

#### "Input does not exist" error
**Cause**: Build tried to read a file that wasn't declared in `sources()`.

**Fix**: Add the missing file to your BUILD.datalog:
```datalog
sources("//your:target", "path/to/missing_file.txt").
```

If the file is generated by a dependency, ensure:
1. The dependency is declared: `deps("//your:target", "//dep:target").`
2. The generated file is in the dependency's `outputs()`: `outputs("//dep:target", "path/to/missing_file.txt").`

#### When to use no_sandbox

Some build tools require access to resources outside the workspace:

**Use `no_sandbox` for**:
- **Cargo builds**: Need access to `~/.cargo` cache and registry
- **Go builds**: Need access to `~/go/pkg` module cache
- **npm/yarn**: Need access to global package cache

**Example**:
```datalog
cargo_binary("//app:server").
sources("//app:server", "src/main.rs").
attr("//app:server", "no_sandbox", "true").
```

Note: The `cargo_binary()`, `system_go()`, and similar helpers in the prelude automatically set `no_sandbox` for you.

**Avoid `no_sandbox` for**:
- Simple compilation tasks (gcc, rustc with explicit dependencies)
- Custom tools that can work in isolation
- Any build that doesn't need external caches

Sandboxed builds are more reliable and reproducible. Use `no_sandbox` only when necessary.

## Part 9: Enforcing Sandbox Policy with Datalog

Redwood's Datalog foundation lets you enforce organizational build policies using constraint rules. This is particularly powerful for sandbox enforcement.

### Example 13: Prohibit Sandbox Bypass

Add this to your BUILD.datalog or prelude to fail builds that bypass the sandbox:

```datalog
# Fail any target that disables sandboxing
constraint_failed(Target, "sandbox bypass not allowed") :-
    target(Target),
    attr(Target, "no_sandbox", "true").
```

Now any target with `no_sandbox` will fail to build:

```bash
redwood build //app:server
# Error: constraint_failed("//app:server", "sandbox bypass not allowed")
```

### Example 14: Allow Exceptions with Approval

A more flexible policy allows sandbox bypass for approved targets:

```datalog
# Fail sandbox bypass unless explicitly approved
constraint_failed(Target, "sandbox bypass requires approval") :-
    target(Target),
    attr(Target, "no_sandbox", "true"),
    not(attr(Target, "sandbox_exception_approved", "true")).
```

Approved targets must declare the exception:

```datalog
cargo_binary("//legacy:app").
sources("//legacy:app", "src/main.rs").
attr("//legacy:app", "no_sandbox", "true").
attr("//legacy:app", "sandbox_exception_approved", "true").
```

### Example 15: Allow Bypass for Specific Tools Only

Restrict sandbox bypass to specific build tools:

```datalog
# Define allowed tools
allowed_no_sandbox_tool("cargo").
allowed_no_sandbox_tool("go").
allowed_no_sandbox_tool("npm").

# Fail if no_sandbox used with disallowed tool
constraint_failed(Target, "only cargo/go/npm may bypass sandbox") :-
    target(Target),
    attr(Target, "no_sandbox", "true"),
    attr(Target, "tool", Tool),
    not(allowed_no_sandbox_tool(Tool)).
```

### Example 16: Audit Sandbox Exceptions

Query which targets bypass the sandbox:

```bash
# Find all targets with sandbox disabled
redwood query "attr(Target, \"no_sandbox\", \"true\")"

# Find unapproved sandbox bypasses
redwood query "attr(Target, \"no_sandbox\", \"true\"), not(attr(Target, \"sandbox_exception_approved\", \"true\"))"
```

### Key Insight: Policy as Code

Traditional build systems enforce policies through:
- External linters
- Pre-commit hooks
- Manual review
- CI scripts

Redwood enforces policies through:
- Datalog constraint rules
- Evaluated during build planning (before execution)
- Queryable for auditing
- Declarative and version-controlled

This approach demonstrates Datalog's power for build system customization: business rules and compliance policies live alongside build definitions, not in separate tooling.

## Part 10: Minimal Lint Targets from Changed Files

A common problem: given changed source files, find the **unique set of targets** to lint. Datalog's set semantics make this trivial.

### The Simple Case: Direct Sources

If each source file belongs to exactly one target (the common case), you just need the unique targets:

```bash
# Inject changed files and query which targets own them
redwood query "sources(Target, _)" \
    --with 'changed_file("src/foo.c").' \
    --with 'changed_file("src/bar.c").' \
    --with 'lint_target(T) :- changed_file(F), sources(T, F).'
```

Or more directly:
```bash
# For each changed file, find its target
for f in $(git diff --name-only HEAD~1); do
    redwood query "sources(T, \"$f\")"
done | sort -u
```

Datalog's set semantics automatically deduplicate—if `foo.c` and `bar.c` both belong to `//lib:utils`, you get one result.

### Example 17: Transitive Source Coverage

For headers included transitively through deps, define a `covers` predicate:

```datalog
# Direct coverage: target lists the source
covers(Target, Source) :- sources(Target, Source).

# Transitive coverage: target depends on something that covers the source
covers(Target, Source) :-
    deps(Target, Dep),
    covers(Dep, Source).
```

Now find all targets affected by a header change:
```bash
redwood query "covers(T, \"include/common.h\")"
```

### Example 18: Minimal Covering Set (Hitting Set)

The harder problem: find the **smallest** set of targets that covers all changed files. This is the [Hitting Set Problem](https://en.wikipedia.org/wiki/Set_cover_problem#Hitting_set_formulation)—NP-complete in general.

**Fast path**: If one target covers everything, use it:

```datalog
changed_file("src/a.h").
changed_file("src/b.h").

# Target covers all changed files
covers_all(Target) :-
    target(Target),
    not(misses_file(Target)).

misses_file(Target) :-
    target(Target),
    changed_file(F),
    not(covers(Target, F)).
```

```bash
redwood query "covers_all(X)"
```

**Greedy approximation**: When no single target suffices, use the greedy algorithm (O(log n) optimal):

```datalog
# Prefer targets covering multiple changed files
covers_multiple(Target) :-
    covers(Target, F1),
    covers(Target, F2),
    changed_file(F1),
    changed_file(F2),
    !=(F1, F2).
```

### Example 19: Practical Lint Target Selection

Combine with the existing `directly_changed` infrastructure:

```datalog
# Targets to lint: those with changed sources
lint_target(T) :-
    sources(T, S),
    sources_added(T, S).

lint_target(T) :-
    sources(T, S),
    sources_removed(T, S).

# Also lint targets whose deps changed (might affect includes)
lint_target(T) :-
    deps_added(T, _).
```

Query:
```bash
redwood query "lint_target(X)"
```
