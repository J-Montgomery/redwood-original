use redwood::datalog::{Engine, Fact, Value};
use redwood::runtime::prelude;

#[test]
fn test_diff_rules_without_git() {
    // Test diff logic without git dependency
    let mut db = Engine::new();

    db.insert_facts(vec![
        fact("baseline_target", vec!["//app:old"]),
        fact("head_target", vec!["//app:old"]),
        fact("head_target", vec!["//app:new"]),
    ]);

    let (_, rules, _) = prelude::get_prelude_with_locations();
    for rule in rules {
        db.compile_rule(rule);
    }

    let added = db.query("target_added", &[]);
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].args[0], Value::String("//app:new".to_string()));
}

#[test]
fn test_deps_added() {
    let mut db = Engine::new();

    // Baseline has old dep
    db.insert_facts(vec![
        fact("baseline_target", vec!["//app:server"]),
        fact("baseline_deps", vec!["//app:server", "//lib:http"]),
    ]);

    // Head has old dep plus new dep
    db.insert_facts(vec![
        fact("head_target", vec!["//app:server"]),
        fact("head_deps", vec!["//app:server", "//lib:http"]),
        fact("head_deps", vec!["//app:server", "//lib:auth"]),
    ]);

    let (_, rules, _) = prelude::get_prelude_with_locations();
    for rule in rules {
        db.compile_rule(rule);
    }

    let added = db.query("deps_added", &[]);
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].args[0], Value::String("//app:server".to_string()));
    assert_eq!(added[0].args[1], Value::String("//lib:auth".to_string()));
}

#[test]
fn test_sources_added() {
    let mut db = Engine::new();

    // Base has one source
    db.insert_facts(vec![
        fact("baseline_target", vec!["//app:cli"]),
        fact("baseline_sources", vec!["//app:cli", "main.rs"]),
    ]);

    // Head has old source plus new source
    db.insert_facts(vec![
        fact("head_target", vec!["//app:cli"]),
        fact("head_sources", vec!["//app:cli", "main.rs"]),
        fact("head_sources", vec!["//app:cli", "lib.rs"]),
    ]);

    let (_, rules, _) = prelude::get_prelude_with_locations();
    for rule in rules {
        db.compile_rule(rule);
    }

    let added = db.query("sources_added", &[]);
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].args[0], Value::String("//app:cli".to_string()));
    assert_eq!(added[0].args[1], Value::String("lib.rs".to_string()));
}

#[test]
fn test_directly_changed() {
    let mut db = Engine::new();

    db.insert_facts(vec![fact("head_target", vec!["//app:new"])]);

    db.insert_facts(vec![
        fact("baseline_target", vec!["//app:old"]),
        fact("head_target", vec!["//app:old"]),
        fact("head_sources", vec!["//app:old", "new_file.rs"]),
    ]);

    let (_, rules, _) = prelude::get_prelude_with_locations();
    for rule in rules {
        db.compile_rule(rule);
    }

    let changed = db.query("directly_changed", &[]);
    assert!(changed.len() >= 2);
}

#[test]
fn test_affected_by_changes() {
    let mut db = Engine::new();

    // lib changed, app depends on lib
    db.insert_facts(vec![
        fact("baseline_target", vec!["//lib:core"]),
        fact("head_target", vec!["//lib:core"]),
        fact("head_sources", vec!["//lib:core", "new_file.rs"]),
        fact("head_target", vec!["//app:server"]),
        fact("head_deps", vec!["//app:server", "//lib:core"]),
    ]);

    let (_, rules, _) = prelude::get_prelude_with_locations();
    for rule in rules {
        db.compile_rule(rule);
    }

    let affected = db.query("affected_by_changes", &[]);

    let targets: Vec<String> = affected
        .iter()
        .map(|f| {
            if let Value::String(s) = &f.args[0] {
                s.clone()
            } else {
                String::new()
            }
        })
        .collect();

    assert!(targets.contains(&"//lib:core".to_string()));
    assert!(targets.contains(&"//app:server".to_string()));
}

fn fact(pred: &str, args: Vec<&str>) -> Fact {
    Fact {
        predicate: pred.to_string(),
        args: args.iter().map(|s| Value::String(s.to_string())).collect(),
    }
}
