use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

#[test]
fn self_join_same_variable_twice() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "same_package".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("//app:server".to_string()),
            ],
        },
        Fact {
            predicate: "same_package".to_string(),
            args: vec![
                Value::String("//lib:core".to_string()),
                Value::String("//lib:util".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "same_prefix".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "same_package".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        }],
    };

    db.compile_rule(rule);
    let results = db.query("same_prefix", &[]);
    assert_eq!(results.len(), 2);
}

#[test]
fn self_join_forces_equality() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "self_loop".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "edge".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("X".to_string()),
            ],
        }],
    };

    db.compile_rule(rule);
    let results = db.query("self_loop", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn cross_product_no_shared_variables() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "lang".to_string(),
            args: vec![Value::String("rust".to_string())],
        },
        Fact {
            predicate: "lang".to_string(),
            args: vec![Value::String("go".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:server".to_string())],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "combination".to_string(),
            args: vec![
                Term::Variable("L".to_string()),
                Term::Variable("T".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "lang".to_string(),
                args: vec![Term::Variable("L".to_string())],
            },
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("combination", &[]);
    assert_eq!(results.len(), 4);
}

#[test]
fn multiple_shared_variables_in_join() {
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
                Value::String("b".to_string()),
                Value::String("d".to_string()),
            ],
        },
        Fact {
            predicate: "pair".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "matched".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
                Term::Variable("Z".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "triple".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            Predicate {
                name: "pair".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("matched", &[]);
    assert_eq!(results.len(), 2);
}

#[test]
fn negation_with_fully_bound_variables() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:server".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//lib:core".to_string())],
        },
        Fact {
            predicate: "exclude".to_string(),
            args: vec![Value::String("//app:server".to_string())],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "included".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "not:exclude".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("included", &[]);
    assert_eq!(results.len(), 2);

    let has_cli = results
        .iter()
        .any(|f| f.args[0] == Value::String("//app:cli".to_string()));
    let has_lib = results
        .iter()
        .any(|f| f.args[0] == Value::String("//lib:core".to_string()));
    let has_server = results
        .iter()
        .any(|f| f.args[0] == Value::String("//app:server".to_string()));

    assert!(has_cli);
    assert!(has_lib);
    assert!(!has_server);
}

#[test]
fn negation_with_wildcard_variable() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:server".to_string())],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("//lib:core".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("//lib:util".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "no_deps".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "not:deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("_".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("no_deps", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("//app:server".to_string())
    );
}

#[test]
fn recursive_with_cycle() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("a".to_string()),
                Value::String("b".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("c".to_string()),
            ],
        },
        Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String("c".to_string()),
                Value::String("a".to_string()),
            ],
        },
    ]);

    let base_rule = Rule {
        head: Predicate {
            name: "reachable".to_string(),
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
    };

    let recursive_rule = Rule {
        head: Predicate {
            name: "reachable".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "edge".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            Predicate {
                name: "reachable".to_string(),
                args: vec![
                    Term::Variable("Z".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    };

    {
        db.compile_rule(base_rule);
        db.compile_rule(recursive_rule);

        let results = db.query("reachable", &[]);

        assert!(results.len() >= 9);

        let has_a_a = results.iter().any(|f| {
            f.args[0] == Value::String("a".to_string())
                && f.args[1] == Value::String("a".to_string())
        });
        let has_b_b = results.iter().any(|f| {
            f.args[0] == Value::String("b".to_string())
                && f.args[1] == Value::String("b".to_string())
        });
        let has_c_c = results.iter().any(|f| {
            f.args[0] == Value::String("c".to_string())
                && f.args[1] == Value::String("c".to_string())
        });

        assert!(has_a_a);
        assert!(has_b_b);
        assert!(has_c_c);
    }
}

#[test]
fn recursive_self_loop() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "edge".to_string(),
        args: vec![
            Value::String("a".to_string()),
            Value::String("a".to_string()),
        ],
    }]);

    let base_rule = Rule {
        head: Predicate {
            name: "reachable".to_string(),
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
    };

    let recursive_rule = Rule {
        head: Predicate {
            name: "reachable".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "edge".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            Predicate {
                name: "reachable".to_string(),
                args: vec![
                    Term::Variable("Z".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    };

    {
        db.compile_rule(base_rule);
        db.compile_rule(recursive_rule);

        let results = db.query("reachable", &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].args[0], Value::String("a".to_string()));
        assert_eq!(results[0].args[1], Value::String("a".to_string()));
    }
}

#[test]
fn builtin_comparison_same_variable() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "value".to_string(),
        args: vec![Value::String("x".to_string()), Value::Integer(5)],
    }]);

    let rule = Rule {
        head: Predicate {
            name: "greater_than_self".to_string(),
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
                    Term::Variable("V".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("greater_than_self", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn builtin_comparison_with_constants() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("low".to_string()), Value::Integer(3)],
        },
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("medium".to_string()), Value::Integer(7)],
        },
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("high".to_string()), Value::Integer(15)],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "in_range".to_string(),
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
            Predicate {
                name: "lt".to_string(),
                args: vec![
                    Term::Variable("V".to_string()),
                    Term::Constant(Value::Integer(10)),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("in_range", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("medium".to_string()));
}

#[test]
fn inequality_filters_duplicates() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:server".to_string())],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "pair".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("Y".to_string())],
            },
            Predicate {
                name: "!=".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("pair", &[]);
    assert_eq!(results.len(), 2);

    for result in &results {
        assert_ne!(result.args[0], result.args[1]);
    }
}

