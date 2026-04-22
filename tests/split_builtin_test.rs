use redwood::datalog::{parser, Engine, Value};

#[test]
fn split_extracts_part_at_index() {
    let mut db = Engine::new();

    let program = r#"
label("//app:cli").
target_name(Label, Name) :- label(Label), split(Label, ":", 1, Name).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("target_name", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
    assert_eq!(results[0].args[1], Value::String("cli".to_string()));
}

#[test]
fn split_extracts_package() {
    let mut db = Engine::new();

    let program = r#"
label("//app:cli").
target_package(Label, Package) :- label(Label), split(Label, ":", 0, Package).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("target_package", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("//app:cli".to_string()));
    assert_eq!(results[0].args[1], Value::String("//app".to_string()));
}

#[test]
fn split_generates_all_parts() {
    let mut db = Engine::new();

    let program = r#"
path("src/main/java/App.java").
part(Path, Index, Part) :- path(Path), split(Path, "/", Index, Part).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("part", &[]);
    assert_eq!(results.len(), 4);

    let parts: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(part) = &f.args[2] {
                Some(part.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(parts.contains(&"src".to_string()));
    assert!(parts.contains(&"main".to_string()));
    assert!(parts.contains(&"java".to_string()));
    assert!(parts.contains(&"App.java".to_string()));
}

#[test]
fn split_finds_indices_for_part() {
    let mut db = Engine::new();

    let program = r#"
path("a/b/a/c").
index_of_a(Path, Index) :- path(Path), split(Path, "/", Index, "a").
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("index_of_a", &[]);
    assert_eq!(results.len(), 2);

    let indices: Vec<i64> = results
        .iter()
        .filter_map(|f| {
            if let Value::Integer(idx) = &f.args[1] {
                Some(*idx)
            } else {
                None
            }
        })
        .collect();

    assert!(indices.contains(&0));
    assert!(indices.contains(&2));
}

#[test]
fn split_checks_specific_index_and_part() {
    let mut db = Engine::new();

    let program = r#"
label("//app:cli").
is_app_package(Label) :- label(Label), split(Label, ":", 0, "//app").
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("is_app_package", &[]);
    assert_eq!(results.len(), 1);
}

#[test]
fn split_fails_on_out_of_bounds_index() {
    let mut db = Engine::new();

    let program = r#"
label("//app:cli").
invalid(Label, Part) :- label(Label), split(Label, ":", 5, Part).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("invalid", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn split_with_empty_delimiter_parts() {
    let mut db = Engine::new();

    let program = r#"
text("a::b").
part(Text, Index, Part) :- text(Text), split(Text, ":", Index, Part).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("part", &[]);
    assert_eq!(results.len(), 3);

    let parts: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(part) = &f.args[2] {
                Some(part.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(parts.contains(&"a".to_string()));
    assert!(parts.contains(&"".to_string()));
    assert!(parts.contains(&"b".to_string()));
}
