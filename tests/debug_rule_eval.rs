use redwood::datalog::{parser, Value};
use redwood::{datalog::Engine, runtime::prelude};

#[test]
fn debug_requires_tool_step_by_step() {
    let build_content = r#"
        system_cc("//example:hello").
        sources("//example:hello", "hello.c").
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

    // Check requires_tool
    let requires_facts = db.query("requires_tool", &[]);

    assert!(
        !requires_facts.is_empty(),
        "Should have requires_tool facts"
    );
    assert_eq!(requires_facts.len(), 1);
    assert_eq!(
        requires_facts[0].args[0],
        Value::String("//example:hello".to_string())
    );
    assert_eq!(requires_facts[0].args[1], Value::String("gcc".to_string()));
}
