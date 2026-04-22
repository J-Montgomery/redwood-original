use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

#[test]
fn topological_sort_orders_dependencies() {
    let mut db = Engine::new();

    // Create dependency chain: A → B → C
    let facts = vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//pkg:a".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//pkg:b".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//pkg:c".to_string())],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//pkg:a".to_string()),
                Value::String("//pkg:b".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//pkg:b".to_string()),
                Value::String("//pkg:c".to_string()),
            ],
        },
    ];

    db.insert_facts(facts);

    let tc_rules = vec![
        Rule {
            head: Predicate {
                name: "transitive_deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("D".to_string()),
                ],
            },
            body: vec![Predicate {
                name: "deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("D".to_string()),
                ],
            }],
        },
        Rule {
            head: Predicate {
                name: "transitive_deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("D".to_string()),
                ],
            },
            body: vec![
                Predicate {
                    name: "deps".to_string(),
                    args: vec![
                        Term::Variable("T".to_string()),
                        Term::Variable("I".to_string()),
                    ],
                },
                Predicate {
                    name: "transitive_deps".to_string(),
                    args: vec![
                        Term::Variable("I".to_string()),
                        Term::Variable("D".to_string()),
                    ],
                },
            ],
        },
    ];

    for rule in tc_rules {
        db.compile_rule(rule);
    }

    let a_deps = db.query("transitive_deps", &[Some("//pkg:a"), None]);
    assert_eq!(a_deps.len(), 2);

    let b_deps = db.query("transitive_deps", &[Some("//pkg:b"), None]);
    assert_eq!(b_deps.len(), 1);
}

#[test]
fn smart_cache_invalidation_preserves_unrelated() {
    let mut db = Engine::new();

    let facts_a = vec![
        Fact {
            predicate: "a".to_string(),
            args: vec![Value::String("1".to_string())],
        },
        Fact {
            predicate: "a".to_string(),
            args: vec![Value::String("2".to_string())],
        },
    ];

    let facts_b = vec![Fact {
        predicate: "b".to_string(),
        args: vec![Value::String("3".to_string())],
    }];

    db.insert_facts(facts_a);
    db.insert_facts(facts_b.clone());

    db.compile_rule(Rule {
        head: Predicate {
            name: "derived_a".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "a".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    db.compile_rule(Rule {
        head: Predicate {
            name: "derived_b".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "b".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    let derived_a_first = db.query("derived_a", &[]);
    let derived_b_first = db.query("derived_b", &[]);

    assert_eq!(derived_a_first.len(), 2);
    assert_eq!(derived_b_first.len(), 1);

    db.insert_facts(vec![Fact {
        predicate: "a".to_string(),
        args: vec![Value::String("4".to_string())],
    }]);

    let derived_a_second = db.query("derived_a", &[]);
    assert_eq!(derived_a_second.len(), 3);

    let derived_b_second = db.query("derived_b", &[]);
    assert_eq!(derived_b_second.len(), 1);
}
