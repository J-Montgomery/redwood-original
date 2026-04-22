# Claude for Redwood Build System

## CRITICAL: Failure Reporting

- Report all failures immediately with 🚨
- State failures directly: "Test failed", "Compilation error", "Type mismatch"
- All issues matter - never minimize with "minor issue" or "small problem"
- Never say "you're absolutely right" or similar validation phrases

## Architecture

Core concepts:
1. Everything is facts in datalog
2. Strongly prefer implementing functionality in the datalog prelude over implementing it in rust
3. The datalog parser is as simple as possible for acceptable performance
4. BuildKind bridges queries to actions - Datalog = WHAT, BuildKind = HOW

Read [SPEC.md](SPEC.md) before making significant changes.

## Code Style

- NO obvious comments - comment intent, not mechanics
- MINIMIZE comments. Write self-documenting code
- NO emojis in code/errors (except 🚨 for failures in this file)
- Concise names - no type encoding
- Direct errors - no pleasantries
- Idiomatic Rust - iterators, `?` operator, pattern matching

Project-specific:
- NO version references ("v2", "updated version", etc.)
- NO premature optimization - profile first
- NO hidden config - everything explicit
- Keep engine implementation as simple as possible. Prefer correctness and readability. The goal is hundreds of lines, not thousands

## Building and Testing

```bash
cargo build        # Build
cargo test         # All tests
cargo test name    # Specific test
cargo check        # Type check
cargo fmt          # Format
cargo clippy       # Lint
```

Project structure:
```
src/
├── datalog/   # Parser and AST
├── dd/        # Database integration
├── build/     # BuildKind trait
├── sandbox/   # Sandboxing
├── cache/     # Caching
└── cli/       # CLI
```

Testing:
- Tests alongside code
- NEVER comment out tests or assertions
- Concise names: `parse_valid_target_label`
- Use `cargo test` only

## Database Implementation

HashMap-based Datalog evaluation:

- Facts stored in HashMap
- Lazy rule compilation (<1ms)
- Semi-naive evaluation for transitive closure
- BFS optimization for filtered queries

Simple:
```rust
fn compile_rule(&mut self, rule: Rule) {
    self.rules
        .entry(rule.head.name.clone())
        .or_insert_with(Vec::new)
        .push(rule);
}

fn query(&mut self, predicate: &str, filters: &[Option<&str>]) -> Vec<Fact> {
    if !self.rules.contains_key(predicate) {
        return self.query_base_facts(predicate, filters);
    }

    if is_tc_predicate(predicate) && has_start_filter(filters) {
        return self.query_tc_lazy(predicate, filters);
    }

    self.compute_derived(predicate, filters)
}
```

## BuildKind

New build kinds:
1. Implement `BuildKind` trait
2. Query database for target attributes
3. Construct `BuildPlan` with explicit inputs/outputs
4. Sandbox handles execution

## Sandboxing

Create clean directory, symlink declared inputs, execute. Undeclared inputs don't exist → build fails naturally.

## Caching

Hash all inputs (sources + deps + attrs + toolchain). Cache key = hash of hashes. xxhash sufficient.

## Multi-Repository

`//third_party/boost//ranges:range_lib`

Format: `//path/to/external//target:name`

## Planning

Plans in `.claude/plans/` as `.md` files. Wait for permission before starting.

## Don't

- Add features not in spec without discussion
- Optimize without profiling
- Add config for things that should be explicit
- Create abstractions for one-time operations
- Add obvious comments
- Use emojis in code/errors (except 🚨)
- Create summary documents

## Do

- Report failures immediately
- Keep database implementation simple
- Make dependencies explicit
- Write concise Rust
- Test edge cases
- Profile before optimizing
- ALWAYS ask before ignoring or deleting tests
- Read [SPEC.md](SPEC.md) first
