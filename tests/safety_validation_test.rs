use redwood::datalog::{parser, Engine};
use std::process::Command;

#[test]
fn unsafe_rule_ungrounded_variable() {
    let output = Command::new("target/debug/test_unsafe_rule_ungrounded")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Variable 'X' appears in head but not in positive body literal"));
}

#[test]
fn unsafe_rule_variable_only_in_negation() {
    let output = Command::new("target/debug/test_unsafe_rule_negation")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Variable 'Y' appears in head but not in positive body literal"));
}

#[test]
fn safe_rule_all_vars_in_positive_literals() {
    let program = r#"
        foo("a").
        bar("a").
        good(X) :- foo(X), not(bar(X)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("good", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn safe_rule_with_anonymous_variables() {
    let program = r#"
        deps("a", "b").
        deps("a", "c").
        has_deps(X) :- deps(X, _).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("has_deps", &[]);
    assert_eq!(results.len(), 1);
}

#[test]
fn non_stratified_self_negation() {
    let output = Command::new("target/debug/test_non_stratified_self")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Non-stratified negation"));
}

#[test]
fn non_stratified_mutual_negation() {
    let output = Command::new("target/debug/test_non_stratified_mutual")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Non-stratified negation"));
}

#[test]
fn stratified_negation_valid() {
    let program = r#"
        base("a").
        base("b").
        derived(X) :- base(X).
        not_derived(X) :- base(X), not(derived(X)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("not_derived", &[]);
    assert_eq!(results.len(), 0);
}
