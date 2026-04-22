use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

#[test]
fn negation_with_bound_variables() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("a".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("b".to_string())],
        },
        Fact {
            predicate: "cached".to_string(),
            args: vec![Value::String("a".to_string())],
        },
    ]);

    // Rule: needs_rebuild(T) :- target(T), not(cached(T))
    db.compile_rule(Rule {
        head: Predicate {
            name: "needs_rebuild".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "not:cached".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
        ],
    });

    // Filtered query - should only check b
    let result = db.query("needs_rebuild", &[Some("b")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("b".to_string()));

    // Unfiltered query - should return only b
    let result_all = db.query("needs_rebuild", &[]);
    assert_eq!(result_all.len(), 1);
    assert_eq!(result_all[0].args[0], Value::String("b".to_string()));
}

#[test]
fn negation_with_unbound_variables() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("a".to_string())],
        },
        Fact {
            predicate: "cached".to_string(),
            args: vec![Value::String("x".to_string())],
        },
    ]);

    // Rule with negation where variable is unbound in negation
    // only_target(T) :- target(T), not(cached(T))
    db.compile_rule(Rule {
        head: Predicate {
            name: "only_target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "not:cached".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
        ],
    });

    let result = db.query("only_target", &[Some("a")]);
    assert_eq!(result.len(), 1, "a is target but not cached");
    assert_eq!(result[0].args[0], Value::String("a".to_string()));
}

#[test]
fn variable_binding_order_independence() {
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
    ]);

    // Rule: result(X, Y, Z) :- a(X, Y), b(Y, Z)
    db.compile_rule(Rule {
        head: Predicate {
            name: "result".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
                Term::Variable("Z".to_string()),
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
        ],
    });

    // Filter on first variable
    let result = db.query("result", &[Some("x")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("x".to_string()));
    assert_eq!(result[0].args[1], Value::String("y".to_string()));
    assert_eq!(result[0].args[2], Value::String("z".to_string()));
}

#[test]
fn derived_predicate_in_negation() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "base".to_string(),
            args: vec![Value::String("a".to_string())],
        },
        Fact {
            predicate: "base".to_string(),
            args: vec![Value::String("b".to_string())],
        },
    ]);

    // Derive filtered predicate
    db.compile_rule(Rule {
        head: Predicate {
            name: "filtered".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "base".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "=".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Constant(Value::String("a".to_string())),
                ],
            },
        ],
    });

    // Use derived predicate in negation
    db.compile_rule(Rule {
        head: Predicate {
            name: "not_filtered".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "base".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "not:filtered".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
        ],
    });

    let result = db.query("not_filtered", &[Some("b")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("b".to_string()));

    let result_all = db.query("not_filtered", &[]);
    assert_eq!(result_all.len(), 1);
    assert_eq!(result_all[0].args[0], Value::String("b".to_string()));
}

#[test]
fn multi_rule_predicate_with_filters() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "type_a".to_string(),
            args: vec![Value::String("x".to_string())],
        },
        Fact {
            predicate: "type_b".to_string(),
            args: vec![Value::String("y".to_string())],
        },
    ]);

    // First rule: target(T) :- type_a(T)
    db.compile_rule(Rule {
        head: Predicate {
            name: "target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "type_a".to_string(),
            args: vec![Term::Variable("T".to_string())],
        }],
    });

    // Second rule: target(T) :- type_b(T)
    db.compile_rule(Rule {
        head: Predicate {
            name: "target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "type_b".to_string(),
            args: vec![Term::Variable("T".to_string())],
        }],
    });

    // Query each separately
    let result_x = db.query("target", &[Some("x")]);
    assert_eq!(result_x.len(), 1);
    assert_eq!(result_x[0].args[0], Value::String("x".to_string()));

    let result_y = db.query("target", &[Some("y")]);
    assert_eq!(result_y.len(), 1);
    assert_eq!(result_y[0].args[0], Value::String("y".to_string()));

    // Query all
    let result_all = db.query("target", &[]);
    assert_eq!(result_all.len(), 2);
}

