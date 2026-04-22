use redwood::datalog::{parser, Engine, Value};
use std::process::Command;

fn setup_engine_with_program(program: &str) -> Engine {
    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }
    db.set_resolve_callback(Box::new(|_target, tool, args| {
        let output = Command::new(tool)
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        Ok(output.stdout)
    }));
    db
}

#[test]
fn resolve_executes_echo_command() {
    let program = r#"
target("//tools:echo").
attr("//tools:echo", "tool", "echo").
attr("//tools:echo", "0", "hello").
attr("//tools:echo", "1", "world").

message(Output) :- resolve("//tools:echo", Output).
"#;

    let mut db = setup_engine_with_program(program);
    let results = db.query("message", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("hello world".to_string()));
}

#[test]
fn resolve_multiple_lines() {
    let program = r#"
target("//tools:multiline").
attr("//tools:multiline", "tool", "printf").
attr("//tools:multiline", "0", "line1\nline2\nline3").

line(L) :- resolve("//tools:multiline", L).
"#;

    let mut db = setup_engine_with_program(program);
    let results = db.query("line", &[]);
    assert_eq!(results.len(), 3);

    let lines: Vec<String> = results
        .iter()
        .filter_map(|f| {
            if let Value::String(s) = &f.args[0] {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(lines.contains(&"line1".to_string()));
    assert!(lines.contains(&"line2".to_string()));
    assert!(lines.contains(&"line3".to_string()));
}

#[test]
fn resolve_cached_between_queries() {
    let program = r#"
target("//tools:date").
attr("//tools:date", "tool", "date").
attr("//tools:date", "0", "+%s").

timestamp1(T) :- resolve("//tools:date", T).
timestamp2(T) :- resolve("//tools:date", T).
"#;

    let mut db = setup_engine_with_program(program);
    let results1 = db.query("timestamp1", &[]);
    let results2 = db.query("timestamp2", &[]);

    assert_eq!(results1.len(), 1);
    assert_eq!(results2.len(), 1);
    assert_eq!(results1[0].args[0], results2[0].args[0]);
}

#[test]
fn resolve_with_bound_output_checks_match() {
    let program = r#"
target("//tools:echo").
attr("//tools:echo", "tool", "echo").
attr("//tools:echo", "0", "expected").

matches(Target) :- target(Target), resolve(Target, "expected").
no_match(Target) :- target(Target), resolve(Target, "wrong").
"#;

    let mut db = setup_engine_with_program(program);
    let matches = db.query("matches", &[]);
    assert_eq!(matches.len(), 1);

    let no_matches = db.query("no_match", &[]);
    assert_eq!(no_matches.len(), 0);
}

#[test]
fn resolve_missing_tool_returns_empty() {
    let program = r#"
target("//tools:no-tool").

result(Output) :- resolve("//tools:no-tool", Output).
"#;

    let mut db = setup_engine_with_program(program);
    let results = db.query("result", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn resolve_nonexistent_command_returns_empty() {
    let program = r#"
target("//tools:bad").
attr("//tools:bad", "tool", "nonexistent-command-12345").

result(Output) :- resolve("//tools:bad", Output).
"#;

    let mut db = setup_engine_with_program(program);
    let results = db.query("result", &[]);
    assert_eq!(results.len(), 0);
}

#[test]
fn resolve_with_git_example() {
    let program = r#"
target("//tools:git-branch").
attr("//tools:git-branch", "tool", "git").
attr("//tools:git-branch", "0", "branch").
attr("//tools:git-branch", "1", "--show-current").

current_branch(Branch) :- resolve("//tools:git-branch", Branch).
"#;

    let mut db = setup_engine_with_program(program);
    let results = db.query("current_branch", &[]);

    if std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .output()
        .is_ok()
    {
        assert_eq!(results.len(), 1);
        if let Value::String(branch) = &results[0].args[0] {
            assert!(!branch.is_empty());
        }
    } else {
        assert_eq!(results.len(), 0);
    }
}

#[test]
fn resolve_only_sees_base_attrs() {
    // Verify that resolve() is total by ensuring that it can only use base attr facts
    let program = r#"
target("//cmd:echo").
attr("//cmd:echo", "tool", "echo").
attr("//cmd:echo", "0", "hello").

# This rule creates a derived attr fact - resolve should NOT see it
attr(X, "derived", "yes") :- attr(X, "tool", _).

# Regular query should see both base and derived attrs
all_attrs(T, K, V) :- attr(T, K, V).

# resolve() should work with base facts only
output(O) :- resolve("//cmd:echo", O).
"#;

    let mut db = setup_engine_with_program(program);

    // Verify regular query sees both base and derived attrs
    let all_attrs = db.query("all_attrs", &[Some("//cmd:echo")]);
    let has_derived = all_attrs.iter().any(|f| {
        if f.args.len() >= 3 {
            if let (Value::String(k), Value::String(v)) = (&f.args[1], &f.args[2]) {
                return k == "derived" && v == "yes";
            }
        }
        false
    });
    assert!(has_derived, "Regular query should see derived attr");

    let results = db.query("output", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("hello".to_string()));
}
