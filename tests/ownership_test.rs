use redwood::datalog::{parser, Engine, Value};
use redwood::runtime::prelude;

#[test]
fn deepest_owner_wins() {
    let program = r#"
        target("//src/datalog:parser").
        target_path("//src/datalog:parser", "src/datalog").

        owner("src", "foo").
        owner("src/datalog", "bar").
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

    let potential = db.query("potential_reviewers", &[Some("//src/datalog:parser")]);
    assert_eq!(potential.len(), 2);

    // target_owner should only return deepest match
    let owners = db.query("target_owner", &[Some("//src/datalog:parser")]);
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].args[1], Value::String("bar".to_string())); // bar, not foo
}

#[test]
fn prefix_matching() {
    let program = r#"
        target("//app/api:server").
        target_path("//app/api:server", "app/api").

        owner("app", "alice").
        owner("app/api", "bob").
        owner("lib", "charlie").
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

    let potential = db.query("potential_reviewers", &[Some("//app/api:server")]);
    assert_eq!(potential.len(), 2);

    let owners = db.query("target_owner", &[Some("//app/api:server")]);
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].args[1], Value::String("bob".to_string()));
}

#[test]
fn direct_vs_impact_owners() {
    let program = r#"
        target("//lib:auth").
        target("//app:server").

        target_path("//lib:auth", "lib").
        target_path("//app:server", "app").

        owner("lib", "security").
        owner("app", "backend").

        deps("//app:server", "//lib:auth").
        directly_changed("//lib:auth").
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

    let direct = db.query("direct_owner", &[]);
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].args[0], Value::String("//lib:auth".to_string()));
    assert_eq!(direct[0].args[1], Value::String("security".to_string()));

    let impact = db.query("impact_owner", &[]);
    assert_eq!(impact.len(), 1);
    assert_eq!(impact[0].args[0], Value::String("//app:server".to_string()));
    assert_eq!(impact[0].args[1], Value::String("backend".to_string()));

    let unique_direct = db.query("unique_direct_owner", &[]);
    assert_eq!(unique_direct.len(), 1);
    assert_eq!(
        unique_direct[0].args[0],
        Value::String("security".to_string())
    );

    let unique_impact = db.query("unique_impact_owner", &[]);
    assert_eq!(unique_impact.len(), 1);
    assert_eq!(
        unique_impact[0].args[0],
        Value::String("backend".to_string())
    );


    let unique_all = db.query("unique_all_owners", &[]);
    assert_eq!(unique_all.len(), 2);
}

#[test]
fn owner_deduplication() {
    let program = r#"
        target("//src/datalog:parser").
        target("//src/datalog:engine").
        target("//src/datalog:builtins").

        target_path("//src/datalog:parser", "src/datalog").
        target_path("//src/datalog:engine", "src/datalog").
        target_path("//src/datalog:builtins", "src/datalog").

        owner("src/datalog", "expert").

        directly_changed("//src/datalog:parser").
        directly_changed("//src/datalog:engine").
        directly_changed("//src/datalog:builtins").
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

    let direct = db.query("direct_owner", &[]);
    assert_eq!(direct.len(), 3);

    // Check deduplication
    let unique = db.query("unique_direct_owner", &[]);
    assert_eq!(unique.len(), 1);
    assert_eq!(unique[0].args[0], Value::String("expert".to_string()));
}

#[test]
fn no_match_returns_empty() {
    let program = r#"
        target("//unowned:target").
        target_path("//unowned:target", "unowned").

        owner("src", "foo").
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

    let owners = db.query("target_owner", &[Some("//unowned:target")]);
    assert_eq!(owners.len(), 0);
}

#[test]
fn multiple_owners_same_prefix() {
    let program = r#"
        target("//app:server").
        target_path("//app:server", "app").

        owner("app", "alice").
        owner("app", "bob").
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

    let owners = db.query("target_owner", &[Some("//app:server")]);
    assert_eq!(owners.len(), 2);

    let owner_names: Vec<String> = owners
        .iter()
        .filter_map(|f| f.args.get(1)?.as_string())
        .map(String::from)
        .collect();

    assert!(owner_names.contains(&"alice".to_string()));
    assert!(owner_names.contains(&"bob".to_string()));
}

#[test]
fn transitive_impact_owners() {
    let program = r#"
        target("//lib:util").
        target("//lib:auth").
        target("//app:server").
        target("//app:client").

        target_path("//lib:util", "lib").
        target_path("//lib:auth", "lib").
        target_path("//app:server", "app").
        target_path("//app:client", "app").

        owner("lib", "lib-team").
        owner("app", "app-team").

        # Dependencies: client -> server -> auth -> util
        deps("//app:client", "//app:server").
        deps("//app:server", "//lib:auth").
        deps("//lib:auth", "//lib:util").

        # lib:util changed directly
        directly_changed("//lib:util").
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

    let direct = db.query("direct_owner", &[]);
    assert_eq!(direct.len(), 1);

    let impact = db.query("impact_owner", &[]);
    assert_eq!(impact.len(), 1); // only lib:auth, owned by lib-team
    let transitive = db.query("transitive_impact_owner", &[]);
    assert_eq!(transitive.len(), 3); // auth, server, client

    let unique_all = db.query("unique_all_owners", &[]);
    assert_eq!(unique_all.len(), 1); // only lib-team (both direct and immediate impact)

    let unique_transitive = db.query("unique_transitive_owner", &[]);
    assert_eq!(unique_transitive.len(), 2); // lib-team and app-team
}

#[test]
fn query_what_owner_owns() {
    let program = r#"
        target("//src/datalog:parser").
        target("//src/datalog:engine").
        target("//app:server").

        target_path("//src/datalog:parser", "src/datalog").
        target_path("//src/datalog:engine", "src/datalog").
        target_path("//app:server", "app").

        owner("src/datalog", "expert").
        owner("app", "alice").
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

    let expert_targets = db.query("target_owner", &[None, Some("expert")]);
    assert_eq!(expert_targets.len(), 2);

    let alice_targets = db.query("target_owner", &[None, Some("alice")]);
    assert_eq!(alice_targets.len(), 1);
}
