use redwood::datalog::{parser, Engine};
use std::process::Command;

#[test]
fn gt_with_unbound_second_arg_fails() {
    let output = Command::new("target/debug/test_gt_unbound")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gt requires second argument to be bound"));
}

#[test]
fn lt_with_unbound_first_arg_fails() {
    let output = Command::new("target/debug/test_lt_unbound")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lt requires first argument to be bound"));
}

#[test]
fn add_with_unbound_second_arg_fails() {
    let output = Command::new("target/debug/test_arithmetic_unbound_add")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("add requires second argument to be bound"));
}

#[test]
fn parse_int_with_unbound_input_fails() {
    let output = Command::new("target/debug/test_arithmetic_unbound_parse_int")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("parse_int requires first argument (string) to be bound"));
}

#[test]
fn to_string_with_unbound_input_fails() {
    let output = Command::new("target/debug/test_arithmetic_unbound_to_string")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("to_string requires first argument (integer) to be bound"));
}

#[test]
fn parse_int_with_invalid_string_fails() {
    let program = r#"
        input("not_a_number").
        input("also_bad").
        input("42").

        valid_int(Str, N) :- input(Str), parse_int(Str, N).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("valid_int", &[]);
    assert_eq!(results.len(), 1, "Only '42' should parse successfully");
}

#[test]
fn div_by_zero_fails_safely() {
    let program = r#"
        numerator(10).
        numerator(20).
        denominator(0).
        denominator(2).

        quotient(N, D, Q) :- numerator(N), denominator(D), div(N, D, Q).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("quotient", &[]);
    assert_eq!(results.len(), 2, "Division by zero should be skipped");

    for result in &results {
        if let Some(redwood::datalog::Value::Integer(d)) = result.args.get(1) {
            assert_ne!(*d, 0, "Should not have results with denominator 0");
        }
    }
}

#[test]
fn mod_by_zero_fails_safely() {
    let program = r#"
        value(10).
        value(11).
        modulus(0).
        modulus(3).

        remainder(V, M, R) :- value(V), modulus(M), mod(V, M, R).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("remainder", &[]);
    assert_eq!(results.len(), 2, "Modulo by zero should be skipped");

    for result in &results {
        if let Some(redwood::datalog::Value::Integer(m)) = result.args.get(1) {
            assert_ne!(*m, 0, "Should not have results with modulus 0");
        }
    }
}

#[test]
fn arithmetic_with_wrong_types_fails() {
    let program = r#"
        string_value("10").
        int_value(10).

        bad_add(Result) :- string_value(S), int_value(I), add(S, I, Result).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("bad_add", &[]);
    assert_eq!(results.len(), 0, "Cannot add string and integer");
}

#[test]
fn arithmetic_checks_exact_values() {
    let program = r#"
        value(10).
        value(20).

        is_sum_30(X, Y) :-
            value(X),
            value(Y),
            X != Y,
            add(X, Y, 30).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("is_sum_30", &[]);
    assert_eq!(results.len(), 2); // (10, 20) and (20, 10)

    for result in &results {
        if let (
            Some(redwood::datalog::Value::Integer(x)),
            Some(redwood::datalog::Value::Integer(y)),
        ) = (result.args.first(), result.args.get(1))
        {
            assert_eq!(x + y, 30);
        } else {
            panic!("Expected integer results");
        }
    }
}

#[test]
fn equality_with_both_unbound_fails() {
    let output = Command::new("target/debug/test_equality_both_unbound")
        .output()
        .expect("Failed to run test binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("equality (=) with two unbound variables"));
}
