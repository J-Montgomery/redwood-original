use redwood::build::system_tool::SystemTool;
use redwood::build::BuildKind;
use redwood::datalog::{Fact, Value};
use redwood::{
    datalog::Engine,
    runtime::{prelude, ToolchainScanner},
};

#[test]
fn cargo_binary_helper_derives_kind_and_attrs() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);

    let prelude_rules = prelude::get_prelude_rules();
    eprintln!("Loading {} prelude rules", prelude_rules.len());
    for rule in &prelude_rules {
        eprintln!("Rule: {} :- ...", rule.head.name);
    }
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    // Insert cargo_binary fact
    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//test:app".to_string())],
        },
        Fact {
            predicate: "license_exempt".to_string(),
            args: vec![Value::String("//test:app".to_string())],
        },
        Fact {
            predicate: "cargo_binary".to_string(),
            args: vec![Value::String("//test:app".to_string())],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("src/main.rs".to_string()),
            ],
        },
    ]);

    // Check if kind is derived
    let kind_facts = db.query("kind", &[]);
    eprintln!("kind facts: {:?}", kind_facts);
    let has_kind = kind_facts.iter().any(|f| {
        if let (Some(Value::String(t)), Some(Value::String(k))) = (f.args.first(), f.args.get(1)) {
            t == "//test:app" && k == "system_tool"
        } else {
            false
        }
    });
    assert!(has_kind, "kind should be derived from cargo_binary");

    // Check if tool attr is derived
    let attr_facts = db.query("attr", &[]);
    eprintln!("attr facts: {:?}", attr_facts);
    let has_tool = attr_facts.iter().any(|f| {
        if let (Some(Value::String(t)), Some(Value::String(k)), Some(Value::String(v))) =
            (f.args.first(), f.args.get(1), f.args.get(2))
        {
            t == "//test:app" && k == "tool" && v == "cargo"
        } else {
            false
        }
    });
    assert!(has_tool, "tool attr should be derived from cargo_binary");

    // Try to plan the build
    let system_tool = SystemTool::new();
    let target = redwood::datalog::TargetLabel::new("//test", "app");
    let result = system_tool.plan(&target, &mut db);
    assert!(result.is_ok(), "Plan should succeed: {:?}", result.err());
}