#[test]
fn cache_consistency_across_queries() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "base".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("1".to_string()),
            ],
        },
        Fact {
            predicate: "base".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("2".to_string()),
            ],
        },
    ]);

    db.compile_rule(Rule {
        head: Predicate {
            name: "derived".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        }],
    });

    // First query: filtered
    let result1 = db.query("derived", &[Some("a")]);
    assert_eq!(result1.len(), 1);
    assert_eq!(result1[0].args[0], Value::String("a".to_string()));

    // Second query: unfiltered (should use cache)
    let result2 = db.query("derived", &[]);
    assert_eq!(result2.len(), 2);

    // Third query: different filter
    let result3 = db.query("derived", &[Some("b")]);
    assert_eq!(result3.len(), 1);
    assert_eq!(result3[0].args[0], Value::String("b".to_string()));

    // Fourth query: back to first filter (should be consistent)
    let result4 = db.query("derived", &[Some("a")]);
    assert_eq!(result4.len(), 1);
    assert_eq!(result4[0].args[0], Value::String("a".to_string()));
}

#[test]
fn equality_in_rule_body_with_lazy_eval() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "item".to_string(),
            args: vec![
                Value::String("x".to_string()),
                Value::String("val1".to_string()),
            ],
        },
        Fact {
            predicate: "item".to_string(),
            args: vec![
                Value::String("y".to_string()),
                Value::String("val2".to_string()),
            ],
        },
    ]);

    // Rule with equality: filtered(X) :- item(X, V), V = "val1"
    db.compile_rule(Rule {
        head: Predicate {
            name: "filtered".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "item".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("V".to_string()),
                ],
            },
            Predicate {
                name: "=".to_string(),
                args: vec![
                    Term::Variable("V".to_string()),
                    Term::Constant(Value::String("val1".to_string())),
                ],
            },
        ],
    });

    // Filtered query
    let result = db.query("filtered", &[Some("x")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("x".to_string()));

    // Query with wrong filter (should return empty)
    let result_empty = db.query("filtered", &[Some("y")]);
    assert_eq!(result_empty.len(), 0);
}

#[test]
fn deep_dependency_chain() {
    let mut db = Engine::new();

    // Create chain: a -> b -> c -> d -> e
    for i in 0..4 {
        db.insert_facts(vec![Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String(format!("n{}", i)),
                Value::String(format!("n{}", i + 1)),
            ],
        }]);
    }

    // Transitive closure
    db.compile_rule(Rule {
        head: Predicate {
            name: "path".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "edge".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        }],
    });

    db.compile_rule(Rule {
        head: Predicate {
            name: "path".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Z".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "edge".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
            Predicate {
                name: "path".to_string(),
                args: vec![
                    Term::Variable("Y".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
        ],
    });

    // Filtered query should only compute needed paths
    let result = db.query("path", &[Some("n0")]);
    assert_eq!(result.len(), 4); // n0->n1, n0->n2, n0->n3, n0->n4

    // Verify correctness
    let targets: Vec<String> = result
        .iter()
        .map(|f| match &f.args[1] {
            Value::String(s) => s.clone(),
            _ => panic!("Expected string"),
        })
        .collect();

    for i in 1..=4 {
        assert!(
            targets.contains(&format!("n{}", i)),
            "Should contain n{}",
            i
        );
    }
}

#[test]
fn inequality_in_membership_check() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "item".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("excluded".to_string()),
            ],
        },
        Fact {
            predicate: "item".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("included".to_string()),
            ],
        },
    ]);

    // Rule: valid(X) :- item(X, V), V != "excluded"
    db.compile_rule(Rule {
        head: Predicate {
            name: "valid".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "item".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("V".to_string()),
                ],
            },
            Predicate {
                name: "!=".to_string(),
                args: vec![
                    Term::Variable("V".to_string()),
                    Term::Constant(Value::String("excluded".to_string())),
                ],
            },
        ],
    });

    // Filtered query for 'b' should succeed
    let result = db.query("valid", &[Some("b")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].args[0], Value::String("b".to_string()));

    // Filtered query for 'a' should fail (excluded)
    let result_excluded = db.query("valid", &[Some("a")]);
    assert_eq!(result_excluded.len(), 0);
}

#[test]
fn partial_variable_binding_in_multi_arg_predicate() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "triple".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ],
        },
        Fact {
            predicate: "triple".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("x".to_string()),
                Value::String("y".to_string()),
            ],
        },
    ]);

    // Rule that uses triple with partial binding
    db.compile_rule(Rule {
        head: Predicate {
            name: "uses_triple".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
                Term::Variable("Z".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "triple".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
                Term::Variable("Z".to_string()),
            ],
        }],
    });

    // Filter on first arg only
    let result = db.query("uses_triple", &[Some("a")]);
    assert_eq!(result.len(), 2);

    // Verify both tuples with first arg "a" are returned
    for fact in &result {
        assert_eq!(fact.args[0], Value::String("a".to_string()));
    }
}
