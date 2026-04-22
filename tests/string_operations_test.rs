use redwood::datalog::{parser, Engine};

#[test]
fn prefix_checks_start_of_string() {
    let program = r#"
        target("//lib:core").
        target("//lib:http").
        target("//app:server").
        target("//other:thing").

        is_lib(T) :- target(T), prefix(T, "//lib:").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("is_lib", &[]);

    let libs: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(t)) = f.args.first() {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(libs.len(), 2);
    assert!(libs.contains(&"//lib:core".to_string()));
    assert!(libs.contains(&"//lib:http".to_string()));
    assert!(!libs.contains(&"//app:server".to_string()));
}

#[test]
fn suffix_checks_end_of_string() {
    let program = r#"
        file("main.rs").
        file("lib.rs").
        file("test.py").
        file("config.toml").

        is_rust_file(F) :- file(F), suffix(F, ".rs").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("is_rust_file", &[]);

    let rust_files: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(file)) = f.args.first() {
                Some(file.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(rust_files.len(), 2);
    assert!(rust_files.contains(&"main.rs".to_string()));
    assert!(rust_files.contains(&"lib.rs".to_string()));
}

#[test]
fn contains_finds_substring() {
    let program = r#"
        file("test_helper.rs").
        file("main.rs").
        file("integration_test.rs").
        file("lib.rs").

        is_test_file(F) :- file(F), contains(F, "test").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("is_test_file", &[]);

    let test_files: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(file)) = f.args.first() {
                Some(file.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(test_files.len(), 2);
    assert!(test_files.contains(&"test_helper.rs".to_string()));
    assert!(test_files.contains(&"integration_test.rs".to_string()));
}

#[test]
fn substring_extracts_slice() {
    let program = r#"
        text("hello world").

        extract_word(Word) :- text(T), substring(T, 0, 5, Word).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("extract_word", &[]);
    assert_eq!(results.len(), 1);

    if let Some(redwood::datalog::Value::String(word)) = results[0].args.first() {
        assert_eq!(word, "hello");
    } else {
        panic!("Expected string result");
    }
}

#[test]
fn split_extracts_package_name() {
    let program = r#"
        target("//lib:core:v1").
        target("//lib:http:v2").
        target("//app:server").

        package_info(Target, Package, Name) :-
            target(Target),
            split(Target, ":", 0, Package),
            split(Target, ":", 1, Name).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("package_info", &[]);

    // Find the one with v1
    let core_result = results.iter().find(|f| {
        if let Some(redwood::datalog::Value::String(target)) = f.args.first() {
            target.contains("v1")
        } else {
            false
        }
    });

    assert!(core_result.is_some());
    let result = core_result.unwrap();

    if let (
        Some(redwood::datalog::Value::String(pkg)),
        Some(redwood::datalog::Value::String(name)),
    ) = (result.args.get(1), result.args.get(2))
    {
        assert_eq!(pkg, "//lib");
        assert_eq!(name, "core");
    } else {
        panic!("Expected string results");
    }
}

#[test]
fn version_conflict_detection_with_split() {
    let program = r#"
        deps("//app", "//lib:core:v1").
        deps("//app", "//lib:core:v2").

        package_name(Target, Package) :-
            deps(_, Target),
            split(Target, ":", 1, Package).

        version_conflict(Package) :-
            deps(_, Target1),
            deps(_, Target2),
            Target1 != Target2,
            split(Target1, ":", 1, Package),
            split(Target2, ":", 1, Package).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("version_conflict", &[]);
    assert_eq!(results.len(), 1);

    if let Some(redwood::datalog::Value::String(package)) = results[0].args.first() {
        assert_eq!(package, "core");
    } else {
        panic!("Expected string result");
    }
}

#[test]
fn combined_string_operations() {
    let program = r#"
        file("src/lib/core.rs").
        file("src/lib/http.rs").
        file("tests/integration.rs").
        file("benches/perf.rs").

        lib_source(F, Module) :-
            file(F),
            prefix(F, "src/"),
            suffix(F, ".rs"),
            contains(F, "/lib/"),
            split(F, "/", 2, ModuleFile),
            substring(ModuleFile, 0, 4, Module).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("lib_source", &[]);
    assert_eq!(results.len(), 2);

    let modules: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Some(redwood::datalog::Value::String(module)) = f.args.get(1) {
                Some(module.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(modules.contains(&"core".to_string()));
    assert!(modules.contains(&"http".to_string()));
}
