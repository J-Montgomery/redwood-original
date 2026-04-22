use redwood::datalog::{parser, Engine, Value};
use redwood::runtime::prelude;

#[test]
fn target_name_extracted_from_label() {
    let mut db = Engine::new();

    let build_content = r#"
target("//app:cli").
target("//lib:http").
"#;

    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let results = db.query("target_name", &[]);
    assert_eq!(results.len(), 2);

    let names: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(name) = &f.args[1] {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(names.contains(&"cli".to_string()));
    assert!(names.contains(&"http".to_string()));
}

#[test]
fn target_package_extracted_from_label() {
    let mut db = Engine::new();

    let build_content = r#"
target("//app:cli").
target("//lib:http").
"#;

    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let results = db.query("target_package", &[]);
    assert_eq!(results.len(), 2);

    let packages: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(pkg) = &f.args[1] {
                Some(pkg.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(packages.contains(&"//app".to_string()));
    assert!(packages.contains(&"//lib".to_string()));
}

#[test]
fn outputs_computed_from_cargo_binary() {
    let mut db = Engine::new();

    let build_content = r#"
cargo_binary("//app:server").
"#;

    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let results = db.query("outputs", &[Some("//app:server")]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("//app:server".to_string())
    );
    assert_eq!(
        results[0].args[1],
        Value::String("target/release/server".to_string())
    );
}

#[test]
fn manual_outputs_not_overridden() {
    let mut db = Engine::new();

    let build_content = r#"
cargo_binary("//app:custom").
outputs("//app:custom", "bin/my-custom-path").
"#;

    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let results = db.query("outputs", &[Some("//app:custom")]);
    assert_eq!(results.len(), 2);

    let paths: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(path) = &f.args[1] {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(paths.contains(&"target/release/custom".to_string()));
    assert!(paths.contains(&"bin/my-custom-path".to_string()));
}

#[test]
fn outputs_computed_for_cargo_test() {
    let mut db = Engine::new();

    let build_content = r#"
cargo_test("//tests:integration").
"#;

    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let results = db.query("outputs", &[Some("//tests:integration")]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("//tests:integration".to_string())
    );
    assert_eq!(
        results[0].args[1],
        Value::String("target/debug/integration".to_string())
    );
}
