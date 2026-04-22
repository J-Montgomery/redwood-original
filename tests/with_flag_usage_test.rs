use redwood::datalog::{parser, Engine};
use redwood::runtime::prelude;

#[test]
fn with_flag_injects_facts() {
    // redwood build --with 'force_rebuild("//app:test")' //app:test

    let mut db = Engine::new();

    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let with_datalog = r#"force_rebuild("//app:test")"#;

    let with_normalized = format!("{}.", with_datalog.trim());
    let (facts, rules) = parser::parse_program(&with_normalized).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("force_rebuild", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0].as_string().unwrap(), "//app:test");
}

#[test]
fn with_flag_injects_rules() {
    // redwood build --with 'my_rule(X) :- target(X).' //app:test

    let mut db = Engine::new();

    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    db.insert_facts(vec![redwood::datalog::Fact::new(
        "target",
        vec![redwood::datalog::Value::String("//app:foo".to_string())],
    )]);

    let with_datalog = r#"test_with_flag_rule(X) :- target(X)."#;
    let (facts, rules) = parser::parse_program(with_datalog).unwrap();

    assert_eq!(facts.len(), 0);
    assert_eq!(rules.len(), 1);

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("test_with_flag_rule", &[Some("//app:foo")]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0].as_string().unwrap(), "//app:foo");
}

#[test]
fn with_flag_multiple_injections() {
    // redwood build --with 'fact1("a")' --with 'fact2("b")' //target

    let mut db = Engine::new();

    let with_args = vec![
        r#"force_rebuild("//app:one")"#,
        r#"force_rebuild("//app:two")"#,
        r#"license("//app:one", "MIT")"#,
    ];

    for with_datalog in with_args {
        let with_normalized = format!("{}.", with_datalog.trim());
        let (facts, rules) = parser::parse_program(&with_normalized).unwrap();
        db.insert_facts(facts);
        for rule in rules {
            db.compile_rule(rule);
        }
    }

    let rebuild_results = db.query("force_rebuild", &[]);
    assert_eq!(rebuild_results.len(), 2);

    let license_results = db.query("license", &[]);
    assert_eq!(license_results.len(), 1);
}
