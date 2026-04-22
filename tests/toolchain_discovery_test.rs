use redwood::datalog::{Fact, Value};
use redwood::{
    datalog::Engine,
    runtime::{prelude, ToolchainScanner},
};

#[test]
fn toolchain_scanner_finds_tools() {
    let scanner = ToolchainScanner::new();
    let facts = scanner.scan();

    assert!(!facts.is_empty(), "Should find at least one tool");

    for fact in &facts {
        assert_eq!(fact.predicate, "tool_available");
        assert_eq!(fact.args.len(), 3);

        if let (Value::String(tool), Value::String(path), _) =
            (&fact.args[0], &fact.args[1], &fact.args[2])
        {
            assert!(!tool.is_empty());
            assert!(!path.is_empty());
        } else {
            panic!("Invalid tool_available fact format");
        }
    }
}

#[test]
fn toolchain_rules_resolve_paths() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    db.insert_facts(vec![Fact {
        predicate: "requires_tool".to_string(),
        args: vec![
            Value::String("//test:target".to_string()),
            Value::String("cargo".to_string()),
        ],
    }]);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let toolchain_facts = db.query("toolchain", &[]);

    let cargo_toolchain = toolchain_facts.iter().find(|f| {
        if f.args.len() >= 3 {
            if let (Value::String(target), Value::String(tool), Value::String(_path)) =
                (&f.args[0], &f.args[1], &f.args[2])
            {
                return target == "//test:target" && tool == "cargo";
            }
        }
        false
    });

    assert!(
        cargo_toolchain.is_some(),
        "Should resolve cargo toolchain for test target"
    );
}

#[test]
fn build_ready_predicate() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    db.insert_facts(vec![
        Fact {
            predicate: "requires_tool".to_string(),
            args: vec![
                Value::String("//test:target".to_string()),
                Value::String("cargo".to_string()),
            ],
        },
        Fact {
            predicate: "requires_tool".to_string(),
            args: vec![
                Value::String("//test:target".to_string()),
                Value::String("rustc".to_string()),
            ],
        },
    ]);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let build_ready_facts = db.query("build_ready", &[]);

    let target_ready = build_ready_facts.iter().any(|f| {
        if let Some(Value::String(target)) = f.args.first() {
            target == "//test:target"
        } else {
            false
        }
    });

    assert!(
        target_ready,
        "Target should be build_ready when all required tools are available"
    );
}
