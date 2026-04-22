use redwood::datalog::{Fact, Value};
use redwood::{
    datalog::Engine,
    runtime::{prelude, ToolchainScanner},
};

#[test]
fn toolchain_override_takes_precedence() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    db.insert_facts(vec![
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String("/custom/path/gcc".to_string()),
            ],
        },
        Fact {
            predicate: "requires_tool".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
    ]);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let toolchain_facts = db.query("toolchain", &[]);

    let gcc_paths: Vec<String> = toolchain_facts
        .iter()
        .filter_map(|fact| {
            if let [Value::String(target), Value::String(tool), Value::String(path)] =
                &fact.args[..]
            {
                if target == "//test:app" && tool == "gcc" {
                    return Some(path.clone());
                }
            }
            None
        })
        .collect();

    assert_eq!(
        gcc_paths.len(),
        1,
        "Should have exactly one gcc toolchain fact"
    );
    assert_eq!(
        gcc_paths[0], "/custom/path/gcc",
        "Override should take precedence over auto-discovered tool"
    );
}

#[test]
fn toolchain_without_override_uses_discovered() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();

    let has_gcc = toolchain_facts.iter().any(|fact| {
        if let Some(Value::String(tool)) = fact.args.first() {
            tool == "gcc"
        } else {
            false
        }
    });

    if !has_gcc {
        eprintln!("Skipping test: gcc not available");
        return;
    }

    db.insert_facts(toolchain_facts);

    db.insert_facts(vec![Fact {
        predicate: "requires_tool".to_string(),
        args: vec![
            Value::String("//test:app".to_string()),
            Value::String("gcc".to_string()),
        ],
    }]);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let toolchain_facts = db.query("toolchain", &[]);

    let gcc_paths: Vec<String> = toolchain_facts
        .iter()
        .filter_map(|fact| {
            if let [Value::String(target), Value::String(tool), Value::String(path)] =
                &fact.args[..]
            {
                if target == "//test:app" && tool == "gcc" {
                    return Some(path.clone());
                }
            }
            None
        })
        .collect();

    assert_eq!(
        gcc_paths.len(),
        1,
        "Should have exactly one gcc toolchain fact"
    );
    assert!(
        gcc_paths[0].contains("gcc"),
        "Should use auto-discovered gcc path: {}",
        gcc_paths[0]
    );
}

#[test]
fn multiple_overrides_independent() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    db.insert_facts(vec![
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("gcc".to_string()),
                Value::String("/custom/gcc".to_string()),
            ],
        },
        Fact {
            predicate: "toolchain_override".to_string(),
            args: vec![
                Value::String("g++".to_string()),
                Value::String("/custom/g++".to_string()),
            ],
        },
        Fact {
            predicate: "requires_tool".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "requires_tool".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("g++".to_string()),
            ],
        },
        Fact {
            predicate: "requires_tool".to_string(),
            args: vec![
                Value::String("//test:app".to_string()),
                Value::String("clang".to_string()),
            ],
        },
    ]);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let toolchain_facts = db.query("toolchain", &[]);

    let gcc_path = toolchain_facts.iter().find_map(|fact| {
        if let [Value::String(t), Value::String(tool), Value::String(path)] = &fact.args[..] {
            (t == "//test:app" && tool == "gcc").then(|| path.clone())
        } else {
            None
        }
    });

    let gpp_path = toolchain_facts.iter().find_map(|fact| {
        if let [Value::String(t), Value::String(tool), Value::String(path)] = &fact.args[..] {
            (t == "//test:app" && tool == "g++").then(|| path.clone())
        } else {
            None
        }
    });

    let clang_path = toolchain_facts.iter().find_map(|fact| {
        if let [Value::String(t), Value::String(tool), Value::String(path)] = &fact.args[..] {
            (t == "//test:app" && tool == "clang").then(|| path.clone())
        } else {
            None
        }
    });

    assert_eq!(gcc_path, Some("/custom/gcc".to_string()));
    assert_eq!(gpp_path, Some("/custom/g++".to_string()));
    assert!(clang_path.is_some(), "clang should use discovered path");
    assert!(
        clang_path.unwrap().contains("clang"),
        "clang should not be overridden"
    );
}
