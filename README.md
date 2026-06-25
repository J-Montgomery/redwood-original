# Redwood Build System

A datalog-based build system

> [!IMPORTANT]
> Every file in this repository (except for this README) was completely
> generated with LLMs to explore their limitations in building systems
> outside the training set and see if datalog-based build system is a good idea.

This prototype explores what's possible if you treat your build graph as a
relational graph of targets. It defines a (probably) buggy datalog engine
with a minimal set of primitives, on which the rest of the functionality is
built.

The build system is implemented in itself. Everything from from how it determines
what files to rebuild, to how it interacts with version control is entirely customizeable
with user code.

The three core concepts behind the design are:

1. All data stored as facts in the underlying database
2. Datalog rules are evaluated to derive further facts
3. BuildKind rules tell the engine what commands to execute along the way.

## What's Possible With Redwood

- Self-hosting
- Building only the changed targets between two git hashes
- Manually rebuilding any set of targets
- Native dry-run builds

Redwood's view of the build graph is (mostly) monotonic. Unused sections of the build graph don't even need their BUILD files loaded, unlike bazel.

## Building

```bash
cargo build --release
cargo test
```

## Self-Hosting

Redwood builds itself:

```bash
cargo build --release
target/release/redwood build //bootstrap:redwood
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
target/release/redwood format                 # Format all BUILD.datalog in workspace
target/release/redwood format BUILD.datalog   # Format specific file
target/release/redwood format src/            # Format directory recursively
```

## Incremental Builds

The rebuild logic is defined in prelude datalog. The engine lazily instantiates
file_exists and file_hash facts into the EDB that can be used to create custom rebuild logic.

If the engine determines if the needs_rebuild fact is true for the output based on xxhash over all rule inputs, including sources, attrs, and toolchain paths.

The engine :
1. Lazily checks for dirtiness
2. Hashes sources, attrs, and other rule inputs into the facts database
3. Queries `needs_rebuild(Target)` to decide if rebuild needed
4. Skips otherwise
5. Stores a hash of the output in `.redwood/cache/`

Default rebuild triggers:
- Output missing
- Source file changed
- Attribute changed (flags, options)
- Tool binary changed (if tracked with `build_input`)


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

### Tracking Tool Binaries

By default, the cache tracks toolchain paths but not binary content. To invalidate the hash cache when tool binaries change (e.g., cargo upgrade), a `build_input` primitive is provided:

```datalog
cargo_binary("//app:server").
sources("//app:server", "src/main.rs").

# Hash cargo binary to detect upgrades
build_input("//app:server", Path) :-
    tool_available("cargo", Path, _).
```

The `build_input(Target, Path)` predicate hashes files without passing them to the build command. 

The `_` wildcard matches any value without binding it. e.g. Use `tool_available(Tool, Path, _)` to get the discovered tool path.

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

Toolchains are wrappers around a generic tool invocation rule called **system_tool** that allows customization via attributes. The prelude provides helpers that automatically configure common toolchains. The engine scans PATH to automatically discover a few known tools (cargo, rustc, gcc, g++, clang, go, python3, node) and inserts `tool_available(Tool, Path, Version)` facts.

Datalog rules in the prelude are used to match define which kinds of targets use which available tools:

Derives:
- `target("//app:cli")` - target existence
- `kind("//app:cli", system_tool)` - build kind
- `attr("//app:cli", "tool", "cargo")` - tool selection
- `attr("//app:cli", "0", "build")` - command args
- `attr("//app:cli", "1", "--release")` - command args


```datalog
target("//app:server").
kind("//app:server", system_tool).
attr("//app:server", "tool", "gcc").
attr("//app:server", "-O3").
attr("//app:server", "-o", "{output}").
sources("//app:server", "server.c").
```

The prelude provides helper predicates and default flags for the following toolchains:
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

```datalog
toolchain(Target, Tool, Path) :-
    requires_tool(Target, Tool),
    tool_available(Tool, Path, Version).

requires_tool(Target, Tool) :-
    kind(Target, system_tool),
    attr(Target, "tool", Tool).
```

You can override default tool paths in any BUILD.datalog. Overrides take precedence over discovered tools.

```datalog
toolchain_override("gcc", "/opt/gcc-12/bin/gcc").
toolchain_override("cargo", "/home/user/.cargo/bin/cargo").
```

Attributes configure command-line invocations. Reserved attributes:
- `tool` - which system binary to invoke (gcc, g++, go, node, python3, etc.)
- `output_dir` - where to place output files (default: `target/{tool}`)

All other attributes become command-line arguments. If the value is empty, only the key is passed (e.g. `-O3`). If the value is non-empty, both key and value are passed separated by a space (e.g. `-o output.bin`).

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

This enables toolchain-wide defaults with per-target overrides and conditional compilation.

## Constraint Checking

Constraints can be used to invalidate erroneous targets in datalog:

```datalog
constraint_failed(Target, "requires at least one source") :-
    system_cc(Target),
    not(has_sources(Target)).

has_sources(Target) :- sources(Target, Path).
```

The engine fails with the provided error message if any `constraint_failed` facts exist for a target. Constraints allow you to violate redwood's monotonicity, so best practice is to keep
them local.

## Implementation



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
