use redwood::datalog::{parser, Term, Value};

#[test]
fn parse_rule_with_negation() {
    let input = r#"
        missing(X) :- target(X), not(file_exists(X)).
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.head.name, "missing");
    assert_eq!(rule.body.len(), 2);
    assert_eq!(rule.body[0].name, "target");
    assert_eq!(rule.body[1].name, "not:file_exists");
}

#[test]
fn parse_rule_with_inequality() {
    let input = r#"
        changed(X) :- new_hash(X, N), old_hash(X, O), N != O.
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.head.name, "changed");
    assert_eq!(rule.body.len(), 3);
    assert_eq!(rule.body[2].name, "!=");
}

#[test]
fn parse_rule_with_gt() {
    let input = r#"
        newer(X) :- timestamp(X, T1), baseline(T2), gt(T1, T2).
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.body[2].name, "gt");
    assert_eq!(rule.body[2].args.len(), 2);
}

#[test]
fn parse_rule_with_lt() {
    let input = r#"
        older(X) :- timestamp(X, T1), baseline(T2), lt(T1, T2).
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.body[2].name, "lt");
}

#[test]
fn parse_negation_with_arguments() {
    let input = r#"
        no_output(T) :- target(T), output_path(T, P), not(file_exists(P)).
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.body.len(), 3);

    if let Term::Variable(v) = &rule.body[2].args[0] {
        assert_eq!(v, "P");
    } else {
        panic!("Expected variable P in not() predicate");
    }
}

#[test]
fn parse_integer_values() {
    let input = r#"
        timestamp("file.txt", 1234567890).
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.0.len(), 1);

    let fact = &result.0[0];
    assert_eq!(fact.args[1], Value::Integer(1234567890));
}

#[test]
fn parse_rule_with_integer_comparison() {
    let input = r#"
        needs_rebuild(T) :-
            sources(T, S),
            file_modified(S, SourceTime),
            outputs(T, O),
            file_modified(O, OutputTime),
            gt(SourceTime, OutputTime).
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.head.name, "needs_rebuild");
    assert_eq!(rule.body.len(), 5);
    assert_eq!(rule.body[4].name, "gt");
}

#[test]
fn parse_complex_needs_rebuild() {
    let input = r#"
        needs_rebuild(T) :-
            target(T),
            outputs(T, O),
            not(file_exists(O)).

        needs_rebuild(T) :-
            target(T),
            sources(T, S),
            file_hash(S, New),
            cached_hash(T, S, Old),
            New != Old.
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 2);

    assert_eq!(result.1[0].head.name, "needs_rebuild");
    assert_eq!(result.1[1].head.name, "needs_rebuild");

    assert!(result.1[0].body.iter().any(|p| p.name == "not:file_exists"));
    assert!(result.1[1].body.iter().any(|p| p.name == "!="));
}

#[test]
fn parse_equality_operator() {
    let input = r#"
        same(X, Y) :- X = Y.
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.head.name, "same");
    assert_eq!(rule.body.len(), 1);
    assert_eq!(rule.body[0].name, "=");
    assert_eq!(rule.body[0].args.len(), 2);
}

#[test]
fn parse_equality_with_constant() {
    let input = r#"
        is_cargo(T) :- T = "cargo".
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.body[0].name, "=");
    if let Term::Variable(v) = &rule.body[0].args[0] {
        assert_eq!(v, "T");
    } else {
        panic!("Expected variable T");
    }
    if let Term::Constant(Value::String(s)) = &rule.body[0].args[1] {
        assert_eq!(s, "cargo");
    } else {
        panic!("Expected constant 'cargo'");
    }
}

#[test]
fn parse_wildcard_variable() {
    let input = r#"
        tool(T, P) :- tool_available(T, P, _).
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.1.len(), 1);

    let rule = &result.1[0];
    assert_eq!(rule.body[0].args.len(), 3);

    if let Term::Variable(v) = &rule.body[0].args[2] {
        assert!(
            v.starts_with("_anon_"),
            "Wildcard should become anonymous variable"
        );
    } else {
        panic!("Expected anonymous variable");
    }
}

#[test]
fn parse_multiple_wildcards() {
    let input = r#"
        exists(X) :- fact(_, X, _, _).
    "#;

    let result = parser::parse_program(input).unwrap();
    let rule = &result.1[0];
    assert_eq!(rule.body[0].args.len(), 4);

    let mut anon_count = 0;
    for arg in &rule.body[0].args {
        if let Term::Variable(v) = arg {
            if v.starts_with("_anon_") {
                anon_count += 1;
            }
        }
    }
    assert_eq!(anon_count, 3, "Should have 3 anonymous variables");

    if let Term::Variable(v1) = &rule.body[0].args[0] {
        if let Term::Variable(v2) = &rule.body[0].args[2] {
            assert_ne!(v1, v2, "Each wildcard should be unique");
        }
    }
}

#[test]
fn parse_variable_arity_facts() {
    let input = r#"
        attr("gcc", "-Wall").
        attr("gcc", "-O", "3").
    "#;

    let result = parser::parse_program(input).unwrap();
    assert_eq!(result.0.len(), 2);

    assert_eq!(result.0[0].args.len(), 2);
    assert_eq!(result.0[1].args.len(), 3);
}
