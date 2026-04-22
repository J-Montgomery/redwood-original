use redwood::datalog::Engine;
use redwood::datalog::{parser, Value};

#[test]
fn test_requires_tool_minimal() {
    let program = r#"
        # Base fact
        system_cc("//test:foo").

        # Helper rules (simplified from prelude)
        target(T) :- system_cc(T).
        kind(T, system_tool) :- system_cc(T).
        attr(T, "tool", "gcc") :- system_cc(T).

        # The requires_tool rule
        requires_tool(T, Tool) :- kind(T, system_tool), attr(T, "tool", Tool).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    eprintln!("Querying system_cc...");
    let base = db.query("system_cc", &[]);
    eprintln!("system_cc: {} facts", base.len());

    eprintln!("\nQuerying kind...");
    let kind_results = db.query("kind", &[]);
    eprintln!("kind: {} facts", kind_results.len());
    for f in &kind_results {
        eprintln!("  {:?}", f);
    }

    eprintln!("\nQuerying attr...");
    let attr_results = db.query("attr", &[]);
    eprintln!("attr: {} facts", attr_results.len());
    for f in &attr_results {
        eprintln!("  {:?}", f);
    }

    eprintln!("\nQuerying requires_tool...");
    let requires_results = db.query("requires_tool", &[]);
    eprintln!("requires_tool: {} facts", requires_results.len());
    for f in &requires_results {
        eprintln!("  {:?}", f);
    }

    assert!(
        !requires_results.is_empty(),
        "Should derive requires_tool facts"
    );
    assert_eq!(requires_results.len(), 1);
    assert_eq!(
        requires_results[0].args[0],
        Value::String("//test:foo".to_string())
    );
    assert_eq!(
        requires_results[0].args[1],
        Value::String("gcc".to_string())
    );
}
