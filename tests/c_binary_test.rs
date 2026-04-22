use redwood::build::system_tool::SystemTool;
use redwood::build::{BuildKind, BuildKindRegistry};
use redwood::datalog::{Fact, Value};
use redwood::{
    datalog::Engine,
    runtime::{prelude, ToolchainScanner},
};

#[test]
fn c_binary_resolves_gcc_toolchain() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//test:hello".to_string())],
        },
        Fact {
            predicate: "license_exempt".to_string(),
            args: vec![Value::String("//test:hello".to_string())],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("test.c".to_string()),
            ],
        },
    ]);

    let system_tool = SystemTool::new();
    let target = redwood::datalog::TargetLabel::new("//test", "hello");

    let result = system_tool.plan(&target, &mut db);

    if let Ok(plan) = result {
        assert!(plan.command.contains("gcc"));
        assert!(plan.args.contains(&"-o".to_string()));
        assert_eq!(plan.outputs.len(), 1);
    } else {
        let has_gcc = db.query("tool_available", &[]).iter().any(|f| {
            if let Some(Value::String(tool)) = f.args.first() {
                tool == "gcc"
            } else {
                false
            }
        });

        if !has_gcc {
            eprintln!("Skipping test: gcc not available");
            return;
        }

        panic!("Failed to create build plan: {:?}", result);
    }
}

#[test]
fn cc_binary_resolves_gpp_toolchain() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String("//test:hello".to_string())],
        },
        Fact {
            predicate: "license_exempt".to_string(),
            args: vec![Value::String("//test:hello".to_string())],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("tool".to_string()),
                Value::String("g++".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:hello".to_string()),
                Value::String("test.cpp".to_string()),
            ],
        },
    ]);

    let system_tool = SystemTool::new();
    let target = redwood::datalog::TargetLabel::new("//test", "hello");

    let result = system_tool.plan(&target, &mut db);

    if let Ok(plan) = result {
        assert!(plan.command.contains("g++"));
        assert!(plan.args.contains(&"-o".to_string()));
        assert_eq!(plan.outputs.len(), 1);
    } else {
        let has_gpp = db.query("tool_available", &[]).iter().any(|f| {
            if let Some(Value::String(tool)) = f.args.first() {
                tool == "g++"
            } else {
                false
            }
        });

        if !has_gpp {
            eprintln!("Skipping test: g++ not available");
            return;
        }

        panic!("Failed to create build plan: {:?}", result);
    }
}

#[test]
fn build_registry_includes_system_tool() {
    let mut registry = BuildKindRegistry::new();
    registry.register(Box::new(SystemTool::new()));

    assert!(registry.get("system_tool").is_some());
}
