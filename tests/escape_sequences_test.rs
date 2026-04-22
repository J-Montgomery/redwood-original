use redwood::datalog::{parser, Engine, Value};

#[test]
fn test_newline_escape() {
    let mut db = Engine::new();

    let program = r#"
message("line1\nline2").
"#;

    let (facts, _rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);

    let results = db.query("message", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("line1\nline2".to_string())
    );
}

#[test]
fn test_tab_escape() {
    let mut db = Engine::new();

    let program = r#"
data("col1\tcol2").
"#;

    let (facts, _rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);

    let results = db.query("data", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("col1\tcol2".to_string()));
}

#[test]
fn test_quote_escape() {
    let mut db = Engine::new();

    let program = r#"
text("He said \"hello\"").
"#;

    let (facts, _rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);

    let results = db.query("text", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("He said \"hello\"".to_string())
    );
}

#[test]
fn test_backslash_escape() {
    let mut db = Engine::new();

    let program = r#"
path("C:\\Users\\test").
"#;

    let (facts, _rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);

    let results = db.query("path", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].args[0],
        Value::String("C:\\Users\\test".to_string())
    );
}

#[test]
fn test_no_escapes_still_works() {
    let mut db = Engine::new();

    let program = r#"
simple("hello world").
"#;

    let (facts, _rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);

    let results = db.query("simple", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("hello world".to_string()));
}

#[test]
fn test_complex_shell_command() {
    let mut db = Engine::new();

    let program = r#"
shell_cmd("git show HEAD:BUILD.datalog | grep -E '^target\\(' | sed 's/target//'").
"#;

    let (facts, _rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);

    let results = db.query("shell_cmd", &[]);
    assert_eq!(results.len(), 1);
    // The \\( in the input becomes \( in the string (backslash-paren, not just paren)
    if let Value::String(s) = &results[0].args[0] {
        // The string should contain \( not just (
        assert!(s.contains("\\("));
    } else {
        panic!("Expected string value");
    }
}
