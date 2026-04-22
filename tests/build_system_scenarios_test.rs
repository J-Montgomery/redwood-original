use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

#[test]
fn diamond_dependency() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:main".to_string()),
                Value::String("//lib:a".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:main".to_string()),
                Value::String("//lib:b".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//lib:a".to_string()),
                Value::String("//lib:core".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//lib:b".to_string()),
                Value::String("//lib:core".to_string()),
            ],
        },
    ]);

    let base_rule = Rule {
        head: Predicate {
            name: "transitive_deps".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "deps".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        }],
    };

    let recursive_rule = Rule {
        head: Predicate {
            name: "transitive_deps".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "deps".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            Predicate {
                name: "transitive_deps".to_string(),
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

        let results = db.query("transitive_deps", &[]);

        let main_to_core_count = results
            .iter()
            .filter(|f| {
                f.args[0] == Value::String("//app:main".to_string())
                    && f.args[1] == Value::String("//lib:core".to_string())
            })
            .count();

        assert_eq!(main_to_core_count, 1);
    }
}

#[test]
fn platform_specific_targets() {
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
            args: vec![Value::String("//app:mobile".to_string())],
        },
        Fact {
            predicate: "platform".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("linux".to_string()),
            ],
        },
        Fact {
            predicate: "platform".to_string(),
            args: vec![
                Value::String("//app:server".to_string()),
                Value::String("linux".to_string()),
            ],
        },
        Fact {
            predicate: "platform".to_string(),
            args: vec![
                Value::String("//app:mobile".to_string()),
                Value::String("android".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "linux_target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "platform".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Constant(Value::String("linux".to_string())),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("linux_target", &[]);
    assert_eq!(results.len(), 2);
}

#[test]
fn source_file_filtering() {
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
                Value::String("src/main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/lib.rs".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("test/test_main.rs".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "test_sources".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Variable("S".to_string()),
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
        ],
    };

    db.compile_rule(rule);
    let results = db.query("test_sources", &[]);
    assert_eq!(results.len(), 3);
}

#[test]
fn exclude_test_dependencies() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
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
                Value::String("//lib:test_utils".to_string()),
            ],
        },
        Fact {
            predicate: "testonly".to_string(),
            args: vec![Value::String("//lib:test_utils".to_string())],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "production_deps".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Variable("D".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("D".to_string()),
                ],
            },
            Predicate {
                name: "not:testonly".to_string(),
                args: vec![Term::Variable("D".to_string())],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("production_deps", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[1], Value::String("//lib:core".to_string()));
}

#[test]
fn visibility_rules() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//lib:internal".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//lib:public".to_string())],
        },
        Fact {
            predicate: "visibility".to_string(),
            args: vec![
                Value::String("//lib:internal".to_string()),
                Value::String("//lib".to_string()),
            ],
        },
        Fact {
            predicate: "visibility".to_string(),
            args: vec![
                Value::String("//lib:public".to_string()),
                Value::String("//visibility:public".to_string()),
            ],
        },
        Fact {
            predicate: "package".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("//app".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "can_access".to_string(),
            args: vec![
                Term::Variable("Target".to_string()),
                Term::Variable("Lib".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("Target".to_string())],
            },
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("Lib".to_string())],
            },
            Predicate {
                name: "visibility".to_string(),
                args: vec![
                    Term::Variable("Lib".to_string()),
                    Term::Constant(Value::String("//visibility:public".to_string())),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("can_access", &[]);

    let app_can_access_public = results.iter().any(|f| {
        f.args[0] == Value::String("//app:cli".to_string())
            && f.args[1] == Value::String("//lib:public".to_string())
    });

    assert!(app_can_access_public);
}

#[test]
fn toolchain_version_constraints() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:modern".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:legacy".to_string())],
        },
        Fact {
            predicate: "min_version".to_string(),
            args: vec![
                Value::String("//app:modern".to_string()),
                Value::Integer(2021),
            ],
        },
        Fact {
            predicate: "min_version".to_string(),
            args: vec![
                Value::String("//app:legacy".to_string()),
                Value::Integer(2015),
            ],
        },
        Fact {
            predicate: "toolchain".to_string(),
            args: vec![Value::Integer(2021)],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "compatible".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "min_version".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("MinV".to_string()),
                ],
            },
            Predicate {
                name: "toolchain".to_string(),
                args: vec![Term::Variable("ToolV".to_string())],
            },
            Predicate {
                name: "gt".to_string(),
                args: vec![
                    Term::Variable("ToolV".to_string()),
                    Term::Variable("MinV".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("compatible", &[]);

    let has_legacy = results
        .iter()
        .any(|f| f.args[0] == Value::String("//app:legacy".to_string()));

    assert!(has_legacy);
    assert!(!results.is_empty());
}

#[test]
fn circular_dependency_detection() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:a".to_string()),
                Value::String("//app:b".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:b".to_string()),
                Value::String("//app:c".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//app:c".to_string()),
                Value::String("//app:a".to_string()),
            ],
        },
    ]);

    let base_rule = Rule {
        head: Predicate {
            name: "reaches".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "deps".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        }],
    };

    let recursive_rule = Rule {
        head: Predicate {
            name: "reaches".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("Y".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "deps".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            },
            Predicate {
                name: "reaches".to_string(),
                args: vec![
                    Term::Variable("Z".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
        ],
    };

    let cycle_rule = Rule {
        head: Predicate {
            name: "cycle".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "reaches".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("X".to_string()),
            ],
        }],
    };

    {
        db.compile_rule(base_rule);
        db.compile_rule(recursive_rule);
        db.compile_rule(cycle_rule);

        let results = db.query("cycle", &[]);
        assert_eq!(results.len(), 3);
    }
}

