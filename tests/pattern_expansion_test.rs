use redwood::datalog::{parser, Engine};
use redwood::runtime::prelude;

#[test]
fn pattern_matching_with_prefix() {
    let mut db = Engine::new();

    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let datalog = r#"
        target("//app:server").
        target("//app:client").
        target("//app/internal:lib").
        target("//lib:utils").
    "#;
    let (facts, rules) = parser::parse_program(datalog).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("matches_pattern", &[None, Some("//app")]);
    assert_eq!(results.len(), 3);

    let matched_targets: Vec<String> = results
        .iter()
        .map(|f| f.args[0].as_string().unwrap().to_string())
        .collect();

    assert!(matched_targets.contains(&"//app:server".to_string()));
    assert!(matched_targets.contains(&"//app:client".to_string()));
    assert!(matched_targets.contains(&"//app/internal:lib".to_string()));
    assert!(!matched_targets.contains(&"//lib:utils".to_string()));
}

#[test]
fn pattern_matching_exact() {
    let mut db = Engine::new();

    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let datalog = r#"
        target("//app:server").
    "#;
    let (facts, rules) = parser::parse_program(datalog).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("matches_pattern", &[None, Some("//app:server")]);
    assert!(!results.is_empty());
    assert!(results
        .iter()
        .any(|f| f.args[0].as_string().unwrap() == "//app:server"));
}

#[test]
fn pattern_matching_with_namespace() {
    let mut db = Engine::new();

    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let datalog = r#"
        target("//external/boost//ranges:lib").
        target("//external/boost//algorithm:sort").
        target("//app:server").
    "#;
    let (facts, rules) = parser::parse_program(datalog).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("matches_pattern", &[None, Some("//external/boost//")]);
    assert_eq!(results.len(), 2);

    let matched_targets: Vec<String> = results
        .iter()
        .map(|f| f.args[0].as_string().unwrap().to_string())
        .collect();

    assert!(matched_targets.contains(&"//external/boost//ranges:lib".to_string()));
    assert!(matched_targets.contains(&"//external/boost//algorithm:sort".to_string()));
    assert!(!matched_targets.contains(&"//app:server".to_string()));
}
