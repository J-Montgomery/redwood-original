use redwood::datalog::parser;
use redwood::{datalog::Engine, runtime::prelude};

#[test]
fn force_rebuild_single_target() {
    let build_content = r#"
        system_cc("//app:main").
        sources("//app:main", "main.c").

        system_cc("//app:other").
        sources("//app:other", "other.c").

        force_rebuild("//app:main").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let needs_rebuild = db.query("needs_rebuild", &[]);

    let needs_rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        needs_rebuild_targets.contains(&"//app:main".to_string()),
        "//app:main should need rebuild (forced)"
    );
    assert!(
        !needs_rebuild_targets.contains(&"//app:other".to_string()),
        "//app:other should not need rebuild (not forced, no cache)"
    );
}

#[test]
fn force_rebuild_all_targets() {
    let build_content = r#"
        system_cc("//app:a").
        sources("//app:a", "a.c").

        system_cc("//app:b").
        sources("//app:b", "b.c").

        system_cc("//app:c").
        sources("//app:c", "c.c").

        force_rebuild(T) :- target(T).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let needs_rebuild = db.query("needs_rebuild", &[]);

    let needs_rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(needs_rebuild_targets.contains(&"//app:a".to_string()));
    assert!(needs_rebuild_targets.contains(&"//app:b".to_string()));
    assert!(needs_rebuild_targets.contains(&"//app:c".to_string()));
    assert!(
        needs_rebuild_targets.len() >= 3,
        "At least the three build targets should need rebuild"
    );
}

#[test]
fn force_rebuild_transitive() {
    let build_content = r#"
        system_cc("//app:main").
        sources("//app:main", "main.c").
        deps("//app:main", "//lib:util").

        system_cc("//lib:util").
        sources("//lib:util", "util.c").
        deps("//lib:util", "//lib:core").

        system_cc("//lib:core").
        sources("//lib:core", "core.c").

        # Force rebuild main and all its dependencies
        # (transitive_deps is now in the prelude)
        force_rebuild("//app:main").
        force_rebuild(D) :- transitive_deps("//app:main", D).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let needs_rebuild = db.query("needs_rebuild", &[]);

    let needs_rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        needs_rebuild_targets.contains(&"//app:main".to_string()),
        "//app:main should need rebuild"
    );
    assert!(
        needs_rebuild_targets.contains(&"//lib:util".to_string()),
        "//lib:util should need rebuild (transitive)"
    );
    assert!(
        needs_rebuild_targets.contains(&"//lib:core".to_string()),
        "//lib:core should need rebuild (transitive)"
    );
}

#[test]
fn force_rebuild_no_target_no_effect() {
    let build_content = r#"
        system_cc("//app:main").
        sources("//app:main", "main.c").

        force_rebuild("//nonexistent:target").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let needs_rebuild = db.query("needs_rebuild", &[]);

    let needs_rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        !needs_rebuild_targets.contains(&"//nonexistent:target".to_string()),
        "Nonexistent target should not appear in needs_rebuild (target(T) fails)"
    );
}

#[test]
fn dependency_output_change_invalidates_cache() {
    let build_content = r#"
        system_cc("//app:main").
        sources("//app:main", "main.c").
        deps("//app:main", "//lib:util").

        system_cc("//lib:util").
        sources("//lib:util", "util.c").

        outputs("//lib:util", "target/gcc/util").
        file_exists("target/gcc/util").
        file_hash("target/gcc/util", "new_hash_456").

        cached_hash("//app:main", "main.c", "main_hash_unchanged").
        cached_hash("//app:main", "target/gcc/util", "old_hash_123").

        file_hash("main.c", "main_hash_unchanged").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let needs_rebuild = db.query("needs_rebuild", &[]);
    let needs_rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        needs_rebuild_targets.contains(&"//app:main".to_string()),
        "//app:main should need rebuild when dependency output changes"
    );
}

