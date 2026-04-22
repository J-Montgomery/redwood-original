# Redwood Build System

Datalog-based build system with HashMap evaluation for speed and simplicity.

## Building

```bash
cargo build --release
cargo test
```

## Usage

Define targets in BUILD.datalog:

```datalog
cargo_binary("//redwood:redwood").
outputs("//redwood:redwood", "target/release/redwood").
```

The `cargo_binary()` helper automatically sets:
- `target("//redwood:redwood")` - defines the target
- `kind("//redwood:redwood", system_tool)` - uses system_tool build kind
- `attr("//redwood:redwood", "tool", "cargo")` - invokes cargo
- `attr("//redwood:redwood", "0", "build")` - runs build subcommand
- `attr("//redwood:redwood", "1", "--release")` - release mode

Build targets:

```bash
target/release/redwood build //redwood:redwood
```

Query the build graph:

```bash
target/release/redwood query "target(X)"
target/release/redwood query "sources(X, Y)"
```

Format BUILD.datalog files:

```bash
target/release/redwood format              # Format all BUILD.datalog in workspace
target/release/redwood format BUILD.datalog   # Format specific file
target/release/redwood format src/          # Format directory recursively
```

## Documentation

- **[TUTORIAL.md](TUTORIAL.md)** - Learn by example - **START HERE**
- [SPEC.md](SPEC.md) - Language specification and design philosophy
- [LICENSE_VALIDATION.md](LICENSE_VALIDATION.md) - License constraint system

The formatter:
- Detects syntax errors without formatting
- Preserves comments
- Consistent indentation and spacing
- One fact per line, rule bodies indented

## Incremental Builds

Content-based caching using xxhash. Cache includes sources, attrs, and toolchain paths.

Rebuild triggers:
- Output missing
- Source file changed
- Attribute changed (flags, options)
- Tool binary changed (if tracked with `build_input`)

Prelude defines rebuild logic in datalog:

```datalog
needs_rebuild(T) :-
    target(T),
    outputs(T, O),
    not(file_exists(O)).

needs_rebuild(T) :-
    target(T),
    file_hash(Input, NewHash),
    cached_hash(T, Input, OldHash),
    NewHash != OldHash.
```

Build system:
1. Checks only files relevant to each target (lazy scanning)
2. Hashes sources, attrs, and build_input files
3. Queries `needs_rebuild(Target)` to decide if rebuild needed
4. Skips up-to-date targets
5. Stores hashes in `.redwood/cache/`

Lazy filesystem scanning: only checks existence of outputs and computes hashes for sources of the target being built. Scales with files-per-target, not total workspace files.

### Tracking Tool Binaries

By default, the cache tracks toolchain paths but not binary content. To invalidate cache when tool binaries change (e.g., cargo upgrade), use `build_input`:

```datalog
cargo_binary("//app:server").
sources("//app:server", "src/main.rs").

# Hash cargo binary to detect upgrades
build_input("//app:server", Path) :-
    tool_available("cargo", Path, _).
```

The `build_input(Target, Path)` predicate hashes files without passing them to the build command. Useful for:
- Tool binaries (cargo, gcc, rustc)
- Build scripts
- Configuration files affecting builds

The `_` wildcard matches any value without binding it. Use `tool_available(Tool, Path, _)` to get the discovered tool path.

For tracking multiple targets:

```datalog
# Track cargo for specific targets
build_input(T, Path) :-
    T = "//app:server",
    tool_available("cargo", Path, _).

build_input(T, Path) :-
    T = "//app:client",
    tool_available("cargo", Path, _).

# Or track gcc for all C targets
build_input(T, Path) :-
    system_cc(T),
    tool_available("gcc", Path, _).
```

## Build Kinds

All builds use **system_tool** - generic tool invocation via attributes. The prelude provides helpers that automatically configure common toolchains.

**Helpers automatically derive facts:**

```datalog
cargo_binary("//app:cli").
```

Derives:
- `target("//app:cli")` - target existence
- `kind("//app:cli", system_tool)` - build kind
- `attr("//app:cli", "tool", "cargo")` - tool selection
- `attr("//app:cli", "0", "build")` - command args
- `attr("//app:cli", "1", "--release")` - command args

