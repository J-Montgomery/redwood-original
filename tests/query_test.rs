use redwood::datalog::{parser, Fact, Value};
use redwood::{
    datalog::Engine,
    runtime::{prelude, ToolchainScanner},
};

#[test]
fn query_sources_for_c_binary() {
    let build_content = r#"
        system_cc("//example:hello").
        sources("//example:hello", "hello.c").
        sources("//example:hello", "utils.c").
        attr("//example:hello", "-o", "hello").
        outputs("//example:hello", "hello").
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

    let results = db.query("sources", &[]);

    assert_eq!(results.len(), 2);

    let source_files: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if f.args.len() == 2 {
                if let Value::String(s) = &f.args[1] {
                    return Some(s.clone());
                }
            }
            None
        })
        .collect();

    assert!(source_files.contains(&"hello.c".to_string()));
    assert!(source_files.contains(&"utils.c".to_string()));
}

#[test]
fn query_requires_tool_after_prelude() {
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

    let results = db.query("requires_tool", &[]);

    assert_eq!(results.len(), 1, "Should find one requires_tool fact");

    if let (Value::String(target), Value::String(tool)) = (&results[0].args[0], &results[0].args[1])
    {
        assert_eq!(target, "//example:hello");
        assert_eq!(tool, "gcc");
    } else {
        panic!("Unexpected requires_tool fact format");
    }
}

#[test]
fn query_tool_available() {
    let mut db = Engine::new();

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let results = db.query("tool_available", &[]);

    let tools: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if !f.args.is_empty() {
                if let Value::String(tool) = &f.args[0] {
                    return Some(tool.clone());
                }
            }
            None
        })
        .collect();

    assert!(!tools.is_empty(), "Should find at least some tools on PATH");
}

#[test]
fn query_dependencies_of_target() {
    let build_content = r#"
        system_cc("//app:main").
        sources("//app:main", "main.c").
        deps("//app:main", "//lib:utils").
        deps("//app:main", "//lib:core").

        system_cc("//lib:utils").
        sources("//lib:utils", "utils.c").
        deps("//lib:utils", "//lib:core").

        system_cc("//lib:core").
        sources("//lib:core", "core.c").
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

    let results = db.query("deps", &[]);

    let main_deps: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if f.args.len() == 2 {
                if let (Value::String(target), Value::String(dep)) = (&f.args[0], &f.args[1]) {
                    if target == "//app:main" {
                        return Some(dep.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert_eq!(main_deps.len(), 2);
    assert!(main_deps.contains(&"//lib:utils".to_string()));
    assert!(main_deps.contains(&"//lib:core".to_string()));

    // Dependencies of //lib:utils
    let utils_deps: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if f.args.len() == 2 {
                if let (Value::String(target), Value::String(dep)) = (&f.args[0], &f.args[1]) {
                    if target == "//lib:utils" {
                        return Some(dep.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert_eq!(utils_deps.len(), 1);
    assert_eq!(utils_deps[0], "//lib:core");
}

#[test]
fn query_reverse_dependencies() {
    let build_content = r#"
        system_cc("//app:main").
        deps("//app:main", "//lib:core").

        system_cc("//app:test").
        deps("//app:test", "//lib:core").

        system_cc("//lib:core").
        sources("//lib:core", "core.c").
    "#;

    let mut db = Engine::new();

    let (facts, rules) = parser::parse_program(build_content).unwrap();
    db.insert_facts(facts);

    for rule in rules {
        db.compile_rule(rule);
    }

    let query_content = r#"
        depends_on(Dependent, Library) :- deps(Dependent, Library).
    "#;

    let (_, query_rules) = parser::parse_program(query_content).unwrap();
    for rule in query_rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let results = db.query("depends_on", &[]);

    let core_dependents: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if f.args.len() == 2 {
                if let (Value::String(dependent), Value::String(library)) = (&f.args[0], &f.args[1])
                {
                    if library == "//lib:core" {
                        return Some(dependent.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert_eq!(core_dependents.len(), 2);
    assert!(core_dependents.contains(&"//app:main".to_string()));
    assert!(core_dependents.contains(&"//app:test".to_string()));
}

#[test]
fn query_attributes_for_toolchain() {
    let build_content = r#"
        system_cc("//example:hello").
        attr("//example:hello", "-O3").
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

    let results = db.query("attr", &[]);

    // Get gcc default attrs (from prelude)
    let gcc_attrs: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if f.args.len() >= 2 {
                if let (Value::String(target), Value::String(key)) = (&f.args[0], &f.args[1]) {
                    if target == "gcc" {
                        return Some(key.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert!(gcc_attrs.contains(&"-fPIC".to_string()));
    assert!(gcc_attrs.contains(&"-Wall".to_string()));

    let target_attrs: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if f.args.len() >= 2 {
                if let (Value::String(target), Value::String(key)) = (&f.args[0], &f.args[1]) {
                    if target == "//example:hello" {
                        return Some(key.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert!(target_attrs.contains(&"-O3".to_string()));
    assert!(target_attrs.contains(&"tool".to_string()));
}

#[test]
fn query_deep_rule_dependency_chain() {
    use redwood::datalog::{Predicate, Rule, Term};

    let mut db = Engine::new();

    // Base fact
    db.insert_facts(vec![Fact {
        predicate: "base".to_string(),
        args: vec![Value::String("x".to_string())],
    }]);

    // Chain of rules: base -> level1 -> level2 -> level3
    db.compile_rule(Rule {
        head: Predicate {
            name: "level1".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "base".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    db.compile_rule(Rule {
        head: Predicate {
            name: "level2".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "level1".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    db.compile_rule(Rule {
        head: Predicate {
            name: "level3".to_string(),
            args: vec![Term::Variable("X".to_string())],
        },
        body: vec![Predicate {
            name: "level2".to_string(),
            args: vec![Term::Variable("X".to_string())],
        }],
    });

    let results = db.query("level3", &[]);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("x".to_string()));
}