#[test]
fn dependency_output_unchanged_keeps_cache() {
    let build_content = r#"
        system_cc("//app:main").
        sources("//app:main", "main.c").
        deps("//app:main", "//lib:util").
        outputs("//app:main", "target/gcc/main").
        file_exists("target/gcc/main").

        system_cc("//lib:util").
        sources("//lib:util", "util.c").
        outputs("//lib:util", "target/gcc/util").
        file_exists("target/gcc/util").

        file_hash("main.c", "main_hash").
        file_hash("util.c", "util_hash").
        file_hash("target/gcc/util", "dep_output_hash").

        cached_hash("//app:main", "main.c", "main_hash").
        cached_hash("//app:main", "target/gcc/util", "dep_output_hash").

        cached_hash("//lib:util", "util.c", "util_hash").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let needs_rebuild = db.query("needs_rebuild", &[]);
    let needs_rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(!needs_rebuild_targets.contains(&"//app:main".to_string()),
        "//app:main should not need rebuild when all inputs including dependency outputs are unchanged");
}

#[test]
fn test_negation_invalidation_on_base_change() {
    use redwood::datalog::{parser, Fact, Value};

    // Helper function to create facts
    fn fact(pred: &str, args: Vec<&str>) -> Fact {
        Fact {
            predicate: pred.to_string(),
            args: args.iter().map(|s| Value::String(s.to_string())).collect(),
        }
    }

    let mut db = Engine::new();

    // Insert base facts
    db.insert_facts(vec![fact("base", vec!["a"]), fact("base", vec!["b"])]);

    // Compile rule with negation: derived(X) :- base(X), not(excluded(X)).
    let program = "derived(X) :- base(X), not(excluded(X)).";
    let (_facts, rules) = parser::parse_program(program).expect("Failed to parse program");
    for rule in rules {
        db.compile_rule(rule);
    }

    // Query - should return a, b
    let result_before = db.query("derived", &[]);
    let derived_before: Vec<String> = result_before
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(s)) = f.args.first() {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(
        derived_before.len(),
        2,
        "Should have 2 derived facts initially"
    );
    assert!(
        derived_before.contains(&"a".to_string()),
        "derived(a) should exist initially"
    );
    assert!(
        derived_before.contains(&"b".to_string()),
        "derived(b) should exist initially"
    );

    // Now exclude "a"
    db.insert_facts(vec![fact("excluded", vec!["a"])]);

    // Query again - should only return b
    let result_after = db.query("derived", &[]);
    let derived_after: Vec<String> = result_after
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(s)) = f.args.first() {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(
        derived_after.len(),
        1,
        "Should have 1 derived fact after exclusion"
    );
    assert!(
        !derived_after.contains(&"a".to_string()),
        "derived(a) should not exist after excluding a"
    );
    assert!(
        derived_after.contains(&"b".to_string()),
        "derived(b) should still exist"
    );
}

#[test]
fn test_tc_cache_invalidation_on_edge_insert() {
    use redwood::datalog::{Fact, Value};

    let mut db = Engine::new();

    // Initial graph: A -> B -> C
    let initial_program = r#"
        edge("A", "B").
        edge("B", "C").

        path(X, Y) :- edge(X, Y).
        path(X, Z) :- edge(X, Y), path(Y, Z).
    "#;

    let (facts, rules) = parser::parse_program(initial_program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Query transitive closure from A - this should populate the TC cache
    let results_before = db.query("path", &[Some("A"), None]);
    let targets_before: Vec<String> = results_before
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(target)) = f.args.get(1) {
                Some(target.clone())
            } else {
                None
            }
        })
        .collect();

    // Before insert: A can reach B and C
    assert!(
        targets_before.contains(&"B".to_string()),
        "A should reach B initially"
    );
    assert!(
        targets_before.contains(&"C".to_string()),
        "A should reach C initially"
    );
    assert!(
        !targets_before.contains(&"D".to_string()),
        "A should not reach D initially"
    );

    // Now insert a new edge: C -> D
    let new_edge = Fact {
        predicate: "edge".to_string(),
        args: vec![
            Value::String("C".to_string()),
            Value::String("D".to_string()),
        ],
    };
    db.insert_facts(vec![new_edge]);

    // Query again - the cache should be invalidated and D should now be reachable from A
    let results_after = db.query("path", &[Some("A"), None]);
    let targets_after: Vec<String> = results_after
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(target)) = f.args.get(1) {
                Some(target.clone())
            } else {
                None
            }
        })
        .collect();

    // After insert: A should reach B, C, AND D (via B -> C -> D)
    assert!(
        targets_after.contains(&"B".to_string()),
        "A should still reach B after insert"
    );
    assert!(
        targets_after.contains(&"C".to_string()),
        "A should still reach C after insert"
    );
    assert!(
        targets_after.contains(&"D".to_string()),
        "A should now reach D after inserting C->D edge (TC cache must be invalidated)"
    );
}

