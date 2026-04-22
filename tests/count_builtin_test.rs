use redwood::datalog::{parser, Engine, Value};

#[test]
fn count_targets() {
    let mut db = Engine::new();

    let program = r#"
target("//app:a").
target("//app:b").
target("//app:c").

target_count(N) :- count("target", N).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("target_count", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::Integer(3));
}

#[test]
fn count_with_bound_value() {
    let mut db = Engine::new();

    let program = r#"
target("//app:a").
target("//app:b").

has_two_targets() :- count("target", 2).
has_three_targets() :- count("target", 3).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let two = db.query("has_two_targets", &[]);
    assert_eq!(two.len(), 1);

    let three = db.query("has_three_targets", &[]);
    assert_eq!(three.len(), 0);
}

#[test]
fn count_derived_predicate() {
    let mut db = Engine::new();

    let program = r#"
target("//app:a").
target("//app:b").
target("//app:c").

deps("//app:a", "//app:b").
deps("//app:b", "//app:c").

needs_rebuild("//app:a").
needs_rebuild("//app:c").

rebuild_count(N) :- count("needs_rebuild", N).
dependency_count(N) :- count("deps", N).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let rebuild = db.query("rebuild_count", &[]);
    assert_eq!(rebuild.len(), 1);
    assert_eq!(rebuild[0].args[0], Value::Integer(2));

    let deps_count = db.query("dependency_count", &[]);
    assert_eq!(deps_count.len(), 1);
    assert_eq!(deps_count[0].args[0], Value::Integer(2));
}

#[test]
fn count_empty_predicate() {
    let mut db = Engine::new();

    let program = r#"
target("//app:a").

nonexistent_count(N) :- count("nonexistent", N).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("nonexistent_count", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::Integer(0));
}