No need to declare `target()` separately. Helpers do it automatically.

**system_tool** - Tool invocation via attributes

```datalog
target("//app:server").
kind("//app:server", system_tool).
attr("//app:server", "tool", "gcc").
attr("//app:server", "-O3").
attr("//app:server", "-o", "{output}").
sources("//app:server", "server.c").
```

The prelude provides helper predicates and default flags for common toolchains:
- `system_cc(Target)` - gcc with `-fPIC`, `-Wall`
- `system_cxx(Target)` - g++ with `-fPIC`, `-Wall`, `-std=c++17`
- `system_clang(Target)` - clang with `-fPIC`, `-Wall`
- `system_clangxx(Target)` - clang++ with `-fPIC`, `-Wall`, `-std=c++17`
- `system_go(Target)` - go with `-v`
- `system_python(Target)` - python3 with `-u`, `-Werror`
- `system_node(Target)` - node with `--trace-warnings`
- `cargo_binary(Target)` - cargo build --release
- `cargo_binary_debug(Target)` - cargo build
- `cargo_lib(Target)` - cargo build --release --lib
- `cargo_test(Target)` - cargo test

Attributes configure command-line invocations. Reserved attributes:
- `tool` - which system binary to invoke (gcc, g++, go, node, python3, etc.)
- `output_dir` - where to place output files (default: `target/{tool}`)

All other attributes become command-line arguments. If the value is empty, only the key is passed (like `-O3`). If the value is non-empty, both key and value are passed (like `-o output.bin`).

Template expansion:
- `{output}` - replaced with output file path
- `{sources}` - replaced with space-separated source files
- `{target}` - replaced with target label

Examples using helpers with defaults:

```datalog
# C binary - uses system_cc() helper
system_cc("//server:main").  # Derives target, kind, tool=gcc, -fPIC, -Wall
attr("//server:main", "-O3").
attr("//server:main", "-o", "{output}").
sources("//server:main", "main.c").

# C++ binary - uses system_cxx() helper
system_cxx("//parser:bin").  # Derives target, kind, tool=g++, -fPIC, -Wall, -std=c++17
attr("//parser:bin", "-o", "{output}").
sources("//parser:bin", "parser.cpp").

# Go binary - uses system_go() helper
system_go("//service:api").  # Derives target, kind, tool=go, -v
attr("//service:api", "build").
attr("//service:api", "-o", "{output}").
sources("//service:api", "main.go").
```

Explicit configuration without helpers:

```datalog
target("//server:main").
kind("//server:main", system_tool).
attr("//server:main", "tool", "gcc").
attr("//server:main", "-O3").
attr("//server:main", "-o", "{output}").
sources("//server:main", "main.c").
```

### Attribute Scopes

Attributes can be set at three levels:

**Target-level** - Applies only to specific target:
```datalog
attr("//app:server", "-O3").
```

**Tool-level** - Applies to all targets using that tool:
```datalog
attr("gcc", "-fPIC").           # All gcc invocations get -fPIC
attr("//app:server", "tool", "gcc"). # Inherits -fPIC
```

**Target:Tool combination** - Applies when specific target uses specific tool:
```datalog
attr("//app:server:gcc", "-march", "native").  # Only //app:server with gcc
```

Priority: Target:Tool > Tool > Target

This enables toolchain-wide defaults (tool-level) with per-target overrides (target-level) and conditional compilation (target:tool).

## Constraint Checking

Constraints validate targets in datalog:

```datalog
constraint_failed(Target, "requires at least one source") :-
    system_cc(Target),
    not(has_sources(Target)).

has_sources(Target) :- sources(Target, Path).
```

Build fails with error message if any `constraint_failed` facts exist for a target. Constraints are evaluated in datalog, not rust. Add new constraints by writing rules in prelude or BUILD.datalog.

## Tool Discovery and Overrides

At build time, system scans PATH for tools (cargo, rustc, gcc, g++, clang, go, python3, node) and inserts `tool_available(Tool, Path, Version)` facts.

