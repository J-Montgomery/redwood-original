use redwood::build::system_tool::SystemTool;
use redwood::build::BuildKind;
use redwood::datalog::{Engine, Fact, TargetLabel, Value};

#[test]
fn test_tool_name_with_command_injection_rejected() {
    let mut db = Engine::new();
    let target = TargetLabel {
        package: "//test".to_string(),
        target: "malicious".to_string(),
    };

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.to_string())],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc; rm -rf /".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("test.c".to_string()),
            ],
        },
        Fact {
            predicate: "toolchain".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("gcc; rm -rf /".to_string()),
                Value::String("/usr/bin/gcc".to_string()),
            ],
        },
    ]);

    let tool = SystemTool::new();
    let result = tool.plan(&target, &mut db);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("dangerous shell metacharacter"));
}

#[test]
fn test_argument_with_command_substitution_rejected() {
    let mut db = Engine::new();
    let target = TargetLabel {
        package: "//test".to_string(),
        target: "malicious_arg".to_string(),
    };

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.to_string())],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("-o".to_string()),
                Value::String("$(whoami)".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("test.c".to_string()),
            ],
        },
        Fact {
            predicate: "toolchain".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("gcc".to_string()),
                Value::String("/usr/bin/gcc".to_string()),
            ],
        },
    ]);

    let tool = SystemTool::new();
    let result = tool.plan(&target, &mut db);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("dangerous"));
}

#[test]
fn test_argument_with_pipe_rejected() {
    let mut db = Engine::new();
    let target = TargetLabel {
        package: "//test".to_string(),
        target: "pipe_attack".to_string(),
    };

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.to_string())],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("0".to_string()),
                Value::String("test.c | cat /etc/passwd".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("test.c".to_string()),
            ],
        },
        Fact {
            predicate: "toolchain".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("gcc".to_string()),
                Value::String("/usr/bin/gcc".to_string()),
            ],
        },
    ]);

    let tool = SystemTool::new();
    let result = tool.plan(&target, &mut db);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("dangerous"));
}

#[test]
fn test_valid_tool_and_arguments_accepted() {
    let mut db = Engine::new();
    let target = TargetLabel {
        package: "//test".to_string(),
        target: "valid".to_string(),
    };

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.to_string())],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("-o".to_string()),
                Value::String("{output}".to_string()),
            ],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("-c".to_string()),
                Value::String("{sources}".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("test.c".to_string()),
            ],
        },
        Fact {
            predicate: "toolchain".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("gcc".to_string()),
                Value::String("/usr/bin/gcc".to_string()),
            ],
        },
    ]);

    let tool = SystemTool::new();
    let result = tool.plan(&target, &mut db);

    assert!(result.is_ok());
    let plan = result.unwrap();
    assert_eq!(plan.command, "/usr/bin/gcc");
}

#[test]
fn test_tool_path_with_command_injection_rejected() {
    let mut db = Engine::new();
    let target = TargetLabel {
        package: "//test".to_string(),
        target: "malicious_path".to_string(),
    };

    db.insert_facts(vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.to_string())],
        },
        Fact {
            predicate: "attr".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("tool".to_string()),
                Value::String("gcc".to_string()),
            ],
        },
        Fact {
            predicate: "sources".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("test.c".to_string()),
            ],
        },
        Fact {
            predicate: "toolchain".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String("gcc".to_string()),
                Value::String("/usr/bin/gcc; echo hacked".to_string()),
            ],
        },
    ]);

    let tool = SystemTool::new();
    let result = tool.plan(&target, &mut db);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("dangerous"));
}
