use redwood::datalog::Engine;
use redwood::datalog::{parser, Fact, Predicate, Rule, Term, Value};

#[test]
fn query_ground_facts() {
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

    let results = db.query("target", &[]);
    assert_eq!(results.len(), 2);
}

#[test]
fn simple_join_rule() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("rust_binary".to_string()),
            ],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:lib".to_string())],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:lib".to_string()),
                Value::String("rust_library".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "rust_binary_target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "kind".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Constant(Value::String("rust_binary".to_string())),
                ],
            },
        ],
    };

    db.compile_rule(rule);

    let results = db.query("rust_binary_target", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
}

#[test]
fn three_way_join() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![
                Value::String("main.rs".to_string()),
                Value::String("abc123".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "target_with_hash".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Variable("H".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "sources".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("S".to_string()),
                ],
            },
            Predicate {
                name: "file_hash".to_string(),
                args: vec![
                    Term::Variable("S".to_string()),
                    Term::Variable("H".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);

    let results = db.query("target_with_hash", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
    assert_eq!(results[0].args[1], Value::String("abc123".to_string()));
}

#[test]
fn negation_excludes_facts() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("target/cli".to_string()),
            ],
        },
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:server".to_string()),
                Value::String("target/server".to_string()),
            ],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("target/cli".to_string())],
        },
    ]);

    let rule = parser::parse_program(
        r#"
        missing_output(T) :-
            outputs(T, O),
            not(file_exists(O)).
    "#,
    )
    .unwrap();

    db.compile_rule(rule.1[0].clone());

    let results = db.query("missing_output", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("//app:server".to_string())
    );
}

#[test]
fn multiple_sources_one_target() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("lib.rs".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("config.toml".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "source_file".to_string(),
            args: vec![Term::Variable("S".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Constant(Value::String("//app:cli".to_string()))],
            },
            Predicate {
                name: "sources".to_string(),
                args: vec![
                    Term::Constant(Value::String("//app:cli".to_string())),
                    Term::Variable("S".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);

    let results = db.query("source_file", &[]);
    assert_eq!(results.len(), 3);
}

#[test]
fn builtin_gt_predicate() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![Value::String("main.rs".to_string()), Value::Integer(1000)],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![Value::String("main.rs".to_string()), Value::Integer(500)],
        },
    ]);

    let rule = parser::parse_program(
        r#"
        hash_newer(F) :-
            file_hash(F, New),
            cached_hash(F, Old),
            gt(New, Old).
    "#,
    )
    .unwrap();

    db.compile_rule(rule.1[0].clone());

    let results = db.query("hash_newer", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("main.rs".to_string()));
}

#[test]
fn hash_unchanged_does_not_match() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![Value::String("main.rs".to_string()), Value::Integer(1000)],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![Value::String("main.rs".to_string()), Value::Integer(1000)],
        },
    ]);

    let rule = parser::parse_program(
        r#"
        hash_changed(F) :-
            file_hash(F, New),
            cached_hash(F, Old),
            gt(New, Old).
    "#,
    )
    .unwrap();

    db.compile_rule(rule.1[0].clone());

    let results = db.query("hash_changed", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn transitive_rule() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("//app:lib".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:lib".to_string()),
                Value::String("//core:utils".to_string()),
            ],
        },
    ]);

    let rules = parser::parse_program(
        r#"
        all_deps(X, Y) :- deps(X, Y).
        all_deps(X, Z) :- deps(X, Y), all_deps(Y, Z).
    "#,
    )
    .unwrap();

    db.compile_rule(rules.1[0].clone());
    db.compile_rule(rules.1[1].clone());

    let results = db.query("all_deps", &[]);
    assert_eq!(results.len(), 3);
}

#[test]
fn needs_rebuild_missing_output() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("target/cli".to_string()),
            ],
        },
    ]);

    let rule = parser::parse_program(
        r#"
        needs_rebuild(T) :-
            target(T),
            outputs(T, O),
            not(file_exists(O)).
    "#,
    )
    .unwrap();

    db.compile_rule(rule.1[0].clone());

    let results = db.query("needs_rebuild", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
}

#[test]
fn needs_rebuild_hash_changed() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![
                Value::String("main.rs".to_string()),
                Value::String("newhash".to_string()),
            ],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("main.rs".to_string()),
                Value::String("oldhash".to_string()),
            ],
        },
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("target/cli".to_string()),
            ],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("target/cli".to_string())],
        },
    ]);

    let rule = parser::parse_program(
        r#"
        needs_rebuild(T) :-
            target(T),
            sources(T, S),
            file_hash(S, New),
            cached_hash(T, S, Old),
            New != Old.
    "#,
    )
    .unwrap();

    db.compile_rule(rule.1[0].clone());

    let results = db.query("needs_rebuild", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
}

