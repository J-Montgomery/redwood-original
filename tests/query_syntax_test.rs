use redwood::datalog::{parser, Engine, Value};
use redwood::runtime::prelude;

#[test]
fn query_syntax_with_trailing_dot() {
    let mut db = Engine::new();

    let query = r#"force_rebuild("//app:test")."#;
    let (facts, rules) = parser::parse_program(query).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("force_rebuild", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:test".to_string()));
}

#[test]
fn query_syntax_without_trailing_dot() {
    let mut db = Engine::new();

    let query = r#"force_rebuild("//app:test")"#;
    let query_with_dot = format!("{}.", query.trim());
    let (facts, rules) = parser::parse_program(&query_with_dot).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("force_rebuild", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:test".to_string()));
}

#[test]
fn multi_predicate_query_creates_rule() {
    let mut db = Engine::new();

    let (prelude_facts, prelude_rules, _) = prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    db.insert_facts(vec![
        redwood::datalog::Fact::new("target", vec![Value::String("//app:foo".to_string())]),
        redwood::datalog::Fact::new(
            "kind",
            vec![
                Value::String("//app:foo".to_string()),
                Value::String("system_tool".to_string()),
            ],
        ),
    ]);

    // Multi-predicate query should create an anonymous rule
    let query = r#"target(X), kind(X, "system_tool")."#;
    let body = parser::parse_query_body(query).unwrap();

    let mut variables = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pred in &body {
        for arg in &pred.args {
            if let redwood::datalog::Term::Variable(v) = arg {
                if !v.starts_with("_anon_") && !seen.contains(v) {
                    variables.push(v.clone());
                    seen.insert(v.clone());
                }
            }
        }
    }

    let head_args: Vec<redwood::datalog::Term> = variables
        .into_iter()
        .map(redwood::datalog::Term::Variable)
        .collect();

    let head = redwood::datalog::Predicate {
        name: "query".to_string(),
        args: head_args,
    };

    let rule = redwood::datalog::Rule::new(head, body);
    db.compile_rule(rule);

    let results = db.query("query", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:foo".to_string()));
}

#[test]
fn rule_syntax_preserved() {
    let query = r#"my_rule(X) :- target(X), kind(X, "system_tool")."#;
    let (facts, rules) = parser::parse_program(query).unwrap();

    assert_eq!(facts.len(), 0);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].head.name, "my_rule");
    assert_eq!(rules[0].body.len(), 2);
}