#[test]
fn multi_language_project() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//backend:api".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//frontend:app".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//shared:proto".to_string())],
        },
        Fact {
            predicate: "lang".to_string(),
            args: vec![
                Value::String("//backend:api".to_string()),
                Value::String("rust".to_string()),
            ],
        },
        Fact {
            predicate: "lang".to_string(),
            args: vec![
                Value::String("//frontend:app".to_string()),
                Value::String("typescript".to_string()),
            ],
        },
        Fact {
            predicate: "lang".to_string(),
            args: vec![
                Value::String("//shared:proto".to_string()),
                Value::String("protobuf".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//backend:api".to_string()),
                Value::String("//shared:proto".to_string()),
            ],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String("//frontend:app".to_string()),
                Value::String("//shared:proto".to_string()),
            ],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "uses_proto".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Variable("L".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
            Predicate {
                name: "lang".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("L".to_string()),
                ],
            },
            Predicate {
                name: "deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("P".to_string()),
                ],
            },
            Predicate {
                name: "lang".to_string(),
                args: vec![
                    Term::Variable("P".to_string()),
                    Term::Constant(Value::String("protobuf".to_string())),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("uses_proto", &[]);
    assert_eq!(results.len(), 2);
}

#[test]
fn incremental_build_marker() {
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
                Value::String("src/main.rs".to_string()),
            ],
        },
        Fact {
            predicate: "file_hash".to_string(),
            args: vec![
                Value::String("src/main.rs".to_string()),
                Value::String("abc123".to_string()),
            ],
        },
        Fact {
            predicate: "cached_hash".to_string(),
            args: vec![
                Value::String("//app:cli".to_string()),
                Value::String("src/main.rs".to_string()),
                Value::String("xyz789".to_string()),
            ],
        },
    ]);

    let rule = Rule {
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
                    Term::Variable("CurrentHash".to_string()),
                ],
            },
            Predicate {
                name: "cached_hash".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("S".to_string()),
                    Term::Variable("CachedHash".to_string()),
                ],
            },
            Predicate {
                name: "!=".to_string(),
                args: vec![
                    Term::Variable("CurrentHash".to_string()),
                    Term::Variable("CachedHash".to_string()),
                ],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("needs_rebuild", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
}

#[test]
fn workspace_target_resolution() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "workspace".to_string(),
            args: vec![
                Value::String("main".to_string()),
                Value::String("//app".to_string()),
            ],
        },
        Fact {
            predicate: "workspace".to_string(),
            args: vec![
                Value::String("main".to_string()),
                Value::String("//lib".to_string()),
            ],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//app:cli".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//lib:core".to_string())],
        },
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//external:tool".to_string())],
        },
    ]);

    let rule = Rule {
        head: Predicate {
            name: "workspace_target".to_string(),
            args: vec![
                Term::Variable("W".to_string()),
                Term::Variable("T".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "workspace".to_string(),
                args: vec![
                    Term::Variable("W".to_string()),
                    Term::Variable("Pkg".to_string()),
                ],
            },
            Predicate {
                name: "target".to_string(),
                args: vec![Term::Variable("T".to_string())],
            },
        ],
    };

    db.compile_rule(rule);
    let results = db.query("workspace_target", &[]);

    let main_targets = results
        .iter()
        .filter(|f| f.args[0] == Value::String("main".to_string()))
        .count();

    assert!(main_targets >= 2);
}
