use redwood::datalog::{parser, Engine, Fact, Value};
use redwood::runtime::prelude;
use std::fs;
use tempfile::tempdir;

fn generate_file_exists_facts_for_dir(dir: &std::path::Path) -> Vec<Fact> {
    let mut facts = Vec::new();

    if let Ok(entries) = glob::glob(&format!("{}/**/*", dir.display())) {
        for entry in entries.flatten() {
            if entry.is_file() {
                if let Some(path_str) = entry.to_str() {
                    facts.push(Fact {
                        predicate: "file_exists".to_string(),
                        args: vec![Value::String(path_str.to_string())],
                    });
                }
            }
        }
    }

    facts
}

#[test]
fn glob_pattern_derives_sources() {
    let dir = tempdir().unwrap();

    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(dir.path().join("src/lib.rs"), "pub fn foo() {}").unwrap();
    fs::write(dir.path().join("src/test.txt"), "not rust").unwrap();

    let build_content = format!(
        r#"
target("//app:bin").
source_glob("//app:bin", "{}/src/**/*.rs").
"#,
        dir.path().display()
    );

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(&build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let file_facts = generate_file_exists_facts_for_dir(dir.path());
    db.insert_facts(file_facts);

    let sources = db.query("sources", &[]);

    let source_files: Vec<String> = sources
        .iter()
        .filter_map(|f| {
            if f.args.len() >= 2 {
                if let (Value::String(target), Value::String(file)) = (&f.args[0], &f.args[1]) {
                    if target == "//app:bin" {
                        return Some(file.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert_eq!(source_files.len(), 2);
    assert!(source_files.iter().any(|f| f.ends_with("main.rs")));
    assert!(source_files.iter().any(|f| f.ends_with("lib.rs")));
    assert!(!source_files.iter().any(|f| f.ends_with("test.txt")));
}

#[test]
fn glob_with_wildcard_matches_all_files() {
    let dir = tempdir().unwrap();

    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/a.c"), "").unwrap();
    fs::write(dir.path().join("src/b.c"), "").unwrap();
    fs::write(dir.path().join("src/c.h"), "").unwrap();

    let build_content = format!(
        r#"
target("//lib:c").
source_glob("//lib:c", "{}/src/*.c").
"#,
        dir.path().display()
    );

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(&build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let file_facts = generate_file_exists_facts_for_dir(dir.path());
    db.insert_facts(file_facts);

    let sources = db.query("sources", &[]);
    let c_files: Vec<String> = sources
        .iter()
        .filter_map(|f| {
            if f.args.len() >= 2 {
                if let (Value::String(target), Value::String(file)) = (&f.args[0], &f.args[1]) {
                    if target == "//lib:c" {
                        return Some(file.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert_eq!(c_files.len(), 2);
    assert!(c_files.iter().any(|f| f.ends_with("a.c")));
    assert!(c_files.iter().any(|f| f.ends_with("b.c")));
    assert!(!c_files.iter().any(|f| f.ends_with("c.h")));
}

#[test]
fn multiple_globs_for_single_target() {
    let dir = tempdir().unwrap();

    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::create_dir_all(dir.path().join("include")).unwrap();
    fs::write(dir.path().join("src/main.cpp"), "").unwrap();
    fs::write(dir.path().join("src/util.cpp"), "").unwrap();
    fs::write(dir.path().join("include/header.h"), "").unwrap();

    let build_content = format!(
        r#"
target("//app:cpp").
source_glob("//app:cpp", "{}/src/*.cpp").
source_glob("//app:cpp", "{}/include/*.h").
"#,
        dir.path().display(),
        dir.path().display()
    );

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(&build_content).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let file_facts = generate_file_exists_facts_for_dir(dir.path());
    db.insert_facts(file_facts);

    let sources = db.query("sources", &[]);
    let files: Vec<String> = sources
        .iter()
        .filter_map(|f| {
            if f.args.len() >= 2 {
                if let (Value::String(target), Value::String(file)) = (&f.args[0], &f.args[1]) {
                    if target == "//app:cpp" {
                        return Some(file.clone());
                    }
                }
            }
            None
        })
        .collect();

    assert_eq!(files.len(), 3);
    assert!(files.iter().any(|f| f.ends_with("main.cpp")));
    assert!(files.iter().any(|f| f.ends_with("util.cpp")));
    assert!(files.iter().any(|f| f.ends_with("header.h")));
}
