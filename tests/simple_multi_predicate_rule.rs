use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

#[test]
fn simple_two_predicate_rule() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "a".to_string(),
            args: vec![Value::String("x".to_string())],
        },
        Fact {
            predicate: "b".to_string(),
            args: vec![
                Value::String("x".to_string()),
                Value::String("y".to_string()),
            ],
        },
    ]);

    // result(X, Y) :- a(X), b(X, Y).
    db.compile_rule(Rule {
        head: Predicate {
            name: "result".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "a".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "b".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    });

    let results = db.query("result", &[]);
    eprintln!("result facts: {:?}", results);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("x".to_string()));
    assert_eq!(results[0].args[1], Value::String("y".to_string()));
}

#[test]
fn two_predicate_rule_with_derived_facts() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "base".to_string(),
        args: vec![Value::String("x".to_string())],
    }]);

    db.compile_rule(Rule {
        head: Predicate {
            name: "a".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    db.compile_rule(Rule {
        head: Predicate {
            name: "b".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Constant(Value::String("y".to_string())),
            ],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    // result(X, Y) :- a(X), b(X, Y).
    db.compile_rule(Rule {
        head: Predicate {
            name: "result".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "a".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "b".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    });

    let results = db.query("result", &[]);
    eprintln!("result facts with derived predicates: {:?}", results);

    assert_eq!(results.len(), 1, "Should derive result from a and b");
    assert_eq!(results[0].args[0], Value::String("x".to_string()));
    assert_eq!(results[0].args[1], Value::String("y".to_string()));
}
