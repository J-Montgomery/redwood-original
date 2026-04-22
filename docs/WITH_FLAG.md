# The --with Flag

The `--with` flag injects datalog (facts or rules) before building.

## Usage

```bash
redwood build <targets> --with '<datalog>'
```

## Examples

### Inject Facts

```bash
# Force rebuild specific target
redwood build //app:server --with 'force_rebuild("//app:server").'

# Override license for a target
redwood build //app:cli --with 'license("//app:cli", "MIT").'
```

### Inject Rules

```bash
# Force rebuild ALL targets
redwood build //... --with 'force_rebuild(X) :- target(X).'

# Override license for all targets
redwood build //... --with 'license(X, "MIT") :- target(X).'

# Conditional rules
redwood build //... --with 'force_rebuild(X) :- target(X), kind(X, rust_binary).'
```

### Multiple Injections

```bash
# Multiple --with flags
redwood build //... \
  --with 'force_rebuild(X) :- target(X).' \
  --with 'license(X, "Apache-2.0") :- target(X).'
```

## Facts vs Rules

**Facts** are ground (concrete) statements:
```datalog
force_rebuild("//app:server").     # ✓ Valid
license("//app:cli", "MIT").       # ✓ Valid
```

**Rules** express universal quantification:
```datalog
force_rebuild(X) :- target(X).     # ✓ Valid - applies to all targets
license(X, "MIT") :- target(X).    # ✓ Valid - all targets are MIT
```

## Common Mistake

```bash
# ✗ WRONG - Facts cannot contain variables
redwood build //... --with 'force_rebuild(_).'

# Warning: Facts cannot contain variables: force_rebuild("_").
#          Did you mean: force_rebuild(X) :- target(X). ?
#          (Use a rule to quantify over all targets)
```

Variables (`X`, `_`, etc.) are only valid in **rules**, not facts.

### Why?

In datalog:
- Facts represent concrete, enumerated knowledge
- Variables in facts are meaningless (which value of `X`?)
- Rules express patterns that derive new facts

To apply something to all targets, use a rule:
```datalog
predicate(X) :- target(X).
```

## Use Cases

### Force Clean Rebuild

```bash
redwood build //... --with 'force_rebuild(X) :- target(X).'
```

### Debug Specific Target

```bash
redwood build //app:server \
  --with 'attr("//app:server", "0", "build").' \
  --with 'attr("//app:server", "1", "--verbose").'
```

### Override Toolchain

```bash
redwood build //app:server \
  --with 'attr("gcc", "-O3").' \
  --with 'attr("gcc", "-march=native").'
```

### Test License Changes

```bash
redwood build //... --with 'license(X, "AGPL-3.0") :- target(X).'
```

## Injection Order

1. Load BUILD.datalog
2. Load prelude rules
3. **Inject --with facts and rules** ← happens here
4. Scan filesystem for file_exists facts
5. Begin building

This means `--with` can:
- Override facts from BUILD.datalog
- Add new facts
- Add new rules that derive from existing predicates
- Cannot override prelude rules (they've already been compiled)

## Implementation

The `--with` flag is processed by `parse_query_with_repl_syntax()`:
1. Tries to parse as complete datalog program
2. Auto-adds trailing `.` if missing
3. Falls back to multi-predicate query syntax
4. Filters out facts with variables (shows warning)
5. Inserts facts and compiles rules

See [src/cli/mod.rs](src/cli/mod.rs) for implementation.
