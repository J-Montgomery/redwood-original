use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

#[test]
fn filter_after_fact_insertion() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "target".to_string(),
        args: vec![Value::String("a".to_string())],
    }]);

    db.compile_rule(Rule {
        head: Predicate {
            name: "derived".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        }],
    });

    // Query before adding more facts
    let result1 = db.query("derived", &[Some("a")]);
    assert_eq!(result1.len(), 1);

    // Add more facts
    db.insert_facts(vec![Fact {
        predicate: "target".to_string(),
        args: vec![Value::String("b".to_string())],
    }]);

    // Query after insertion - cache should be invalidated
    let result2 = db.query("derived", &[Some("b")]);
    assert_eq!(result2.len(), 1);
    assert_eq!(result2[0].args[0], Value::String("b".to_string()));

    // Query original should still work
    let result3 = db.query("derived", &[Some("a")]);
    assert_eq!(result3.len(), 1);
}

#[test]
fn constant_in_rule_head() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "base".to_string(),
            args: vec![Value::String("x".to_string())],
        },
        Fact {
            predicate: "base".to_string(),
            args: vec![Value::String("y".to_string())],
        },
    ]);

    // Rule with constant in head: derived("fixed", X) :- base(X)
    db.compile_rule(Rule {
        head: Predicate {
            name: "derived".to_string(),
            args: vec![
                Term::Constant(Value::String("fixed".to_string())),
                Term::Variable("X".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    // Filter on constant
    let result = db.query("derived", &[Some("fixed")]);
    assert_eq!(result.len(), 2);
    for fact in &result {
        assert_eq!(fact.args[0], Value::String("fixed".to_string()));
    }

    // Filter on different constant (should return empty)
    let result_empty = db.query("derived", &[Some("wrong")]);
    assert_eq!(result_empty.len(), 0);
}

#[test]
fn multiple_joins_with_shared_variables() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "a".to_string(),
            args: vec![
                Value::String("x".to_string()),
                Value::String("y".to_string()),
            ],
        },
        Fact {
            predicate: "b".to_string(),
            args: vec![
                Value::String("y".to_string()),
                Value::String("z".to_string()),
            ],
        },
        Fact {
            predicate: "c".to_string(),
            args: vec![
                Value::String("z".to_string()),
                Value::String("w".to_string()),
            ],
        },
    ]);

    // Rule: result(X, W) :- a(X, Y), b(Y, Z), c(Z, W)
    db.compile_rule(Rule {
        head: Predicate {
            name: "result".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("W".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "a".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
            Predicate {
                name: "b".to_string(),
                args: vec![
                    Term::Variable("Y".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            Predicate {
                name: "c".to_string(),
                args: vec![
                    Term::Variable("Z".to_string()),
                    Term::Variable("W".to_string()),
                ],
            },
        ],
    });

    // Filtered query
    let result = db.query("result", &[Some("x")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("x".to_string()));
    assert_eq!(result[0].args[1], Value::String("w".to_string()));
}

#[test]
fn empty_result_with_filters() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "target".to_string(),
        args: vec![Value::String("a".to_string())],
    }]);

    db.compile_rule(Rule {
        head: Predicate {
            name: "derived".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        }],
    });

    // Query for non-existent target
    let result = db.query("derived", &[Some("nonexistent")]);
    assert_eq!(result.len(), 0);
}

#[test]
fn self_join_with_filter() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "related".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ],
        },
        Fact {
            predicate: "related".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ],
        },
    ]);

    // Rule: transitive(X, Z) :- related(X, Y), related(Y, Z)
    db.compile_rule(Rule {
        head: Predicate {
            name: "transitive".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Z".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "related".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
            Predicate {
                name: "related".to_string(),
                args: vec![
                    Term::Variable("Y".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
        ],
    });

    let result = db.query("transitive", &[Some("a")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("a".to_string()));
    assert_eq!(result[0].args[1], Value::String("c".to_string()));
}

#[test]
fn derived_predicate_used_multiple_times() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "base".to_string(),
        args: vec![Value::String("x".to_string())],
    }]);

    // First derived predicate
    db.compile_rule(Rule {
        head: Predicate {
            name: "step1".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    // Second derived predicate using first
    db.compile_rule(Rule {
        head: Predicate {
            name: "step2".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "step1".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    // Third using second
    db.compile_rule(Rule {
        head: Predicate {
            name: "final".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "step2".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    // Filtered query should work through chain
    let result = db.query("final", &[Some("x")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("x".to_string()));
}

#[test]
fn comparison_with_lazy_evaluation() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("a".to_string()), Value::Integer(5)],
        },
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("b".to_string()), Value::Integer(10)],
        },
    ]);

    // Rule: high_value(X) :- value(X, V), gt(V, 5)
    db.compile_rule(Rule {
        head: Predicate {
            name: "high_value".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "value".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("V".to_string()),
                ],
            },
            Predicate {
                name: "gt".to_string(),
                args: vec![
                    Term::Variable("V".to_string()),
                    Term::Constant(Value::Integer(5)),
                ],
            },
        ],
    });

    // First test unfiltered to see if rule works at all
    let result_all = db.query("high_value", &[]);
    eprintln!("Unfiltered results: {:?}", result_all);

    let result = db.query("high_value", &[Some("b")]);
    eprintln!("Filtered results for 'b': {:?}", result);
    assert_eq!(result.len(), 1, "Expected 10 > 5");
    assert_eq!(result[0].args[0], Value::String("b".to_string()));

    let result_a = db.query("high_value", &[Some("a")]);
    eprintln!("Filtered results for 'a': {:?}", result_a);
    assert_eq!(result_a.len(), 0); // 5 is not > 5
}

#[test]
fn double_negation() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "a".to_string(),
            args: vec![Value::String("x".to_string())],
        },
        Fact {
            predicate: "excluded".to_string(),
            args: vec![Value::String("y".to_string())],
        },
    ]);

    // not_excluded(X) :- a(X), not(excluded(X))
    db.compile_rule(Rule {
        head: Predicate {
            name: "not_excluded".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "a".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "not:excluded".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
        ],
    });

    // not_not_excluded(X) :- a(X), not(not_excluded(X))
    db.compile_rule(Rule {
        head: Predicate {
            name: "not_not_excluded".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "a".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "not:not_excluded".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
        ],
    });

    let result = db.query("not_excluded", &[Some("x")]);
    assert_eq!(result.len(), 1);

    let result2 = db.query("not_not_excluded", &[Some("x")]);
    assert_eq!(result2.len(), 0); // Double negation: x is not_excluded, so not(not_excluded(x)) is false
}

#[test]
fn mixed_constants_and_variables() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("type1".to_string()),
                Value::String("b".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("type2".to_string()),
                Value::String("c".to_string()),
            ],
        },
    ]);

    // Rule: typed_edge(X, Z) :- edge(X, "type1", Z)
    db.compile_rule(Rule {
        head: Predicate {
            name: "typed_edge".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Z".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "edge".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Constant(Value::String("type1".to_string())),
                Term::Variable("Z".to_string()),
            ],
        }],
    });

    let result = db.query("typed_edge", &[Some("a")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("a".to_string()));
    assert_eq!(result[0].args[1], Value::String("b".to_string()));
}