#[test]
fn multiple_rules_same_head_disjunction() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("rust_binary".to_string()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//lib:core".to_string()),
                Value::String("rust_library".to_string()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:server".to_string()),
                Value::String("go_binary".to_string()),
            ],
        },
    ]);

    let rule1 = Rule {
        head: Predicate {
            name: "rust_target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "kind".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Constant(Value::String("rust_binary".to_string())),
            ],
        }],
    };

    let rule2 = Rule {
        head: Predicate {
            name: "rust_target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "kind".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Constant(Value::String("rust_library".to_string())),
            ],
        }],
    };

    db.compile_rule(rule1);
    db.compile_rule(rule2);

    let results = db.query("rust_target", &[]);
    assert_eq!(results.len(), 2);

    let has_cli = results
        .iter()
        .any(|f| f.args[0] == Value::String("//app:cli".to_string()));
    let has_lib = results
        .iter()
        .any(|f| f.args[0] == Value::String("//lib:core".to_string()));

    assert!(has_cli);
    assert!(has_lib);
}

#[test]
fn chained_derivation() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "base".to_string(),
        args: vec![Value::String("a".to_string())],
    }]);

    let rule1 = Rule {
        head: Predicate {
            name: "level1".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    };

    let rule2 = Rule {
        head: Predicate {
            name: "level2".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "level1".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    };

    let rule3 = Rule {
        head: Predicate {
            name: "level3".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "level2".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    };

    db.compile_rule(rule1);
    db.compile_rule(rule2);
    db.compile_rule(rule3);

    assert_eq!(db.query("level1", &[]).len(), 1);
    assert_eq!(db.query("level2", &[]).len(), 1);
    assert_eq!(db.query("level3", &[]).len(), 1);
}

#[test]
fn empty_result_sets_propagate() {
    let mut db = Engine::new();

    let rule = Rule {
        head: Predicate {
            name: "derived".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "nonexistent".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "another_nonexistent".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("derived", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn constant_only_predicate() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "config".to_string(),
            args: vec![Value::String("debug".to_string()), Value::Bool(true)],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "debug_target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "config".to_string(),
                args: vec![
                    Term::Constant(Value::String("debug".to_string())),
                    Term::Constant(Value::Bool(true)),
                ],
            },
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("debug_target", &[]);
    assert_eq!(results.len(), 1);
}

#[test]
fn large_fan_out() {
    let mut db = Engine::new();

    let mut facts = vec![Fact {
        predicate: "root".to_string(),
        args: vec![Value::String("//app:main".to_string())],
    }];

    for i in 0..100 {
        facts.push(Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:main".to_string()),
                Value::String(format!("//lib:dep{}", i)),
            ],
        });
    }

    db.insert_facts(facts);

    let rule = Rule {
        head: Predicate {
            name: "root_deps".to_string(),
            args: vec![
                Term::Variable("R".to_string()),
                Term::Variable("D".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "root".to_string(),
                args: vec![Term::Variable("R".to_string())],
            },
            Predicate {
                name: "deps".to_string(),
                args: vec![
                    Term::Variable("R".to_string()),
                    Term::Variable("D".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("root_deps", &[]);
    assert_eq!(results.len(), 100);
}

#[test]
fn deep_transitive_chain() {
    let mut db = Engine::new();

    let mut facts = vec![];
    for i in 0..50 {
        facts.push(Fact {
            predicate: "edge".to_string(),
            args: vec![
                Value::String(format!("n{}", i)),
                Value::String(format!("n{}", i + 1)),
            ],
        });
    }

    db.insert_facts(facts);

    let base_rule = Rule {
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
    };

    let recursive_rule = Rule {
        head: Predicate {
            name: "path".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "edge".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            Predicate {
                name: "path".to_string(),
                args: vec![
                    Term::Variable("Z".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    };

    {
        // Use public API
        db.compile_rule(base_rule);
        db.compile_rule(recursive_rule);

        let results = db.query("path", &[]);

        assert!(results.len() >= 50);

        let has_full_path = results.iter().any(|f| {
            f.args[0] == Value::String("n0".to_string())
                && f.args[1] == Value::String("n50".to_string())
        });
        assert!(has_full_path);
    }
}

#[test]
fn test_membership_check_with_negation() {
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
        Fact {
            predicate: "base".to_string(),
            args: vec![Value::String("c".to_string())],
        },
    ]);

    let derived_rule = Rule {
        head: Predicate {
            name: "derived".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    };

    let excluded_rule = Rule {
        head: Predicate {
            name: "excluded".to_string(),
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
                    Term::Constant(Value::String("b".to_string())),
                ],
            },
        ],
    };

    let result_rule = Rule {
        head: Predicate {
            name: "result".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![
            Predicate {
                name: "derived".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
            Predicate {
                name: "not:excluded".to_string(),
                args: vec![Term::Variable("X".to_string())],
            },
        ],
    };

    db.compile_rule(derived_rule);
    db.compile_rule(excluded_rule);
    db.compile_rule(result_rule);

    let results = db.query("result", &[]);
    assert_eq!(results.len(), 2);

    let has_a = results
        .iter()
        .any(|f| f.args[0] == Value::String("a".to_string()));
    let has_b = results
        .iter()
        .any(|f| f.args[0] == Value::String("b".to_string()));
    let has_c = results
        .iter()
        .any(|f| f.args[0] == Value::String("c".to_string()));

    assert!(has_a);
    assert!(!has_b);
    assert!(has_c);
}
