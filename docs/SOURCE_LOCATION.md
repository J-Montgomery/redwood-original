# Source Location Tracking

Query predicate definitions using `source_location(Predicate, File, Line)`.

## API

```rust
// Record location when parsing
let (facts, rules, locations) = parser::parse_program_with_file(input, "BUILD.datalog")?;
for (key, loc) in locations {
    engine.record_source_location(&key, loc);
}

// Query all locations
engine.query("source_location", &[]);

// Query specific predicate
engine.query("source_location", &[Some("system_cc")]);

// Query by file
engine.query("source_location", &[None, Some("BUILD.datalog")]);
```

## Example

```rust
let input = r#"
system_cc("//app:main").
sources("//app:main", "main.c").
deps(X, Y) :- depends_on(X, Y).
"#;

let (facts, rules, locations) = parser::parse_program_with_file(input, "BUILD.datalog")?;

let mut engine = Engine::new();
for (key, loc) in locations {
    engine.record_source_location(&key, loc);
}
engine.insert_facts(facts);
for rule in rules {
    engine.compile_rule(rule);
}

// Find where deps rule is defined
let results = engine.query("source_location", &[Some("deps")]);
// Returns: source_location("deps", "BUILD.datalog", 4)
```

## Use Cases

- Error messages with file/line context
- "Where is target X defined?"
- Navigate from query result to source
- Syntax error reporting