#[test]
fn test_tc_cache_invalidation_on_edge_retract() {
    use redwood::datalog::{Fact, Value};

    let mut db = Engine::new();

    // Initial graph: A -> B -> C -> D
    let initial_program = r#"
        edge("A", "B").
        edge("B", "C").
        edge("C", "D").

        path(X, Y) :- edge(X, Y).
        path(X, Z) :- edge(X, Y), path(Y, Z).
    "#;

    let (facts, rules) = parser::parse_program(initial_program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Query transitive closure from A - this should populate the TC cache
    let results_before = db.query("path", &[Some("A"), None]);
    let targets_before: Vec<String> = results_before
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(target)) = f.args.get(1) {
                Some(target.clone())
            } else {
                None
            }
        })
        .collect();

    // Before retract: A can reach B, C, and D
    assert!(
        targets_before.contains(&"B".to_string()),
        "A should reach B initially"
    );
    assert!(
        targets_before.contains(&"C".to_string()),
        "A should reach C initially"
    );
    assert!(
        targets_before.contains(&"D".to_string()),
        "A should reach D initially"
    );

    // Now retract the edge: B -> C (breaking the path from A to D)
    let retract_edge = Fact {
        predicate: "edge".to_string(),
        args: vec![
            Value::String("B".to_string()),
            Value::String("C".to_string()),
        ],
    };
    db.retract_facts(vec![retract_edge]);

    // Query again - the cache should be invalidated
    let results_after = db.query("path", &[Some("A"), None]);
    let targets_after: Vec<String> = results_after
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(target)) = f.args.get(1) {
                Some(target.clone())
            } else {
                None
            }
        })
        .collect();

    // After retract: A should only reach B (not C or D anymore)
    assert!(
        targets_after.contains(&"B".to_string()),
        "A should still reach B after retract"
    );
    assert!(
        !targets_after.contains(&"C".to_string()),
        "A should not reach C after retracting B->C edge (TC cache must be invalidated)"
    );
    assert!(
        !targets_after.contains(&"D".to_string()),
        "A should not reach D after retracting B->C edge (TC cache must be invalidated)"
    );
}

#[test]
fn test_tc_cache_invalidation_multiple_sources() {
    use redwood::datalog::{Fact, Value};

    let mut db = Engine::new();

    // Initial graph with two separate paths: A -> B -> C and X -> Y -> Z
    let initial_program = r#"
        edge("A", "B").
        edge("B", "C").
        edge("X", "Y").
        edge("Y", "Z").

        path(X, Y) :- edge(X, Y).
        path(X, Z) :- edge(X, Y), path(Y, Z).
    "#;

    let (facts, rules) = parser::parse_program(initial_program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Query from both A and X to populate cache
    let results_a_before = db.query("path", &[Some("A"), None]);
    let results_x_before = db.query("path", &[Some("X"), None]);

    let targets_a_before: Vec<String> = results_a_before
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(t)) = f.args.get(1) {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    let targets_x_before: Vec<String> = results_x_before
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(t)) = f.args.get(1) {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(
        targets_a_before.contains(&"C".to_string()),
        "A should reach C initially"
    );
    assert!(
        targets_x_before.contains(&"Z".to_string()),
        "X should reach Z initially"
    );

    // Insert edge that affects only the A path: C -> D
    let new_edge = Fact {
        predicate: "edge".to_string(),
        args: vec![
            Value::String("C".to_string()),
            Value::String("D".to_string()),
        ],
    };
    db.insert_facts(vec![new_edge]);

    // Query both paths again
    let results_a_after = db.query("path", &[Some("A"), None]);
    let results_x_after = db.query("path", &[Some("X"), None]);

    let targets_a_after: Vec<String> = results_a_after
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(t)) = f.args.get(1) {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    let targets_x_after: Vec<String> = results_x_after
        .iter()
        .filter_map(|f| {
            if let Some(Value::String(t)) = f.args.get(1) {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    // The A path should be updated to include D
    assert!(
        targets_a_after.contains(&"D".to_string()),
        "A should reach D after inserting C->D (cache invalidated for affected sources)"
    );

    // The X path should remain unchanged and still reach Z
    assert!(
        targets_x_after.contains(&"Z".to_string()),
        "X should still reach Z (cache for unaffected source should be preserved)"
    );
}
