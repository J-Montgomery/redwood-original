use redwood::datalog::{parser, Engine};

#[test]
fn consistent_arity_accepted() {
    let mut engine = Engine::new();

    let program = r#"
        foo(X, Y) :- bar(X, Y).
        foo(X, Y) :- baz(X, Y).
    "#;

    let (_, rules) = parser::parse_program(program).unwrap();

    for rule in rules {
        engine.compile_rule(rule);
    }
}

#[test]
fn single_predicate_arity_accepted() {
    let mut engine = Engine::new();

    let program = r#"
        target(X) :- kind(X, rust_binary).
    "#;

    let (_, rules) = parser::parse_program(program).unwrap();

    for rule in rules {
        engine.compile_rule(rule);
    }
}
