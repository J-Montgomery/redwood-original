# Pattern Matching for Target Selection

Redwood supports Bazel-style recursive patterns using the `/...` suffix.

## Usage

Build all targets in a directory and subdirectories:

```bash
# All targets under app directory
redwood build //app/...

# Specific subdirectory
redwood build //app/internal/...

# Root namespace only (excludes external)
redwood build //...
```

## How It Works

1. **CLI Detection**: Pattern expansion happens in CLI before build starts
2. **Suffix Stripping**: `//app/...` → prefix `//app`
3. **Datalog Query**: Queries `matches_pattern(Target, "//app")` using built-in `prefix/2` predicate
4. **Lazy Namespace Loading**: All `root()` facts are loaded to discover namespaces before expansion

## Pattern Syntax

- `//path/...` → All targets starting with `//path`
- `//...` → All targets in main workspace (excludes `//external/`, `//internal/`)
- `//foo:bar` → Exact target (no expansion)

## Namespaced Patterns

Patterns work with namespaced targets:

```bash
# All boost libraries
redwood build //external/boost//...

# Specific namespace subdirectory
redwood build //external/opencv//core/...
```

## Implementation

**Datalog** ([prelude/patterns.datalog](prelude/patterns.datalog)):
```datalog
matches_pattern(Target, Prefix) :-
    target(Target),
    prefix(Target, Prefix).

matches_pattern(Target, Target) :- target(Target).
```

**CLI** ([src/cli/mod.rs](src/cli/mod.rs)):
```rust
fn expand_target_patterns(patterns: Vec<String>, db: &mut Engine, loader: &mut NamespaceLoader) -> Result<Vec<String>, String> {
    for pattern in patterns {
        if pattern.ends_with("/...") {
            // Load all namespaces
            let roots = db.query("root", &[]);
            for fact in roots {
                if let Value::String(namespace) = &fact.args[0] {
                    loader.load_namespace(namespace, db)?;
                }
            }

            // Strip suffix and query
            let prefix = pattern.strip_suffix("/...").unwrap();
            let results = db.query("matches_pattern", &[None, Some(prefix)]);
            // ... collect results
        }
    }
}
```

## Examples

```bash
# Build everything in app directory
redwood build //app/...

# Build with condition
redwood build --with 'force_rebuild("//app:server")' //app/...

# Query what would match
redwood query 'matches_pattern(T, "//app")'
```

## String Built-ins Used

- `prefix(String, Prefix)`: Check if String starts with Prefix
- `suffix(String, Suffix)`: Check if String ends with Suffix
- `contains(String, Substring)`: Check if String contains Substring

All implemented as built-in predicates in the engine ([src/datalog/builtins.rs](src/datalog/builtins.rs)).
