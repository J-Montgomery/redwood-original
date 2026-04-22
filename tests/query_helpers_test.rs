use redwood::datalog::{parser, Engine};
use redwood::runtime::prelude;

#[test]
fn rdeps_reverses_dependencies() {
    let program = r#"
        deps("//app:server", "//lib:http").
        deps("//app:cli", "//lib:http").
        deps("//lib:http", "//lib:core").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let all_rdeps = db.query("rdeps", &[]);
    let rdeps: Vec<_> = all_rdeps
        .iter()
        .filter(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.first() {
                s == "//lib:http"
            } else {
                false
            }
        })
        .collect();

    assert_eq!(rdeps.len(), 2);

    let targets: Vec<String> = rdeps
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.get(1) {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(targets.contains(&"//app:server".to_string()));
    assert!(targets.contains(&"//app:cli".to_string()));
}

#[test]
fn all_rdeps_computes_transitive_reverse_deps() {
    let program = r#"
        deps("//app:server", "//lib:http").
        deps("//app:cli", "//lib:http").
        deps("//lib:http", "//lib:core").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let all_rdeps_unfiltered = db.query("all_rdeps", &[]);
    let all_rdeps: Vec<_> = all_rdeps_unfiltered
        .iter()
        .filter(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.first() {
                s == "//lib:core"
            } else {
                false
            }
        })
        .collect();

    assert_eq!(
        all_rdeps.len(),
        3,
        "Should have all transitive reverse deps"
    );

    let targets: Vec<String> = all_rdeps
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.get(1) {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(targets.contains(&"//lib:http".to_string()));
    assert!(targets.contains(&"//app:server".to_string()));
    assert!(targets.contains(&"//app:cli".to_string()));
}

#[test]
fn direct_deps_aliases_deps() {
    let program = r#"
        deps("//app:server", "//lib:http").
        deps("//app:server", "//lib:db").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let all_direct = db.query("direct_deps", &[]);
    let direct: Vec<_> = all_direct
        .iter()
        .filter(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.first() {
                s == "//app:server"
            } else {
                false
            }
        })
        .collect();

    let all_orig = db.query("deps", &[]);
    let original: Vec<_> = all_orig
        .iter()
        .filter(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.first() {
                s == "//app:server"
            } else {
                false
            }
        })
        .collect();

    assert_eq!(direct.len(), original.len());
    assert_eq!(direct.len(), 2);
}

#[test]
fn all_deps_includes_transitive() {
    let program = r#"
        deps("//app:server", "//lib:http").
        deps("//lib:http", "//lib:core").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let all_deps_unfiltered = db.query("all_deps", &[]);
    let all_deps: Vec<_> = all_deps_unfiltered
        .iter()
        .filter(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.first() {
                s == "//app:server"
            } else {
                false
            }
        })
        .collect();

    assert_eq!(all_deps.len(), 2);

    let deps: Vec<String> = all_deps
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(s)) = f.args.get(1) {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(deps.contains(&"//lib:http".to_string()));
    assert!(deps.contains(&"//lib:core".to_string()));
}