Prelude rules match requirements to available tools:

```datalog
toolchain(Target, Tool, Path) :-
    requires_tool(Target, Tool),
    tool_available(Tool, Path, Version).

requires_tool(Target, Tool) :-
    kind(Target, system_tool),
    attr(Target, "tool", Tool).
```

Override tool paths in BUILD.datalog:

```datalog
toolchain_override("gcc", "/opt/gcc-12/bin/gcc").
toolchain_override("cargo", "/home/user/.cargo/bin/cargo").
```

Overrides take precedence over discovered tools.

## Implementation

### HashMap-Based Evaluation

Simple and fast Datalog evaluation:

- HashMap storage with lazy rule compilation
- First-argument indexing for O(1) filtered queries
- TC memoization per starting node
- Semi-naive evaluation for transitive closure
- BFS optimization for filtered TC queries
- <3µs rule compilation for build system workloads
- Topological sort for dependency ordering
- Smart cache invalidation (only clears affected predicates)

Three core concepts:

1. **Everything is facts** - All data stored as facts in database
2. **Datalog evaluation** - Rules evaluated to derive facts
3. **BuildKind bridges queries to actions** - Datalog describes WHAT, BuildKind executes HOW

Datalog features:

- Variables (uppercase) and constants (lowercase)
- Rules with joins: `target(X) :- kind(X, rust_binary).`
- Negation: `not(file_exists(O))`
- Equality: `X = Y` (unification), `T = "cargo"` (filtering)
- Inequality: `X != Y`
- Comparisons: `gt(T1, T2)`, `lt(T1, T2)`
- Wildcard: `_` matches any value without binding
- Variable arity: 2-arg facts match 3-arg patterns with trailing wildcards

Filesystem predicates:

- `file_exists(Path)` - file presence
- `file_modified(Path, Timestamp)` - modification time
- `file_hash(Path, Hash)` - xxhash content hash
- `glob(Pattern, Path)` - pattern matching

### Equality Operator

The `=` operator unifies variables or filters by constant:

```datalog
# Filter by constant
is_cargo(T) :- T = "cargo".

# Unify variables
same_tool(X, Y) :- tool(X), tool(Y), X = Y.

# Bind variable to constant in rule body
build_input(T, Path) :-
    T = "//app:server",
    tool_available("cargo", Path, _).
```

When both sides are variables, `=` unifies them (binds them to the same value). When one side is a constant, `=` filters to matching values.

### Wildcard Variable

The `_` wildcard matches any value without binding a variable:

```datalog
# Get tool path without binding version
tool_path(Tool, Path) :- tool_available(Tool, Path, _).

# Match facts with varying arity
result(X, Y) :- attr(X, Y, _).  # Matches 2-arg or 3-arg attr facts
```

Each `_` is internally converted to a unique anonymous variable (`_anon_0`, `_anon_1`, etc.).

### Variable Arity

Facts with N arguments match patterns with M arguments (M >= N) if trailing pattern arguments are wildcards:

```datalog
# Prelude defines 2-arg facts
attr("gcc", "-fPIC").
attr("gcc", "-Wall").

# Patterns with trailing wildcards match
compiler_flag(Tool, Flag) :- attr(Tool, Flag, _).  # Matches 2-arg facts
```

This eliminates empty strings in facts. Instead of `attr("gcc", "-Wall", "")`, write `attr("gcc", "-Wall")`.

## Project Structure

```
src/
├── datalog/          # Parser and AST
├── database/         # Database, rule compiler, toolchain scanner
├── build/            # BuildKind trait and system_tool implementation
│   └── system_tool.rs   # Generic tool invocation via attributes
├── cache/            # Build hash caching
├── cli/              # Command handlers
└── main.rs           # Entry point

prelude/              # System-loaded Datalog rules
├── needs_rebuild.datalog       # Incremental build logic
└── toolchain_discovery.datalog # Helpers and constraints
```

## Self-Hosting

Redwood builds itself:

```bash
cargo build --release
target/release/redwood build //redwood:redwood
```

Verified through 3-generation cycle (gen1 == gen2).
