use redwood::datalog::Engine;
use redwood::datalog::{parser, Fact, Value};

#[test]
fn variable_arity_matching() {
    let mut db = Engine::new();

    // Insert 2-arg fact
    db.insert_facts(vec![Fact {
        predicate: "attr".to_string(),
        args: vec![
            Value::String("gcc".to_string()),
            Value::String("-Wall".to_string()),
        ],
    }]);

    // Insert 3-arg fact
    db.insert_facts(vec![Fact {
        predicate: "attr".to_string(),
        args: vec![
            Value::String("gcc".to_string()),
            Value::String("-O".to_string()),
            Value::String("3".to_string()),
        ],
    }]);

    // Query with wildcard in third position should match both
    let input = r#"
        result(X, Y) :- attr(X, Y, _).
    "#;
    let (_, rules) = parser::parse_program(input).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("result", &[]);
    assert_eq!(
        results.len(),
        2,
        "Both 2-arg and 3-arg facts should match when last arg is wildcard"
    );
}

#[test]
fn equality_operator_binding() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "tool".to_string(),
            args: vec![Value::String("cargo".to_string())],
        },
        Fact {
            predicate: "tool".to_string(),
            args: vec![Value::String("gcc".to_string())],
        },
    ]);

    let input = r#"
        is_cargo(T) :- tool(T), T = "cargo".
    "#;
    let (_, rules) = parser::parse_program(input).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("is_cargo", &[]);
    assert_eq!(results.len(), 1, "Only cargo should match");
    assert_eq!(results[0].args[0], Value::String("cargo".to_string()));
}

#[test]
fn equality_operator_unification() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "name".to_string(),
        args: vec![Value::String("test".to_string())],
    }]);

    let input = r#"
        same(X, Y) :- name(X), Y = X.
    "#;
    let (_, rules) = parser::parse_program(input).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("same", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0], results[0].args[1],
        "X and Y should be unified"
    );
}

#[test]
fn wildcard_ignores_values() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "tool_available".to_string(),
            args: vec![
                Value::String("cargo".to_string()),
                Value::String("/usr/bin/cargo".to_string()),
                Value::String("1.70.0".to_string()),
            ],
        },
        Fact {
            predicate: "tool_available".to_string(),
            args: vec![
                Value::String("cargo".to_string()),
                Value::String("/usr/bin/cargo".to_string()),
                Value::String("1.71.0".to_string()),
            ],
        },
    ]);

    let input = r#"
        tool_path(T, P) :- tool_available(T, P, _).
    "#;
    let (_, rules) = parser::parse_program(input).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("tool_path", &[]);
    // Both tool_available facts match, but produce identical tool_path results which are deduplicated
    assert_eq!(
        results.len(),
        1,
        "Wildcard matches both but deduplicates identical results"
    );
}

#[test]
fn build_input_with_tool_available() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "tool_available".to_string(),
            args: vec![
                Value::String("cargo".to_string()),
                Value::String("/home/user/.cargo/bin/cargo".to_string()),
                Value::String("1.93.0".to_string()),
            ],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:server".to_string())],
        },
    ]);

    let input = r#"
        build_input(T, Path) :-
            T = "//app:server",
            tool_available("cargo", Path, _).
    "#;
    let (_, rules) = parser::parse_program(input).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("build_input", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("//app:server".to_string())
    );
    assert_eq!(
        results[0].args[1],
        Value::String("/home/user/.cargo/bin/cargo".to_string())
    );
}

#[test]
fn inequality_operator_integers() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("a".to_string()), Value::Integer(1)],
        },
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("b".to_string()), Value::Integer(2)],
        },
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("c".to_string()), Value::Integer(2)],
        },
    ]);

    let input = r#"
        different(X) :- value(X, V), V != 2.
    "#;
    let (_, rules) = parser::parse_program(input).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("different", &[]);
    assert_eq!(results.len(), 1, "Only value 'a' has V != 2");
    assert_eq!(results[0].args[0], Value::String("a".to_string()));
}

#[test]
fn inequality_operator_strings() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "name".to_string(),
            args: vec![
                Value::String("x".to_string()),
                Value::String("cargo".to_string()),
            ],
        },
        Fact {
            predicate: "name".to_string(),
            args: vec![
                Value::String("y".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "name".to_string(),
            args: vec![
                Value::String("z".to_string()),
                Value::String("cargo".to_string()),
            ],
        },
    ]);

    let input = r#"
        not_cargo(X) :- name(X, N), N != "cargo".
    "#;
    let (_, rules) = parser::parse_program(input).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("not_cargo", &[]);
    assert_eq!(results.len(), 1, "Only 'y' has name != 'cargo'");
    assert_eq!(results[0].args[0], Value::String("y".to_string()));
}
