use redwood::build::system_tool::SystemTool;
use redwood::build::BuildKind;
use redwood::datalog::{Fact, Value};
use redwood::{
    datalog::Engine,
    runtime::{prelude, ToolchainScanner},
};
use std::path::PathBuf;

fn get_fake_tool_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/debug/fake_tool")
        .to_string_lossy()
        .to_string()
}

#[test]
fn target_specific_attrs_become_flags() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let fake_tool = get_fake_tool_path();

    db.insert_facts(vec![
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String(fake_tool.clone()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("-O3".to_string()),
                Value::String("".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("-Wall".to_string()),
                Value::String("".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("main.c".to_string()),
            ],
        },
    ]);

    let system_tool = SystemTool::new();
    let target = redwood::datalog::TargetLabel::new("//test", "app");
    let result = system_tool.plan(&target, &mut db);

    if let Err(e) = &result {
        eprintln!("Plan error: {}", e);
    }
    assert!(result.is_ok(), "Plan should succeed");
    let plan = result.unwrap();

    assert!(
        plan.args.contains(&"-O3".to_string()),
        "Should include -O3 flag"
    );
    assert!(
        plan.args.contains(&"-Wall".to_string()),
        "Should include -Wall flag"
    );
}

#[test]
fn tool_specific_attrs_apply_to_all_targets() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let fake_tool = get_fake_tool_path();

    db.insert_facts(vec![
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String(fake_tool.clone()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String("-fPIC".to_string()),
                Value::String("".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("main.c".to_string()),
            ],
        },
    ]);

    let system_tool = SystemTool::new();
    let target = redwood::datalog::TargetLabel::new("//test", "app");
    let result = system_tool.plan(&target, &mut db);

    if let Err(e) = &result {
        eprintln!("Plan error: {}", e);
    }
    assert!(result.is_ok(), "Plan should succeed");
    let plan = result.unwrap();

    assert!(
        plan.args.contains(&"-fPIC".to_string()),
        "Should include tool-level -fPIC flag"
    );
}

#[test]
fn specific_target_tool_combination_attrs() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let fake_tool = get_fake_tool_path();

    db.insert_facts(vec![
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String(fake_tool.clone()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app:gcc".to_string()),
                Value::String("-march".to_string()),
                Value::String("-march=native".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("main.c".to_string()),
            ],
        },
    ]);

    let system_tool = SystemTool::new();
    let target = redwood::datalog::TargetLabel::new("//test", "app");
    let result = system_tool.plan(&target, &mut db);

    assert!(result.is_ok(), "Plan should succeed with target:tool combo");
    let plan = result.unwrap();

    assert!(
        plan.args.contains(&"-march".to_string()),
        "Should include -march flag"
    );
    assert!(
        plan.args.contains(&"-march=native".to_string()),
        "Should include value"
    );
}

#[test]
fn c_and_cc_different_tools() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let fake_tool = get_fake_tool_path();

    db.insert_facts(vec![
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String(fake_tool.clone()),
            ],
        },
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("g++".to_string()),
                Value::String(fake_tool.clone()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:c_app".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:c_app".to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:c_app".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:c_app".to_string()),
                Value::String("main.c".to_string()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:cc_app".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:cc_app".to_string()),
                Value::String("tool".to_string()),
                Value::String("g++".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:cc_app".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:cc_app".to_string()),
                Value::String("main.cpp".to_string()),
            ],
        },
    ]);

    let system_tool = SystemTool::new();

    let c_target = redwood::datalog::TargetLabel::new("//test", "c_app");
    let c_result = system_tool.plan(&c_target, &mut db);
    assert!(c_result.is_ok());

    let cc_target = redwood::datalog::TargetLabel::new("//test", "cc_app");
    let cc_result = system_tool.plan(&cc_target, &mut db);
    assert!(cc_result.is_ok());
}

#[test]
fn attrs_without_values() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let fake_tool = get_fake_tool_path();

    db.insert_facts(vec![
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String(fake_tool.clone()),
            ],
        },
        Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("system_tool".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("-c".to_string()),
                Value::String("".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("main.c".to_string()),
            ],
        },
    ]);

    let system_tool = SystemTool::new();
    let target = redwood::datalog::TargetLabel::new("//test", "app");
    let result = system_tool.plan(&target, &mut db);

    if let Err(e) = &result {
        eprintln!("Plan error: {}", e);
    }
    assert!(result.is_ok(), "Plan should succeed");
    let plan = result.unwrap();

    assert!(
        plan.args.contains(&"-c".to_string()),
        "Should include -c flag without value"
    );
}