#[test]
fn no_rebuild_when_hashes_match() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![
                Value::String("main.rs".to_string()),
                Value::String("samehash".to_string()),
            ],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("main.rs".to_string()),
                Value::String("samehash".to_string()),
            ],
        },
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("target/cli".to_string()),
            ],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("target/cli".to_string())],
        },
    ]);

    let rules = parser::parse_program(
        r#"
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
    "#,
    )
    .unwrap();

    db.compile_rule(rules.1[0].clone());
    db.compile_rule(rules.1[1].clone());

    let results = db.query("needs_rebuild", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn cartesian_product_without_join_variable() {
    let mut db = Engine::new();
    db.insert_facts(vec![
        Fact {
            predicate: "color".to_string(),
            args: vec![Value::String("red".to_string())],
        },
        Fact {
            predicate: "color".to_string(),
            args: vec![Value::String("blue".to_string())],
        },
        Fact {
            predicate: "size".to_string(),
            args: vec![Value::String("small".to_string())],
        },
        Fact {
            predicate: "size".to_string(),
            args: vec![Value::String("large".to_string())],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "combination".to_string(),
            args: vec![
                Term::Variable("C".to_string()),
                Term::Variable("S".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "color".to_string(),
                args: vec![Term::Variable("C".to_string())],
            },
            Predicate {
                name: "size".to_string(),
                args: vec![Term::Variable("S".to_string())],
            },
        ],
    };

    db.compile_rule(rule);

    let results = db.query("combination", &[]);
    assert_eq!(results.len(), 4);
}

#[test]
fn constant_filter_in_body() {
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
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("rust_binary".to_string()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//app:server".to_string()),
                Value::String("rust_binary".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "is_cli".to_string(),
            args: vec![],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Constant(Value::String("//app:cli".to_string()))],
            },
            Predicate {
                name: "kind".to_string(),
                args: vec![
                    Term::Constant(Value::String("//app:cli".to_string())),
                    Term::Constant(Value::String("rust_binary".to_string())),
                ],
            },
        ],
    };

    db.compile_rule(rule);

    let results = db.query("is_cli", &[]);
    assert_eq!(results.len(), 1);
}

#[test]
fn needs_rebuild_detects_deleted_output() {
    let rules = parser::parse_program(
        r#"
        needs_rebuild(T) :-
            target(T),
            outputs(T, O),
            not(file_exists(O)).

        needs_rebuild(T) :-
            target(T),
            sources(T, S),
            file_hash(S, NewHash),
            cached_hash(T, S, OldHash),
            NewHash != OldHash.
    "#,
    )
    .unwrap();

    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("target/cli".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![
                Value::String("src/main.rs".to_string()),
                Value::String("hash123".to_string()),
            ],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/main.rs".to_string()),
                Value::String("hash123".to_string()),
            ],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("target/cli".to_string())],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("src/main.rs".to_string())],
        },
    ]);

    for rule in &rules.1 {
        db.compile_rule(rule.clone());
    }

    let results = db.query("needs_rebuild", &[]);
    assert_eq!(
        results.len(),
        0,
        "Output exists and hashes match, should not need rebuild"
    );

    let mut db_after_delete = Engine::new();
    db_after_delete.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("target/cli".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![
                Value::String("src/main.rs".to_string()),
                Value::String("hash123".to_string()),
            ],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/main.rs".to_string()),
                Value::String("hash123".to_string()),
            ],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("src/main.rs".to_string())],
        },
    ]);

    for rule in &rules.1 {
        db_after_delete.compile_rule(rule.clone());
    }

    let results = db_after_delete.query("needs_rebuild", &[]);
    assert_eq!(
        results.len(),
        1,
        "Output file deleted (file_exists fact missing), should need rebuild"
    );
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));

    let mut db_after_restore = Engine::new();
    db_after_restore.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("target/cli".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![
                Value::String("src/main.rs".to_string()),
                Value::String("hash123".to_string()),
            ],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/main.rs".to_string()),
                Value::String("hash123".to_string()),
            ],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("target/cli".to_string())],
        },
        Fact {
            predicate: "file_exists".to_string(),
            args: vec![Value::String("src/main.rs".to_string())],
        },
    ]);

    for rule in &rules.1 {
        db_after_restore.compile_rule(rule.clone());
    }

    let results = db_after_restore.query("needs_rebuild", &[]);
    assert_eq!(
        results.len(),
        0,
        "Output file restored, should not need rebuild"
    );
}
