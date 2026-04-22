# Namespace Loading Example

This document demonstrates the lazy namespace loading feature.

## Directory Structure

```
workspace/
├── BUILD.datalog              # Main workspace
└── .external/
    └── boost/
        └── BUILD.datalog      # External dependency
```

## Main Workspace BUILD.datalog

```datalog
# Declare namespace roots
root("//", ".").
root("//external/boost", ".external/boost").

# Main application target
target("//app:server").
cargo_binary("//app:server").
sources("//app:server", "src/main.rs").

# Dependency on external library
deps("//app:server", "//external/boost//ranges:lib").
```

## External Boost BUILD.datalog

Located at `.external/boost/BUILD.datalog`:

```datalog
# Boost library targets
target("//ranges:lib").
cargo_lib("//ranges:lib").
sources("//ranges:lib", "src/ranges.rs").

target("//algorithm:sort").
cargo_lib("//algorithm:sort").
sources("//algorithm:sort", "src/algorithm.rs").
deps("//algorithm:sort", "//ranges:lib").
```

## How Lazy Loading Works

1. **Initial Load**: Only main workspace `BUILD.datalog` is parsed
   - `root()` facts are extracted and registered
   - Main workspace facts are inserted into engine
   - Boost namespace is NOT loaded yet

2. **Build Command**: `redwood build //app:server`

3. **Dependency Traversal**: During `topological_sort`:
   - Query `deps("//app:server", X)` → finds `"//external/boost//ranges:lib"`
   - Extract namespace from dependency: `"//external/boost"`
   - Check if namespace loaded → NO
   - **Lazy load triggered**: Load `.external/boost/BUILD.datalog`

4. **Namespace Rewriting**: When loading boost's BUILD.datalog:
   - Parse `target("//ranges:lib")`
   - Rewrite with namespace: `target("//external/boost//ranges:lib")`
   - Parse `deps("//algorithm:sort", "//ranges:lib")`
   - Rewrite: `deps("//external/boost//algorithm:sort", "//external/boost//ranges:lib")`

5. **Build Execution**: All namespaces loaded, build proceeds

## Benefits

- **Fast Startup**: Only parse files you need
- **Scalable**: 100+ dependencies? Only load what you use
- **Flexible**: Any namespace structure (not just `//third_party/`)
- **Explicit**: `root()` facts declare all mappings

## Cross-Namespace References

Dependencies can only reference:
- **Same namespace** (relative): `//ranges:lib` → `//external/boost//ranges:lib`
- **Other namespaces** (fully qualified): `//other/lib//foo:bar` → unchanged

Main workspace can reference any namespace:
```datalog
deps("//app:server", "//external/boost//ranges:lib").
deps("//app:server", "//internal/auth//core:jwt").
deps("//app:server", "//externe/opencv//core:cv").
```

External dependencies cannot reference main workspace (prevents circular dependencies).
Namespacing happens at parse time, not query time.

