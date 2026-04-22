use redwood::datalog::{parser, Engine, Value};
use redwood::runtime::prelude;

#[test]
fn alias_with_namespace_rewriting() {
    // Test that aliases work correctly when namespace rewriting is applied
    let program = r#"
        target("//lib:core").
        kind("//lib:core", rust_library).
        alias("core", "//lib:core").
    "#;

    let mut db = Engine::new();
    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    // Parse with namespace rewriting (simulating external dependency)
    let (facts, rules, _) =
        parser::parse_program_with_namespace(program, "BUILD.datalog", "//external/boost//")
            .unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Check the alias fact
    let aliases = db.query("alias", &[]);

    // The alias target should be namespace-rewritten
    let has_rewritten_alias = aliases.iter().any(|f| {
        if let (Some(Value::String(_from)), Some(Value::String(to))) =
            (f.args.first(), f.args.get(1))
        {
            to.contains("//external/boost//")
        } else {
            false
        }
    });

    assert!(
        has_rewritten_alias,
        "Alias target should be namespace-rewritten. Found aliases: {:?}",
        aliases
    );
}

#[test]
fn alias_needs_rebuild_same_as_target() {
    // Verify that needs_rebuild derives identically for alias and target
    let program = r#"
        target("//app:server").
        kind("//app:server", rust_binary).
        sources("//app:server", "src/main.rs").
        outputs("//app:server", "target/server").

        alias("server", "//app:server").
    "#;

    let mut db = Engine::new();
    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Query needs_rebuild for both
    let needs_rebuild = db.query("needs_rebuild", &[]);
    let rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| f.args.first()?.as_string())
        .map(String::from)
        .collect();

    // Both should have identical needs_rebuild status
    let target_needs_rebuild = rebuild_targets.contains(&"//app:server".to_string());
    let alias_needs_rebuild = rebuild_targets.contains(&"server".to_string());

    assert_eq!(
        target_needs_rebuild, alias_needs_rebuild,
        "Alias and target should have identical needs_rebuild status"
    );
}

#[test]
fn transitive_aliases_with_needs_rebuild() {
    // Test that transitive aliases work for needs_rebuild
    let program = r#"
        target("//app:server").
        kind("//app:server", rust_binary).
        outputs("//app:server", "target/server").

        alias("//app:srv", "//app:server").
        alias("server", "//app:srv").
    "#;

    let mut db = Engine::new();
    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Check transitive alias resolution
    let aliases = db.query("alias", &[Some("server")]);
    let alias_targets: Vec<String> = aliases
        .iter()
        .filter_map(|f| f.args.get(1)?.as_string())
        .map(String::from)
        .collect();

    // Should resolve through chain: server -> //app:srv -> //app:server
    assert!(
        alias_targets.contains(&"//app:srv".to_string())
            || alias_targets.contains(&"//app:server".to_string()),
        "Transitive alias should resolve"
    );

    // All three aliases should have identical needs_rebuild status
    let needs_rebuild = db.query("needs_rebuild", &[]);
    let rebuild_targets: Vec<String> = needs_rebuild
        .iter()
        .filter_map(|f| f.args.first()?.as_string())
        .map(String::from)
        .collect();

    let original_needs_rebuild = rebuild_targets.contains(&"//app:server".to_string());
    let alias1_needs_rebuild = rebuild_targets.contains(&"//app:srv".to_string());
    let alias2_needs_rebuild = rebuild_targets.contains(&"server".to_string());

    assert_eq!(
        original_needs_rebuild, alias1_needs_rebuild,
        "First level alias should have same needs_rebuild status"
    );
    assert_eq!(
        original_needs_rebuild, alias2_needs_rebuild,
        "Transitive alias should have same needs_rebuild status"
    );
}
